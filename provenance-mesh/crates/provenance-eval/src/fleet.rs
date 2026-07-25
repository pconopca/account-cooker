//! Finding star-funded wallet clusters in a real funding graph.
//!
//! The synthetic topologies in [`scenario`] show what a star costs you. This
//! module looks for the same shape in data nobody constructed, so the claim
//! stops being "a simulated fleet would be attributable" and becomes "these
//! wallets, funded this way, are attributable right now".
//!
//! # The discriminator
//!
//! Both an exchange hot wallet and a fleet's funding wallet pay a great many
//! accounts. What separates them is who pays *them*:
//!
//! - a pool is paid by many, so each payout it makes could have come from any
//!   of its depositors, and its recipients inherit that crowd;
//! - a star is paid by nobody in particular, so every wallet it funds traces
//!   back to one identifiable origin and inherits nothing.
//!
//! Out-degree alone cannot tell them apart. Out-degree together with in-degree
//! can.
//!
//! [`scenario`]: crate::scenario

use provenance_core::{FundingGraph, WalletId};

/// What a funder's shape says about the anonymity it passes on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunderShape {
    /// Paid by many. Its recipients inherit a crowd.
    Pool,
    /// Paid by almost nobody. Its recipients inherit a single identifiable origin.
    Star,
}

/// A wallet that funds several others, and the shape of its own provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    /// The funding wallet.
    pub funder: WalletId,
    /// Distinct wallets it paid within the observed window.
    pub recipients: usize,
    /// Distinct wallets that paid *it* within the observed window.
    pub inbound_funders: usize,
    /// Whether it passes on a crowd or a single origin.
    pub shape: FunderShape,
}

/// Result of scanning a graph for funding clusters.
#[derive(Clone, Debug, Default)]
pub struct Scan {
    /// Clusters found, largest first.
    pub clusters: Vec<Cluster>,
}

impl Scan {
    /// Clusters whose recipients inherit no crowd.
    pub fn stars(&self) -> impl Iterator<Item = &Cluster> {
        self.clusters
            .iter()
            .filter(|cluster| cluster.shape == FunderShape::Star)
    }

    /// Clusters whose recipients inherit a crowd.
    pub fn pools(&self) -> impl Iterator<Item = &Cluster> {
        self.clusters
            .iter()
            .filter(|cluster| cluster.shape == FunderShape::Pool)
    }

    /// Wallets funded by a star, and therefore individually attributable.
    #[must_use]
    pub fn wallets_under_stars(&self) -> usize {
        self.stars().map(|cluster| cluster.recipients).sum()
    }

    /// Wallets funded by a pool.
    #[must_use]
    pub fn wallets_under_pools(&self) -> usize {
        self.pools().map(|cluster| cluster.recipients).sum()
    }
}

/// Scan `graph` for wallets that funded at least `min_recipients` others.
///
/// A funder paid by at most `star_inbound_ceiling` distinct wallets is
/// classified as a star.
///
/// # Window truncation
///
/// On a graph built from a bounded block window, a genuine pool that happened
/// to receive nothing during the window looks like a star. The classification
/// is therefore a *candidate* one, and the star count is an upper bound. The
/// recipients' measured anonymity is not affected by this: their effective `k`
/// is computed from what the ledger actually shows, and a funder with no
/// observed depositors genuinely offers its recipients no observable crowd.
#[must_use]
pub fn scan(graph: &FundingGraph, min_recipients: usize, star_inbound_ceiling: usize) -> Scan {
    let mut clusters: Vec<Cluster> = graph
        .funding_wallets()
        .into_iter()
        .filter_map(|funder| {
            let recipients = graph.direct_recipients(&funder).len();
            if recipients < min_recipients {
                return None;
            }
            let inbound_funders = graph.direct_funders(&funder).len();
            let shape = if inbound_funders <= star_inbound_ceiling {
                FunderShape::Star
            } else {
                FunderShape::Pool
            };
            Some(Cluster {
                funder,
                recipients,
                inbound_funders,
                shape,
            })
        })
        .collect();

    clusters.sort_by(|left, right| {
        right
            .recipients
            .cmp(&left.recipients)
            .then_with(|| left.funder.cmp(&right.funder))
    });
    Scan { clusters }
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
            signers: Vec::new(),
        }
    }

    fn star_and_pool() -> FundingGraph {
        let mut edges = vec![];
        // A star: one operator funds five agents, nobody funds the operator.
        for index in 0..5 {
            edges.push(edge("operator", &format!("agent-{index}")));
        }
        // A pool: eight depositors in, five payouts out.
        for index in 0..8 {
            edges.push(edge(&format!("depositor-{index}"), "pool"));
        }
        for index in 0..5 {
            edges.push(edge("pool", &format!("payee-{index}")));
        }
        FundingGraph::from_edges(edges)
    }

    #[test]
    fn a_star_and_a_pool_with_equal_fan_out_are_told_apart() {
        let scan = scan(&star_and_pool(), 5, 1);
        // Both pay exactly five wallets, so out-degree alone cannot separate them.
        assert_eq!(scan.clusters.len(), 2);
        let stars: Vec<&WalletId> = scan.stars().map(|cluster| &cluster.funder).collect();
        let pools: Vec<&WalletId> = scan.pools().map(|cluster| &cluster.funder).collect();
        assert_eq!(stars, vec![&WalletId::from("operator")]);
        assert_eq!(pools, vec![&WalletId::from("pool")]);
    }

    #[test]
    fn coverage_is_reported_per_shape() {
        let scan = scan(&star_and_pool(), 5, 1);
        assert_eq!(scan.wallets_under_stars(), 5);
        assert_eq!(scan.wallets_under_pools(), 5);
    }

    #[test]
    fn funders_below_the_threshold_are_not_clusters() {
        let graph = FundingGraph::from_edges(vec![edge("small", "a"), edge("small", "b")]);
        assert!(scan(&graph, 5, 1).clusters.is_empty());
        assert_eq!(scan(&graph, 2, 1).clusters.len(), 1);
    }

    #[test]
    fn clusters_are_ordered_by_reach() {
        let mut edges = vec![];
        for index in 0..3 {
            edges.push(edge("small", &format!("s-{index}")));
        }
        for index in 0..9 {
            edges.push(edge("large", &format!("l-{index}")));
        }
        let scan = scan(&FundingGraph::from_edges(edges), 3, 1);
        assert_eq!(scan.clusters[0].funder, WalletId::from("large"));
        assert_eq!(scan.clusters[0].recipients, 9);
    }

    #[test]
    fn the_ceiling_controls_the_classification() {
        // One inbound funder: a star under the default ceiling, a pool if the
        // caller insists on a stricter reading.
        let graph = FundingGraph::from_edges(vec![
            edge("upstream", "middle"),
            edge("middle", "a"),
            edge("middle", "b"),
        ]);
        assert_eq!(scan(&graph, 2, 1).stars().count(), 1);
        assert_eq!(scan(&graph, 2, 0).pools().count(), 1);
    }
}
