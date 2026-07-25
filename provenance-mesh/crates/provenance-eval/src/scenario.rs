//! Reproducible synthetic fleets under different funding topologies.
//!
//! Every scenario holds the *same* agents owned by the *same* operators, so the
//! only variable across a comparison is how those agents received their
//! lamports. That isolation is the point: it attributes any change in attack
//! performance to the funding topology and nothing else.

use std::collections::{BTreeMap, BTreeSet};

use provenance_core::{EntityId, FundingEdge, FundingGraph, WalletId};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Fleet shape, held constant across topologies.
#[derive(Clone, Copy, Debug)]
pub struct FleetSpec {
    /// Independent operators, each of which would like to stay unlinkable.
    pub entities: usize,
    /// Agent wallets each operator runs.
    pub agents_per_entity: usize,
}

impl FleetSpec {
    /// Total agent wallets in the fleet.
    #[must_use]
    pub fn total_agents(&self) -> usize {
        self.entities * self.agents_per_entity
    }
}

/// How agents received their funding.
#[derive(Clone, Copy, Debug)]
pub enum FundingTopology {
    /// One source wallet per operator pays every one of its agents directly.
    ///
    /// The status quo for every account-cooker implementation submitted so far.
    NaiveStar,
    /// Each agent is paid through a private chain of `depth` intermediates.
    ///
    /// The intuitive "just add hops" defence. It is included precisely because
    /// it looks like it should work and does not.
    Cascade {
        /// Intermediate wallets between the operator's source and each agent.
        depth: usize,
    },
    /// Agents are paid by shared pool accounts that several operators fund.
    ///
    /// The defence this workspace proposes. Recipients are assigned to rounds
    /// independently of ownership, so a pool's payouts carry no operator signal.
    MeshRound {
        /// Payouts settled per round.
        round_size: usize,
    },
}

impl FundingTopology {
    /// Stable name used in reports.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::NaiveStar => "naive-star".to_owned(),
            Self::Cascade { depth } => format!("cascade-depth-{depth}"),
            Self::MeshRound { round_size } => format!("mesh-round-{round_size}"),
        }
    }
}

/// A generated fleet plus the ground truth needed to score attacks against it.
#[derive(Clone, Debug)]
pub struct Scenario {
    /// Public funding transfers — everything an observer gets to see.
    pub graph: FundingGraph,
    /// The agent wallets under evaluation.
    pub agents: Vec<WalletId>,
    /// Ground-truth ownership. Never visible to an attack.
    pub labels: BTreeMap<WalletId, EntityId>,
    /// Distinct operators funding each round, for round-based topologies.
    ///
    /// This is the anonymity a round actually delivered, as opposed to the `k`
    /// it was configured with. A single-operator deployment produces all-ones
    /// here, which is the honest reason mesh rounds cannot help one operator
    /// acting alone.
    pub achieved_k: Vec<usize>,
}

impl Scenario {
    /// Whether two wallets belong to the same operator.
    #[must_use]
    pub fn same_entity(&self, left: &WalletId, right: &WalletId) -> Option<bool> {
        Some(self.labels.get(left)? == self.labels.get(right)?)
    }

    /// Mean distinct-operator count across rounds, or `None` when not round-based.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn mean_achieved_k(&self) -> Option<f64> {
        if self.achieved_k.is_empty() {
            return None;
        }
        let total: usize = self.achieved_k.iter().sum();
        Some(total as f64 / self.achieved_k.len() as f64)
    }
}

fn source_wallet(entity: usize) -> WalletId {
    WalletId(format!("src-{entity:04}"))
}

fn agent_wallet(entity: usize, index: usize) -> WalletId {
    WalletId(format!("agent-{entity:04}-{index:04}"))
}

/// Generate a fleet under `topology`.
///
/// `seed` fixes the round assignment, so a given `(spec, topology, seed)` always
/// produces byte-identical output.
#[must_use]
pub fn generate(spec: FleetSpec, topology: FundingTopology, seed: u64) -> Scenario {
    let mut agents = Vec::with_capacity(spec.total_agents());
    let mut labels = BTreeMap::new();
    for entity in 0..spec.entities {
        for index in 0..spec.agents_per_entity {
            let wallet = agent_wallet(entity, index);
            labels.insert(wallet.clone(), EntityId(format!("entity-{entity:04}")));
            agents.push(wallet);
        }
    }

    let mut edges = Vec::new();
    let mut achieved_k = Vec::new();
    let mut slot = 1_u64;

    match topology {
        FundingTopology::NaiveStar => {
            for entity in 0..spec.entities {
                for index in 0..spec.agents_per_entity {
                    edges.push(FundingEdge {
                        source: source_wallet(entity),
                        target: agent_wallet(entity, index),
                        lamports: 10_000_000,
                        slot,
                        signers: Vec::new(),
                    });
                    slot += 1;
                }
            }
        }
        FundingTopology::Cascade { depth } => {
            for entity in 0..spec.entities {
                for index in 0..spec.agents_per_entity {
                    // Walk source -> hop_1 -> ... -> hop_depth -> agent.
                    let mut current = source_wallet(entity);
                    for hop in 0..depth {
                        let next = WalletId(format!("hop-{entity:04}-{index:04}-{hop:02}"));
                        edges.push(FundingEdge {
                            source: current,
                            target: next.clone(),
                            lamports: 10_000_000,
                            slot,
                            signers: Vec::new(),
                        });
                        slot += 1;
                        current = next;
                    }
                    edges.push(FundingEdge {
                        source: current,
                        target: agent_wallet(entity, index),
                        lamports: 10_000_000,
                        slot,
                        signers: Vec::new(),
                    });
                    slot += 1;
                }
            }
        }
        FundingTopology::MeshRound { round_size } => {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let mut shuffled = agents.clone();
            shuffled.shuffle(&mut rng);

            let round_size = round_size.max(1);
            for (round, recipients) in shuffled.chunks(round_size).enumerate() {
                let pool = WalletId(format!("pool-{round:04}"));

                // One deposit per payout, from the payout's owner — exactly what
                // the on-chain program enforces, since it settles only when
                // deposits equal payouts.
                //
                // This is deliberately *not* one deposit per operator. Doing
                // that would create lamports out of nothing, and it would also
                // hide a real leak: an operator with three agents in this round
                // must deposit three times, and those deposit counts are public.
                let depositors: BTreeSet<&EntityId> = recipients
                    .iter()
                    .filter_map(|wallet| labels.get(wallet))
                    .collect();
                achieved_k.push(depositors.len());

                for recipient in recipients {
                    let entity_index: usize = labels
                        .get(recipient)
                        .and_then(|entity| entity.0.strip_prefix("entity-"))
                        .and_then(|digits| digits.parse().ok())
                        .unwrap_or(0);
                    edges.push(FundingEdge {
                        source: source_wallet(entity_index),
                        target: pool.clone(),
                        lamports: 10_000_000,
                        slot,
                        signers: Vec::new(),
                    });
                    slot += 1;
                }
                for recipient in recipients {
                    edges.push(FundingEdge {
                        source: pool.clone(),
                        target: recipient.clone(),
                        lamports: 10_000_000,
                        slot,
                        signers: Vec::new(),
                    });
                    slot += 1;
                }
            }
        }
    }

    Scenario {
        graph: FundingGraph::from_edges(edges),
        agents,
        labels,
        achieved_k,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: FleetSpec = FleetSpec {
        entities: 4,
        agents_per_entity: 5,
    };

    #[test]
    fn every_topology_funds_every_agent() {
        for topology in [
            FundingTopology::NaiveStar,
            FundingTopology::Cascade { depth: 3 },
            FundingTopology::MeshRound { round_size: 6 },
        ] {
            let scenario = generate(SPEC, topology, 7);
            assert_eq!(scenario.agents.len(), SPEC.total_agents());
            for agent in &scenario.agents {
                assert!(
                    !scenario.graph.direct_funders(agent).is_empty(),
                    "{} unfunded under {}",
                    agent.as_str(),
                    topology.name()
                );
            }
        }
    }

    #[test]
    fn generation_is_deterministic_for_a_seed() {
        let topology = FundingTopology::MeshRound { round_size: 6 };
        let first = generate(SPEC, topology, 11);
        let second = generate(SPEC, topology, 11);
        assert_eq!(first.graph.edges(), second.graph.edges());
        assert_eq!(first.achieved_k, second.achieved_k);
    }

    #[test]
    fn different_seeds_reassign_rounds() {
        let topology = FundingTopology::MeshRound { round_size: 6 };
        let first = generate(SPEC, topology, 11);
        let second = generate(SPEC, topology, 12);
        assert_ne!(first.graph.edges(), second.graph.edges());
    }

    #[test]
    fn star_agents_share_exactly_one_direct_funder() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let left = agent_wallet(0, 0);
        let right = agent_wallet(0, 1);
        assert_eq!(
            scenario.graph.direct_funders(&left),
            scenario.graph.direct_funders(&right)
        );
    }

    #[test]
    fn pools_pay_out_exactly_what_they_took_in() {
        // The on-chain program refuses to settle unless deposits equal payouts,
        // so a scenario where a pool disburses more than it received models a
        // round the program would never produce — and quietly overstates the
        // privacy on offer.
        let scenario = generate(SPEC, FundingTopology::MeshRound { round_size: 6 }, 5);
        let pools: BTreeSet<&WalletId> = scenario
            .graph
            .edges()
            .iter()
            .map(|edge| &edge.source)
            .filter(|wallet| wallet.as_str().starts_with("pool-"))
            .collect();
        assert!(!pools.is_empty(), "topology should produce pools");

        for pool in pools {
            let deposits_in: u64 = scenario
                .graph
                .edges()
                .iter()
                .filter(|edge| &edge.target == pool)
                .map(|edge| edge.lamports)
                .sum();
            let payouts_out: u64 = scenario
                .graph
                .edges()
                .iter()
                .filter(|edge| &edge.source == pool)
                .map(|edge| edge.lamports)
                .sum();
            assert_eq!(
                deposits_in,
                payouts_out,
                "pool {} conjured lamports",
                pool.as_str()
            );
        }
    }

    #[test]
    fn a_lone_operator_cannot_reach_k_above_one() {
        // The honest limit: mesh rounds need a crowd. One operator funding its
        // own rounds achieves k = 1 in every round, and the report must say so.
        let solo = FleetSpec {
            entities: 1,
            agents_per_entity: 20,
        };
        let scenario = generate(solo, FundingTopology::MeshRound { round_size: 5 }, 3);
        assert!(scenario.achieved_k.iter().all(|k| *k == 1));
        assert_eq!(scenario.mean_achieved_k(), Some(1.0));
    }

    #[test]
    fn labels_are_never_part_of_the_graph() {
        let scenario = generate(SPEC, FundingTopology::NaiveStar, 1);
        let addresses: BTreeSet<&str> = scenario
            .graph
            .edges()
            .iter()
            .flat_map(|edge| [edge.source.as_str(), edge.target.as_str()])
            .collect();
        for label in scenario.labels.values() {
            assert!(!addresses.contains(label.0.as_str()));
        }
    }
}
