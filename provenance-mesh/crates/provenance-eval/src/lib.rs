//! Adversarial evaluation of funding-provenance linkage.
//!
//! Behavioural noise — human-looking timing, plausible amounts, realistic
//! protocol mixes — is measured extensively by existing account-cooker work.
//! The funding graph is not, and it is a harder signal: an operator that pays
//! for its agents from one wallet is identifiable from that single fact no
//! matter how convincing the behaviour on top of it looks.
//!
//! This crate measures that channel directly. It generates identical fleets
//! under different funding topologies, runs the attacks an observer would
//! actually run, and reports separation as ROC AUC so the numbers are
//! comparable with what `cooker-eval` already publishes.

pub mod anonymity;
pub mod attack;
pub mod breakage;
pub mod evaluate;
pub mod fleet;
pub mod metrics;
pub mod scenario;

pub use anonymity::{measure as measure_anonymity, AnonymityReport};
pub use attack::{
    AncestorJaccard, DepositShareAttack, DirectFunderJaccard, PairwiseAttack,
    SharedAncestorIndicator,
};
pub use breakage::{measure as measure_breakage, Breakage};
pub use evaluate::{evaluate, linked_share, with_observed_funding, AttackResult};
pub use fleet::{scan as scan_fleets, Cluster, FunderShape, Scan};
pub use metrics::{effective_set_size, min_entropy, roc_auc, shannon_entropy};
pub use scenario::{generate, FleetSpec, FundingTopology, Scenario};
