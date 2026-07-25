//! Failure modes of a funding round.
//!
//! Every variant is a refusal to settle, never a degraded settlement. A round
//! that cannot deliver the anonymity it advertised must abort so the operator
//! learns about it, rather than paying out with a silently weaker guarantee the
//! operator would never know to compensate for.

use solana_program_error::ProgramError;

/// Errors returned by the provenance-mesh program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvenanceError {
    /// Denomination was zero, carrying no value and no cover.
    ZeroDenomination = 0,
    /// A round with fewer than two funders hides nothing.
    DegenerateAnonymitySet = 1,
    /// Capacity was zero, or smaller than the minimum funder count.
    InvalidCapacity = 2,
    /// The configured funder floor exceeds what the account can record.
    TooManyDepositors = 3,
    /// The round already holds its full complement of deposits.
    RoundFull = 4,
    /// Settlement has begun; the anonymity set is frozen.
    RoundSettling = 5,
    /// Payouts were attempted before every deposit landed.
    DepositsIncomplete = 6,
    /// Fewer distinct funders than the round's floor.
    AnonymitySetTooSmall = 7,
    /// Payouts would exceed what the round took in.
    PayoutExceedsCapacity = 8,
    /// The same recipient appeared twice in one settlement.
    DuplicateRecipient = 9,
    /// A payout targeted the round account itself.
    RecipientIsRound = 10,
    /// Paying out would strip the round's rent exemption.
    RentExemptionViolated = 11,
    /// Supplied account does not match the derived round address.
    RoundAddressMismatch = 12,
    /// Stored state failed to decode.
    MalformedRoundState = 13,
}

impl From<ProvenanceError> for ProgramError {
    fn from(value: ProvenanceError) -> Self {
        Self::Custom(value as u32)
    }
}
