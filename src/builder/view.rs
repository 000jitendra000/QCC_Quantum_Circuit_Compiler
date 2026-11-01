//! Read-only, immutable circuit inspection view.

use std::fmt;

use crate::builder::errors::BuilderError;
use crate::ir::circuit::{Circuit, ClassicalRegister, QuantumRegister};
use crate::ir::node::NodeData;
use crate::ir::parameter::ParameterExpr;

/// An immutable, read-only view of a quantum circuit or in-progress builder state.
#[derive(Clone, Copy, Debug)]
pub struct CircuitView<'a> {
    circuit: &'a Circuit,
}

impl<'a> CircuitView<'a> {
    /// Creates a new `CircuitView` borrowing the given `Circuit`.
    #[inline]
    pub fn new(circuit: &'a Circuit) -> Self {
        Self { circuit }
    }

    /// Returns the name of the circuit.
    #[inline]
    pub fn name(&self) -> &str {
        &self.circuit.name
    }

    /// Returns the total number of declared qubits.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.circuit.num_qubits()
    }

    /// Returns the total number of declared classical bits.
    #[inline]
    pub fn num_classical_bits(&self) -> usize {
        self.circuit.num_classical_bits()
    }

    /// Returns the total number of operations currently in the circuit DAG.
    #[inline]
    pub fn num_operations(&self) -> usize {
        self.circuit.graph.node_count()
    }

    /// Returns the total number of dependency edges in the circuit DAG.
    #[inline]
    pub fn num_dependencies(&self) -> usize {
        self.circuit.graph.edge_count()
    }

    /// Returns a reference to the circuit global phase.
    #[inline]
    pub fn global_phase(&self) -> &ParameterExpr {
        self.circuit.global_phase()
    }

    /// Returns the slice of declared quantum registers.
    #[inline]
    pub fn quantum_registers(&self) -> &[QuantumRegister] {
        &self.circuit.quantum_registers
    }

    /// Returns the slice of declared classical registers.
    #[inline]
    pub fn classical_registers(&self) -> &[ClassicalRegister] {
        &self.circuit.classical_registers
    }

    /// Returns the underlying circuit reference.
    #[inline]
    pub fn circuit(&self) -> &'a Circuit {
        self.circuit
    }

    /// Returns all operations in deterministic topological order.
    pub fn topological_operations(&self) -> Result<Vec<&'a NodeData>, BuilderError> {
        let order = self.circuit.graph.deterministic_topological_order()?;
        let mut ops = Vec::with_capacity(order.len());
        for id in order {
            if let Some(node) = self.circuit.graph.get_node(id) {
                ops.push(node);
            }
        }
        Ok(ops)
    }

    /// Returns a formatted multi-line summary of the circuit.
    pub fn display_summary(&self) -> String {
        format!("{self}")
    }
}

impl fmt::Display for CircuitView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Circuit: {}", self.name())?;
        writeln!(
            f,
            "  Qubits: {} | Classical Bits: {} | Operations: {} | Dependencies: {}",
            self.num_qubits(),
            self.num_classical_bits(),
            self.num_operations(),
            self.num_dependencies()
        )?;
        writeln!(f, "  Global Phase: {}", self.global_phase())?;
        if !self.quantum_registers().is_empty() {
            write!(f, "  Quantum Registers:")?;
            for reg in self.quantum_registers() {
                write!(f, " {}[{}]", reg.name, reg.size)?;
            }
            writeln!(f)?;
        }
        if !self.classical_registers().is_empty() {
            write!(f, "  Classical Registers:")?;
            for reg in self.classical_registers() {
                write!(f, " {}[{}]", reg.name, reg.size)?;
            }
            writeln!(f)?;
        }
        if let Ok(ops) = self.topological_operations() {
            writeln!(f, "  Operations (Topological Order):")?;
            for (idx, node) in ops.iter().enumerate() {
                writeln!(f, "    {:3}: [{}] {}", idx + 1, node.id, node.operation)?;
            }
        }
        Ok(())
    }
}
