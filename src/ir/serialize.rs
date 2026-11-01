//! Deterministic JSON serialization and deserialization for QccIR circuits.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::{Circuit, ClassicalRegister, QuantumRegister};
use crate::ir::edge::WireDependency;
use crate::ir::error::IrError;
use crate::ir::graph::CircuitGraph;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;

/// Serialized representation of a single operation node in QccIR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializedOperation {
    /// Stable semantic operation ID.
    pub id: OperationId,
    /// Sequence number reflecting deterministic topological position or insertion order.
    pub sequence_number: u64,
    /// Concrete operation payload.
    pub operation: Operation,
    /// Provenance tracking IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<OperationId>,
}

/// Serialized representation of a directed dependency edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SerializedDependency {
    /// Source operation ID.
    pub from: OperationId,
    /// Target operation ID.
    pub to: OperationId,
    /// Wire dependency kind and affected wire.
    pub dependency: WireDependency,
}

/// Complete serialized intermediate representation of a quantum circuit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializedCircuit {
    /// Schema format version for forward compatibility.
    pub schema_version: u32,
    /// Circuit program name.
    pub name: String,
    /// Circuit-level global phase expression.
    pub global_phase: ParameterExpr,
    /// Declared quantum registers.
    pub quantum_registers: Vec<QuantumRegister>,
    /// Declared classical registers.
    pub classical_registers: Vec<ClassicalRegister>,
    /// Nodes sorted in deterministic topological order.
    pub operations: Vec<SerializedOperation>,
    /// Dependency edges sorted deterministically.
    pub dependencies: Vec<SerializedDependency>,
    /// Arbitrary circuit metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Current QccIR schema format version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Serializes a [`Circuit`] to a deterministic JSON string with indentation.
///
/// Nodes are strictly emitted in deterministic topological order, and dependency edges
/// are sorted deterministically by `(from, to, dependency)`, ensuring byte-for-byte reproducibility.
///
/// # Errors
/// Returns [`IrError::CycleDetected`] if the circuit contains a cycle, or
/// [`IrError::SerializationError`] if JSON encoding fails.
pub fn circuit_to_json(circuit: &Circuit) -> Result<String, IrError> {
    circuit_to_json_pretty(circuit)
}

/// Serializes a [`Circuit`] to a deterministic pretty-printed JSON string.
pub fn circuit_to_json_pretty(circuit: &Circuit) -> Result<String, IrError> {
    let order = circuit.graph.deterministic_topological_order()?;

    let mut operations = Vec::with_capacity(order.len());
    for op_id in order {
        let node = circuit
            .graph
            .get_node(op_id)
            .ok_or(IrError::UnknownOperation { id: op_id })?;
        operations.push(SerializedOperation {
            id: node.id,
            sequence_number: node.sequence_number,
            operation: node.operation.clone(),
            provenance: node.provenance.clone(),
        });
    }

    let mut dependencies: Vec<SerializedDependency> = circuit
        .graph
        .all_edges()
        .into_iter()
        .map(|(from, to, dependency)| SerializedDependency {
            from,
            to,
            dependency,
        })
        .collect();

    // Deterministic sort: from -> to -> dependency
    dependencies.sort();

    let serialized = SerializedCircuit {
        schema_version: CURRENT_SCHEMA_VERSION,
        name: circuit.name.clone(),
        global_phase: circuit.global_phase.clone(),
        quantum_registers: circuit.quantum_registers.clone(),
        classical_registers: circuit.classical_registers.clone(),
        operations,
        dependencies,
        metadata: circuit.metadata.clone(),
    };

    serde_json::to_string_pretty(&serialized).map_err(|e| IrError::SerializationError {
        message: e.to_string(),
    })
}

/// Deserializes a [`Circuit`] from a JSON string and validates all structural invariants.
///
/// # Errors
/// Returns [`IrError::SerializationError`] if JSON parsing fails,
/// [`IrError::CycleDetected`] if a cycle is present, or
/// [`IrError::Validation`] if any semantic invariant is violated.
pub fn circuit_from_json(json: &str) -> Result<Circuit, IrError> {
    let serialized: SerializedCircuit =
        serde_json::from_str(json).map_err(|e| IrError::SerializationError {
            message: e.to_string(),
        })?;

    let nodes = serialized
        .operations
        .into_iter()
        .map(|op| NodeData::with_provenance(op.id, op.sequence_number, op.operation, op.provenance))
        .collect();

    let edges = serialized
        .dependencies
        .into_iter()
        .map(|dep| (dep.from, dep.to, dep.dependency))
        .collect();

    let graph = CircuitGraph::from_nodes_and_edges(nodes, edges)?;

    let circuit = Circuit {
        name: serialized.name,
        global_phase: serialized.global_phase,
        quantum_registers: serialized.quantum_registers,
        classical_registers: serialized.classical_registers,
        graph,
        metadata: serialized.metadata,
    };

    circuit.validate()?;
    Ok(circuit)
}
