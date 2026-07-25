//! The funding graph: who paid for a wallet, and who paid for *them*.
//!
//! Wallet attribution on a public ledger starts here. A fleet whose wallets all
//! trace back to one funding source is trivially clustered regardless of how
//! human its timing or amounts look, because the funding edge is a hard,
//! permanent, first-order fact rather than a statistical signal.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

/// An on-chain account, identified by its base58 address.
///
/// Kept as an opaque string rather than a `Pubkey` so that the evaluator can
/// run on synthetic fleets without pulling in the Solana SDK.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WalletId(pub String);

impl WalletId {
    /// Borrow the underlying address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for WalletId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// The real-world operator a wallet belongs to.
///
/// This is ground truth available only to the evaluator. No attack in this
/// crate is permitted to read it; it exists solely to score attacks after the
/// fact.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EntityId(pub String);

/// A single value-bearing transfer that funded `target` from `source`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FundingEdge {
    /// Account the lamports came from.
    pub source: WalletId,
    /// Account the lamports landed in.
    pub target: WalletId,
    /// Transferred amount.
    pub lamports: u64,
    /// Slot the transfer settled in.
    pub slot: u64,
    /// Fee payer of the transaction that carried this transfer, when known.
    ///
    /// This is what decides whether a pooled account actually breaks
    /// provenance. If a pool pays out in a transaction signed by one of its own
    /// depositors, that depositor has published the link between its deposit
    /// and that payout, and the pool hid nothing. If the payout is signed by an
    /// unrelated party, the link exists only off-chain.
    ///
    /// `None` for synthetic graphs, which model transfers without transactions.
    #[serde(default)]
    pub payer: Option<WalletId>,
}

/// A directed multigraph of funding transfers, indexed for reverse traversal.
///
/// Edges point in the direction value moved (`source -> target`); attribution
/// walks them backwards, which is why the reverse index is the one built eagerly.
#[derive(Clone, Debug, Default)]
pub struct FundingGraph {
    edges: Vec<FundingEdge>,
    /// target -> source -> number of transfers.
    ///
    /// Counts rather than a set, because the multiplicity is itself an attack
    /// surface and recomputing it per query is quadratic on a real graph.
    inbound: BTreeMap<WalletId, BTreeMap<WalletId, usize>>,
    /// source -> set of direct targets.
    outbound: BTreeMap<WalletId, BTreeSet<WalletId>>,
}

impl FundingGraph {
    /// Build a graph from a set of transfers.
    ///
    /// Both directions are indexed eagerly. Attribution walks the graph
    /// backwards from tens of thousands of wallets, so an index built once at
    /// construction is the difference between a report that finishes and one
    /// that does not.
    #[must_use]
    pub fn from_edges(edges: Vec<FundingEdge>) -> Self {
        let mut inbound: BTreeMap<WalletId, BTreeMap<WalletId, usize>> = BTreeMap::new();
        let mut outbound: BTreeMap<WalletId, BTreeSet<WalletId>> = BTreeMap::new();
        for edge in &edges {
            *inbound
                .entry(edge.target.clone())
                .or_default()
                .entry(edge.source.clone())
                .or_insert(0) += 1;
            outbound
                .entry(edge.source.clone())
                .or_default()
                .insert(edge.target.clone());
        }
        Self {
            edges,
            inbound,
            outbound,
        }
    }

    /// Accounts this wallet paid directly, in one hop.
    ///
    /// Paired with [`direct_funders`], this is what separates a pool from a
    /// star: an exchange hot wallet pays many and is paid by many, while a
    /// fleet's funding wallet pays many and is paid by almost nobody.
    ///
    /// [`direct_funders`]: Self::direct_funders
    #[must_use]
    pub fn direct_recipients(&self, wallet: &WalletId) -> BTreeSet<WalletId> {
        self.outbound.get(wallet).cloned().unwrap_or_default()
    }

    /// Every wallet that sent at least one transfer.
    #[must_use]
    pub fn funding_wallets(&self) -> BTreeSet<WalletId> {
        self.outbound.keys().cloned().collect()
    }

    /// All recorded transfers.
    #[must_use]
    pub fn edges(&self) -> &[FundingEdge] {
        &self.edges
    }

    /// Number of recorded transfers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the graph holds no transfers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Accounts that funded `wallet` directly, in one hop.
    ///
    /// This is the exact signal `cooker-eval`'s `FeatureFamily::Funding` reads.
    #[must_use]
    pub fn direct_funders(&self, wallet: &WalletId) -> BTreeSet<WalletId> {
        self.inbound
            .get(wallet)
            .map(|counts| counts.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Every account reachable backwards from `wallet` within `max_depth` hops.
    ///
    /// A `max_depth` of 0 yields the empty set. Cycles are handled by the visited
    /// set, so a wallet that funds its own funder terminates rather than looping.
    /// The wallet itself is never included in its own ancestor set.
    #[must_use]
    pub fn ancestors(&self, wallet: &WalletId, max_depth: usize) -> BTreeSet<WalletId> {
        let mut seen = BTreeSet::new();
        if max_depth == 0 {
            return seen;
        }
        let mut queue = VecDeque::new();
        queue.push_back((wallet.clone(), 0_usize));
        let mut visited: BTreeSet<WalletId> = BTreeSet::new();
        visited.insert(wallet.clone());

        while let Some((current, depth)) = queue.pop_front() {
            if depth == max_depth {
                continue;
            }
            for funder in self.direct_funders(&current) {
                if visited.insert(funder.clone()) {
                    seen.insert(funder.clone());
                    queue.push_back((funder, depth + 1));
                }
            }
        }
        seen
    }

    /// Every wallet that appears as the target of at least one transfer.
    #[must_use]
    pub fn funded_wallets(&self) -> BTreeSet<WalletId> {
        self.inbound.keys().cloned().collect()
    }

    /// How many separate transfers each source sent into `wallet`.
    ///
    /// [`direct_funders`] collapses repeat transfers into a set; this keeps the
    /// multiplicity. For a pooled account the multiplicities *are* an attack
    /// surface: a funder that deposited three times into a pool which made
    /// eight payouts has told the observer that exactly three of those eight
    /// payouts are its own.
    ///
    /// [`direct_funders`]: Self::direct_funders
    #[must_use]
    pub fn inbound_edge_counts(&self, wallet: &WalletId) -> BTreeMap<WalletId, usize> {
        self.inbound.get(wallet).cloned().unwrap_or_default()
    }

    /// Borrow the inbound counts without cloning them.
    ///
    /// Preferred on hot paths that only need to read the distribution.
    #[must_use]
    pub fn inbound_edge_counts_ref(&self, wallet: &WalletId) -> Option<&BTreeMap<WalletId, usize>> {
        self.inbound.get(wallet)
    }
}

#[cfg(test)]
mod tests {
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
    fn direct_funders_are_one_hop_only() {
        let graph = FundingGraph::from_edges(vec![edge("src", "mid"), edge("mid", "leaf")]);
        assert_eq!(
            graph.direct_funders(&"leaf".into()),
            BTreeSet::from(["mid".into()])
        );
    }

    #[test]
    fn ancestors_traverse_transitively_up_to_depth() {
        let graph = FundingGraph::from_edges(vec![edge("src", "mid"), edge("mid", "leaf")]);
        let leaf = WalletId::from("leaf");
        assert_eq!(graph.ancestors(&leaf, 1), BTreeSet::from(["mid".into()]));
        assert_eq!(
            graph.ancestors(&leaf, 2),
            BTreeSet::from(["mid".into(), "src".into()])
        );
        // Depth beyond the graph adds nothing rather than erroring.
        assert_eq!(graph.ancestors(&leaf, 99), graph.ancestors(&leaf, 2));
    }

    #[test]
    fn ancestors_at_depth_zero_are_empty() {
        let graph = FundingGraph::from_edges(vec![edge("src", "leaf")]);
        assert!(graph.ancestors(&"leaf".into(), 0).is_empty());
    }

    #[test]
    fn cycles_terminate_and_exclude_the_wallet_itself() {
        let graph = FundingGraph::from_edges(vec![edge("a", "b"), edge("b", "a")]);
        let ancestors = graph.ancestors(&"a".into(), 16);
        assert_eq!(ancestors, BTreeSet::from(["b".into()]));
        assert!(!ancestors.contains(&"a".into()));
    }

    #[test]
    fn outbound_and_inbound_together_separate_a_pool_from_a_star() {
        // A star: one funder pays three, and nobody pays it.
        let star = FundingGraph::from_edges(vec![
            edge("operator", "a"),
            edge("operator", "b"),
            edge("operator", "c"),
        ]);
        let operator = WalletId::from("operator");
        assert_eq!(star.direct_recipients(&operator).len(), 3);
        assert_eq!(star.direct_funders(&operator).len(), 0);

        // A pool: pays three, and is paid by three.
        let pool = FundingGraph::from_edges(vec![
            edge("x", "pool"),
            edge("y", "pool"),
            edge("z", "pool"),
            edge("pool", "a"),
            edge("pool", "b"),
            edge("pool", "c"),
        ]);
        let pool_id = WalletId::from("pool");
        assert_eq!(pool.direct_recipients(&pool_id).len(), 3);
        assert_eq!(pool.direct_funders(&pool_id).len(), 3);
    }

    #[test]
    fn funding_wallets_lists_every_source() {
        let graph = FundingGraph::from_edges(vec![edge("a", "b"), edge("b", "c")]);
        assert_eq!(
            graph.funding_wallets(),
            BTreeSet::from(["a".into(), "b".into()])
        );
    }

    #[test]
    fn inbound_counts_keep_multiplicity_that_the_funder_set_discards() {
        let graph = FundingGraph::from_edges(vec![
            edge("a", "pool"),
            edge("a", "pool"),
            edge("a", "pool"),
            edge("b", "pool"),
        ]);
        let pool = WalletId::from("pool");
        // The set says "two funders" and loses the ratio entirely.
        assert_eq!(graph.direct_funders(&pool).len(), 2);
        // The counts say "three of four payouts belong to a".
        let counts = graph.inbound_edge_counts(&pool);
        assert_eq!(counts.get(&"a".into()), Some(&3));
        assert_eq!(counts.get(&"b".into()), Some(&1));
    }

    #[test]
    fn inbound_counts_of_an_unfunded_wallet_are_empty() {
        let graph = FundingGraph::from_edges(vec![edge("src", "leaf")]);
        assert!(graph.inbound_edge_counts(&"src".into()).is_empty());
    }

    #[test]
    fn unfunded_wallets_have_no_provenance() {
        let graph = FundingGraph::from_edges(vec![edge("src", "leaf")]);
        assert!(graph.direct_funders(&"src".into()).is_empty());
        assert!(graph.ancestors(&"src".into(), 8).is_empty());
    }
}
