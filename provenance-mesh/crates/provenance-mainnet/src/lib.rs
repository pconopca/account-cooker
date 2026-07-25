//! Sampling real funding edges from Solana mainnet.
//!
//! Everything else in this workspace measures synthetic fleets, where the same
//! author wrote both the attack and the defence. This crate exists to break
//! that circularity: it reconstructs a funding graph from actual mainnet blocks
//! and runs the same attacks against it.
//!
//! # What a block window can and cannot show
//!
//! The graph is built from a contiguous window of blocks, so it sees only the
//! transfers that occurred inside that window. An account funded before the
//! window appears to have no funder at all, and a busy account's depositor set
//! is truncated to whoever happened to pay it during the window.
//!
//! That truncation is one-directional: it can only *understate* how many
//! distinct depositors an account has, never overstate it. Every anonymity
//! figure this crate reports is therefore a lower bound.

use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

use provenance_core::{FundingEdge, FundingGraph, WalletId};
use serde::{Deserialize, Serialize};

/// Default public mainnet endpoint.
pub const DEFAULT_RPC: &str = "https://api.mainnet-beta.solana.com";

/// A sampled window of mainnet funding edges, cached to disk.
///
/// Persisting the sample is what makes a measurement reproducible: a reviewer
/// re-runs the analysis against the committed bytes rather than against
/// whatever mainnet looks like today.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    /// First slot requested.
    pub start_slot: u64,
    /// Slots requested, including any that were skipped or unavailable.
    pub slots_requested: u64,
    /// Slots that actually returned a block.
    pub slots_fetched: u64,
    /// Extracted system-program transfers.
    pub edges: Vec<FundingEdge>,
}

impl Sample {
    /// Build a funding graph from the sampled edges.
    #[must_use]
    pub fn graph(&self) -> FundingGraph {
        FundingGraph::from_edges(self.edges.clone())
    }

    /// Distinct accounts that received at least one transfer.
    #[must_use]
    pub fn funded_wallets(&self) -> Vec<WalletId> {
        self.graph().funded_wallets().into_iter().collect()
    }
}

/// Errors raised while sampling.
#[derive(Debug)]
pub enum SampleError {
    /// The endpoint could not be reached, or replied with an error.
    Rpc(String),
    /// The reply did not have the shape the RPC contract promises.
    Malformed(String),
}

impl std::fmt::Display for SampleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rpc(detail) => write!(formatter, "rpc error: {detail}"),
            Self::Malformed(detail) => write!(formatter, "malformed rpc reply: {detail}"),
        }
    }
}

impl std::error::Error for SampleError {}

fn rpc_call(
    endpoint: &str,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, SampleError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    // The public endpoint rate-limits aggressively; back off rather than
    // hammering it, and give up loudly instead of silently returning a short
    // sample that would quietly bias the measurement.
    let mut delay = Duration::from_millis(400);
    let mut last_error = String::new();
    for _ in 0..5 {
        match ureq::post(endpoint).send_json(body.clone()) {
            Ok(response) => {
                let value: serde_json::Value = response
                    .into_json()
                    .map_err(|error| SampleError::Malformed(error.to_string()))?;
                if let Some(error) = value.get("error") {
                    last_error = error.to_string();
                    // Skipped slots are expected and not worth retrying.
                    if last_error.contains("was skipped") || last_error.contains("not available") {
                        return Err(SampleError::Rpc(last_error));
                    }
                } else {
                    return Ok(value);
                }
            }
            Err(error) => last_error = error.to_string(),
        }
        sleep(delay);
        delay *= 2;
    }
    Err(SampleError::Rpc(last_error))
}

/// Fetch the current slot.
///
/// # Errors
/// Returns [`SampleError`] if the endpoint is unreachable or replies oddly.
pub fn current_slot(endpoint: &str) -> Result<u64, SampleError> {
    let value = rpc_call(endpoint, "getSlot", &serde_json::json!([]))?;
    value
        .get("result")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SampleError::Malformed("getSlot returned no result".to_owned()))
}

/// Pull every system-program transfer out of one parsed instruction list.
fn collect_transfers(
    instructions: &serde_json::Value,
    slot: u64,
    payer: Option<&WalletId>,
    into: &mut Vec<FundingEdge>,
) {
    let Some(list) = instructions.as_array() else {
        return;
    };
    for instruction in list {
        if instruction
            .get("program")
            .and_then(serde_json::Value::as_str)
            != Some("system")
        {
            continue;
        }
        let Some(parsed) = instruction.get("parsed") else {
            continue;
        };
        let kind = parsed.get("type").and_then(serde_json::Value::as_str);
        if !matches!(kind, Some("transfer" | "transferWithSeed")) {
            continue;
        }
        let Some(fields) = parsed.get("info") else {
            continue;
        };
        let (Some(source), Some(destination), Some(lamports)) = (
            fields.get("source").and_then(serde_json::Value::as_str),
            fields
                .get("destination")
                .and_then(serde_json::Value::as_str),
            fields.get("lamports").and_then(serde_json::Value::as_u64),
        ) else {
            continue;
        };
        // Self-transfers move no value between owners and would inflate an
        // account's apparent depositor count with itself.
        if source == destination {
            continue;
        }
        into.push(FundingEdge {
            source: WalletId(source.to_owned()),
            target: WalletId(destination.to_owned()),
            lamports,
            slot,
            payer: payer.cloned(),
        });
    }
}

/// Extract transfers from one `getBlock` result.
#[must_use]
pub fn edges_from_block(block: &serde_json::Value, slot: u64) -> Vec<FundingEdge> {
    let mut edges = Vec::new();
    let Some(transactions) = block
        .get("transactions")
        .and_then(serde_json::Value::as_array)
    else {
        return edges;
    };
    for transaction in transactions {
        // Failed transactions moved nothing and must not enter the graph.
        if transaction
            .pointer("/meta/err")
            .is_some_and(|err| !err.is_null())
        {
            continue;
        }
        // The fee payer is the first account key, and it always signs. It is
        // the party that authorised this transfer, which is what decides
        // whether a pooled payout is linkable back to a depositor.
        let payer = transaction
            .pointer("/transaction/message/accountKeys/0/pubkey")
            .and_then(serde_json::Value::as_str)
            .map(|key| WalletId(key.to_owned()));

        if let Some(top) = transaction.pointer("/transaction/message/instructions") {
            collect_transfers(top, slot, payer.as_ref(), &mut edges);
        }
        // CPI transfers are just as real as top-level ones, and a great deal of
        // funding on Solana happens through a program rather than directly.
        if let Some(inner) = transaction
            .pointer("/meta/innerInstructions")
            .and_then(serde_json::Value::as_array)
        {
            for group in inner {
                if let Some(list) = group.get("instructions") {
                    collect_transfers(list, slot, payer.as_ref(), &mut edges);
                }
            }
        }
    }
    edges
}

/// Sample `blocks` consecutive slots starting at `start_slot`.
///
/// Slots that were skipped by the cluster are counted but contribute nothing.
///
/// # Errors
/// Returns [`SampleError`] only when the endpoint itself fails; individual
/// unavailable slots are tolerated.
pub fn sample(endpoint: &str, start_slot: u64, blocks: u64) -> Result<Sample, SampleError> {
    let mut edges = Vec::new();
    let mut fetched = 0_u64;

    for offset in 0..blocks {
        let slot = start_slot + offset;
        let params = serde_json::json!([
            slot,
            {
                "encoding": "jsonParsed",
                "transactionDetails": "full",
                "rewards": false,
                "maxSupportedTransactionVersion": 0
            }
        ]);
        match rpc_call(endpoint, "getBlock", &params) {
            Ok(value) => {
                if let Some(result) = value.get("result") {
                    edges.extend(edges_from_block(result, slot));
                    fetched += 1;
                }
            }
            // A skipped slot is normal cluster behaviour, not a sampling failure.
            Err(SampleError::Rpc(_)) => {}
            Err(other) => return Err(other),
        }
        sleep(Duration::from_millis(120));
    }

    Ok(Sample {
        start_slot,
        slots_requested: blocks,
        slots_fetched: fetched,
        edges,
    })
}

/// Count how many distinct depositors each account received from.
#[must_use]
pub fn depositor_counts(graph: &FundingGraph) -> BTreeMap<WalletId, usize> {
    graph
        .funded_wallets()
        .into_iter()
        .map(|wallet| {
            let count = graph.direct_funders(&wallet).len();
            (wallet, count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_with(instructions: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "transactions": [{
                "meta": { "err": null, "innerInstructions": [] },
                "transaction": { "message": { "instructions": instructions } }
            }]
        })
    }

    fn transfer(source: &str, destination: &str, lamports: u64) -> serde_json::Value {
        serde_json::json!({
            "program": "system",
            "parsed": {
                "type": "transfer",
                "info": { "source": source, "destination": destination, "lamports": lamports }
            }
        })
    }

    #[test]
    fn system_transfers_become_edges() {
        let block = block_with(&serde_json::json!([transfer("alice", "bob", 5_000)]));
        let edges = edges_from_block(&block, 42);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, WalletId::from("alice"));
        assert_eq!(edges[0].target, WalletId::from("bob"));
        assert_eq!(edges[0].lamports, 5_000);
        assert_eq!(edges[0].slot, 42);
    }

    #[test]
    fn non_system_and_non_transfer_instructions_are_ignored() {
        let block = block_with(&serde_json::json!([
            { "program": "spl-token", "parsed": { "type": "transfer", "info": {
                "source": "a", "destination": "b", "lamports": 1 } } },
            { "program": "system", "parsed": { "type": "createAccount", "info": {
                "source": "a", "destination": "b", "lamports": 1 } } },
        ]));
        assert!(edges_from_block(&block, 1).is_empty());
    }

    #[test]
    fn failed_transactions_move_nothing_and_are_skipped() {
        let mut block = block_with(&serde_json::json!([transfer("alice", "bob", 5_000)]));
        block["transactions"][0]["meta"]["err"] = serde_json::json!({"InstructionError": []});
        assert!(edges_from_block(&block, 1).is_empty());
    }

    #[test]
    fn self_transfers_do_not_inflate_depositor_counts() {
        let block = block_with(&serde_json::json!([transfer("alice", "alice", 5_000)]));
        assert!(edges_from_block(&block, 1).is_empty());
    }

    #[test]
    fn inner_instructions_are_collected_too() {
        let mut block = block_with(&serde_json::json!([]));
        block["transactions"][0]["meta"]["innerInstructions"] = serde_json::json!([
            { "instructions": [transfer("pool", "user", 9_000)] }
        ]);
        let edges = edges_from_block(&block, 7);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, WalletId::from("pool"));
    }

    #[test]
    fn malformed_instructions_are_skipped_rather_than_panicking() {
        let block = block_with(&serde_json::json!([
            { "program": "system", "parsed": { "type": "transfer", "info": {} } },
            { "program": "system" },
            serde_json::Value::Null,
        ]));
        assert!(edges_from_block(&block, 1).is_empty());
    }

    #[test]
    fn depositor_counts_reflect_distinct_sources() {
        let graph = FundingGraph::from_edges(vec![
            FundingEdge {
                source: "a".into(),
                target: "pool".into(),
                lamports: 1,
                slot: 1,
                payer: None,
            },
            FundingEdge {
                source: "b".into(),
                target: "pool".into(),
                lamports: 1,
                slot: 1,
                payer: None,
            },
            FundingEdge {
                source: "a".into(),
                target: "pool".into(),
                lamports: 1,
                slot: 2,
                payer: None,
            },
        ]);
        let counts = depositor_counts(&graph);
        assert_eq!(counts.get(&WalletId::from("pool")), Some(&2));
    }
}
