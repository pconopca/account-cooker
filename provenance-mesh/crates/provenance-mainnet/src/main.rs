//! Sample mainnet funding edges and report provenance anonymity on real wallets.
//!
//! ```text
//! cargo run -p provenance-mainnet -- fetch 300 window-a   # sample into data/window-a.json
//! cargo run -p provenance-mainnet -- report window-a      # analyse that sample
//! cargo run -p provenance-mainnet -- scan wallets.txt     # audit your own fleet
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use provenance_core::WalletId;
use provenance_eval::anonymity::measure as measure_anonymity;
use provenance_eval::attack::{AncestorJaccard, DirectFunderJaccard, PairwiseAttack};
use provenance_eval::breakage::measure as measure_breakage;
use provenance_eval::fleet::scan as scan_fleets;
use provenance_mainnet::{current_slot, sample, Sample, DEFAULT_RPC};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn cache_path(name: &str) -> PathBuf {
    data_dir().join(format!("{name}.json"))
}

fn load_sample(name: &str) -> Option<Sample> {
    let bytes = std::fs::read(cache_path(name)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index]
}

fn do_fetch(blocks: u64, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("PROVENANCE_RPC").unwrap_or_else(|_| DEFAULT_RPC.to_owned());
    let tip = current_slot(&endpoint)?;
    // Stay well behind the tip so every slot in the window is finalized and the
    // sample is stable for anyone who re-runs it.
    let start = tip.saturating_sub(blocks + 300);
    eprintln!("tip slot {tip}; sampling {blocks} slots from {start}");

    let sampled = sample(&endpoint, start, blocks)?;
    eprintln!(
        "fetched {}/{} slots, {} transfer edges",
        sampled.slots_fetched,
        sampled.slots_requested,
        sampled.edges.len()
    );

    let path = cache_path(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec(&sampled)?)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// Star-funded clusters observed in the wild, and what their wallets inherit.
fn report_fleets(graph: &provenance_core::FundingGraph) {
    const MIN_RECIPIENTS: usize = 5;

    let scan = scan_fleets(graph, MIN_RECIPIENTS, 1);
    println!("\n## Funding clusters observed in the wild\n");
    println!(
        "Wallets that funded {MIN_RECIPIENTS} or more others inside the window, split by whether \
         anyone funded *them*.\n"
    );
    println!("| shape | clusters | wallets funded | mean effective k of those wallets |");
    println!("|---|---|---|---|");

    for (label, clusters) in [
        ("star (paid by <= 1)", scan.stars().collect::<Vec<_>>()),
        ("pool (paid by > 1)", scan.pools().collect::<Vec<_>>()),
    ] {
        let recipients: Vec<WalletId> = clusters
            .iter()
            .flat_map(|cluster| graph.direct_recipients(&cluster.funder))
            .collect();
        let covered = recipients.len();
        let mean_k = if recipients.is_empty() {
            f64::NAN
        } else {
            measure_anonymity(graph, &recipients).mean_effective_k
        };
        println!(
            "| {} | {} | {} | {:.2} |",
            label,
            clusters.len(),
            covered,
            mean_k
        );
    }

    println!("\n### Largest star-funded clusters\n");
    println!("| funder | wallets funded | funders of the funder |");
    println!("|---|---|---|");
    for cluster in scan.stars().take(5) {
        println!(
            "| `{}` | {} | {} |",
            cluster.funder.as_str(),
            cluster.recipients,
            cluster.inbound_funders
        );
    }
}

/// Which pooled accounts actually break the deposit-to-payout link.
///
/// A crowd is necessary but not sufficient. If a pool's payouts are signed by
/// the same accounts that deposited, the transaction publishes the link and the
/// crowd is decoration.
fn report_breakage(graph: &provenance_core::FundingGraph) {
    const MIN_DEPOSITORS: usize = 8;

    let mut measured: Vec<provenance_eval::breakage::Breakage> = graph
        .funded_wallets()
        .into_iter()
        .filter(|wallet| graph.direct_funders(wallet).len() >= MIN_DEPOSITORS)
        .map(|wallet| measure_breakage(graph, &wallet))
        .filter(|breakage| breakage.payouts > 0)
        .collect();

    measured.sort_by_key(|breakage| std::cmp::Reverse(breakage.depositors));

    println!("\n## Do pooled accounts actually break provenance?\n");
    println!(
        "Accounts with {MIN_DEPOSITORS} or more distinct depositors that also paid out inside the \
         window. A payout signed by one of the pool's own depositors publishes the link; a payout \
         signed by anyone else does not.\n"
    );

    let total = measured.len();
    let effective = measured
        .iter()
        .filter(|breakage| breakage.is_effective(MIN_DEPOSITORS, 0.9))
        .count();
    let passthrough = measured
        .iter()
        .filter(|breakage| breakage.break_rate().is_some_and(|rate| rate <= 0.1))
        .count();

    println!("| verdict | pools |");
    println!("|---|---|");
    println!("| breaks the link (>= 90% third-party signed) | {effective} |");
    println!("| passthrough (<= 10% third-party signed) | {passthrough} |");
    println!("| partial | {} |", total - effective - passthrough);

    let self_service = measured
        .iter()
        .filter(|breakage| {
            breakage
                .recipient_signed_rate()
                .is_some_and(|rate| rate >= 0.5)
        })
        .count();
    println!(
        "\nOf the same {total} pools, **{self_service}** pay out mostly to recipients that signed \
         for themselves — a self-service withdrawal, which is linkable regardless of what the \
         deposit history shows."
    );

    println!("\n### Largest pools by depositor count\n");
    println!("| pool | depositors | payouts | self-signed | break rate | recipient-signed |");
    println!("|---|---|---|---|---|---|");
    for breakage in measured.iter().take(10) {
        let rate = breakage
            .break_rate()
            .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.2}"));
        let recipient = breakage
            .recipient_signed_rate()
            .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.2}"));
        println!(
            "| `{}` | {} | {} | {} | {} | {} |",
            breakage.pool.as_str(),
            breakage.depositors,
            breakage.payouts,
            breakage.self_signed,
            rate,
            recipient
        );
    }
}

/// Per-wallet provenance table. Returns the fleet report and how many wallets
/// had no observable funder.
fn print_wallet_provenance(
    graph: &provenance_core::FundingGraph,
    wallets: &[WalletId],
) -> (provenance_eval::anonymity::AnonymityReport, usize) {
    println!("## Per-wallet provenance\n");
    println!("| wallet | funder | funder's depositors | effective k |");
    println!("|---|---|---|---|");

    let mut unfunded = 0_usize;
    for wallet in wallets {
        let funders = graph.direct_funders(wallet);
        let Some(funder) = funders.iter().next() else {
            unfunded += 1;
            println!("| `{}` | none observed | - | - |", wallet.as_str());
            continue;
        };
        println!(
            "| `{}` | `{}` | {} | {:.2} |",
            wallet.as_str(),
            funder.as_str(),
            graph.direct_funders(funder).len(),
            measure_anonymity(graph, std::slice::from_ref(wallet)).mean_effective_k
        );
    }
    (measure_anonymity(graph, wallets), unfunded)
}

/// Pairwise linkage table. Returns the strongest attack's hit rate.
fn print_linkage(graph: &provenance_core::FundingGraph, wallets: &[WalletId]) -> f64 {
    println!("\n## Can an observer link these wallets to each other?\n");
    println!("| attack | pairs scoring above zero | share |");
    println!("|---|---|---|");

    let attacks: [&dyn PairwiseAttack; 2] = [&DirectFunderJaccard, &AncestorJaccard::new(4)];
    let mut worst = 0.0_f64;
    for attack in attacks {
        let mut linked = 0_usize;
        let mut pairs = 0_usize;
        for (index, left) in wallets.iter().enumerate() {
            for right in &wallets[index + 1..] {
                pairs += 1;
                if attack.score(graph, left, right) > 0.0 {
                    linked += 1;
                }
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let share = if pairs == 0 {
            0.0
        } else {
            linked as f64 / pairs as f64
        };
        worst = worst.max(share);
        println!(
            "| {} | {} of {} | {:.0}% |",
            attack.name(),
            linked,
            pairs,
            share * 100.0
        );
    }
    worst
}

fn print_verdict(worst: f64, report: &provenance_eval::anonymity::AnonymityReport) {
    println!("\n## Verdict\n");
    if worst >= 0.5 {
        println!(
            "**Linkable.** {:.0}% of wallet pairs share a funding ancestor an observer can see. \
             These addresses read as one operator.",
            worst * 100.0
        );
    } else if report.mean_effective_k < 2.0 {
        println!(
            "**Individually attributable.** Pairs are not obviously linked to each other, but each \
             wallet traces to a single identifiable funder (effective k {:.2}).",
            report.mean_effective_k
        );
    } else {
        println!(
            "**No linkage found in observed history.** Effective k {:.2}. Note the limits below \
             before treating this as private.",
            report.mean_effective_k
        );
    }

    println!(
        "\n### Limits of this audit\n\n\
         - Only the most recent {} transactions per address are read. Older funding is invisible \
         here and an observer with full history sees more.\n\
         - Two hops. A shared ancestor further back is not reported.\n\
         - On-chain only. Correlated IPs, RPC metadata, and timing are not covered.\n\
         - A clean result is evidence of nothing found, not proof of nothing there.",
        provenance_mainnet::scan::SIGNATURES_PER_ADDRESS
    );
}

/// Audit a set of wallets the caller controls.
///
/// This is the question an operator actually has, and answering it needs no
/// crowd, no adoption, and no agreement with anything else here: point it at
/// your own addresses and it reports whether they are attributable.
fn do_scan(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("PROVENANCE_RPC").unwrap_or_else(|_| DEFAULT_RPC.to_owned());
    let wallets = provenance_mainnet::scan::parse_wallet_list(&std::fs::read_to_string(path)?);
    if wallets.len() < 2 {
        eprintln!("need at least 2 wallets to say anything about linkage");
        std::process::exit(2);
    }
    eprintln!("scanning {} wallets against {endpoint}", wallets.len());

    let graph = provenance_mainnet::scan::scan_wallets(&endpoint, &wallets, |stage, at, total| {
        eprintln!("  {stage} {at}/{total}");
    })?;

    println!("# Provenance audit\n");
    println!("Wallets scanned: {}\n", wallets.len());

    let (report, unfunded) = print_wallet_provenance(&graph, &wallets);
    println!(
        "\n**Mean effective anonymity set: {:.2}** (min-entropy {:.2})",
        report.mean_effective_k, report.mean_effective_k_min
    );
    if unfunded > 0 {
        println!(
            "\n{unfunded} wallet(s) had no inbound transfer in recent history; they are excluded \
             from the linkage test below rather than counted as private."
        );
    }

    let worst = print_linkage(&graph, &wallets);
    print_verdict(worst, &report);
    Ok(())
}

fn do_report(name: &str) {
    let Some(sampled) = load_sample(name) else {
        eprintln!("no cached sample named {name}; run `fetch` first");
        std::process::exit(1);
    };
    let graph = sampled.graph();
    let funded: Vec<WalletId> = graph.funded_wallets().into_iter().collect();

    println!("# Mainnet funding provenance\n");
    println!(
        "Window: {} slots from {} ({} returned a block)",
        sampled.slots_requested, sampled.start_slot, sampled.slots_fetched
    );
    println!(
        "Transfers: {}   Funded accounts: {}\n",
        sampled.edges.len(),
        funded.len()
    );

    // Per-wallet effective anonymity set, read from the composition of its funder.
    let mut effective: Vec<f64> = Vec::with_capacity(funded.len());
    let mut effective_min: Vec<f64> = Vec::with_capacity(funded.len());
    for wallet in &funded {
        let report = measure_anonymity(&graph, std::slice::from_ref(wallet));
        effective.push(report.mean_effective_k);
        effective_min.push(report.mean_effective_k_min);
    }
    effective.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    effective_min.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let perfectly_attributable = effective.iter().filter(|k| **k < 1.000_001).count();
    #[allow(clippy::cast_precision_loss)]
    let share = perfectly_attributable as f64 / effective.len().max(1) as f64;

    println!("## Effective provenance anonymity set, real accounts\n");
    println!("| statistic | effective k | effective k (min-entropy) |");
    println!("|---|---|---|");
    for (label, fraction) in [
        ("p50", 0.50),
        ("p75", 0.75),
        ("p90", 0.90),
        ("p99", 0.99),
        ("max", 1.00),
    ] {
        println!(
            "| {} | {:.2} | {:.2} |",
            label,
            percentile(&effective, fraction),
            percentile(&effective_min, fraction)
        );
    }
    println!(
        "\n**{:.1}% of funded accounts have an effective anonymity set of 1** \
         - one identifiable funder, no ambiguity at all.\n",
        share * 100.0
    );

    // The other half of the story: accounts whose funder is a busy pool.
    let counts: BTreeMap<WalletId, usize> = funded
        .iter()
        .map(|wallet| (wallet.clone(), graph.inbound_edge_counts(wallet).len()))
        .collect();
    let mut busiest: Vec<(&WalletId, &usize)> = counts.iter().collect();
    busiest.sort_by(|left, right| right.1.cmp(left.1));

    println!("## Busiest funders in the window\n");
    println!("| account | distinct depositors observed |");
    println!("|---|---|");
    for (wallet, depositors) in busiest.iter().take(5) {
        println!("| `{}` | {} |", wallet.as_str(), depositors);
    }

    report_fleets(&graph);
    report_breakage(&graph);

    let pooled: Vec<WalletId> = funded
        .iter()
        .filter(|wallet| {
            graph
                .direct_funders(wallet)
                .iter()
                .next()
                .is_some_and(|funder| graph.inbound_edge_counts(funder).len() >= 8)
        })
        .cloned()
        .collect();
    println!(
        "\nAccounts paid by a funder with >= 8 observed depositors: {} of {}",
        pooled.len(),
        funded.len()
    );
    if pooled.len() >= 2 {
        let report = measure_anonymity(&graph, &pooled);
        println!(
            "Their mean effective k: {:.2} (min-entropy {:.2})",
            report.mean_effective_k, report.mean_effective_k_min
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fetch") => {
            let blocks = args
                .get(1)
                .and_then(|value| value.parse().ok())
                .unwrap_or(40);
            let name = args.get(2).map_or("window-b", String::as_str);
            do_fetch(blocks, name)
        }
        Some("report") => {
            let name = args.get(1).map_or("window-b", String::as_str);
            do_report(name);
            Ok(())
        }
        Some("scan") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: provenance-mainnet scan <wallet-list-file>");
                std::process::exit(2);
            };
            do_scan(path)
        }
        _ => {
            eprintln!(
                "usage: provenance-mainnet <fetch [slots] [name] | report [name] | scan <file>>"
            );
            std::process::exit(2);
        }
    }
}
