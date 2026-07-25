//! Scanning a specific set of wallets rather than a window of blocks.
//!
//! The block sampler answers "what does Solana look like". This answers the
//! question an operator actually has: **are my wallets attributable, and to
//! what?**
//!
//! It needs no crowd, no protocol adoption, and no agreement with anything else
//! in this workspace. You point it at addresses you control and it reports a
//! number about them.
//!
//! # How the graph is reconstructed
//!
//! For each wallet, its recent transactions are fetched and every transfer
//! *into* it recorded — those senders are its funders. Each distinct funder is
//! then scanned the same way, which reveals how many separate parties pay into
//! it, and therefore how large a crowd its payouts hide among.
//!
//! Two hops is enough. The question is not the full ancestry of the money; it
//! is whether the wallet's immediate funder was itself paid by a crowd.

use std::collections::BTreeSet;
use std::thread::sleep;
use std::time::Duration;

use provenance_core::{FundingEdge, FundingGraph, WalletId};

use crate::{rpc_call, SampleError};

/// Transactions fetched per address.
///
/// A newly funded agent wallet has very few, so this reaches its funding event
/// comfortably. A busy funder has far more, in which case the sample bounds how
/// much of its crowd is observed — always understating it, never the reverse.
pub const SIGNATURES_PER_ADDRESS: usize = 100;

/// Transactions requested per batched RPC round-trip.
const BATCH_SIZE: usize = 25;

/// Pause between address scans, to stay under the public endpoint's rate limit.
const PACING: Duration = Duration::from_millis(600);

/// Fetch recent transaction signatures for `address`, newest first.
///
/// # Errors
/// Returns [`SampleError`] when the endpoint fails.
pub fn signatures_for(
    endpoint: &str,
    address: &WalletId,
    limit: usize,
) -> Result<Vec<String>, SampleError> {
    let params = serde_json::json!([address.as_str(), { "limit": limit }]);
    let value = rpc_call(endpoint, "getSignaturesForAddress", &params)?;
    Ok(value
        .get("result")
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| entry.get("err").is_none_or(serde_json::Value::is_null))
                .filter_map(|entry| {
                    entry
                        .get("signature")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Fetch and parse transactions, batching to keep round-trips down.
///
/// A signature that cannot be fetched is skipped rather than failing the scan:
/// public endpoints prune history, and a partial graph understates linkage
/// rather than inventing it.
///
/// # Errors
/// Returns [`SampleError`] when the endpoint itself fails.
pub fn edges_for_signatures(
    endpoint: &str,
    signatures: &[String],
) -> Result<Vec<FundingEdge>, SampleError> {
    let mut edges = Vec::new();
    for chunk in signatures.chunks(BATCH_SIZE) {
        let batch: Vec<serde_json::Value> = chunk
            .iter()
            .enumerate()
            .map(|(index, signature)| {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": index,
                    "method": "getTransaction",
                    "params": [signature, {
                        "encoding": "jsonParsed",
                        "maxSupportedTransactionVersion": 0
                    }],
                })
            })
            .collect();

        let response = crate::rpc_raw(endpoint, &serde_json::Value::Array(batch))?;
        let Some(entries) = response.as_array() else {
            continue;
        };
        for entry in entries {
            let Some(result) = entry.get("result") else {
                continue;
            };
            if result.is_null() {
                continue;
            }
            let slot = result
                .get("slot")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            edges.extend(crate::edges_from_transaction(result, slot));
        }
    }
    Ok(edges)
}

/// Every transfer into or out of `address` that recent history shows.
///
/// # Errors
/// Returns [`SampleError`] when the endpoint fails.
pub fn edges_for_address(
    endpoint: &str,
    address: &WalletId,
    limit: usize,
) -> Result<Vec<FundingEdge>, SampleError> {
    let signatures = signatures_for(endpoint, address, limit)?;
    sleep(PACING);
    let edges = edges_for_signatures(endpoint, &signatures)?;
    sleep(PACING);
    Ok(edges)
}

/// Build a funding graph for `wallets` and the accounts that funded them.
///
/// Reports progress through `on_progress`, since a fleet scan is slow enough
/// that silence looks like a hang.
///
/// # Errors
/// Returns [`SampleError`] when the endpoint fails.
pub fn scan_wallets<F>(
    endpoint: &str,
    wallets: &[WalletId],
    mut on_progress: F,
) -> Result<FundingGraph, SampleError>
where
    F: FnMut(&str, usize, usize),
{
    let mut edges: Vec<FundingEdge> = Vec::new();

    // Hop one: what paid these wallets.
    for (index, wallet) in wallets.iter().enumerate() {
        on_progress("wallet", index + 1, wallets.len());
        edges.extend(edges_for_address(endpoint, wallet, SIGNATURES_PER_ADDRESS)?);
    }

    // Hop two: what paid *those*, which is what decides whether the funder is a
    // crowd or a single identifiable origin.
    let targets: BTreeSet<WalletId> = wallets.iter().cloned().collect();
    let funders: BTreeSet<WalletId> = edges
        .iter()
        .filter(|edge| targets.contains(&edge.target))
        .map(|edge| edge.source.clone())
        .filter(|source| !targets.contains(source))
        .collect();

    for (index, funder) in funders.iter().enumerate() {
        on_progress("funder", index + 1, funders.len());
        edges.extend(edges_for_address(endpoint, funder, SIGNATURES_PER_ADDRESS)?);
    }

    // The same transfer can arrive twice when both endpoints were scanned.
    edges.sort_by(|left, right| {
        (
            &left.source,
            &left.target,
            left.lamports,
            left.slot,
            &left.payer,
        )
            .cmp(&(
                &right.source,
                &right.target,
                right.lamports,
                right.slot,
                &right.payer,
            ))
    });
    edges.dedup();

    Ok(FundingGraph::from_edges(edges))
}

/// Parse addresses from a newline-separated list, ignoring blanks and comments.
#[must_use]
pub fn parse_wallet_list(text: &str) -> Vec<WalletId> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| WalletId(line.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_lists_ignore_blanks_and_comments() {
        let parsed =
            parse_wallet_list("# my fleet\n\nAlice111\n  Bob222  \n\n# a note\nCarol333\n");
        assert_eq!(
            parsed,
            vec![
                WalletId("Alice111".to_owned()),
                WalletId("Bob222".to_owned()),
                WalletId("Carol333".to_owned()),
            ]
        );
    }

    #[test]
    fn an_empty_list_yields_no_wallets() {
        assert!(parse_wallet_list("").is_empty());
        assert!(parse_wallet_list("# only comments\n\n").is_empty());
    }
}
