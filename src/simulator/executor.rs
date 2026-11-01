//! QccIR DAG execution adapter connecting compiled circuits to statevector simulation.

use std::collections::{HashMap, HashSet};

use crate::ir::circuit::Circuit;
use crate::ir::error::IrError;
use crate::ir::operation::Operation;
use crate::simulator::error::ExecutionError;
use crate::simulator::measurement::MeasurementSelection;
use crate::simulator::operation_dispatch::{dispatch_gate, eval_parameter};
use crate::simulator::result::ExecutionResult;
use crate::simulator::sampling::SeededRng;
use crate::simulator::statevector::StateVector;
use crate::simulator::trace::ExecutionTrace;

/// Configurable DAG-based circuit execution adapter.
///
/// Executes QccIR circuits by topologically traversing operation dependencies,
/// dispatching unitary gates to in-place kernels, applying circuit global phase,
/// recording execution traces, and performing terminal measurement sampling.
#[derive(Clone, Debug, Default)]
pub struct CircuitExecutor {
    /// Variable environment for evaluating symbolic parameter expressions.
    environment: HashMap<String, f64>,
}

impl CircuitExecutor {
    /// Creates a new default `CircuitExecutor`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a named symbolic parameter to a numeric floating-point value.
    pub fn with_parameter(mut self, name: impl Into<String>, value: f64) -> Self {
        self.environment.insert(name.into(), value);
        self
    }

    /// Sets the entire parameter evaluation environment.
    pub fn with_environment(mut self, env: HashMap<String, f64>) -> Self {
        self.environment = env;
        self
    }

    /// Pre-validates the circuit and verifies that it adheres strictly to the terminal execution model.
    ///
    /// # Rejections
    /// - Structural/semantic IR invalidity (acyclicity, register bounds, arity).
    /// - Dynamic mid-circuit measurements (qubit measured and subsequently evolved).
    /// - Unsupported active resets.
    /// - Classical dynamic feedback or uninterpreted classical computation.
    fn validate_execution_model(&self, circuit: &Circuit) -> Result<(), ExecutionError> {
        if circuit.num_qubits() == 0 {
            return Err(ExecutionError::ZeroQubitCircuit);
        }

        // 1. Full QccIR semantic and structural validation
        circuit.validate()?;

        // 2. Obtain deterministic topological order
        let order = circuit.graph.deterministic_topological_order()?;

        // 3. Verify operation types and terminal-only measurement semantics
        let mut measured_qubits: HashSet<usize> = HashSet::new();
        let mut measurement_destinations: HashSet<usize> = HashSet::new();

        for op_id in order {
            let node = circuit
                .graph
                .get_node(op_id)
                .ok_or(IrError::UnknownOperation { id: op_id })?;

            match &node.operation {
                Operation::Reset { .. } => {
                    return Err(ExecutionError::UnsupportedOperation {
                        operation: "reset".to_string(),
                        reason: "active reset is unsupported in terminal-only statevector execution (dynamic trajectory simulation is scheduled for Milestone M7)".to_string(),
                    });
                }
                Operation::Classical { reads, .. } => {
                    // Check if classical op reads from a measurement outcome
                    for c in reads {
                        if measurement_destinations.contains(&c.0) {
                            return Err(ExecutionError::DynamicFeedbackUnsupported { op_id });
                        }
                    }
                    return Err(ExecutionError::UnsupportedOperation {
                        operation: "classical".to_string(),
                        reason: "classical computation within circuit is unsupported in pure statevector simulation".to_string(),
                    });
                }
                Operation::Measure { qubit, destination } => {
                    if !measured_qubits.insert(qubit.0) {
                        return Err(ExecutionError::MidCircuitMeasurement {
                            op_id,
                            reason: format!("qubit {} was measured multiple times", qubit.0),
                        });
                    }
                    if !measurement_destinations.insert(destination.0) {
                        return Err(ExecutionError::MidCircuitMeasurement {
                            op_id,
                            reason: format!(
                                "classical bit {} is written by multiple measurements",
                                destination.0
                            ),
                        });
                    }
                }
                Operation::Gate {
                    gate_name, qubits, ..
                } => {
                    for q in qubits {
                        if measured_qubits.contains(&q.0) {
                            return Err(ExecutionError::MidCircuitMeasurement {
                                op_id,
                                reason: format!(
                                    "qubit {} was measured earlier and cannot undergo subsequent gate '{}'",
                                    q.0, gate_name
                                ),
                            });
                        }
                    }
                }
                Operation::Barrier { qubits } => {
                    for q in qubits {
                        if measured_qubits.contains(&q.0) {
                            return Err(ExecutionError::MidCircuitMeasurement {
                                op_id,
                                reason: format!(
                                    "qubit {} was measured earlier and cannot undergo subsequent barrier",
                                    q.0
                                ),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Executes the circuit program deterministically, returning the evolved [`StateVector`]
    /// and a comprehensive [`ExecutionTrace`].
    ///
    /// Preserves explicit circuit global phase $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$.
    ///
    /// # Errors
    /// Returns [`ExecutionError`] if circuit validation fails, mid-circuit measurement is detected,
    /// an unsupported operation is encountered, or simulator memory bounds are exceeded.
    pub fn execute(
        &self,
        circuit: &Circuit,
    ) -> Result<(StateVector, ExecutionTrace), ExecutionError> {
        self.validate_execution_model(circuit)?;

        let num_qubits = circuit.num_qubits();
        let mut state = StateVector::new(num_qubits)?;
        let mut trace = ExecutionTrace::new(&circuit.name, num_qubits);

        let order = circuit.graph.deterministic_topological_order()?;

        for op_id in order {
            let node = circuit
                .graph
                .get_node(op_id)
                .ok_or(IrError::UnknownOperation { id: op_id })?;

            match &node.operation {
                Operation::Gate {
                    gate_name,
                    qubits,
                    parameters,
                } => {
                    let qids: Vec<usize> = qubits.iter().map(|q| q.0).collect();
                    dispatch_gate(
                        &mut state,
                        gate_name,
                        &qids,
                        parameters,
                        Some(&self.environment),
                    )?;
                    trace.record_step(
                        op_id,
                        gate_name.clone(),
                        qids,
                        format!("applied gate '{gate_name}'"),
                    );
                }
                Operation::Barrier { qubits } => {
                    let qids: Vec<usize> = qubits.iter().map(|q| q.0).collect();
                    trace.record_step(op_id, "barrier", qids, "synchronization barrier");
                }
                Operation::Measure { qubit, destination } => {
                    trace.record_measurement(op_id, qubit.0, destination.0);
                    trace.record_step(
                        op_id,
                        "measure",
                        vec![qubit.0],
                        format!(
                            "terminal measurement q[{}] -> c[{}]",
                            qubit.0, destination.0
                        ),
                    );
                }
                _ => unreachable!("unsupported operations rejected by validate_execution_model"),
            }
        }

        // Apply circuit-level global phase factor e^(i * phi) to final statevector amplitudes
        let global_phase_rad = eval_parameter(&circuit.global_phase, Some(&self.environment))?;
        if global_phase_rad.abs() > 1e-15 {
            state.apply_global_phase(global_phase_rad);
            trace.set_global_phase_applied(global_phase_rad);
        }

        Ok((state, trace))
    }

    /// Executes the circuit once and draws `num_shots` measurement samples from the final evolved
    /// probability distribution using a seeded pseudorandom number generator.
    ///
    /// The circuit is simulated **strictly once**; all shots are sampled via CDF binary search
    /// without re-executing unitary gates.
    ///
    /// # Errors
    /// Returns [`ExecutionError`] if circuit validation fails, shot count is zero, or execution fails.
    pub fn sample<R: SeededRng>(
        &self,
        circuit: &Circuit,
        num_shots: usize,
        rng: R,
    ) -> Result<(ExecutionResult, ExecutionTrace), ExecutionError> {
        let (state, trace) = self.execute(circuit)?;

        let selection = if !trace.measurements.is_empty() {
            // Sort measurements by destination classical bit to preserve canonical result bit ordering
            let mut sorted_measurements = trace.measurements.clone();
            sorted_measurements.sort_by_key(|m| m.destination);
            let measured_qubits: Vec<usize> = sorted_measurements.iter().map(|m| m.qubit).collect();
            MeasurementSelection::new(circuit.num_qubits(), measured_qubits)?
        } else {
            // Default to measuring all qubits in register order [0..n) if no explicit measurements exist
            MeasurementSelection::all(circuit.num_qubits())?
        };

        let result = state.sample_terminal(selection, num_shots, rng)?;
        Ok((result, trace))
    }
}

/// Convenience function executing a circuit program with default configuration.
#[inline]
pub fn execute_circuit(circuit: &Circuit) -> Result<(StateVector, ExecutionTrace), ExecutionError> {
    CircuitExecutor::new().execute(circuit)
}

/// Convenience function sampling a circuit program with default configuration and a seeded RNG.
#[inline]
pub fn sample_circuit<R: SeededRng>(
    circuit: &Circuit,
    num_shots: usize,
    rng: R,
) -> Result<(ExecutionResult, ExecutionTrace), ExecutionError> {
    CircuitExecutor::new().sample(circuit, num_shots, rng)
}
