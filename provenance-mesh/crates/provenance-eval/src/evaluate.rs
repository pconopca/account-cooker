//! Running attacks against scenarios and reporting what they achieved.

use serde::{Deserialize, Serialize};

use crate::attack::PairwiseAttack;
use crate::metrics::roc_auc;
use crate::scenario::Scenario;

/// How one attack performed against one scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackResult {
    /// Attack name.
    pub attack: String,
    /// Wallet pairs scored.
    pub pairs: usize,
    /// Pairs that genuinely share an operator.
    pub linked_pairs: usize,
    /// Separation achieved, or `None` when one class was absent.
    ///
    /// 1.0 is perfect attribution; 0.5 is indistinguishable from guessing.
    pub roc_auc: Option<f64>,
}

/// Score `attacks` against `scenario` over every unordered agent pair.
///
/// Ground-truth labels are read only after each attack has produced its score,
/// so no attack can be accidentally advantaged by them.
#[must_use]
pub fn evaluate(scenario: &Scenario, attacks: &[&dyn PairwiseAttack]) -> Vec<AttackResult> {
    attacks
        .iter()
        .map(|attack| {
            let mut samples = Vec::new();
            let mut linked_pairs = 0_usize;
            for (index, left) in scenario.agents.iter().enumerate() {
                for right in &scenario.agents[index + 1..] {
                    let score = attack.score(&scenario.graph, left, right);
                    let Some(linked) = scenario.same_entity(left, right) else {
                        continue;
                    };
                    if linked {
                        linked_pairs += 1;
                    }
                    samples.push((score, linked));
                }
            }
            AttackResult {
                attack: attack.name().to_owned(),
                pairs: samples.len(),
                linked_pairs,
                roc_auc: roc_auc(&samples),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::attack::{AncestorJaccard, DirectFunderJaccard, SharedAncestorIndicator};
    use crate::scenario::{generate, FleetSpec, FundingTopology};

    use super::*;

    const SPEC: FleetSpec = FleetSpec {
        entities: 8,
        agents_per_entity: 8,
    };

    #[test]
    fn a_star_funded_fleet_is_perfectly_attributable() {
        // Reproduces the baseline cooker-eval locks in with its own
        // `common_funder_limit_remains_measurable` test: the funding channel
        // alone identifies the operator with no error at all.
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let results = evaluate(&scenario, &[&DirectFunderJaccard]);
        let auc = results[0].roc_auc.expect("both classes present");
        assert!(
            (auc - 1.0).abs() < 1e-12,
            "expected perfect attribution, got {auc}"
        );
    }

    #[test]
    fn hop_chaining_only_moves_the_attack_one_level_up() {
        let scenario = generate(SPEC, FundingTopology::Cascade { depth: 3 }, 1);
        let shallow = evaluate(&scenario, &[&DirectFunderJaccard])[0]
            .roc_auc
            .expect("both classes present");
        let deep = evaluate(&scenario, &[&AncestorJaccard::new(8)])[0]
            .roc_auc
            .expect("both classes present");

        // Cascading looks like a fix against a one-hop observer...
        assert!(
            shallow < 0.6,
            "shallow attack should be blinded, got {shallow}"
        );
        // ...and is worth nothing against one willing to walk the chain.
        assert!(deep > 0.95, "deep attack should still win, got {deep}");
    }

    #[test]
    fn every_pair_is_scored_exactly_once() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let results = evaluate(&scenario, &[&DirectFunderJaccard]);
        let agents = SPEC.total_agents();
        assert_eq!(results[0].pairs, agents * (agents - 1) / 2);
        assert_eq!(
            results[0].linked_pairs,
            SPEC.entities * (SPEC.agents_per_entity * (SPEC.agents_per_entity - 1) / 2)
        );
    }

    #[test]
    fn results_are_reported_per_attack_in_order() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let results = evaluate(
            &scenario,
            &[
                &DirectFunderJaccard,
                &AncestorJaccard::new(4),
                &SharedAncestorIndicator::new(4),
            ],
        );
        let names: Vec<&str> = results.iter().map(|r| r.attack.as_str()).collect();
        assert_eq!(
            names,
            [
                "direct-funder-jaccard",
                "ancestor-jaccard",
                "shared-ancestor-indicator"
            ]
        );
    }
}
