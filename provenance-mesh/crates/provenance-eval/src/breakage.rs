//! Does passing through a pooled account actually break provenance?
//!
//! A pool with a thousand depositors looks like it must confer anonymity. Very
//! often it confers none, and the reason is not visible in the funding graph
//! alone: it is visible in who *signed* the payout.
//!
//! # The test
//!
//! Take a pooled account `P`. Someone deposits into it, and later it pays out
//! to `B`. Two cases:
//!
//! - **The payout is signed by one of `P`'s own depositors.** That depositor
//!   authorised the transfer and named the destination, so the transaction
//!   itself publishes the link between a specific deposit and a specific
//!   payout. The pool hid nothing. This is what a protocol vault typically
//!   looks like: you deposit, and later you sign a withdrawal naming where the
//!   funds go.
//!
//! - **The payout is signed by someone with no deposit into `P`.** No on-chain
//!   record connects any depositor to `B`. The link, if it exists at all, lives
//!   off-chain. This is what a custodial hot wallet looks like: the exchange
//!   signs, and the mapping is in its database.
//!
//! Fan-in and fan-out cannot tell these apart. The signer can.

use std::collections::BTreeSet;

use provenance_core::{FundingGraph, WalletId};

/// How much of a pooled account's outflow is unlinkable.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakage {
    /// The pooled account under test.
    pub pool: WalletId,
    /// Distinct accounts that deposited into it.
    pub depositors: usize,
    /// Outbound transfers examined.
    pub payouts: usize,
    /// Payouts signed by one of the pool's own depositors.
    pub self_signed: usize,
    /// Payouts whose signer never deposited into the pool.
    pub third_party_signed: usize,
    /// Payouts whose signer could not be determined.
    pub unknown_signer: usize,
    /// Payouts signed by the account receiving them.
    ///
    /// The truncation-robust half of the test. Whether a payout's signer also
    /// *deposited* depends on how far back the observation window reaches, and
    /// a short window will call a genuine passthrough a break. Whether the
    /// signer is the recipient is decided inside the payout transaction alone,
    /// so no window can distort it.
    ///
    /// A recipient that signs its own payout is pulling funds it already had a
    /// claim on. That claim is what links it to the pool, whatever the deposit
    /// history shows.
    pub recipient_signed: usize,
}

impl Breakage {
    /// Share of payouts that carry no on-chain link back to a depositor.
    ///
    /// 1.0 means every payout was authorised by a stranger, so the pool is a
    /// genuine provenance break. 0.0 means every payout was signed by someone
    /// who had deposited, so the pool is a passthrough that hides nothing.
    ///
    /// Payouts with an unknown signer are excluded from the denominator rather
    /// than guessed either way; `None` when nothing could be classified.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn break_rate(&self) -> Option<f64> {
        let classified = self.self_signed + self.third_party_signed;
        if classified == 0 {
            return None;
        }
        Some(self.third_party_signed as f64 / classified as f64)
    }

    /// Whether this pool confers anonymity on the wallets it pays.
    ///
    /// Requires both a crowd to hide in and payouts that a stranger authorised.
    #[must_use]
    pub fn is_effective(&self, min_depositors: usize, min_break_rate: f64) -> bool {
        self.depositors >= min_depositors
            && self.break_rate().is_some_and(|rate| rate >= min_break_rate)
    }

    /// Share of payouts the recipient signed for itself.
    ///
    /// Unlike [`break_rate`], this is unaffected by how far back the window
    /// reaches. A high value means the pool is a self-service withdrawal
    /// mechanism, and self-service is linkable.
    ///
    /// [`break_rate`]: Self::break_rate
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn recipient_signed_rate(&self) -> Option<f64> {
        let known = self.payouts - self.unknown_signer;
        if known == 0 {
            return None;
        }
        Some(self.recipient_signed as f64 / known as f64)
    }
}

/// Measure whether `pool` breaks the link between its deposits and its payouts.
#[must_use]
pub fn measure(graph: &FundingGraph, pool: &WalletId) -> Breakage {
    let depositors: BTreeSet<WalletId> = graph.direct_funders(pool);

    let mut self_signed = 0_usize;
    let mut third_party_signed = 0_usize;
    let mut unknown_signer = 0_usize;
    let mut recipient_signed = 0_usize;

    for edge in graph.edges().iter().filter(|edge| &edge.source == pool) {
        if edge.signers.contains(&edge.target) {
            recipient_signed += 1;
        }
        if edge.signers.is_empty() {
            unknown_signer += 1;
        } else if edge
            .signers
            .iter()
            // The pool signing for itself is not a depositor authorising a
            // withdrawal: it is the pool acting as its own authority, which
            // says nothing about which deposit funded this payout.
            .any(|signer| signer != pool && depositors.contains(signer))
        {
            self_signed += 1;
        } else {
            third_party_signed += 1;
        }
    }

    Breakage {
        pool: pool.clone(),
        depositors: depositors.len(),
        payouts: self_signed + third_party_signed + unknown_signer,
        self_signed,
        third_party_signed,
        unknown_signer,
        recipient_signed,
    }
}

#[cfg(test)]
mod tests {
    use provenance_core::FundingEdge;

    use super::*;

    fn edge(source: &str, target: &str, payer: Option<&str>) -> FundingEdge {
        signed(source, target, payer.into_iter().collect())
    }

    /// An edge carrying an explicit signer set.
    fn signed(source: &str, target: &str, signers: Vec<&str>) -> FundingEdge {
        FundingEdge {
            source: source.into(),
            target: target.into(),
            lamports: 1_000_000,
            slot: 1,
            signers: signers.into_iter().map(Into::into).collect(),
        }
    }

    #[test]
    fn a_vault_whose_depositors_sign_their_own_withdrawals_breaks_nothing() {
        // Three depositors, three payouts, each signed by the depositor that
        // put the money in. Fan-in of 3 looks like a crowd and is not one.
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "vault", Some("alice")),
            edge("bob", "vault", Some("bob")),
            edge("carol", "vault", Some("carol")),
            edge("vault", "alice-2", Some("alice")),
            edge("vault", "bob-2", Some("bob")),
            edge("vault", "carol-2", Some("carol")),
        ]);
        let breakage = measure(&graph, &"vault".into());
        assert_eq!(breakage.depositors, 3);
        assert_eq!(breakage.self_signed, 3);
        assert_eq!(breakage.break_rate(), Some(0.0));
        assert!(!breakage.is_effective(2, 0.5));
    }

    #[test]
    fn a_custodian_signing_its_own_payouts_is_a_real_break() {
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "exchange", Some("alice")),
            edge("bob", "exchange", Some("bob")),
            edge("exchange", "withdrawal-1", Some("exchange")),
            edge("exchange", "withdrawal-2", Some("exchange")),
        ]);
        let breakage = measure(&graph, &"exchange".into());
        assert_eq!(breakage.break_rate(), Some(1.0));
        assert!(breakage.is_effective(2, 0.5));
    }

    #[test]
    fn a_relayer_unrelated_to_the_pool_also_breaks_the_link() {
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "pool", Some("alice")),
            edge("bob", "pool", Some("bob")),
            edge("pool", "out-1", Some("relayer")),
            edge("pool", "out-2", Some("relayer")),
        ]);
        assert_eq!(measure(&graph, &"pool".into()).break_rate(), Some(1.0));
    }

    #[test]
    fn mixed_pools_report_a_fractional_rate() {
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "pool", Some("alice")),
            edge("bob", "pool", Some("bob")),
            edge("pool", "out-1", Some("alice")),
            edge("pool", "out-2", Some("relayer")),
        ]);
        let breakage = measure(&graph, &"pool".into());
        assert_eq!(breakage.self_signed, 1);
        assert_eq!(breakage.third_party_signed, 1);
        assert_eq!(breakage.break_rate(), Some(0.5));
    }

    #[test]
    fn a_depositor_who_signs_but_does_not_pay_the_fee_still_counts() {
        // The reason this reads every signer instead of the fee payer alone.
        // Alice deposited and authorised her own withdrawal; a relayer merely
        // covered the fee. Reading only the payer would call this a break.
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "vault", Some("alice")),
            edge("bob", "vault", Some("bob")),
            signed("vault", "alice-2", vec!["relayer", "alice"]),
        ]);
        let breakage = measure(&graph, &"vault".into());
        assert_eq!(breakage.self_signed, 1);
        assert_eq!(breakage.third_party_signed, 0);
        assert_eq!(breakage.break_rate(), Some(0.0));
    }

    #[test]
    fn unknown_signers_are_excluded_rather_than_assumed() {
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "pool", Some("alice")),
            edge("bob", "pool", Some("bob")),
            edge("pool", "out-1", None),
            edge("pool", "out-2", Some("relayer")),
        ]);
        let breakage = measure(&graph, &"pool".into());
        assert_eq!(breakage.unknown_signer, 1);
        // One unknown, one genuine break: the rate reflects only what is known.
        assert_eq!(breakage.break_rate(), Some(1.0));
        assert_eq!(breakage.payouts, 2);
    }

    #[test]
    fn a_pool_with_no_classifiable_payouts_reports_no_rate() {
        let graph = FundingGraph::from_edges(vec![edge("alice", "pool", Some("alice"))]);
        assert_eq!(measure(&graph, &"pool".into()).break_rate(), None);
    }

    #[test]
    fn a_break_without_a_crowd_is_not_effective() {
        // Perfect break rate, but only one depositor to hide among.
        let graph = FundingGraph::from_edges(vec![
            edge("alice", "pool", Some("alice")),
            edge("pool", "out-1", Some("relayer")),
        ]);
        let breakage = measure(&graph, &"pool".into());
        assert_eq!(breakage.break_rate(), Some(1.0));
        assert!(!breakage.is_effective(2, 0.5));
    }
}
