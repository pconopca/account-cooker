//! End-to-end proof that a funding round settles on a live cluster.
//!
//! Runs three scenarios against a deployed program and prints a table of real,
//! verifiable signatures:
//!
//! 1. A round funded by `k` distinct depositors settles, and every recipient is
//!    paid exactly one denomination.
//! 2. Settlement before the deposits are complete is refused on chain.
//! 3. A round whose deposits all come from one key is refused on chain, because
//!    it would deliver no anonymity while appearing full.
//! 4. A stranger cannot settle a funded round to recipients of their choosing.
//!
//! The negative cases matter as much as the positive one: they are what
//! separate an enforced guarantee from a documented intention.
//!
//! ```text
//! cargo run -p provenance-e2e -- <PROGRAM_ID> [RPC_URL] [PAYER_KEYPAIR]
//! ```

use std::path::{Path, PathBuf};
use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use provenance_program::instruction::{deposit, open_round, round_address, settle};

/// One denomination. Above the rent-exempt floor for an empty account, so a
/// recipient survives its payout and its balance can be checked afterwards.
const DENOMINATION: u64 = 1_000_000;
/// Deposits the round accepts, and payouts it makes.
const CAPACITY: u32 = 8;
/// Distinct depositors required before settlement.
const K_MIN: u32 = 4;
/// Left behind in every funded key so it stays rent-exempt after depositing.
///
/// Solana refuses a transaction that would leave an account holding a non-zero
/// balance below the rent-exempt floor, so a depositor must retain that floor
/// plus fees rather than being funded for the denomination alone.
const RENT_BUFFER: u64 = 2_000_000;

struct Runner {
    client: RpcClient,
    payer: Keypair,
    program: Pubkey,
}

impl Runner {
    fn send(&self, instructions: &[Instruction], signers: &[&Keypair]) -> Result<String, String> {
        let blockhash = self
            .client
            .get_latest_blockhash()
            .map_err(|error| error.to_string())?;
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&self.payer.pubkey()),
            signers,
            blockhash,
        );
        self.client
            .send_and_confirm_transaction(&transaction)
            .map(|signature| signature.to_string())
            .map_err(|error| error.to_string())
    }

    /// Give `count` fresh keys enough for one denomination plus fees.
    fn fund_depositors(&self, count: usize) -> Result<Vec<Keypair>, String> {
        let depositors: Vec<Keypair> = (0..count).map(|_| Keypair::new()).collect();
        let instructions: Vec<Instruction> = depositors
            .iter()
            .map(|depositor| {
                solana_system_interface::instruction::transfer(
                    &self.payer.pubkey(),
                    &depositor.pubkey(),
                    DENOMINATION + RENT_BUFFER,
                )
            })
            .collect();
        // Chunked so the funding transaction stays inside the size limit.
        for chunk in instructions.chunks(10) {
            self.send(chunk, &[&self.payer])?;
        }
        Ok(depositors)
    }

    fn open(&self, nonce: u64, k_min: u32, capacity: u32) -> Result<(Pubkey, String), String> {
        let (round, _) = round_address(&self.program, &self.payer.pubkey(), nonce);
        let signature = self.send(
            &[open_round(
                &self.program,
                &self.payer.pubkey(),
                nonce,
                DENOMINATION,
                k_min,
                capacity,
            )],
            &[&self.payer],
        )?;
        Ok((round, signature))
    }
}

fn scenario_happy_path(runner: &Runner, nonce: u64) -> Result<(), String> {
    println!("\n## Scenario 1 - a full round settles\n");
    let (round, open_signature) = runner.open(nonce, K_MIN, CAPACITY)?;
    println!("| step | detail | signature |");
    println!("|---|---|---|");
    println!("| open round | `{round}` | `{open_signature}` |");

    let depositors = runner.fund_depositors(CAPACITY as usize)?;
    for (index, depositor) in depositors.iter().enumerate() {
        let signature = runner.send(
            &[deposit(&runner.program, &depositor.pubkey(), &round)],
            &[&runner.payer, depositor],
        )?;
        println!(
            "| deposit {}/{} | `{}` | `{}` |",
            index + 1,
            CAPACITY,
            depositor.pubkey(),
            signature
        );
    }

    let recipients: Vec<Pubkey> = (0..CAPACITY).map(|_| Keypair::new().pubkey()).collect();
    let signature = runner.send(
        &[settle(
            &runner.program,
            &runner.payer.pubkey(),
            &round,
            &recipients,
        )],
        &[&runner.payer],
    )?;
    println!("| settle {CAPACITY} payouts | - | `{signature}` |");

    // Value conservation, checked against the cluster rather than asserted.
    let mut paid = 0_u64;
    for recipient in &recipients {
        let balance = runner
            .client
            .get_balance(recipient)
            .map_err(|error| error.to_string())?;
        if balance != DENOMINATION {
            return Err(format!(
                "recipient {recipient} holds {balance}, expected {DENOMINATION}"
            ));
        }
        paid += balance;
    }
    println!(
        "\nAll {CAPACITY} recipients hold exactly {DENOMINATION} lamports; {paid} lamports paid \
         out against {} deposited.",
        u64::from(CAPACITY) * DENOMINATION
    );
    Ok(())
}

fn scenario_settle_too_early(runner: &Runner, nonce: u64) -> Result<(), String> {
    println!("\n## Scenario 2 - settlement before the round fills is refused\n");
    let (round, _) = runner.open(nonce, K_MIN, CAPACITY)?;
    let depositors = runner.fund_depositors(2)?;
    for depositor in &depositors {
        runner.send(
            &[deposit(&runner.program, &depositor.pubkey(), &round)],
            &[&runner.payer, depositor],
        )?;
    }
    let recipients: Vec<Pubkey> = (0..2).map(|_| Keypair::new().pubkey()).collect();
    match runner.send(
        &[settle(
            &runner.program,
            &runner.payer.pubkey(),
            &round,
            &recipients,
        )],
        &[&runner.payer],
    ) {
        Ok(signature) => Err(format!(
            "settlement should have been refused, but succeeded: {signature}"
        )),
        Err(error) => {
            println!("Refused on chain, as required: `{error}`");
            // DepositsIncomplete is custom error 6.
            if error.contains("0x6") {
                Ok(())
            } else {
                Err(format!("expected custom error 0x6, got: {error}"))
            }
        }
    }
}

fn scenario_single_depositor(runner: &Runner, nonce: u64) -> Result<(), String> {
    println!("\n## Scenario 3 - a round filled by one key is refused\n");
    // Capacity equals k_min, so one key can complete the deposits alone. The
    // round then looks full while offering an anonymity set of exactly one.
    let (round, _) = runner.open(nonce, K_MIN, K_MIN)?;
    let solo = Keypair::new();
    runner.send(
        &[solana_system_interface::instruction::transfer(
            &runner.payer.pubkey(),
            &solo.pubkey(),
            u64::from(K_MIN) * DENOMINATION + RENT_BUFFER,
        )],
        &[&runner.payer],
    )?;
    for _ in 0..K_MIN {
        runner.send(
            &[deposit(&runner.program, &solo.pubkey(), &round)],
            &[&runner.payer, &solo],
        )?;
    }
    println!(
        "Round filled to capacity by the single key `{}`.",
        solo.pubkey()
    );

    let recipients: Vec<Pubkey> = (0..K_MIN).map(|_| Keypair::new().pubkey()).collect();
    match runner.send(
        &[settle(
            &runner.program,
            &runner.payer.pubkey(),
            &round,
            &recipients,
        )],
        &[&runner.payer],
    ) {
        Ok(signature) => Err(format!(
            "settlement should have been refused, but succeeded: {signature}"
        )),
        Err(error) => {
            println!("Refused on chain, as required: `{error}`");
            // AnonymitySetTooSmall is custom error 7.
            if error.contains("0x7") {
                Ok(())
            } else {
                Err(format!("expected custom error 0x7, got: {error}"))
            }
        }
    }
}

fn scenario_unauthorized_settler(runner: &Runner, nonce: u64) -> Result<(), String> {
    println!("\n## Scenario 4 - a stranger cannot settle a funded round\n");
    // Every other invariant is satisfied: the round is complete and has enough
    // distinct funders. Only the authority check stands between an outsider and
    // every deposit in the pool.
    let (round, _) = runner.open(nonce, K_MIN, CAPACITY)?;
    let depositors = runner.fund_depositors(CAPACITY as usize)?;
    for depositor in &depositors {
        runner.send(
            &[deposit(&runner.program, &depositor.pubkey(), &round)],
            &[&runner.payer, depositor],
        )?;
    }

    let stranger = Keypair::new();
    runner.send(
        &[solana_system_interface::instruction::transfer(
            &runner.payer.pubkey(),
            &stranger.pubkey(),
            RENT_BUFFER,
        )],
        &[&runner.payer],
    )?;
    println!(
        "Round is full and valid. Stranger `{}` attempts to settle it to their own addresses.",
        stranger.pubkey()
    );

    let stolen: Vec<Pubkey> = (0..CAPACITY).map(|_| Keypair::new().pubkey()).collect();
    match runner.send(
        &[settle(
            &runner.program,
            &stranger.pubkey(),
            &round,
            &stolen,
        )],
        &[&runner.payer, &stranger],
    ) {
        Ok(signature) => Err(format!(
            "a stranger drained the round, which is the bug this check exists to prevent: {signature}"
        )),
        Err(error) => {
            println!("Refused on chain, as required: `{error}`");
            // UnauthorizedSettler is custom error 14 = 0xe.
            if error.contains("0xe") {
                Ok(())
            } else {
                Err(format!("expected custom error 0xe, got: {error}"))
            }
        }
    }
}

/// Read the CLI's keypair format: a JSON array of 64 bytes.
fn read_keypair(path: &Path) -> Result<Keypair, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let trimmed = text.trim().trim_start_matches('[').trim_end_matches(']');
    let bytes: Vec<u8> = trimmed
        .split(',')
        .map(|piece| {
            piece
                .trim()
                .parse::<u8>()
                .map_err(|error| format!("bad keypair byte: {error}"))
        })
        .collect::<Result<_, _>>()?;
    Keypair::try_from(bytes.as_slice()).map_err(|error| error.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(program_id) = args.first() else {
        eprintln!("usage: provenance-e2e <PROGRAM_ID> [RPC_URL] [PAYER_KEYPAIR]");
        std::process::exit(2);
    };
    let rpc = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "https://api.devnet.solana.com".to_owned());
    let keypair_path = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| {
        format!(
            "{}/.config/solana/devnet-provenance.json",
            std::env::var("HOME").unwrap_or_default()
        )
    }));

    let program = match Pubkey::from_str(program_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("bad program id: {error}");
            std::process::exit(2);
        }
    };
    let payer = match read_keypair(&keypair_path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!(
                "cannot read payer keypair at {}: {error}",
                keypair_path.display()
            );
            std::process::exit(2);
        }
    };

    let runner = Runner {
        client: RpcClient::new_with_commitment(rpc.clone(), CommitmentConfig::confirmed()),
        payer,
        program,
    };

    println!("# provenance-mesh live proof\n");
    println!("- cluster: `{rpc}`");
    println!("- program: `{program}`");
    println!("- payer: `{}`", runner.payer.pubkey());

    let nonce_base = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    let outcomes = [
        (
            "full round settles",
            scenario_happy_path(&runner, nonce_base),
        ),
        (
            "early settlement refused",
            scenario_settle_too_early(&runner, nonce_base + 1),
        ),
        (
            "single-depositor round refused",
            scenario_single_depositor(&runner, nonce_base + 2),
        ),
        (
            "unauthorized settler refused",
            scenario_unauthorized_settler(&runner, nonce_base + 3),
        ),
    ];

    println!("\n## Result\n");
    let mut failed = false;
    for (name, outcome) in &outcomes {
        match outcome {
            Ok(()) => println!("- PASS - {name}"),
            Err(error) => {
                failed = true;
                println!("- FAIL - {name}: {error}");
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
