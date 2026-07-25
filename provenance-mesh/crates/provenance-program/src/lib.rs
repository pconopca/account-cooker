//! k-anonymous funding rounds for Solana.
//!
//! # What this program does
//!
//! Several independent operators deposit an identical amount into one pool
//! account. Once the pool is full, it pays that same amount out to a list of
//! recipients. The chain records who deposited and who was paid; it does not
//! record, and cannot reconstruct, which deposit paid for which recipient.
//!
//! # Why that matters
//!
//! A wallet fleet funded from one hot wallet is perfectly attributable from the
//! funding graph alone, no matter how human its on-chain behaviour looks. The
//! `provenance-eval` crate in this workspace measures exactly that: a
//! star-funded fleet scores ROC AUC 1.000 against a one-line attack. Routing
//! funding through rounds drives the same attack to 0.500 — chance.
//!
//! # What the program guarantees, and what it does not
//!
//! Enforced on chain: uniform denominations, a minimum count of *distinct*
//! funders before any payout, and conservation of value — a round pays out
//! exactly what it took in, never more.
//!
//! Not enforced, and stated plainly: the funder floor counts distinct *keys*,
//! not distinct people. One operator with `k` keys can fill a round alone and
//! obtain no privacy while appearing to satisfy the floor. Sybil resistance
//! needs a cost the protocol does not yet impose; see `THREAT_MODEL.md`.

#![allow(clippy::doc_markdown)]
// `entrypoint!` expands to `cfg(target_os = "solana")` and `cfg(feature =
// "custom-heap" / "custom-panic")` checks that only resolve under the SBF
// target, so a host-target lint pass flags every one of them.
#![allow(unexpected_cfgs)]

pub mod error;
pub mod instruction;
pub mod state;

use borsh::BorshDeserialize;
use solana_account_info::{next_account_info, AccountInfo};
use solana_cpi::{invoke, invoke_signed};
use solana_msg::msg;
use solana_program_entrypoint::ProgramResult;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_system_interface::instruction as system_instruction;
use solana_system_interface::program as system_program;
use solana_sysvar::Sysvar;

use crate::error::ProvenanceError;
use crate::instruction::{ProvenanceInstruction, ROUND_SEED};
use crate::state::{Round, ROUND_ACCOUNT_LEN};

#[cfg(not(feature = "no-entrypoint"))]
solana_program_entrypoint::entrypoint!(process_instruction);

/// Program entrypoint.
///
/// # Errors
/// Returns a [`ProvenanceError`] as a custom program error when an invariant
/// is violated.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    match ProvenanceInstruction::try_from_slice(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?
    {
        ProvenanceInstruction::OpenRound {
            nonce,
            denomination,
            k_min,
            capacity,
        } => open_round(program_id, accounts, nonce, denomination, k_min, capacity),
        ProvenanceInstruction::Deposit => deposit(program_id, accounts),
        ProvenanceInstruction::Settle => settle(program_id, accounts),
    }
}

/// Load and decode a round account that this program owns.
fn load_round(program_id: &Pubkey, account: &AccountInfo) -> Result<Round, ProgramError> {
    if account.owner != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    // The account is sized for a full depositor roster, so the encoded state is
    // almost always shorter than the account. `try_from_slice` treats those
    // trailing zero bytes as corruption; `deserialize` reads exactly what the
    // struct needs and leaves the remainder alone.
    let data = account.try_borrow_data()?;
    let mut cursor: &[u8] = &data;
    Round::deserialize(&mut cursor).map_err(|_| ProvenanceError::MalformedRoundState.into())
}

/// Re-encode a round into its account, which is sized for the maximum roster.
fn store_round(account: &AccountInfo, round: &Round) -> ProgramResult {
    let encoded = borsh::to_vec(round).map_err(|_| ProgramError::AccountDataTooSmall)?;
    let mut data = account.try_borrow_mut_data()?;
    if encoded.len() > data.len() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    data[..encoded.len()].copy_from_slice(&encoded);
    Ok(())
}

fn open_round(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    nonce: u64,
    denomination: u64,
    k_min: u32,
    capacity: u32,
) -> ProgramResult {
    let accounts = &mut accounts.iter();
    let authority = next_account_info(accounts)?;
    let round_account = next_account_info(accounts)?;
    let system = next_account_info(accounts)?;

    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    let seeds: &[&[u8]] = &[ROUND_SEED, authority.key.as_ref(), &nonce.to_le_bytes()];
    let (expected, bump) = Pubkey::find_program_address(seeds, program_id);
    if round_account.key != &expected {
        return Err(ProvenanceError::RoundAddressMismatch.into());
    }

    // Validate before creating the account, so a misconfigured round costs the
    // caller nothing and cannot strand rent in an unusable pool.
    let round = Round::new(bump, *authority.key, denomination, k_min, capacity)?;

    let rent = Rent::get()?.minimum_balance(ROUND_ACCOUNT_LEN);
    invoke_signed_create(
        authority,
        round_account,
        system,
        program_id,
        rent,
        seeds,
        bump,
    )?;
    store_round(round_account, &round)?;

    msg!(
        "round opened: denomination={} k_min={} capacity={}",
        denomination,
        k_min,
        capacity
    );
    Ok(())
}

fn invoke_signed_create<'a>(
    authority: &AccountInfo<'a>,
    round_account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    program_id: &Pubkey,
    lamports: u64,
    seeds: &[&[u8]],
    bump: u8,
) -> ProgramResult {
    let bump = [bump];
    let signer_seeds: Vec<&[u8]> = seeds
        .iter()
        .copied()
        .chain(std::iter::once(&bump[..]))
        .collect();
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            round_account.key,
            lamports,
            ROUND_ACCOUNT_LEN as u64,
            program_id,
        ),
        &[authority.clone(), round_account.clone(), system.clone()],
        &[&signer_seeds],
    )
}

fn deposit(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts = &mut accounts.iter();
    let depositor = next_account_info(accounts)?;
    let round_account = next_account_info(accounts)?;
    let system = next_account_info(accounts)?;

    if !depositor.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut round = load_round(program_id, round_account)?;
    // Record first: `record_deposit` rejects a full or settling round, so a
    // refused deposit never moves lamports.
    round.record_deposit(*depositor.key)?;

    invoke(
        &system_instruction::transfer(depositor.key, round_account.key, round.denomination),
        &[depositor.clone(), round_account.clone(), system.clone()],
    )?;
    store_round(round_account, &round)?;

    msg!(
        "deposit {}/{} distinct_funders={}",
        round.deposit_count,
        round.capacity,
        round.achieved_k()
    );
    Ok(())
}

fn settle(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_slice = accounts;
    let accounts = &mut accounts.iter();
    let settler = next_account_info(accounts)?;
    let round_account = next_account_info(accounts)?;

    if !settler.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let recipients = &accounts_slice[2..];
    if recipients.is_empty() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let mut round = load_round(program_id, round_account)?;
    let payouts = u32::try_from(recipients.len()).map_err(|_| ProgramError::InvalidArgument)?;
    // Authorisation first: a refused settlement must move no lamports and
    // leave no trace in the round's state.
    round.authorize_payouts(settler.key, payouts)?;

    // Reject duplicates and self-payment before moving any lamports: a partial
    // settlement that aborts halfway would leave the round unusable.
    for (index, recipient) in recipients.iter().enumerate() {
        if recipient.key == round_account.key {
            return Err(ProvenanceError::RecipientIsRound.into());
        }
        if recipients[..index]
            .iter()
            .any(|earlier| earlier.key == recipient.key)
        {
            return Err(ProvenanceError::DuplicateRecipient.into());
        }
    }

    let total = round
        .denomination
        .checked_mul(u64::from(payouts))
        .ok_or(ProgramError::ArithmeticOverflow)?;
    let rent_floor = Rent::get()?.minimum_balance(ROUND_ACCOUNT_LEN);
    let remaining = round_account
        .lamports()
        .checked_sub(total)
        .ok_or(ProvenanceError::RentExemptionViolated)?;
    if remaining < rent_floor {
        return Err(ProvenanceError::RentExemptionViolated.into());
    }

    for recipient in recipients {
        **round_account.try_borrow_mut_lamports()? -= round.denomination;
        **recipient.try_borrow_mut_lamports()? += round.denomination;
    }
    store_round(round_account, &round)?;

    msg!(
        "settled {}/{} payouts, anonymity_set={}",
        round.settled_count,
        round.capacity,
        round.achieved_k()
    );
    Ok(())
}
