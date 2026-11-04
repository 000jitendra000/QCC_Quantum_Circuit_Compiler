//! Integration tests for Phase 38: Target Cost Models and Critical Path Analysis.
//!
//! Validates:
//! 1. Single operation execution cost evaluation.
//! 2. Wire-level and DAG-based critical path depth and two-qubit depth tracking.
//! 3. Critical-path duration concurrency across disjoint physical qubits.
//! 4. The Explicit Exit Gate: Capability and cost queries pass fixtures with partial metadata.
//! 5. Multi-objective scoring with configurable weights and deterministic evaluations.

use std::collections::BTreeMap;

use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::wire::QubitId;
use qcc::target::{
    CostModel, CostWeights, DurationCost, ErrorCost, OperationConnectivity, PhysicalQubitId,
    Provenance, Target, TargetCalibration, TargetOperationDescriptor, TupleCalibration,
};

/// Constructs a calibrated 4-qubit target device for cost modeling tests.
fn create_cost_test_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut target = Target::new("cost_test_device", 4);
    target.provenance = Some(Provenance::new("device_datasheet"));

    // 1Q: SX pulse (nominal 30.0 ns, error 0.0004)
    target.add_operation(
        TargetOperationDescriptor::builder("sx", 1)
            .connectivity(OperationConnectivity::all_single_qubits(4))
            .duration(30.0)
            .error_estimate(0.0004)
            .build(),
    );

    // 1Q: RZ rotation (nominal 0.0 ns, error 0.0)
    target.add_operation(
        TargetOperationDescriptor::builder("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(4))
            .duration(0.0)
            .error_estimate(0.0)
            .build(),
    );

    // 2Q: CX directed on linear chain (0->1, 1->2, 2->3)
    // Nominal descriptor duration is None! Must rely on calibration.
    target.add_operation(
        TargetOperationDescriptor::builder("cx", 2)
            .connectivity(OperationConnectivity::directed_two_qubit(vec![
                (p0, p1),
                (p1, p2),
                (p2, p3),
            ]))
            .build(),
    );

    // Add tuple calibration only for (0, 1) and (1, 2). (2, 3) remains uncalibrated!
    let mut cal = TargetCalibration::new();
    let prov = Provenance::new("benchmarking_cal");

    cal.set_tuple(
        "cx",
        vec![p0, p1],
        TupleCalibration::new()
            .with_duration(250.0)
            .with_error(0.008)
            .with_provenance(prov.clone()),
    );

    cal.set_tuple(
        "cx",
        vec![p1, p2],
        TupleCalibration::new()
            .with_duration(320.0)
            .with_error(0.012)
            .with_provenance(prov),
    );

    target.calibration = Some(cal);
    target.validate().expect("cost test target must be valid");
    target
}

// ============================================================================
// 1. Single Operation Execution Cost
// ============================================================================

#[test]
fn test_evaluate_operation_cost() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    // Calibrated 2Q operation CX(0, 1)
    let op_cost_01 = cost_model.evaluate_operation(&target, "cx", &[p0, p1]);
    assert_eq!(op_cost_01.operation.as_str(), "cx");
    assert_eq!(op_cost_01.operands, vec![p0, p1]);
    assert_eq!(op_cost_01.quantum_arity, 2);
    assert!(op_cost_01.is_two_qubit);
    assert!(op_cost_01.duration.is_measured());
    assert_eq!(op_cost_01.duration.value(), Some(&250.0));
    assert_eq!(op_cost_01.error_estimate.value(), Some(&0.008));

    // Nominal 1Q operation SX(0)
    let op_cost_sx = cost_model.evaluate_operation(&target, "sx", &[p0]);
    assert_eq!(op_cost_sx.quantum_arity, 1);
    assert!(!op_cost_sx.is_two_qubit);
    assert!(op_cost_sx.duration.is_estimated());
    assert_eq!(op_cost_sx.duration.value(), Some(&30.0));

    // Uncalibrated 2Q operation CX(2, 3) (descriptor duration is None, no tuple calibration)
    let op_cost_23 = cost_model.evaluate_operation(&target, "cx", &[p2, p3]);
    assert!(
        op_cost_23.duration.is_absent(),
        "Uncalibrated duration must be Absent"
    );
    assert!(
        op_cost_23.error_estimate.is_absent(),
        "Uncalibrated error must be Absent"
    );
}

// ============================================================================
// 2. Physical Sequence Concurrency & Critical Path
// ============================================================================

#[test]
fn test_evaluate_empty_sequence() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let empty: Vec<(&str, [PhysicalQubitId; 0])> = Vec::new();
    let cost = cost_model.evaluate_physical_sequence(&target, empty);
    assert_eq!(cost.total_gates, 0);
    assert_eq!(cost.single_qubit_gates, 0);
    assert_eq!(cost.two_qubit_gates, 0);
    assert_eq!(cost.depth, 0);
    assert_eq!(cost.two_qubit_depth, 0);
    assert_eq!(
        cost.duration,
        DurationCost::Exact {
            total_ns: 0.0,
            is_measured_only: true,
        }
    );
    assert_eq!(
        cost.error,
        ErrorCost::Exact {
            infidelity_bound: 0.0,
            is_measured_only: true,
        }
    );
}

#[test]
fn test_parallel_concurrency_across_disjoint_qubits() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Two SX operations on disjoint qubits p0 and p1 (each 30.0 ns)
    let ops = [("sx", [p0]), ("sx", [p1])];
    let cost = cost_model.evaluate_physical_sequence(&target, ops);

    assert_eq!(cost.total_gates, 2);
    assert_eq!(cost.single_qubit_gates, 2);
    assert_eq!(cost.two_qubit_gates, 0);
    // Because p0 and p1 are disjoint, they execute concurrently in parallel:
    assert_eq!(cost.depth, 1, "Disjoint gates must have depth 1");
    assert_eq!(cost.two_qubit_depth, 0);
    assert_eq!(
        cost.duration.exact_ns(),
        Some(30.0),
        "Disjoint gates of 30.0 ns execute concurrently in max(30, 30) = 30.0 ns"
    );

    // Two SX operations on the SAME qubit p0 (30.0 + 30.0 ns)
    let seq_ops = [("sx", [p0]), ("sx", [p0])];
    let seq_cost = cost_model.evaluate_physical_sequence(&target, seq_ops);

    assert_eq!(seq_cost.total_gates, 2);
    assert_eq!(
        seq_cost.depth, 2,
        "Sequential gates on same qubit must have depth 2"
    );
    assert_eq!(
        seq_cost.duration.exact_ns(),
        Some(60.0),
        "Sequential gates on same qubit must take 60.0 ns"
    );
}

#[test]
fn test_critical_path_depth_and_two_qubit_depth() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    // CX(0, 1) [dur: 250 ns] followed by CX(1, 2) [dur: 320 ns]
    // They share qubit p1, so CX(1, 2) must execute after CX(0, 1).
    let ops = [("cx", [p0, p1]), ("cx", [p1, p2])];
    let cost = cost_model.evaluate_physical_sequence(&target, ops);

    assert_eq!(cost.total_gates, 2);
    assert_eq!(cost.two_qubit_gates, 2);
    assert_eq!(cost.depth, 2);
    assert_eq!(cost.two_qubit_depth, 2);
    assert_eq!(
        cost.duration.exact_ns(),
        Some(570.0), // 250.0 + 320.0
        "Shared wire p1 serializes 250 + 320 = 570.0 ns"
    );
    assert!(cost.duration.is_exact());
}

#[test]
fn test_interleaved_single_and_two_qubit_gates() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Sequence: SX(p0) [30 ns] -> CX(p0, p1) [250 ns] -> SX(p1) [30 ns]
    let ops = vec![("sx", vec![p0]), ("cx", vec![p0, p1]), ("sx", vec![p1])];
    let cost = cost_model.evaluate_physical_sequence(&target, ops);

    assert_eq!(cost.total_gates, 3);
    assert_eq!(cost.single_qubit_gates, 2);
    assert_eq!(cost.two_qubit_gates, 1);
    assert_eq!(cost.depth, 3);
    assert_eq!(
        cost.two_qubit_depth, 1,
        "Only one 2Q gate along the critical path"
    );
    assert_eq!(
        cost.duration.exact_ns(),
        Some(310.0), // 30 + 250 + 30
    );
}

// ============================================================================
// 3. Evaluation on QccIR Circuit DAG
// ============================================================================

#[test]
fn test_evaluate_circuit_dag() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let mut circuit = Circuit::new("test_circ");
    circuit.add_quantum_register("q", 3).unwrap();

    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    // Add gates into DAG:
    // q0: SX
    // q1: SX
    // (q0, q1): CX
    // (q1, q2): CX
    circuit
        .add_operation(Operation::Gate {
            gate_name: "sx".to_string(),
            qubits: vec![q0],
            parameters: Vec::new(),
        })
        .unwrap();

    circuit
        .add_operation(Operation::Gate {
            gate_name: "sx".to_string(),
            qubits: vec![q1],
            parameters: Vec::new(),
        })
        .unwrap();

    circuit
        .add_operation(Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![q0, q1],
            parameters: Vec::new(),
        })
        .unwrap();

    circuit
        .add_operation(Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![q1, q2],
            parameters: Vec::new(),
        })
        .unwrap();

    let cost = cost_model.evaluate_circuit(&target, &circuit);

    assert_eq!(cost.total_gates, 4);
    assert_eq!(cost.single_qubit_gates, 2);
    assert_eq!(cost.two_qubit_gates, 2);
    // Depth: max(SX_q0, SX_q1) is depth 1 -> CX(q0, q1) is depth 2 -> CX(q1, q2) is depth 3
    assert_eq!(cost.depth, 3);
    assert_eq!(cost.two_qubit_depth, 2);
    // Timing: 30.0 (parallel SX) + 250.0 (CX 01) + 320.0 (CX 12) = 600.0 ns
    assert_eq!(cost.duration.exact_ns(), Some(600.0));

    // Custom layout mapping: map logical (q0, q1, q2) -> physical (p1, p0, p2)
    let mut layout = BTreeMap::new();
    layout.insert(q0, PhysicalQubitId::new(1));
    layout.insert(q1, PhysicalQubitId::new(0));
    layout.insert(q2, PhysicalQubitId::new(2));

    let layout_cost = cost_model.evaluate_circuit_with_layout(&target, &circuit, &layout);
    assert_eq!(layout_cost.total_gates, 4);
}

// ============================================================================
// 4. Exit Gate Proof: Partial Metadata Handling
// ============================================================================

#[test]
fn test_exit_gate_partial_metadata_fixtures() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    // Case A: Complete calibration for all constituent operations
    let complete_ops = vec![("sx", vec![p0]), ("cx", vec![p0, p1])];
    let cost_complete = cost_model.evaluate_physical_sequence(&target, complete_ops);
    assert!(
        cost_complete.duration.is_exact(),
        "Fully calibrated operations must produce Exact duration"
    );
    assert_eq!(cost_complete.duration.exact_ns(), Some(280.0));

    // Case B: Partial calibration
    // Sequence contains calibrated CX(0, 1) [250 ns] and UNCALIBRATED CX(2, 3) [Absent]
    let partial_ops = vec![
        ("cx", vec![p0, p1]), // Calibrated: 250.0 ns
        ("cx", vec![p2, p3]), // UNCALIBRATED: Absent!
    ];
    let cost_partial = cost_model.evaluate_physical_sequence(&target, partial_ops);

    // Must be DurationCost::Incomplete, NOT Exact, and NOT defaulted to 0.0!
    assert!(
        cost_partial.duration.is_incomplete(),
        "Partial calibration must yield DurationCost::Incomplete"
    );
    match cost_partial.duration {
        DurationCost::Incomplete {
            accumulated_known_ns,
            uncalibrated_operations,
        } => {
            assert_eq!(accumulated_known_ns, 250.0);
            assert_eq!(
                uncalibrated_operations, 1,
                "Exactly 1 operation lacks calibration"
            );
        }
        other => panic!("expected Incomplete duration, got {other:?}"),
    }

    // Case C: Completely absent calibration
    // Sequence contains only uncalibrated CX(2, 3)
    let absent_ops = vec![("cx", vec![p2, p3])];
    let cost_absent = cost_model.evaluate_physical_sequence(&target, absent_ops);

    assert!(
        cost_absent.duration.is_unknown(),
        "Sequence with zero calibrated operations must yield DurationCost::Unknown"
    );
    assert!(
        cost_absent.error.is_unknown(),
        "Sequence with zero calibrated operations must yield ErrorCost::Unknown"
    );
}

// ============================================================================
// 5. Multi-Objective Scoring & Determinism
// ============================================================================

#[test]
fn test_multi_objective_score_calculation() {
    let target = create_cost_test_target();

    let weights = CostWeights {
        total_gates: 1.0,
        two_qubit_gates: 10.0,
        depth: 2.0,
        two_qubit_depth: 5.0,
        duration_ns: 0.1,
        error_rate: 100.0,
    };
    let cost_model = CostModel::new(weights);

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // 1 SX (30 ns, error 0.0004) + 1 CX (250 ns, error 0.008)
    // total_gates = 2
    // two_qubit_gates = 1
    // depth = 2
    // two_qubit_depth = 1
    // duration = 280.0 ns
    // error = 0.0084
    let ops = vec![("sx", vec![p0]), ("cx", vec![p0, p1])];
    let cost = cost_model.evaluate_physical_sequence(&target, ops);

    let score = cost_model.calculate_score(&cost);
    assert!(score.is_some());
    let expected = 1.0 * 2.0 + 10.0 * 1.0 + 2.0 * 2.0 + 5.0 * 1.0 + 0.1 * 280.0 + 100.0 * 0.0084;
    assert!((score.unwrap() - expected).abs() < 1e-10);

    // If duration is required (weight > 0) but Unknown, score is None
    let uncal_ops = vec![("cx", vec![PhysicalQubitId::new(2), PhysicalQubitId::new(3)])];
    let uncal_cost = cost_model.evaluate_physical_sequence(&target, uncal_ops);
    assert_eq!(cost_model.calculate_score(&uncal_cost), None);
}

#[test]
fn test_deterministic_cost_evaluation() {
    let target = create_cost_test_target();
    let cost_model = CostModel::default();

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let ops = vec![
        ("sx", vec![p0]),
        ("sx", vec![p1]),
        ("cx", vec![p0, p1]),
        ("cx", vec![p1, p2]),
    ];

    let baseline_cost = cost_model.evaluate_physical_sequence(&target, ops.clone());
    let baseline_score = cost_model.calculate_score(&baseline_cost).unwrap();

    // Verify 100 repeated evaluations produce bitwise-identical results
    for _ in 0..100 {
        let repeated_cost = cost_model.evaluate_physical_sequence(&target, ops.clone());
        assert_eq!(baseline_cost, repeated_cost);
        let repeated_score = cost_model.calculate_score(&repeated_cost).unwrap();
        assert_eq!(baseline_score.to_bits(), repeated_score.to_bits());
    }
}
