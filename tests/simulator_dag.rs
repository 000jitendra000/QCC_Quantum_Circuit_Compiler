//! Comprehensive integration tests for the QccIR execution adapter (Phase 8).

use std::collections::HashMap;
use std::f64::consts::{FRAC_1_SQRT_2, PI};

use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::ir::operation::Operation;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::quantum::gate::Gate;
use qcc::simulator::executor::{execute_circuit, sample_circuit, CircuitExecutor};
use qcc::simulator::sampling::SplitMix64;
use qcc::simulator::statevector::StateVector;
use qcc::simulator::ExecutionError;
use qcc::{Angle, ClassicalBitOrder};

/// Independent sequential reference gate representation for comparison against DAG execution.
enum RefOp {
    H(usize),
    X(usize),
    Y(usize),
    Z(usize),
    S(usize),
    Sdg(usize),
    T(usize),
    Tdg(usize),
    Rx(usize, f64),
    Ry(usize, f64),
    Rz(usize, f64),
    Cx(usize, usize),
    Cz(usize, usize),
    Swap(usize, usize),
}

/// Independent gate-list reference execution engine using simulator kernels directly.
///
/// This provides a completely independent verification path that does NOT use QccIR or the DAG executor.
fn reference_gate_list_execution(
    num_qubits: usize,
    ops: &[RefOp],
    global_phase_rad: f64,
) -> StateVector {
    let mut state = StateVector::new(num_qubits).expect("reference state initialization");
    for op in ops {
        match op {
            RefOp::H(q) => state.apply_h(*q).unwrap(),
            RefOp::X(q) => state.apply_x(*q).unwrap(),
            RefOp::Y(q) => state.apply_y(*q).unwrap(),
            RefOp::Z(q) => state.apply_z(*q).unwrap(),
            RefOp::S(q) => state.apply_s(*q).unwrap(),
            RefOp::Sdg(q) => state.apply_sdg(*q).unwrap(),
            RefOp::T(q) => state.apply_t(*q).unwrap(),
            RefOp::Tdg(q) => state.apply_tdg(*q).unwrap(),
            RefOp::Rx(q, theta) => state.apply_rx(*q, Angle::from_radians(*theta)).unwrap(),
            RefOp::Ry(q, theta) => state.apply_ry(*q, Angle::from_radians(*theta)).unwrap(),
            RefOp::Rz(q, theta) => state.apply_rz(*q, Angle::from_radians(*theta)).unwrap(),
            RefOp::Cx(c, t) => state.apply_cx(*c, *t).unwrap(),
            RefOp::Cz(q0, q1) => state.apply_cz(*q0, *q1).unwrap(),
            RefOp::Swap(q0, q1) => state.apply_swap(*q0, *q1).unwrap(),
        }
    }
    if global_phase_rad.abs() > 1e-15 {
        state.apply_global_phase(global_phase_rad);
    }
    state
}

#[test]
fn test_bell_circuit_dag_execution() {
    let mut builder = CircuitBuilder::new("bell");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().unwrap();

    let (state, trace) = execute_circuit(&circuit).expect("Bell DAG execution must succeed");

    assert_eq!(state.num_qubits(), 2);
    assert_eq!(state.len(), 4);

    // Amplitudes: (|00> + |11>) / sqrt(2)
    let amp00 = state.get(0).unwrap();
    let amp01 = state.get(1).unwrap();
    let amp10 = state.get(2).unwrap();
    let amp11 = state.get(3).unwrap();

    assert!((amp00.re - FRAC_1_SQRT_2).abs() < 1e-10);
    assert!(amp00.im.abs() < 1e-10);
    assert!(amp01.norm() < 1e-10);
    assert!(amp10.norm() < 1e-10);
    assert!((amp11.re - FRAC_1_SQRT_2).abs() < 1e-10);
    assert!(amp11.im.abs() < 1e-10);

    // Verify trace
    assert_eq!(trace.circuit_name, "bell");
    assert_eq!(trace.num_qubits, 2);
    assert_eq!(trace.steps.len(), 4);
    assert_eq!(trace.measurements.len(), 2);
    assert_eq!(trace.measurements[0].qubit, 0);
    assert_eq!(trace.measurements[0].destination, 0);
    assert_eq!(trace.measurements[1].qubit, 1);
    assert_eq!(trace.measurements[1].destination, 1);
}

#[test]
fn test_dag_vs_gate_list_reference_execution() {
    let mut builder = CircuitBuilder::new("ref_test");
    let q = builder.add_quantum_register("q", 3).unwrap();

    // H(0), X(1), Ry(2, 0.6), CX(0, 1), CZ(1, 2), SWAP(0, 2), Rz(1, 0.4)
    builder.h(q[0]).unwrap();
    builder.x(q[1]).unwrap();
    builder.ry(q[2], 0.6).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cz(q[1], q[2]).unwrap();
    builder.swap(q[0], q[2]).unwrap();
    builder.rz(q[1], 0.4).unwrap();

    let circuit = builder.finish().unwrap();

    let (dag_state, _) = execute_circuit(&circuit).unwrap();

    let ref_ops = vec![
        RefOp::H(0),
        RefOp::X(1),
        RefOp::Ry(2, 0.6),
        RefOp::Cx(0, 1),
        RefOp::Cz(1, 2),
        RefOp::Swap(0, 2),
        RefOp::Rz(1, 0.4),
    ];
    let ref_state = reference_gate_list_execution(3, &ref_ops, 0.0);

    assert!(
        dag_state.approx_eq(&ref_state, 1e-10),
        "DAG state must match independent reference gate-list state"
    );
}

#[test]
fn test_all_canonical_gates_dag_execution() {
    let mut builder = CircuitBuilder::new("all_gates");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.x(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.tdg(q[0]).unwrap();
    builder.rx(q[0], 0.3).unwrap();
    builder.ry(q[0], 0.4).unwrap();
    builder.rz(q[0], 0.5).unwrap();

    builder.cx(q[0], q[1]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.swap(q[0], q[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let (dag_state, _) = execute_circuit(&circuit).unwrap();

    let ref_ops = vec![
        RefOp::X(0),
        RefOp::Y(0),
        RefOp::Z(0),
        RefOp::H(0),
        RefOp::S(0),
        RefOp::Sdg(0),
        RefOp::T(0),
        RefOp::Tdg(0),
        RefOp::Rx(0, 0.3),
        RefOp::Ry(0, 0.4),
        RefOp::Rz(0, 0.5),
        RefOp::Cx(0, 1),
        RefOp::Cz(0, 1),
        RefOp::Swap(0, 1),
    ];
    let ref_state = reference_gate_list_execution(2, &ref_ops, 0.0);

    assert!(dag_state.approx_eq(&ref_state, 1e-10));
}

#[test]
fn test_non_adjacent_qubit_operands() {
    let mut builder = CircuitBuilder::new("non_adjacent");
    let q = builder.add_quantum_register("q", 4).unwrap();

    // Entangle non-adjacent qubits q[0] and q[3]
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[3]).unwrap();
    builder.swap(q[1], q[3]).unwrap();

    let circuit = builder.finish().unwrap();
    let (dag_state, _) = execute_circuit(&circuit).unwrap();

    let ref_ops = vec![RefOp::H(0), RefOp::Cx(0, 3), RefOp::Swap(1, 3)];
    let ref_state = reference_gate_list_execution(4, &ref_ops, 0.0);

    assert!(dag_state.approx_eq(&ref_state, 1e-10));
}

#[test]
fn test_independent_operations_topological_execution() {
    let mut builder = CircuitBuilder::new("independent_ops");
    let q = builder.add_quantum_register("q", 2).unwrap();

    // H on q[0] and X on q[1] are completely independent
    builder.h(q[0]).unwrap();
    builder.x(q[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let (dag_state, _) = execute_circuit(&circuit).unwrap();

    // Evolved state is (|0> + |1>)/sqrt(2) (x) |1> = (|01> + |11>) / sqrt(2)
    // In little-endian, index 2 is |01> (q0=0, q1=1), index 3 is |11> (q0=1, q1=1)
    let amp01 = dag_state.get(2).unwrap();
    let amp11 = dag_state.get(3).unwrap();
    assert!((amp01.re - FRAC_1_SQRT_2).abs() < 1e-10);
    assert!((amp11.re - FRAC_1_SQRT_2).abs() < 1e-10);
}

#[test]
fn test_quantum_dependency_ordering_strictness() {
    // 1. H then X on q[0]
    let mut builder1 = CircuitBuilder::new("hx");
    let q1 = builder1.add_quantum_register("q", 1).unwrap();
    builder1.h(q1[0]).unwrap();
    builder1.x(q1[0]).unwrap();
    let circuit1 = builder1.finish().unwrap();
    let (state_hx, _) = execute_circuit(&circuit1).unwrap();

    // 2. X then H on q[0]
    let mut builder2 = CircuitBuilder::new("xh");
    let q2 = builder2.add_quantum_register("q", 1).unwrap();
    builder2.x(q2[0]).unwrap();
    builder2.h(q2[0]).unwrap();
    let circuit2 = builder2.finish().unwrap();
    let (state_xh, _) = execute_circuit(&circuit2).unwrap();

    // X H |0> = X (|0> + |1>)/sqrt(2) = (|1> + |0>)/sqrt(2) = (|0> + |1>)/sqrt(2)
    // H X |0> = H |1> = (|0> - |1>)/sqrt(2)
    // Amplitude for basis state 1 should be +1/sqrt(2) for HX, and -1/sqrt(2) for XH!
    let amp1_hx = state_hx.get(1).unwrap().re;
    let amp1_xh = state_xh.get(1).unwrap().re;

    assert!((amp1_hx - FRAC_1_SQRT_2).abs() < 1e-10);
    assert!((amp1_xh - (-FRAC_1_SQRT_2)).abs() < 1e-10);
    assert!(!state_hx.approx_eq(&state_xh, 1e-5));
}

#[test]
fn test_global_phase_preservation() {
    // 1. Circuit without global phase
    let mut b1 = CircuitBuilder::new("no_phase");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.x(q1[0]).unwrap();
    let c1 = b1.finish().unwrap();
    let (state1, _) = execute_circuit(&c1).unwrap();

    // 2. Circuit with global phase = pi / 2
    let mut b2 = CircuitBuilder::new("with_phase");
    let q2 = b2.add_quantum_register("q", 1).unwrap();
    b2.x(q2[0]).unwrap();
    b2.set_global_phase(PI / 2.0);
    let c2 = b2.finish().unwrap();
    let (state2, trace2) = execute_circuit(&c2).unwrap();

    assert!((trace2.global_phase() - (PI / 2.0)).abs() < 1e-10);

    // State 1 is |1> (amplitude [0.0, 1.0])
    let amp1 = state1.get(1).unwrap();
    assert!((amp1.re - 1.0).abs() < 1e-10);
    assert!(amp1.im.abs() < 1e-10);

    // State 2 has global phase e^(i * pi/2) = i
    // Amplitude is i * |1> = [0.0, i]
    let amp2 = state2.get(1).unwrap();
    assert!(amp2.re.abs() < 1e-10);
    assert!((amp2.im - 1.0).abs() < 1e-10);

    // Probabilities are identical (|i|^2 = 1.0)
    assert!((state1.probability(1).unwrap() - 1.0).abs() < 1e-10);
    assert!((state2.probability(1).unwrap() - 1.0).abs() < 1e-10);

    // But statevectors are NOT equal without phase accounting!
    assert!(!state1.approx_eq(&state2, 1e-5));
}

#[test]
fn test_symbolic_parameter_evaluation() {
    let mut builder = CircuitBuilder::new("param_circuit");
    let q = builder.add_quantum_register("q", 1).unwrap();

    builder.rx(q[0], "theta").unwrap();
    builder.set_global_phase("phi");
    let circuit = builder.finish().unwrap();

    // Fails without environment
    let err = execute_circuit(&circuit);
    assert!(
        matches!(err, Err(ExecutionError::UnevaluatedParameter { symbol }) if symbol == "theta")
    );

    // Succeeds with environment
    let mut env = HashMap::new();
    env.insert("theta".to_string(), PI);
    env.insert("phi".to_string(), 0.0);

    let executor = CircuitExecutor::new().with_environment(env);
    let (state, trace) = executor.execute(&circuit).unwrap();

    // Rx(pi)|0> = -i |1>
    let amp1 = state.get(1).unwrap();
    assert!(amp1.re.abs() < 1e-10);
    assert!((amp1.im - (-1.0)).abs() < 1e-10);
    assert_eq!(trace.steps.len(), 1);
}

#[test]
fn test_bell_measurement_and_seeded_sampling() {
    let mut builder = CircuitBuilder::new("bell_sampling");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().unwrap();

    let num_shots = 2000;
    let rng = SplitMix64::new(42);
    let (result, _) = sample_circuit(&circuit, num_shots, rng).unwrap();

    assert_eq!(result.num_shots(), num_shots);

    let counts = result.counts(ClassicalBitOrder::MsbFirst);
    let count_00 = counts.get("00").copied().unwrap_or(0);
    let count_11 = counts.get("11").copied().unwrap_or(0);
    let count_01 = counts.get("01").copied().unwrap_or(0);
    let count_10 = counts.get("10").copied().unwrap_or(0);

    // Strictly zero occurrences of non-entangled outcomes
    assert_eq!(count_01, 0, "Bell state must never produce 01");
    assert_eq!(count_10, 0, "Bell state must never produce 10");
    assert_eq!(count_00 + count_11, num_shots);

    // Statistical check: each within 4-sigma plausible range [850, 1150]
    assert!((850..=1150).contains(&count_00));
    assert!((850..=1150).contains(&count_11));

    // Reproducibility check: same seed produces identical counts
    let (result2, _) = sample_circuit(&circuit, num_shots, SplitMix64::new(42)).unwrap();
    assert_eq!(
        result.counts(ClassicalBitOrder::MsbFirst),
        result2.counts(ClassicalBitOrder::MsbFirst)
    );
}

#[test]
fn test_rejection_of_mid_circuit_measurement() {
    let mut circuit = qcc::Circuit::new("mid_circuit_meas");
    circuit.add_quantum_register("q", 1).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    // 1. Measure q0 -> c0
    circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();

    // 2. Quantum gate on q0 AFTER measurement
    circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();

    let err = execute_circuit(&circuit);
    assert!(matches!(
        err,
        Err(ExecutionError::MidCircuitMeasurement { op_id, .. }) if op_id.0 == 2
    ));
}

#[test]
fn test_rejection_of_dynamic_feedback() {
    let mut circuit = qcc::Circuit::new("dynamic_feedback");
    circuit.add_quantum_register("q", 1).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    // 1. Measure q0 -> c0
    circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();

    // 2. Classical operation reading c0
    circuit
        .add_operation(Operation::classical(
            "branch",
            vec![ClassicalBitId(0)],
            vec![],
        ))
        .unwrap();

    let err = execute_circuit(&circuit);
    assert!(matches!(
        err,
        Err(ExecutionError::DynamicFeedbackUnsupported { .. })
    ));
}

#[test]
fn test_rejection_of_unsupported_operations() {
    // 1. Reset operation rejection
    let mut circuit_reset = qcc::Circuit::new("reset_test");
    circuit_reset.add_quantum_register("q", 1).unwrap();
    circuit_reset
        .add_operation(Operation::reset(QubitId(0)))
        .unwrap();

    let err_reset = execute_circuit(&circuit_reset);
    assert!(matches!(
        err_reset,
        Err(ExecutionError::UnsupportedOperation { operation, .. }) if operation == "reset"
    ));

    // 2. Unrecognized gate rejection
    let mut circuit_bad_gate = qcc::Circuit::new("bad_gate");
    circuit_bad_gate.add_quantum_register("q", 1).unwrap();
    circuit_bad_gate
        .add_operation(Operation::custom_gate(
            "nonexistent_gate",
            vec![QubitId(0)],
            vec![],
        ))
        .unwrap();

    let err_gate = execute_circuit(&circuit_bad_gate);
    assert!(matches!(
        err_gate,
        Err(ExecutionError::UnsupportedGate { gate, .. }) if gate == "nonexistent_gate"
    ));
}

#[test]
fn test_invalid_circuit_rejected_before_execution() {
    // Zero-qubit circuit
    let empty_circuit = qcc::Circuit::new("empty");
    let err_zero = execute_circuit(&empty_circuit);
    assert_eq!(err_zero, Err(ExecutionError::ZeroQubitCircuit));
}

#[test]
fn test_barrier_is_noop_in_execution() {
    let mut builder = CircuitBuilder::new("barrier_test");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.barrier(&[q[0], q[1]]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let (state, trace) = execute_circuit(&circuit).unwrap();

    // State is Bell state
    assert!((state.get(0).unwrap().re - FRAC_1_SQRT_2).abs() < 1e-10);
    assert!((state.get(3).unwrap().re - FRAC_1_SQRT_2).abs() < 1e-10);

    // Trace records barrier
    assert_eq!(trace.steps.len(), 3);
    assert_eq!(trace.steps[1].operation_name, "barrier");
}

proptest! {
    #[test]
    fn prop_dag_matches_reference_random_rotations(
        theta_x in -PI..PI,
        theta_y in -PI..PI,
        theta_z in -PI..PI,
        phase in -PI..PI
    ) {
        let mut builder = CircuitBuilder::new("prop_circuit");
        let q = builder.add_quantum_register("q", 2).unwrap();

        builder.rx(q[0], theta_x).unwrap();
        builder.ry(q[1], theta_y).unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder.rz(q[0], theta_z).unwrap();
        builder.set_global_phase(phase);

        let circuit = builder.finish().unwrap();
        let (dag_state, _) = execute_circuit(&circuit).unwrap();

        let ref_ops = vec![
            RefOp::Rx(0, theta_x),
            RefOp::Ry(1, theta_y),
            RefOp::Cx(0, 1),
            RefOp::Rz(0, theta_z),
        ];
        let ref_state = reference_gate_list_execution(2, &ref_ops, phase);

        prop_assert!(dag_state.approx_eq(&ref_state, 1e-9));
    }
}
