//! Funding-graph attacks an observer can run with nothing but public chain data.
//!
//! Each attack scores an unordered pair of wallets: higher means "more likely
//! the same operator". None of them may read [`EntityId`]; ground truth is used
//! only afterwards, to score the attack.
//!
//! [`EntityId`]: provenance_core::EntityId

use std::collections::BTreeSet;

use provenance_core::{FundingGraph, WalletId};

/// Jaccard similarity of two sets. Two empty sets share no evidence, so the
/// similarity is 0 rather than the vacuous 1 that `0/0` would suggest.
fn jaccard(left: &BTreeSet<WalletId>, right: &BTreeSet<WalletId>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.union(right).count();
    if union == 0 {
        return 0.0;
    }
    // Counts are bounded by the wallet count, far inside f64's exact range.
    #[allow(clippy::cast_precision_loss)]
    {
        intersection as f64 / union as f64
    }
}

/// A pairwise wallet-linkage attack.
pub trait PairwiseAttack {
    /// Stable name used in reports.
    fn name(&self) -> &'static str;

    /// Linkage score in `[0, 1]` for an unordered pair of wallets.
    fn score(&self, graph: &FundingGraph, left: &WalletId, right: &WalletId) -> f64;
}

/// Overlap of the wallets that funded each side directly, in one hop.
///
/// This is the exact signal `cooker-eval` reads as `FeatureFamily::Funding`,
/// reimplemented here so results are comparable against the number that crate
/// already publishes. Against a fleet funded from one hot wallet it is a
/// perfect classifier.
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectFunderJaccard;

impl PairwiseAttack for DirectFunderJaccard {
    fn name(&self) -> &'static str {
        "direct-funder-jaccard"
    }

    fn score(&self, graph: &FundingGraph, left: &WalletId, right: &WalletId) -> f64 {
        jaccard(&graph.direct_funders(left), &graph.direct_funders(right))
    }
}

/// Overlap of everything reachable backwards within `max_depth` hops.
///
/// The strictly stronger attack, and the reason hop-chaining is not a defence:
/// inserting intermediate wallets between a source and its agents defeats
/// [`DirectFunderJaccard`] while leaving this one untouched, because the source
/// is still a common ancestor a few hops further back.
#[derive(Clone, Copy, Debug)]
pub struct AncestorJaccard {
    /// How many hops backwards the observer is willing to walk.
    pub max_depth: usize,
}

impl AncestorJaccard {
    /// Construct an attack that walks `max_depth` hops backwards.
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }
}

impl PairwiseAttack for AncestorJaccard {
    fn name(&self) -> &'static str {
        "ancestor-jaccard"
    }

    fn score(&self, graph: &FundingGraph, left: &WalletId, right: &WalletId) -> f64 {
        jaccard(
            &graph.ancestors(left, self.max_depth),
            &graph.ancestors(right, self.max_depth),
        )
    }
}

/// Binary "do these two share any funding ancestor at all" test.
///
/// Coarser than [`AncestorJaccard`] and included because it is the cheapest
/// attack that still works: it needs no similarity threshold and no tuning, so
/// it is what a commodity analytics pipeline actually runs at scale.
#[derive(Clone, Copy, Debug)]
pub struct SharedAncestorIndicator {
    /// How many hops backwards the observer is willing to walk.
    pub max_depth: usize,
}

impl SharedAncestorIndicator {
    /// Construct an indicator attack walking `max_depth` hops backwards.
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }
}

impl PairwiseAttack for SharedAncestorIndicator {
    fn name(&self) -> &'static str {
        "shared-ancestor-indicator"
    }

    fn score(&self, graph: &FundingGraph, left: &WalletId, right: &WalletId) -> f64 {
        let left_ancestors = graph.ancestors(left, self.max_depth);
        let right_ancestors = graph.ancestors(right, self.max_depth);
        f64::from(u8::from(
            left_ancestors
                .intersection(&right_ancestors)
                .next()
                .is_some(),
        ))
    }
}

/// Exploits the deposit counts a pooled round publishes.
///
/// A round hides *which* payout a deposit paid for, but not *how many* deposits
/// each funder made. If a pool received eight deposits and one funder sent three
/// of them, an observer knows three of the eight payouts belong to that funder
/// without knowing which — and that is enough to compute, for any two payouts
/// from that pool, the exact probability they share an owner.
///
/// The score is that probability: `sum n_i(n_i - 1) / N(N - 1)` over funders,
/// which is the chance two distinct payouts drawn from the pool came from the
/// same funder. It is a genuine posterior from public data, not a heuristic.
///
/// Pairs funded by different pools score 0: this attack speaks only to pairs a
/// pool could have confused, and says nothing about the rest.
#[derive(Clone, Copy, Debug, Default)]
pub struct DepositShareAttack;

impl PairwiseAttack for DepositShareAttack {
    fn name(&self) -> &'static str {
        "deposit-share"
    }

    fn score(&self, graph: &FundingGraph, left: &WalletId, right: &WalletId) -> f64 {
        let left_funders = graph.direct_funders(left);
        // Only meaningful when both were paid by the same single account.
        if left_funders.len() != 1 || left_funders != graph.direct_funders(right) {
            return 0.0;
        }
        let Some(pool) = left_funders.iter().next() else {
            return 0.0;
        };

        let counts = graph.inbound_edge_counts(pool);
        let total: usize = counts.values().sum();
        if total < 2 {
            return 0.0;
        }
        let collisions: usize = counts
            .values()
            .map(|count| count * count.saturating_sub(1))
            .sum();

        #[allow(clippy::cast_precision_loss)]
        {
            collisions as f64 / (total * (total - 1)) as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use provenance_core::FundingEdge;

    use super::*;

    fn edge(source: &str, target: &str) -> FundingEdge {
        FundingEdge {
            source: source.into(),
            target: target.into(),
            lamports: 1_000_000,
            slot: 1,
            payer: None,
        }
    }

    #[test]
    fn shared_hot_wallet_is_perfectly_linkable() {
        let graph = FundingGraph::from_edges(vec![edge("hot", "a"), edge("hot", "b")]);
        let score = DirectFunderJaccard.score(&graph, &"a".into(), &"b".into());
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distinct_funders_score_zero() {
        let graph = FundingGraph::from_edges(vec![edge("hot-1", "a"), edge("hot-2", "b")]);
        let score = DirectFunderJaccard.score(&graph, &"a".into(), &"b".into());
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn hop_chaining_defeats_the_shallow_attack_only() {
        // src funds both agents, but through one private intermediate each.
        let graph = FundingGraph::from_edges(vec![
            edge("src", "hop-a"),
            edge("hop-a", "a"),
            edge("src", "hop-b"),
            edge("hop-b", "b"),
        ]);
        let left = WalletId::from("a");
        let right = WalletId::from("b");

        // One hop back: the intermediates differ, so the naive attack sees nothing.
        assert!(DirectFunderJaccard.score(&graph, &left, &right).abs() < f64::EPSILON);

        // Two hops back: the shared source reappears and the pair links again.
        assert!(AncestorJaccard::new(2).score(&graph, &left, &right) > 0.0);
        assert!(
            (SharedAncestorIndicator::new(2).score(&graph, &left, &right) - 1.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn unfunded_pairs_share_no_evidence() {
        let graph = FundingGraph::from_edges(vec![edge("hot", "a")]);
        // Neither side has a funder; the score must be 0, not a vacuous 1.
        let score = DirectFunderJaccard.score(&graph, &"x".into(), &"y".into());
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn deposit_share_reads_the_pool_composition() {
        // A pool of four payouts where one funder sent three deposits: two
        // distinct payouts collide with probability (3*2 + 1*0) / (4*3) = 0.5.
        let graph = FundingGraph::from_edges(vec![
            edge("src-a", "pool"),
            edge("src-a", "pool"),
            edge("src-a", "pool"),
            edge("src-b", "pool"),
            edge("pool", "w1"),
            edge("pool", "w2"),
            edge("pool", "w3"),
            edge("pool", "w4"),
        ]);
        let score = DepositShareAttack.score(&graph, &"w1".into(), &"w2".into());
        assert!((score - 0.5).abs() < 1e-12, "got {score}");
    }

    #[test]
    fn a_balanced_pool_leaks_less_than_a_dominated_one() {
        let dominated = FundingGraph::from_edges(vec![
            edge("src-a", "pool"),
            edge("src-a", "pool"),
            edge("src-a", "pool"),
            edge("src-b", "pool"),
            edge("pool", "w1"),
            edge("pool", "w2"),
            edge("pool", "w3"),
            edge("pool", "w4"),
        ]);
        let balanced = FundingGraph::from_edges(vec![
            edge("src-a", "pool"),
            edge("src-b", "pool"),
            edge("src-c", "pool"),
            edge("src-d", "pool"),
            edge("pool", "w1"),
            edge("pool", "w2"),
            edge("pool", "w3"),
            edge("pool", "w4"),
        ]);
        let left = WalletId::from("w1");
        let right = WalletId::from("w2");
        // Four funders one deposit each: no two payouts can share an owner.
        assert!(DepositShareAttack.score(&balanced, &left, &right).abs() < f64::EPSILON);
        assert!(
            DepositShareAttack.score(&dominated, &left, &right)
                > DepositShareAttack.score(&balanced, &left, &right)
        );
    }

    #[test]
    fn deposit_share_is_silent_across_different_pools() {
        let graph = FundingGraph::from_edges(vec![
            edge("src-a", "pool-1"),
            edge("src-a", "pool-1"),
            edge("pool-1", "w1"),
            edge("pool-1", "w2"),
            edge("src-a", "pool-2"),
            edge("src-a", "pool-2"),
            edge("pool-2", "w3"),
            edge("pool-2", "w4"),
        ]);
        assert!(
            DepositShareAttack
                .score(&graph, &"w1".into(), &"w3".into())
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn indicator_is_binary() {
        let graph = FundingGraph::from_edges(vec![edge("hot", "a"), edge("hot", "b")]);
        let score = SharedAncestorIndicator::new(4).score(&graph, &"a".into(), &"b".into());
        assert!((score - 1.0).abs() < f64::EPSILON || score.abs() < f64::EPSILON);
    }
}
