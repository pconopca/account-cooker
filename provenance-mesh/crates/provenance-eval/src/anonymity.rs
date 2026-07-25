//! Absolute provenance anonymity, as opposed to attacker advantage.
//!
//! ROC AUC answers "does the attacker beat the base rate?". It is the right
//! question for a linkage attack and the wrong one for absolute anonymity: a
//! fleet split between two operators scores AUC 0.5 — the attacker learns
//! nothing *beyond* the prior — while the prior alone already narrows every
//! wallet down to one of two owners.
//!
//! This module reports the other half: given only public data, how uncertain is
//! an observer about who owns a given wallet? The answer is expressed as an
//! effective anonymity set, directly comparable to the nominal `k` a pooled
//! round advertises.

use provenance_core::{FundingGraph, WalletId};

use crate::metrics::{effective_set_size, min_entropy, shannon_entropy};

/// Absolute provenance anonymity across a set of wallets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnonymityReport {
    /// Wallets scored.
    pub wallets: usize,
    /// Mean effective anonymity set under Shannon entropy.
    ///
    /// The size of the uniform candidate set that would leave an observer
    /// equally uncertain on average.
    pub mean_effective_k: f64,
    /// Mean effective anonymity set under min-entropy.
    ///
    /// Governed by the single most likely candidate, so it is the number that
    /// bounds a guessing attacker. Always at most `mean_effective_k`.
    pub mean_effective_k_min: f64,
}

/// The observer's posterior over who ultimately paid for `wallet`, as
/// unnormalised weights.
///
/// A wallet paid directly by a root source yields a point mass: the observer is
/// certain. A wallet paid by a pool yields the pool's deposit counts, since a
/// payout is equally likely to belong to any deposit the pool received.
///
/// A wallet with several funders is not a special case and must not be treated
/// as one. Each funder contributes its own candidate set, weighted by how much
/// of this wallet's funding arrived through it. An earlier version returned a
/// point mass here while its comment claimed to make no assertion — recording
/// the strongest possible claim about roughly 5% of accounts it had declared
/// out of scope. The two disagreed, and the comment was the honest one.
fn provenance_posterior(graph: &FundingGraph, wallet: &WalletId) -> Vec<f64> {
    // Borrowed rather than cloned throughout: this runs once per wallet across
    // tens of thousands of wallets.
    let Some(funders) = graph.inbound_edge_counts_ref(wallet) else {
        return vec![1.0];
    };
    if funders.is_empty() {
        return vec![1.0];
    }

    let mut weights = Vec::new();
    for (funder, arrivals) in funders {
        #[allow(clippy::cast_precision_loss)]
        let via = *arrivals as f64;
        match graph.inbound_edge_counts_ref(funder) {
            // The funder is a root within the observed window: it *is* the
            // origin, and it accounts for this share of the wallet's funding.
            None => weights.push(via),
            Some(crowd) if crowd.is_empty() => weights.push(via),
            Some(crowd) => {
                #[allow(clippy::cast_precision_loss)]
                let total: f64 = crowd.values().map(|count| *count as f64).sum();
                if total <= 0.0 {
                    weights.push(via);
                    continue;
                }
                // Spread this funder's share across the crowd it hides among.
                for count in crowd.values() {
                    #[allow(clippy::cast_precision_loss)]
                    weights.push(via * (*count as f64) / total);
                }
            }
        }
    }
    weights
}

/// Measure absolute provenance anonymity over `wallets`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn measure(graph: &FundingGraph, wallets: &[WalletId]) -> AnonymityReport {
    if wallets.is_empty() {
        return AnonymityReport {
            wallets: 0,
            mean_effective_k: 0.0,
            mean_effective_k_min: 0.0,
        };
    }

    let mut shannon_total = 0.0_f64;
    let mut min_total = 0.0_f64;
    for wallet in wallets {
        let posterior = provenance_posterior(graph, wallet);
        shannon_total += effective_set_size(shannon_entropy(&posterior));
        min_total += effective_set_size(min_entropy(&posterior));
    }

    let count = wallets.len() as f64;
    AnonymityReport {
        wallets: wallets.len(),
        mean_effective_k: shannon_total / count,
        mean_effective_k_min: min_total / count,
    }
}

#[cfg(test)]
mod tests {
    use crate::scenario::{generate, FleetSpec, FundingTopology};

    use super::*;

    const SPEC: FleetSpec = FleetSpec {
        entities: 8,
        agents_per_entity: 8,
    };

    #[test]
    fn star_funding_leaves_the_observer_certain() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let report = measure(&scenario.graph, &scenario.agents);
        // One funder, no ambiguity: an effective set of exactly one.
        assert!(
            (report.mean_effective_k - 1.0).abs() < 1e-12,
            "got {}",
            report.mean_effective_k
        );
        assert!((report.mean_effective_k_min - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cascading_hops_do_not_add_a_single_bit() {
        // The hops are private to one operator, so walking back through them
        // never widens the candidate set. This is the same conclusion the AUC
        // table reaches, measured a different way.
        let scenario = generate(SPEC, FundingTopology::Cascade { depth: 3 }, 1);
        let report = measure(&scenario.graph, &scenario.agents);
        assert!(
            (report.mean_effective_k - 1.0).abs() < 1e-12,
            "got {}",
            report.mean_effective_k
        );
    }

    #[test]
    fn pooled_rounds_widen_the_candidate_set() {
        let scenario = generate(SPEC, FundingTopology::MeshRound { round_size: 16 }, 1);
        let report = measure(&scenario.graph, &scenario.agents);
        assert!(
            report.mean_effective_k > 4.0,
            "pooling should buy real uncertainty, got {}",
            report.mean_effective_k
        );
        // Min-entropy is the stricter reading and must never exceed Shannon.
        assert!(report.mean_effective_k_min <= report.mean_effective_k);
    }

    #[test]
    fn a_lone_operator_buys_nothing_by_pooling() {
        // The honest floor: one operator's pool contains only its own deposits,
        // so every payout traces back to the one candidate that existed anyway.
        let solo = FleetSpec {
            entities: 1,
            agents_per_entity: 32,
        };
        let scenario = generate(solo, FundingTopology::MeshRound { round_size: 8 }, 1);
        let report = measure(&scenario.graph, &scenario.agents);
        assert!(
            (report.mean_effective_k - 1.0).abs() < 1e-12,
            "got {}",
            report.mean_effective_k
        );
    }

    #[test]
    fn several_funders_widen_the_candidate_set_rather_than_collapsing_it() {
        use provenance_core::FundingEdge;

        let edge = |source: &str, target: &str| FundingEdge {
            source: source.into(),
            target: target.into(),
            lamports: 1,
            slot: 1,
            signers: Vec::new(),
        };
        // One wallet paid by two roots. An observer knows it was one of two, so
        // the effective set is 2 — not 1, which is what a point-mass shortcut
        // would report.
        let graph = provenance_core::FundingGraph::from_edges(vec![
            edge("root-a", "wallet"),
            edge("root-b", "wallet"),
        ]);
        let report = measure(&graph, &[WalletId::from("wallet")]);
        assert!(
            (report.mean_effective_k - 2.0).abs() < 1e-12,
            "got {}",
            report.mean_effective_k
        );
    }

    #[test]
    fn a_multi_funder_wallet_inherits_every_crowd_it_was_paid_from() {
        use provenance_core::FundingEdge;

        let edge = |source: &str, target: &str| FundingEdge {
            source: source.into(),
            target: target.into(),
            lamports: 1,
            slot: 1,
            signers: Vec::new(),
        };
        let mut edges = vec![edge("pool", "wallet"), edge("root", "wallet")];
        for index in 0..8 {
            edges.push(edge(&format!("depositor-{index}"), "pool"));
        }
        let graph = provenance_core::FundingGraph::from_edges(edges);
        let report = measure(&graph, &[WalletId::from("wallet")]);
        // Half the funding came through a crowd of eight, half from a known
        // root: more uncertainty than the root alone, far less than the pool.
        assert!(
            report.mean_effective_k > 2.0 && report.mean_effective_k < 8.0,
            "got {}",
            report.mean_effective_k
        );
    }

    #[test]
    fn empty_input_is_reported_rather_than_dividing_by_zero() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let report = measure(&scenario.graph, &[]);
        assert_eq!(report.wallets, 0);
    }
}
