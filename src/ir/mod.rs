//! Intermediate Representation (QccIR) for quantum circuits.
//!
//! QccIR represents a quantum program as one dependency Directed Acyclic Graph (DAG) per basic
//! block, tracking both quantum state evolutions and classical data hazards (RAW, WAR, WAW)
//! across explicit registers and wires.
//!
//! # Core Invariants
//! - **Stable Semantic Operation IDs**: All nodes are addressed via [`OperationId`], strictly
//!   decoupling API consumers from underlying `petgraph` internal indices.
//! - **Per-Wire Hazard Tracking**: Classical control dependencies strictly maintain
//!   [`WireAccessState`] with last-writer and outstanding-reader sets to prevent race hazards.
//! - **Explicit Circuit Global Phase**: Global phase factor $e^{i\phi}$ is tracked directly on
//!   [`Circuit`] as a [`ParameterExpr`].
//! - **Deterministic Traversals**: Kahn's topological sort breaks ties using a deterministic
//!   priority queue on [`OperationId`], ensuring reproducible compiler passes.

pub mod circuit;
pub mod edge;
pub mod error;
pub mod graph;
pub mod node;
pub mod operation;
pub mod parameter;
pub mod rewrite;
pub mod serialize;
pub mod validation;
pub mod wire;

// Re-export primary types for ergonomic compiler access
pub use circuit::{Circuit, ClassicalRegister, QuantumRegister};
pub use edge::{DependencyKind, WireDependency};
pub use error::{IrError, IrValidationError};
pub use graph::CircuitGraph;
pub use node::{NodeData, OperationId};
pub use operation::{Operation, WireEffects};
pub use parameter::ParameterExpr;
pub use rewrite::{
    check_convexity, expand_operation, fuse_operations, insert_after, insert_at_end,
    insert_at_start, insert_before, remove_operation, replace_operation, CircuitTransaction,
    RewriteError, RewriteResult,
};
pub use serialize::{
    circuit_from_json, circuit_to_json, circuit_to_json_pretty, SerializedCircuit,
    SerializedDependency, SerializedOperation, CURRENT_SCHEMA_VERSION,
};
pub use validation::{canonical_gate_signature, validate_circuit};
pub use wire::{ClassicalBitId, QubitId, WireAccessState, WireId};
