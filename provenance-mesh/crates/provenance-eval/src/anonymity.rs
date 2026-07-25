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

/// The observer's posterior over who funded `wallet`, as unnormalised weights.
///
/// A wallet paid directly by a root source yields a point mass: the observer is
/// certain. A wallet paid by a pool yields the pool's deposit counts, since a
/// payout is equally likely to belong to any deposit the pool received.
fn provenance_posterior(graph: &FundingGraph, wallet: &WalletId) -> Vec<f64> {
    // Borrowed rather than cloned throughout: this runs once per wallet across
    // tens of thousands of wallets.
    let Some(funders) = graph.inbound_edge_counts_ref(wallet) else {
        return vec![1.0];
    };
    if funders.len() != 1 {
        // No funder, or an ambiguous multi-funder wallet we make no claim about.
        return vec![1.0];
    }
    let Some(funder) = funders.keys().next() else {
        return vec![1.0];
    };

    let Some(counts) = graph.inbound_edge_counts_ref(funder) else {
        // The funder is a root: it *is* the origin, and the observer knows it.
        return vec![1.0];
    };
    if counts.is_empty() {
        return vec![1.0];
    }
    #[allow(clippy::cast_precision_loss)]
    counts.values().map(|count| *count as f64).collect()
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
    fn empty_input_is_reported_rather_than_dividing_by_zero() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let report = measure(&scenario.graph, &[]);
        assert_eq!(report.wallets, 0);
    }
}
