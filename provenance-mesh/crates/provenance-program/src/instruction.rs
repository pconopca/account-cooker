//! Instruction encoding and client-side builders.

use borsh::{BorshDeserialize, BorshSerialize};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_system_interface::program as system_program;

/// Seed prefix for round PDAs.
pub const ROUND_SEED: &[u8] = b"round";

/// Instructions accepted by the program.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum ProvenanceInstruction {
    /// Create a round and fix its parameters.
    ///
    /// Accounts: `[authority (signer, writable), round (writable), system_program]`
    OpenRound {
        /// Distinguishes concurrent rounds opened by the same authority.
        nonce: u64,
        /// Hash of the recipient set this round is allowed to pay.
        commitment: [u8; 32],
        /// Uniform payout size in lamports.
        denomination: u64,
        /// Distinct funders required before any payout.
        k_min: u32,
        /// Deposits accepted, and payouts made.
        capacity: u32,
    },
    /// Deposit exactly one denomination into the round.
    ///
    /// Accounts: `[depositor (signer, writable), round (writable), system_program]`
    Deposit,
    /// Pay one denomination to each supplied recipient.
    ///
    /// Accounts: `[settler (signer), round (writable), recipients.. (writable)]`
    Settle,
}

/// Encode `payload` into an [`Instruction`].
///
/// Borsh encoding of a type this crate owns cannot fail, so a failure here is a
/// bug rather than a caller error.
fn borsh_instruction(
    program_id: &Pubkey,
    payload: &ProvenanceInstruction,
    accounts: Vec<AccountMeta>,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(payload).expect("instruction encodes"),
    }
}

/// Commitment to the exact set of recipients a round will pay.
///
/// Recipients must be in strictly ascending order. That gives the set one
/// canonical encoding, so the commitment is unambiguous, and it rules out
/// duplicates without a separate check.
///
/// Returns `None` if the order is violated, which is the caller's bug rather
/// than a runtime condition.
#[must_use]
pub fn recipient_commitment(recipients: &[Pubkey]) -> Option<[u8; 32]> {
    if recipients.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    let refs: Vec<&[u8]> = recipients.iter().map(Pubkey::as_ref).collect();
    Some(solana_sha256_hasher::hashv(&refs).to_bytes())
}

/// Derive a round's address and bump.
#[must_use]
pub fn round_address(program_id: &Pubkey, authority: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[ROUND_SEED, authority.as_ref(), &nonce.to_le_bytes()],
        program_id,
    )
}

/// Build an [`ProvenanceInstruction::OpenRound`] instruction.
#[must_use]
pub fn open_round(
    program_id: &Pubkey,
    authority: &Pubkey,
    nonce: u64,
    commitment: [u8; 32],
    denomination: u64,
    k_min: u32,
    capacity: u32,
) -> Instruction {
    let (round, _) = round_address(program_id, authority, nonce);
    borsh_instruction(
        program_id,
        &ProvenanceInstruction::OpenRound {
            nonce,
            commitment,
            denomination,
            k_min,
            capacity,
        },
        vec![
            AccountMeta::new(*authority, true),
            AccountMeta::new(round, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    )
}

/// Build a [`ProvenanceInstruction::Deposit`] instruction.
#[must_use]
pub fn deposit(program_id: &Pubkey, depositor: &Pubkey, round: &Pubkey) -> Instruction {
    borsh_instruction(
        program_id,
        &ProvenanceInstruction::Deposit,
        vec![
            AccountMeta::new(*depositor, true),
            AccountMeta::new(*round, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    )
}

/// Build a [`ProvenanceInstruction::Settle`] instruction.
///
/// Recipients are supplied as a flat list. Nothing in this encoding, or in the
/// resulting transaction, associates a recipient with any particular depositor.
#[must_use]
pub fn settle(
    program_id: &Pubkey,
    settler: &Pubkey,
    round: &Pubkey,
    recipients: &[Pubkey],
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(*settler, true),
        AccountMeta::new(*round, false),
    ];
    accounts.extend(
        recipients
            .iter()
            .map(|recipient| AccountMeta::new(*recipient, false)),
    );
    borsh_instruction(program_id, &ProvenanceInstruction::Settle, accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_round_trip() {
        let original = ProvenanceInstruction::OpenRound {
            nonce: 9,
            commitment: [7; 32],
            denomination: 1_000_000,
            k_min: 4,
            capacity: 16,
        };
        let encoded = borsh::to_vec(&original).expect("encodes");
        assert_eq!(
            ProvenanceInstruction::try_from_slice(&encoded).expect("decodes"),
            original
        );
    }

    #[test]
    fn commitment_requires_ascending_order_and_rejects_duplicates() {
        let mut keys: Vec<Pubkey> = (0..4).map(|_| Pubkey::new_unique()).collect();
        keys.sort();
        assert!(recipient_commitment(&keys).is_some());

        let mut reversed = keys.clone();
        reversed.reverse();
        assert!(recipient_commitment(&reversed).is_none());

        let duplicated = vec![keys[0], keys[0]];
        assert!(recipient_commitment(&duplicated).is_none());
    }

    #[test]
    fn commitment_changes_with_the_set() {
        let mut keys: Vec<Pubkey> = (0..4).map(|_| Pubkey::new_unique()).collect();
        keys.sort();
        let baseline = recipient_commitment(&keys).expect("ordered");

        let mut swapped = keys.clone();
        swapped[3] = Pubkey::new_unique();
        swapped.sort();
        assert_ne!(recipient_commitment(&swapped).expect("ordered"), baseline);
    }

    #[test]
    fn round_addresses_are_distinct_per_nonce_and_authority() {
        let program = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let other = Pubkey::new_unique();
        assert_ne!(
            round_address(&program, &authority, 0).0,
            round_address(&program, &authority, 1).0
        );
        assert_ne!(
            round_address(&program, &authority, 0).0,
            round_address(&program, &other, 0).0
        );
    }

    #[test]
    fn settle_marks_every_recipient_writable_and_unsigned() {
        let program = Pubkey::new_unique();
        let settler = Pubkey::new_unique();
        let (round, _) = round_address(&program, &settler, 0);
        let recipients: Vec<Pubkey> = (0..4).map(|_| Pubkey::new_unique()).collect();
        let instruction = settle(&program, &settler, &round, &recipients);

        assert_eq!(instruction.accounts.len(), 2 + recipients.len());
        for meta in &instruction.accounts[2..] {
            assert!(meta.is_writable);
            // Recipients never sign: a payout must not require the recipient's
            // key to be online, or the round could not be settled by a relayer.
            assert!(!meta.is_signer);
        }
    }
}
