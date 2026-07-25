//! Running attacks against scenarios and reporting what they achieved.

use serde::{Deserialize, Serialize};

use provenance_core::{FundingGraph, WalletId};

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

/// Wallets whose funding this graph actually shows.
///
/// Everything scored against a wallet set has to run over this, not the set the
/// caller handed in. A wallet with no observed funder cannot score above zero
/// against anything, so including it dilutes a linkage share downward and can
/// turn a genuinely linkable fleet into a reassuring verdict. In a tool someone
/// relies on for privacy, silence must not read as evidence of it.
#[must_use]
pub fn with_observed_funding(graph: &FundingGraph, wallets: &[WalletId]) -> Vec<WalletId> {
    wallets
        .iter()
        .filter(|wallet| !graph.direct_funders(wallet).is_empty())
        .cloned()
        .collect()
}

/// Share of scorable pairs an attack links, ignoring wallets it cannot see.
///
/// Returns `None` when fewer than two wallets have observed funding, which is a
/// different answer from zero.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn linked_share(
    graph: &FundingGraph,
    wallets: &[WalletId],
    attack: &dyn PairwiseAttack,
) -> Option<f64> {
    let observed = with_observed_funding(graph, wallets);
    if observed.len() < 2 {
        return None;
    }
    let mut linked = 0_usize;
    let mut pairs = 0_usize;
    for (index, left) in observed.iter().enumerate() {
        for right in &observed[index + 1..] {
            pairs += 1;
            if attack.score(graph, left, right) > 0.0 {
                linked += 1;
            }
        }
    }
    Some(linked as f64 / pairs as f64)
}

#[cfg(test)]
mod exclusion_tests {
    use provenance_core::{FundingEdge, FundingGraph, WalletId};

    use crate::attack::DirectFunderJaccard;

    use super::{linked_share, with_observed_funding};

    fn edge(source: &str, target: &str) -> FundingEdge {
        FundingEdge {
            source: source.into(),
            target: target.into(),
            lamports: 1,
            slot: 1,
            signers: Vec::new(),
        }
    }

    #[test]
    fn wallets_with_no_observed_funder_are_left_out() {
        let graph = FundingGraph::from_edges(vec![edge("hot", "a"), edge("hot", "b")]);
        let asked: Vec<WalletId> = vec!["a".into(), "b".into(), "never-seen".into()];
        assert_eq!(
            with_observed_funding(&graph, &asked),
            vec![WalletId::from("a"), WalletId::from("b")]
        );
    }

    #[test]
    fn unseen_wallets_do_not_dilute_a_linkable_verdict() {
        // Two wallets from one hot wallet, plus eight the history never showed.
        // Counting the unseen ones as unlinked would report 2 of 45 pairs — 4%,
        // comfortably below any threshold — for a fleet that is fully linked.
        let graph = FundingGraph::from_edges(vec![edge("hot", "a"), edge("hot", "b")]);
        let mut asked: Vec<WalletId> = vec!["a".into(), "b".into()];
        for index in 0..8 {
            asked.push(WalletId(format!("unseen-{index}")));
        }
        assert_eq!(
            linked_share(&graph, &asked, &DirectFunderJaccard),
            Some(1.0)
        );
    }

    #[test]
    fn too_little_observed_funding_is_not_an_answer_of_zero() {
        let graph = FundingGraph::from_edges(vec![edge("hot", "a")]);
        let asked: Vec<WalletId> = vec!["a".into(), "unseen".into()];
        assert_eq!(linked_share(&graph, &asked, &DirectFunderJaccard), None);
    }
}
