//! Circuit container representing registers, circuit-level global phase, and basic-block DAG.

use std::collections::HashMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::ir::error::{IrError, IrValidationError};
use crate::ir::graph::CircuitGraph;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::validation::validate_circuit;
use crate::ir::wire::{ClassicalBitId, QubitId};

/// Metadata and layout description for a quantum register.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantumRegister {
    /// Human-readable register name (e.g. `"q"` or `"ancilla"`).
    pub name: String,
    /// Number of qubits in this register.
    pub size: usize,
    /// Base starting qubit index in the circuit's flat qubit index space.
    pub start_qubit: usize,
}

impl QuantumRegister {
    /// Returns the slice range of flat qubit IDs in this register.
    pub fn range(&self) -> Range<usize> {
        self.start_qubit..(self.start_qubit + self.size)
    }

    /// Returns the global `QubitId` for the $k$-th qubit in this register.
    pub fn qubit(&self, index: usize) -> Option<QubitId> {
        if index < self.size {
            Some(QubitId(self.start_qubit + index))
        } else {
            None
        }
    }
}

/// Metadata and layout description for a classical register.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassicalRegister {
    /// Human-readable register name (e.g. `"c"` or `"meas"`).
    pub name: String,
    /// Number of classical bits in this register.
    pub size: usize,
    /// Base starting classical bit index in the circuit's flat bit index space.
    pub start_bit: usize,
}

impl ClassicalRegister {
    /// Returns the slice range of flat classical bit IDs in this register.
    pub fn range(&self) -> Range<usize> {
        self.start_bit..(self.start_bit + self.size)
    }

    /// Returns the global `ClassicalBitId` for the $k$-th bit in this register.
    pub fn bit(&self, index: usize) -> Option<ClassicalBitId> {
        if index < self.size {
            Some(ClassicalBitId(self.start_bit + index))
        } else {
            None
        }
    }
}

/// Canonical circuit intermediate representation (QccIR).
#[derive(Clone, Debug, PartialEq)]
pub struct Circuit {
    /// Name or identifier of the circuit program.
    pub name: String,
    /// Explicit circuit-level global phase factor: $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$.
    pub global_phase: ParameterExpr,
    /// Declared quantum registers.
    pub quantum_registers: Vec<QuantumRegister>,
    /// Declared classical registers.
    pub classical_registers: Vec<ClassicalRegister>,
    /// Operation dependency directed acyclic graph.
    pub graph: CircuitGraph,
    /// Arbitrary user or compiler metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}

impl Circuit {
    /// Creates a new empty `Circuit` with zero global phase.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            global_phase: ParameterExpr::zero(),
            quantum_registers: Vec::new(),
            classical_registers: Vec::new(),
            graph: CircuitGraph::new(),
            metadata: HashMap::new(),
        }
    }

    /// Adds a declared quantum register with the given name and size.
    ///
    /// Returns the flat qubit index range allocated to this register.
    ///
    /// # Errors
    /// Returns [`IrError::DuplicateRegister`] if a quantum register with `name` already exists.
    pub fn add_quantum_register(
        &mut self,
        name: impl Into<String>,
        size: usize,
    ) -> Result<Range<usize>, IrError> {
        let name = name.into();
        if self.quantum_registers.iter().any(|r| r.name == name) {
            return Err(IrError::DuplicateRegister { name });
        }

        let start_qubit = self.num_qubits();
        let reg = QuantumRegister {
            name,
            size,
            start_qubit,
        };
        let range = reg.range();
        self.quantum_registers.push(reg);
        Ok(range)
    }

    /// Adds a declared classical register with the given name and size.
    ///
    /// Returns the flat classical bit index range allocated to this register.
    ///
    /// # Errors
    /// Returns [`IrError::DuplicateRegister`] if a classical register with `name` already exists.
    pub fn add_classical_register(
        &mut self,
        name: impl Into<String>,
        size: usize,
    ) -> Result<Range<usize>, IrError> {
        let name = name.into();
        if self.classical_registers.iter().any(|r| r.name == name) {
            return Err(IrError::DuplicateRegister { name });
        }

        let start_bit = self.num_classical_bits();
        let reg = ClassicalRegister {
            name,
            size,
            start_bit,
        };
        let range = reg.range();
        self.classical_registers.push(reg);
        Ok(range)
    }

    /// Total number of declared qubits across all quantum registers.
    pub fn num_qubits(&self) -> usize {
        self.quantum_registers.iter().map(|r| r.size).sum()
    }

    /// Total number of declared classical bits across all classical registers.
    pub fn num_classical_bits(&self) -> usize {
        self.classical_registers.iter().map(|r| r.size).sum()
    }

    /// Total number of operations (nodes) in the circuit dependency DAG.
    #[inline]
    pub fn num_operations(&self) -> usize {
        self.graph.node_count()
    }

    /// Sets the circuit global phase.
    #[inline]
    pub fn set_global_phase(&mut self, phase: ParameterExpr) {
        self.global_phase = phase;
    }

    /// Adds a global phase contribution to the existing global phase.
    #[inline]
    pub fn add_global_phase(&mut self, phase: ParameterExpr) {
        self.global_phase = self.global_phase.clone() + phase;
    }

    /// Returns a reference to the circuit global phase.
    #[inline]
    pub fn global_phase(&self) -> &ParameterExpr {
        &self.global_phase
    }

    /// Checks that all quantum and classical wires referenced by an operation
    /// belong to declared registers in this circuit.
    pub fn check_operation_wire_bounds(&self, operation: &Operation) -> Result<(), IrError> {
        let effects = operation.wire_effects();
        let total_qubits = self.num_qubits();
        let total_bits = self.num_classical_bits();

        for q in &effects.quantum_writes {
            if q.0 >= total_qubits {
                return Err(IrError::InvalidQubit {
                    qubit: q.0,
                    total: total_qubits,
                });
            }
        }

        for c in effects
            .classical_reads
            .iter()
            .chain(&effects.classical_writes)
        {
            if c.0 >= total_bits {
                return Err(IrError::InvalidClassicalBit {
                    bit: c.0,
                    total: total_bits,
                });
            }
        }

        Ok(())
    }

    /// Returns the next operation ID that will be allocated by the circuit graph.
    #[inline]
    pub fn next_operation_id(&self) -> OperationId {
        self.graph.next_operation_id()
    }

    /// Returns the next sequence number that will be assigned by the circuit graph.
    #[inline]
    pub fn next_sequence_number(&self) -> u64 {
        self.graph.next_sequence_number()
    }

    /// Adds an operation to the circuit DAG after validating that all referenced qubits and
    /// classical bits belong to declared registers.
    pub fn add_operation(&mut self, operation: Operation) -> Result<OperationId, IrError> {
        self.check_operation_wire_bounds(&operation)?;
        self.graph.add_operation(operation)
    }

    /// Adds an operation to the circuit dependency DAG with explicit provenance tracking.
    pub fn add_operation_with_provenance(
        &mut self,
        operation: Operation,
        provenance: Vec<OperationId>,
    ) -> Result<OperationId, IrError> {
        self.check_operation_wire_bounds(&operation)?;
        self.graph
            .add_operation_with_provenance(operation, provenance)
    }

    /// Rebuilds the circuit's dependency graph from an ordered list of nodes.
    pub(crate) fn rebuild_graph(
        &mut self,
        ordered_nodes: Vec<crate::ir::node::NodeData>,
    ) -> Result<(), IrError> {
        for node in &ordered_nodes {
            self.check_operation_wire_bounds(&node.operation)?;
        }
        let new_graph = self.graph.rebuild_from_ordered_nodes(ordered_nodes)?;
        self.graph = new_graph;
        Ok(())
    }

    /// Validates all structural and semantic invariants of the circuit and its dependency DAG.
    pub fn validate(&self) -> Result<(), IrValidationError> {
        validate_circuit(self)
    }
}
