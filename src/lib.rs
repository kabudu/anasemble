//! Bounded service-reconstruction research harness.

pub mod campaign;
pub mod canonical;
pub mod checker;
pub mod checker_wire;
pub mod corpus;
pub mod deployment;
pub mod fragments;
pub mod ledger;
pub mod model;
pub mod oracle;
pub mod protocol;
pub mod sandbox;
pub mod service;
pub mod state_store;
pub mod synthesizer;

pub use model::{Error, RefusalCode};
