//! How large the crowd must be before pooled rounds actually help.
//!
//! The fleet is held at a constant 128 agent wallets and redistributed across
//! progressively fewer operators. Fewer operators means each one owns a larger
//! share of every round it joins, which is exactly the condition under which a
//! pool stops hiding anything.
//!
//! Run with:
//! ```text
//! cargo run -p provenance-eval --example crowd_size
//! ```

use provenance_eval::anonymity::measure as measure_anonymity;
use provenance_eval::attack::{AncestorJaccard, DepositShareAttack, DirectFunderJaccard};
use provenance_eval::evaluate::evaluate;
use provenance_eval::scenario::{generate, FleetSpec, FundingTopology};
use provenance_eval::PairwiseAttack;

const SEEDS: [u64; 5] = [1, 2, 3, 4, 5];
const TOTAL_AGENTS: usize = 128;
const ROUND_SIZE: usize = 16;

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn main() {
    let attacks: Vec<Box<dyn PairwiseAttack>> = vec![
        Box::new(DirectFunderJaccard),
        Box::new(AncestorJaccard::new(8)),
        Box::new(DepositShareAttack),
    ];
    let attack_refs: Vec<&dyn PairwiseAttack> = attacks.iter().map(AsRef::as_ref).collect();

    println!(
        "{TOTAL_AGENTS} agent wallets, round size {ROUND_SIZE}, {} seeds\n",
        SEEDS.len()
    );
    println!(
        "| operators | agents each | nominal k | effective k | effective k (min) | direct-funder | ancestor (d=8) | deposit-share |"
    );
    println!("|---|---|---|---|---|---|---|---|");

    for operators in [2_usize, 4, 8, 16, 32] {
        let spec = FleetSpec {
            entities: operators,
            agents_per_entity: TOTAL_AGENTS / operators,
        };
        let mut columns: Vec<Vec<f64>> = vec![Vec::new(); attack_refs.len()];
        let mut achieved = Vec::new();
        let mut effective = Vec::new();
        let mut effective_min = Vec::new();

        for seed in SEEDS {
            let scenario = generate(
                spec,
                FundingTopology::MeshRound {
                    round_size: ROUND_SIZE,
                },
                seed,
            );
            if let Some(k) = scenario.mean_achieved_k() {
                achieved.push(k);
            }
            let anonymity = measure_anonymity(&scenario.graph, &scenario.agents);
            effective.push(anonymity.mean_effective_k);
            effective_min.push(anonymity.mean_effective_k_min);
            for (index, result) in evaluate(&scenario, &attack_refs).iter().enumerate() {
                if let Some(auc) = result.roc_auc {
                    columns[index].push(auc);
                }
            }
        }

        println!(
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.3} | {:.3} | {:.3} |",
            operators,
            spec.agents_per_entity,
            mean(&achieved),
            mean(&effective),
            mean(&effective_min),
            mean(&columns[0]),
            mean(&columns[1]),
            mean(&columns[2]),
        );
    }

    println!("\nROC AUC 1.000 = perfect operator attribution, 0.500 = chance.");
}
