//! Execution trace recording deterministic operation steps and measurements.

use std::fmt;

use crate::ir::node::OperationId;

/// Individual step recorded during circuit DAG execution.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionStep {
    /// 1-based sequential step index in the execution order.
    pub step_index: usize,
    /// QCC-owned stable semantic operation ID.
    pub operation_id: OperationId,
    /// Name or mnemonic of the executed operation.
    pub operation_name: String,
    /// Target qubit indices touched by this step.
    pub qubits: Vec<usize>,
    /// Human-readable description of the operation execution.
    pub details: String,
}

/// Record of a terminal measurement mapping a quantum wire to a classical destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeasurementRecord {
    /// Semantic ID of the measurement operation.
    pub operation_id: OperationId,
    /// Source qubit index measured.
    pub qubit: usize,
    /// Destination classical bit index written.
    pub destination: usize,
}

/// Complete execution trace returned by the circuit execution adapter.
///
/// Keeps execution metadata strictly decoupled from the mathematical simulator state.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionTrace {
    /// Name of the executed circuit.
    pub circuit_name: String,
    /// Total number of qubits in the simulated circuit.
    pub num_qubits: usize,
    /// Ordered sequence of executed steps in topological order.
    pub steps: Vec<ExecutionStep>,
    /// Terminal measurements registered during execution.
    pub measurements: Vec<MeasurementRecord>,
    /// Global phase factor angle in radians applied to the statevector.
    pub global_phase_applied: f64,
}

impl ExecutionTrace {
    /// Creates a new empty `ExecutionTrace`.
    pub fn new(circuit_name: impl Into<String>, num_qubits: usize) -> Self {
        Self {
            circuit_name: circuit_name.into(),
            num_qubits,
            steps: Vec::new(),
            measurements: Vec::new(),
            global_phase_applied: 0.0,
        }
    }

    /// Appends an execution step to the trace.
    pub fn record_step(
        &mut self,
        operation_id: OperationId,
        operation_name: impl Into<String>,
        qubits: Vec<usize>,
        details: impl Into<String>,
    ) {
        let step_index = self.steps.len() + 1;
        self.steps.push(ExecutionStep {
            step_index,
            operation_id,
            operation_name: operation_name.into(),
            qubits,
            details: details.into(),
        });
    }

    /// Appends a measurement record to the trace.
    pub fn record_measurement(
        &mut self,
        operation_id: OperationId,
        qubit: usize,
        destination: usize,
    ) {
        self.measurements.push(MeasurementRecord {
            operation_id,
            qubit,
            destination,
        });
    }

    /// Sets the applied global phase angle in radians.
    pub fn set_global_phase_applied(&mut self, phase_rad: f64) {
        self.global_phase_applied = phase_rad;
    }

    /// Returns the total number of executed steps.
    #[inline]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns an ordered list of all executed semantic [`OperationId`]s.
    pub fn executed_operation_ids(&self) -> Vec<OperationId> {
        self.steps.iter().map(|s| s.operation_id).collect()
    }

    /// Returns the slice of terminal measurement records.
    #[inline]
    pub fn measurements(&self) -> &[MeasurementRecord] {
        &self.measurements
    }

    /// Returns the global phase angle applied to the statevector in radians.
    #[inline]
    pub fn global_phase(&self) -> f64 {
        self.global_phase_applied
    }
}

impl fmt::Display for ExecutionTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "ExecutionTrace for '{}' ({} qubits, {} steps, global phase = {:.6} rad):",
            self.circuit_name,
            self.num_qubits,
            self.steps.len(),
            self.global_phase_applied
        )?;
        for step in &self.steps {
            writeln!(
                f,
                "  Step {:3}: [{}] {} (qubits: {:?}) - {}",
                step.step_index, step.operation_id, step.operation_name, step.qubits, step.details
            )?;
        }
        if !self.measurements.is_empty() {
            writeln!(f, "Measurements:")?;
            for m in &self.measurements {
                writeln!(
                    f,
                    "  [{}] measure q[{}] -> c[{}]",
                    m.operation_id, m.qubit, m.destination
                )?;
            }
        }
        Ok(())
    }
}
