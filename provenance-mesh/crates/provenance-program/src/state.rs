//! Round account state.
//!
//! What this account records, and what it deliberately does not, *is* the
//! privacy property. It records who deposited — that is public regardless,
//! since depositors sign their own transfers. It never records which recipient
//! a given deposit paid for, because that association exists nowhere on chain:
//! recipients arrive at settlement as a flat list, and the program has no way
//! to attribute them even if it wanted to.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_pubkey::Pubkey;

use crate::error::ProvenanceError;

/// Upper bound on distinct funders a single round can record.
///
/// Bounded so the account has a fixed rent cost known at open time.
pub const MAX_DEPOSITORS: usize = 32;

/// Serialized size of a [`Round`] with a full depositor roster.
pub const ROUND_ACCOUNT_LEN: usize = 1  // bump
    + 1                                 // settling flag
    + 8                                 // denomination
    + 4                                 // k_min
    + 4                                 // capacity
    + 4                                 // deposit_count
    + 4                                 // settled_count
    + 4                                 // depositor vec length prefix
    + MAX_DEPOSITORS * 32;

/// A funding round's on-chain state.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct Round {
    /// PDA bump seed.
    pub bump: u8,
    /// Whether settlement has started, freezing the anonymity set.
    pub settling: bool,
    /// Uniform payout size in lamports.
    pub denomination: u64,
    /// Distinct funders required before any payout may occur.
    pub k_min: u32,
    /// Total deposits the round accepts, and total payouts it will make.
    pub capacity: u32,
    /// Deposits received so far.
    pub deposit_count: u32,
    /// Payouts made so far.
    pub settled_count: u32,
    /// Distinct funders seen, capped at [`MAX_DEPOSITORS`].
    pub depositors: Vec<Pubkey>,
}

impl Round {
    /// Validate configuration and build the initial state.
    ///
    /// # Errors
    /// Returns the first violated invariant.
    pub fn new(
        bump: u8,
        denomination: u64,
        k_min: u32,
        capacity: u32,
    ) -> Result<Self, ProvenanceError> {
        if denomination == 0 {
            return Err(ProvenanceError::ZeroDenomination);
        }
        if k_min < 2 {
            return Err(ProvenanceError::DegenerateAnonymitySet);
        }
        if k_min as usize > MAX_DEPOSITORS {
            return Err(ProvenanceError::TooManyDepositors);
        }
        // A round cannot reach `k_min` distinct funders if it accepts fewer than
        // `k_min` deposits in total.
        if capacity == 0 || capacity < k_min {
            return Err(ProvenanceError::InvalidCapacity);
        }
        Ok(Self {
            bump,
            settling: false,
            denomination,
            k_min,
            capacity,
            deposit_count: 0,
            settled_count: 0,
            depositors: Vec::new(),
        })
    }

    /// Record one deposit from `depositor`.
    ///
    /// # Errors
    /// Fails if the round is full or already settling.
    pub fn record_deposit(&mut self, depositor: Pubkey) -> Result<(), ProvenanceError> {
        if self.settling {
            return Err(ProvenanceError::RoundSettling);
        }
        if self.deposit_count >= self.capacity {
            return Err(ProvenanceError::RoundFull);
        }
        if !self.depositors.contains(&depositor) && self.depositors.len() < MAX_DEPOSITORS {
            self.depositors.push(depositor);
        }
        self.deposit_count = self.deposit_count.saturating_add(1);
        Ok(())
    }

    /// Whether every expected deposit has landed.
    #[must_use]
    pub fn deposits_complete(&self) -> bool {
        self.deposit_count == self.capacity
    }

    /// Distinct funders recorded.
    #[must_use]
    pub fn achieved_k(&self) -> usize {
        self.depositors.len()
    }

    /// Check that `payouts` more payouts may be made, and account for them.
    ///
    /// # Errors
    /// Fails when deposits are incomplete, the funder floor is unmet, or the
    /// payouts would exceed what the round took in.
    pub fn authorize_payouts(&mut self, payouts: u32) -> Result<(), ProvenanceError> {
        if !self.deposits_complete() {
            return Err(ProvenanceError::DepositsIncomplete);
        }
        if self.achieved_k() < self.k_min as usize {
            return Err(ProvenanceError::AnonymitySetTooSmall);
        }
        let next = self
            .settled_count
            .checked_add(payouts)
            .ok_or(ProvenanceError::PayoutExceedsCapacity)?;
        if next > self.capacity {
            return Err(ProvenanceError::PayoutExceedsCapacity);
        }
        self.settling = true;
        self.settled_count = next;
        Ok(())
    }

    /// Total lamports the round must disburse over its lifetime.
    #[must_use]
    pub fn total_payout_lamports(&self) -> u64 {
        u64::from(self.capacity).saturating_mul(self.denomination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round() -> Round {
        Round::new(255, 1_000_000, 3, 6).expect("valid configuration")
    }

    #[test]
    fn serialized_state_fits_the_reserved_account() {
        let mut full = round();
        for index in 0..MAX_DEPOSITORS {
            let tag = u8::try_from(index).expect("MAX_DEPOSITORS fits in a byte");
            full.depositors.push(Pubkey::new_from_array([tag; 32]));
        }
        let encoded = borsh::to_vec(&full).expect("state encodes");
        assert_eq!(encoded.len(), ROUND_ACCOUNT_LEN);
    }

    #[test]
    fn state_round_trips() {
        let original = round();
        let encoded = borsh::to_vec(&original).expect("state encodes");
        let decoded = Round::try_from_slice(&encoded).expect("state decodes");
        assert_eq!(original, decoded);
    }

    #[test]
    fn degenerate_configurations_are_rejected() {
        assert_eq!(
            Round::new(255, 0, 3, 6),
            Err(ProvenanceError::ZeroDenomination)
        );
        for k in [0, 1] {
            assert_eq!(
                Round::new(255, 1_000, k, 6),
                Err(ProvenanceError::DegenerateAnonymitySet)
            );
        }
        assert_eq!(
            Round::new(255, 1_000, 33, 64),
            Err(ProvenanceError::TooManyDepositors)
        );
    }

    #[test]
    fn capacity_below_the_funder_floor_is_unreachable_and_rejected() {
        // Four deposits can never produce five distinct funders, so the round
        // would take money and then be unable to ever settle.
        assert_eq!(
            Round::new(255, 1_000, 5, 4),
            Err(ProvenanceError::InvalidCapacity)
        );
        assert!(Round::new(255, 1_000, 5, 5).is_ok());
    }

    #[test]
    fn repeat_deposits_do_not_inflate_the_anonymity_set() {
        let mut round = round();
        let solo = Pubkey::new_from_array([7; 32]);
        for _ in 0..6 {
            round.record_deposit(solo).expect("deposit accepted");
        }
        assert!(round.deposits_complete());
        // Six deposits, one funder: the set is 1, not 6.
        assert_eq!(round.achieved_k(), 1);
        // And that is exactly what blocks settlement.
        assert_eq!(
            round.authorize_payouts(6),
            Err(ProvenanceError::AnonymitySetTooSmall)
        );
    }

    #[test]
    fn deposits_stop_at_capacity() {
        let mut round = round();
        for index in 0..6_u8 {
            round
                .record_deposit(Pubkey::new_from_array([index; 32]))
                .expect("deposit accepted");
        }
        assert_eq!(
            round.record_deposit(Pubkey::new_from_array([99; 32])),
            Err(ProvenanceError::RoundFull)
        );
    }

    #[test]
    fn payouts_require_a_complete_round() {
        let mut round = round();
        round
            .record_deposit(Pubkey::new_from_array([1; 32]))
            .expect("deposit accepted");
        assert_eq!(
            round.authorize_payouts(1),
            Err(ProvenanceError::DepositsIncomplete)
        );
    }

    #[test]
    fn payouts_may_be_batched_but_never_exceed_deposits() {
        let mut round = round();
        for index in 0..6_u8 {
            round
                .record_deposit(Pubkey::new_from_array([index; 32]))
                .expect("deposit accepted");
        }
        round.authorize_payouts(4).expect("first batch");
        round.authorize_payouts(2).expect("second batch");
        assert_eq!(round.settled_count, 6);
        // Value conservation: not one payout more than the round took in.
        assert_eq!(
            round.authorize_payouts(1),
            Err(ProvenanceError::PayoutExceedsCapacity)
        );
    }

    #[test]
    fn settlement_freezes_the_anonymity_set() {
        let mut round = round();
        for index in 0..6_u8 {
            round
                .record_deposit(Pubkey::new_from_array([index; 32]))
                .expect("deposit accepted");
        }
        round.authorize_payouts(1).expect("first batch");
        // Late deposits cannot join a round whose payouts are already visible.
        assert_eq!(
            round.record_deposit(Pubkey::new_from_array([50; 32])),
            Err(ProvenanceError::RoundSettling)
        );
    }
}
