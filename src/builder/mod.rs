//! Public Rust Circuit Builder API for constructing validated QccIR circuits.
//!
//! Provides a safe, fluent, strongly typed interface for creating registers, allocating
//! handles, applying gates, measuring qubits, and enforcing compile-time provenance.
//!
//! # Core Invariants
//! - **Encapsulation**: Does not expose `petgraph` internal indices or raw graph mutation.
//! - **Provenance Protection**: Handles carry builder IDs and are rejected if passed to foreign builders.
//! - **Consuming Finalization**: [`CircuitBuilder::finish`] consumes the builder by value to prevent
//!   double-finishing or post-finalization mutation.
//! - **Full Validation**: Circuits must pass comprehensive semantic validation before release.

pub mod circuit_builder;
pub mod errors;
pub mod handles;
pub mod view;

pub use circuit_builder::CircuitBuilder;
pub use errors::BuilderError;
pub use handles::{
    ClassicalBitHandle, ClassicalRegisterHandle, QuantumRegisterHandle, QubitHandle,
};
pub use view::CircuitView;
