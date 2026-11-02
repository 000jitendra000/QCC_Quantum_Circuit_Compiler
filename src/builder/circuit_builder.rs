//! Canonical `CircuitBuilder` implementation providing safe, ergonomic circuit construction.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::builder::errors::BuilderError;
use crate::builder::handles::{
    ClassicalBitHandle, ClassicalRegisterHandle, QuantumRegisterHandle, QubitHandle,
};
use crate::builder::view::CircuitView;
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::{ClassicalBitId, QubitId};
use crate::quantum::gate::Gate;

/// Monotonically increasing atomic generator for unique builder instance IDs.
static NEXT_BUILDER_ID: AtomicU64 = AtomicU64::new(1);

/// Fluent, strongly typed builder for constructing validated QccIR circuits.
///
/// Ensures all registers, handles, operations, and wire hazards are checked at construction
/// time, and enforces complete semantic validation upon calling [`finish`](Self::finish).
#[derive(Debug)]
pub struct CircuitBuilder {
    /// Unique instance ID for provenance verification of handles.
    builder_id: u64,
    /// Underlying QccIR circuit program.
    circuit: Circuit,
    /// Declared quantum register handles indexed by name.
    quantum_registers: HashMap<String, QuantumRegisterHandle>,
    /// Declared classical register handles indexed by name.
    classical_registers: HashMap<String, ClassicalRegisterHandle>,
}

impl CircuitBuilder {
    /// Creates a new `CircuitBuilder` for a circuit with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        let builder_id = NEXT_BUILDER_ID.fetch_add(1, Ordering::Relaxed);
        let circuit = Circuit::new(name);
        Self {
            builder_id,
            circuit,
            quantum_registers: HashMap::new(),
            classical_registers: HashMap::new(),
        }
    }

    /// Returns the unique ID of this builder instance.
    #[inline]
    pub fn builder_id(&self) -> u64 {
        self.builder_id
    }

    /// Declares a new quantum register with `name` and `size` qubits.
    ///
    /// # Errors
    /// Returns [`BuilderError::EmptyRegister`] if `size == 0`.
    /// Returns [`BuilderError::DuplicateRegister`] if a register with `name` is already declared.
    pub fn add_quantum_register(
        &mut self,
        name: impl Into<String>,
        size: usize,
    ) -> Result<QuantumRegisterHandle, BuilderError> {
        let name = name.into();
        if size == 0 {
            return Err(BuilderError::EmptyRegister { name });
        }
        if self.quantum_registers.contains_key(&name) {
            return Err(BuilderError::DuplicateRegister { name });
        }

        let range = self.circuit.add_quantum_register(name.clone(), size)?;
        let handle = QuantumRegisterHandle::new(self.builder_id, name.clone(), range.start, size);
        self.quantum_registers.insert(name, handle.clone());
        Ok(handle)
    }

    /// Declares a new classical register with `name` and `size` classical bits.
    ///
    /// # Errors
    /// Returns [`BuilderError::EmptyRegister`] if `size == 0`.
    /// Returns [`BuilderError::DuplicateRegister`] if a register with `name` is already declared.
    pub fn add_classical_register(
        &mut self,
        name: impl Into<String>,
        size: usize,
    ) -> Result<ClassicalRegisterHandle, BuilderError> {
        let name = name.into();
        if size == 0 {
            return Err(BuilderError::EmptyRegister { name });
        }
        if self.classical_registers.contains_key(&name) {
            return Err(BuilderError::DuplicateRegister { name });
        }

        let range = self.circuit.add_classical_register(name.clone(), size)?;
        let handle = ClassicalRegisterHandle::new(self.builder_id, name.clone(), range.start, size);
        self.classical_registers.insert(name, handle.clone());
        Ok(handle)
    }

    /// Retrieves a typed [`QubitHandle`] for the qubit at `index` in quantum register `reg_name`.
    ///
    /// # Errors
    /// Returns [`BuilderError::UnknownRegister`] if `reg_name` is not declared.
    /// Returns [`BuilderError::IndexOutOfBounds`] if `index >= size`.
    pub fn q(&self, reg_name: &str, index: usize) -> Result<QubitHandle, BuilderError> {
        let reg =
            self.quantum_registers
                .get(reg_name)
                .ok_or_else(|| BuilderError::UnknownRegister {
                    name: reg_name.to_string(),
                })?;
        reg.q(index)
    }

    /// Convenient alias for [`q`](Self::q).
    #[inline]
    pub fn qubit(&self, reg_name: &str, index: usize) -> Result<QubitHandle, BuilderError> {
        self.q(reg_name, index)
    }

    /// Retrieves a typed [`ClassicalBitHandle`] for the bit at `index` in classical register `reg_name`.
    ///
    /// # Errors
    /// Returns [`BuilderError::UnknownRegister`] if `reg_name` is not declared.
    /// Returns [`BuilderError::IndexOutOfBounds`] if `index >= size`.
    pub fn c(&self, reg_name: &str, index: usize) -> Result<ClassicalBitHandle, BuilderError> {
        let reg = self.classical_registers.get(reg_name).ok_or_else(|| {
            BuilderError::UnknownRegister {
                name: reg_name.to_string(),
            }
        })?;
        reg.c(index)
    }

    /// Convenient alias for [`c`](Self::c).
    #[inline]
    pub fn bit(&self, reg_name: &str, index: usize) -> Result<ClassicalBitHandle, BuilderError> {
        self.c(reg_name, index)
    }

    // --- Validation Helpers ---

    /// Verifies that a `QubitHandle` originates from this builder and points to a declared qubit.
    fn validate_qubit(&self, q: QubitHandle) -> Result<QubitId, BuilderError> {
        if q.builder_id != self.builder_id {
            return Err(BuilderError::ForeignHandle {
                expected: self.builder_id,
                actual: q.builder_id,
            });
        }
        let total = self.circuit.num_qubits();
        if q.qubit_id.0 >= total {
            return Err(BuilderError::Ir(crate::ir::error::IrError::InvalidQubit {
                qubit: q.qubit_id.0,
                total,
            }));
        }
        Ok(q.qubit_id)
    }

    /// Verifies that a `ClassicalBitHandle` originates from this builder and points to a declared bit.
    fn validate_bit(&self, c: ClassicalBitHandle) -> Result<ClassicalBitId, BuilderError> {
        if c.builder_id != self.builder_id {
            return Err(BuilderError::ForeignHandle {
                expected: self.builder_id,
                actual: c.builder_id,
            });
        }
        let total = self.circuit.num_classical_bits();
        if c.bit_id.0 >= total {
            return Err(BuilderError::Ir(
                crate::ir::error::IrError::InvalidClassicalBit {
                    bit: c.bit_id.0,
                    total,
                },
            ));
        }
        Ok(c.bit_id)
    }

    /// Validates a list of qubit handles, ensuring they are valid and mutually distinct.
    fn validate_qubits_distinct(
        &self,
        op_name: &str,
        qubits: &[QubitHandle],
    ) -> Result<Vec<QubitId>, BuilderError> {
        let mut qids = Vec::with_capacity(qubits.len());
        for &q in qubits {
            let qid = self.validate_qubit(q)?;
            if qids.contains(&qid) {
                return Err(BuilderError::DuplicateOperandQubit {
                    operation: op_name.to_string(),
                    qubit: qid,
                });
            }
            qids.push(qid);
        }
        Ok(qids)
    }

    // --- Single-Qubit Fixed Gates ---

    /// Applies a Hadamard gate ($H$) to qubit `q`.
    pub fn h(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::H, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies a Pauli-X (NOT) gate to qubit `q`.
    pub fn x(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::X, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies a Pauli-Y gate to qubit `q`.
    pub fn y(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::Y, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies a Pauli-Z (phase-flip) gate to qubit `q`.
    pub fn z(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::Z, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies a phase S gate ($\sqrt{Z}$) to qubit `q`.
    pub fn s(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::S, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies an inverse phase Sdg gate ($S^\dagger$) to qubit `q`.
    pub fn sdg(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::Sdg, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies a phase T gate ($\sqrt{S}$) to qubit `q`.
    pub fn t(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::T, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies an inverse phase Tdg gate ($T^\dagger$) to qubit `q`.
    pub fn tdg(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::Tdg, vec![qid]))?;
        Ok(op_id)
    }

    /// Applies an explicit single-qubit identity gate (`id`) to qubit `q`.
    pub fn id_gate(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op_id =
            self.circuit
                .add_operation(Operation::custom_gate("id", vec![qid], Vec::new()))?;
        Ok(op_id)
    }

    /// Applies an explicit single-qubit identity gate (`id`) to qubit `q`.
    ///
    /// Alias for [`Self::id_gate`].
    pub fn id(&mut self, q: QubitHandle) -> Result<OperationId, BuilderError> {
        self.id_gate(q)
    }

    // --- Parameterized Rotation Gates ---

    /// Applies a rotation around the X-axis by angle `theta` to qubit `q`.
    pub fn rx(
        &mut self,
        q: QubitHandle,
        theta: impl Into<ParameterExpr>,
    ) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op = Operation::custom_gate("rx", vec![qid], vec![theta.into()]);
        let op_id = self.circuit.add_operation(op)?;
        Ok(op_id)
    }

    /// Applies a rotation around the Y-axis by angle `theta` to qubit `q`.
    pub fn ry(
        &mut self,
        q: QubitHandle,
        theta: impl Into<ParameterExpr>,
    ) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op = Operation::custom_gate("ry", vec![qid], vec![theta.into()]);
        let op_id = self.circuit.add_operation(op)?;
        Ok(op_id)
    }

    /// Applies a rotation around the Z-axis by angle `theta` to qubit `q`.
    pub fn rz(
        &mut self,
        q: QubitHandle,
        theta: impl Into<ParameterExpr>,
    ) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(q)?;
        let op = Operation::custom_gate("rz", vec![qid], vec![theta.into()]);
        let op_id = self.circuit.add_operation(op)?;
        Ok(op_id)
    }

    // --- Multi-Qubit Entanglers ---

    /// Applies a Controlled-NOT ($CX$) gate with `control` as Operand 0 and `target` as Operand 1.
    ///
    /// # Invariant
    /// In accordance with QCC semantic contracts, `control` and `target` must be distinct qubits.
    pub fn cx(
        &mut self,
        control: QubitHandle,
        target: QubitHandle,
    ) -> Result<OperationId, BuilderError> {
        let qids = self.validate_qubits_distinct("cx", &[control, target])?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::CX, qids))?;
        Ok(op_id)
    }

    /// Applies a Controlled-Z ($CZ$) gate across `q0` and `q1`.
    pub fn cz(&mut self, q0: QubitHandle, q1: QubitHandle) -> Result<OperationId, BuilderError> {
        let qids = self.validate_qubits_distinct("cz", &[q0, q1])?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::CZ, qids))?;
        Ok(op_id)
    }

    /// Applies a SWAP gate exchanging the states of `q0` and `q1`.
    pub fn swap(&mut self, q0: QubitHandle, q1: QubitHandle) -> Result<OperationId, BuilderError> {
        let qids = self.validate_qubits_distinct("swap", &[q0, q1])?;
        let op_id = self
            .circuit
            .add_operation(Operation::gate(&Gate::SWAP, qids))?;
        Ok(op_id)
    }

    // --- Generic Gate Application ---

    /// Applies a canonical Phase 2 [`Gate`] over the specified operand `qubits`.
    pub fn gate(
        &mut self,
        gate: &Gate,
        qubits: &[QubitHandle],
    ) -> Result<OperationId, BuilderError> {
        if qubits.len() != gate.arity() {
            return Err(BuilderError::GateArityMismatch {
                gate: gate.name().to_string(),
                expected: gate.arity(),
                actual: qubits.len(),
            });
        }
        let qids = self.validate_qubits_distinct(gate.name(), qubits)?;
        let op_id = self.circuit.add_operation(Operation::gate(gate, qids))?;
        Ok(op_id)
    }

    /// Applies a custom named gate with explicit operand qubits and parameters.
    pub fn custom_gate(
        &mut self,
        name: impl Into<String>,
        qubits: &[QubitHandle],
        parameters: Vec<ParameterExpr>,
    ) -> Result<OperationId, BuilderError> {
        let name = name.into();
        let qids = self.validate_qubits_distinct(&name, qubits)?;
        let op = Operation::custom_gate(name, qids, parameters);
        let op_id = self.circuit.add_operation(op)?;
        Ok(op_id)
    }

    // --- Non-Unitary Operations ---

    /// Adds a projective quantum measurement writing to classical destination `destination`.
    pub fn measure(
        &mut self,
        qubit: QubitHandle,
        destination: ClassicalBitHandle,
    ) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(qubit)?;
        let cid = self.validate_bit(destination)?;
        let op_id = self.circuit.add_operation(Operation::measure(qid, cid))?;
        Ok(op_id)
    }

    /// Adds an active state reset operation setting `qubit` to $|0\rangle$.
    pub fn reset(&mut self, qubit: QubitHandle) -> Result<OperationId, BuilderError> {
        let qid = self.validate_qubit(qubit)?;
        let op_id = self.circuit.add_operation(Operation::reset(qid))?;
        Ok(op_id)
    }

    /// Inserts a synchronization barrier boundary across the specified `qubits`.
    pub fn barrier(&mut self, qubits: &[QubitHandle]) -> Result<OperationId, BuilderError> {
        let qids = self.validate_qubits_distinct("barrier", qubits)?;
        let op_id = self.circuit.add_operation(Operation::barrier(qids))?;
        Ok(op_id)
    }

    /// Adds a classical computation with declared read and write classical bits.
    pub fn classical(
        &mut self,
        name: impl Into<String>,
        reads: &[ClassicalBitHandle],
        writes: &[ClassicalBitHandle],
    ) -> Result<OperationId, BuilderError> {
        let mut read_cids = Vec::with_capacity(reads.len());
        for &c in reads {
            read_cids.push(self.validate_bit(c)?);
        }
        let mut write_cids = Vec::with_capacity(writes.len());
        for &c in writes {
            write_cids.push(self.validate_bit(c)?);
        }
        let op = Operation::classical(name, read_cids, write_cids);
        let op_id = self.circuit.add_operation(op)?;
        Ok(op_id)
    }

    // --- Global Phase ---

    /// Sets the explicit circuit-level global phase factor.
    #[inline]
    pub fn set_global_phase(&mut self, phase: impl Into<ParameterExpr>) {
        self.circuit.set_global_phase(phase.into());
    }

    /// Adds a contribution to the explicit circuit-level global phase factor.
    #[inline]
    pub fn add_global_phase(&mut self, phase: impl Into<ParameterExpr>) {
        self.circuit.add_global_phase(phase.into());
    }

    /// Returns a reference to the current circuit global phase.
    #[inline]
    pub fn global_phase(&self) -> &ParameterExpr {
        self.circuit.global_phase()
    }

    // --- Metadata ---

    /// Inserts an arbitrary key-value metadata entry into the circuit.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.circuit.metadata.insert(key.into(), value.into());
    }

    // --- Public Immutable View ---

    /// Returns an immutable, read-only [`CircuitView`] of the in-progress circuit.
    #[inline]
    pub fn view(&self) -> CircuitView<'_> {
        CircuitView::new(&self.circuit)
    }

    /// Returns a reference to the underlying [`Circuit`].
    #[inline]
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
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

    /// Returns the total number of operations currently in the DAG.
    #[inline]
    pub fn num_operations(&self) -> usize {
        self.circuit.graph.node_count()
    }

    // --- Finishing and Consuming ---

    /// Finalizes construction, executes complete semantic validation of the DAG, and returns the [`Circuit`].
    ///
    /// Consumes the builder to guarantee that finalized circuits cannot be mutated inconsistently.
    ///
    /// # Errors
    /// Returns [`BuilderError::Validation`] if any structural, register, or causality invariant fails.
    pub fn finish(self) -> Result<Circuit, BuilderError> {
        self.circuit.validate()?;
        Ok(self.circuit)
    }
}
