//! Core domain types for funding-provenance analysis on Solana.
//!
//! The crate is deliberately independent of any RPC client so that the same
//! types serve three callers: the offline evaluator, the on-chain round client,
//! and any external tool that wants to score its own wallets.

pub mod graph;

pub use graph::{EntityId, FundingEdge, FundingGraph, WalletId};
