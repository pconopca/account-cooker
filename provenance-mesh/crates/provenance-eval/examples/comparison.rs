//! Reproducible funding-topology comparison.
//!
//! Run with:
//! ```text
//! cargo run -p provenance-eval --example comparison
//! ```

use provenance_eval::attack::{
    AncestorJaccard, DepositShareAttack, DirectFunderJaccard, SharedAncestorIndicator,
};
use provenance_eval::evaluate::evaluate;
use provenance_eval::scenario::{generate, FleetSpec, FundingTopology};
use provenance_eval::PairwiseAttack;

const SEEDS: [u64; 5] = [1, 2, 3, 4, 5];

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
    let spec = FleetSpec {
        entities: 16,
        agents_per_entity: 8,
    };

    let attacks: Vec<Box<dyn PairwiseAttack>> = vec![
        Box::new(DirectFunderJaccard),
        Box::new(AncestorJaccard::new(8)),
        Box::new(SharedAncestorIndicator::new(8)),
        Box::new(DepositShareAttack),
    ];
    let attack_refs: Vec<&dyn PairwiseAttack> = attacks.iter().map(AsRef::as_ref).collect();

    let topologies = [
        FundingTopology::NaiveStar,
        FundingTopology::Cascade { depth: 3 },
        FundingTopology::MeshRound { round_size: 8 },
        FundingTopology::MeshRound { round_size: 16 },
        FundingTopology::MeshRound { round_size: 32 },
        FundingTopology::MeshRound { round_size: 64 },
    ];

    println!(
        "Fleet: {} operators x {} agents = {} wallets, {} seeds\n",
        spec.entities,
        spec.agents_per_entity,
        spec.total_agents(),
        SEEDS.len()
    );
    println!(
        "| topology | mean achieved k | direct-funder | ancestor (d=8) | shared-ancestor | deposit-share |"
    );
    println!("|---|---|---|---|---|---|");

    for topology in topologies {
        let mut columns: Vec<Vec<f64>> = vec![Vec::new(); attack_refs.len()];
        let mut achieved = Vec::new();

        for seed in SEEDS {
            let scenario = generate(spec, topology, seed);
            if let Some(k) = scenario.mean_achieved_k() {
                achieved.push(k);
            }
            for (index, result) in evaluate(&scenario, &attack_refs).iter().enumerate() {
                if let Some(auc) = result.roc_auc {
                    columns[index].push(auc);
                }
            }
        }

        let k_column = if achieved.is_empty() {
            "n/a".to_owned()
        } else {
            format!("{:.2}", mean(&achieved))
        };
        println!(
            "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} |",
            topology.name(),
            k_column,
            mean(&columns[0]),
            mean(&columns[1]),
            mean(&columns[2]),
            mean(&columns[3]),
        );
    }

    println!("\nROC AUC 1.000 = perfect operator attribution, 0.500 = no better than guessing.");
}
