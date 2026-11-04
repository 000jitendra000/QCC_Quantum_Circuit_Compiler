//! Integration tests for Phase 37: Generic Target Data Model.
//!
//! Verifies:
//! 1. Synthetic target fixtures (CX directed, CZ undirected, ECR native, mixed multi-family).
//! 2. Operation-specific directed vs undirected connectivity and operand legality.
//! 3. Validation rejections (zero qubits, out-of-bounds qubits, self-loops, arity mismatches, invalid metrics).
//! 4. The Explicit Unknown Principle (missing metrics return `None`, never zero).
//! 5. Calibration and timing provenance survival across serialization.
//! 6. Deterministic JSON serialization and round-trip fidelity.
//! 7. Deserialization validation and rejection of invalid target descriptions.
//! 8. Complete decoupling from compiler-core logical `GateId` catalog.

use qcc::gate_signature::ParameterDomain;
use qcc::target::{
    CouplingGraph, Directionality, OperationConnectivity, PhysicalQubitId, Provenance,
    QubitCalibration, Target, TargetBuilder, TargetCalibration, TargetError, TargetOpId,
    TargetOperationDescriptor, TupleCalibration,
};

// ============================================================================
// Fixture Constructors
// ============================================================================

/// Creates a synthetic IBM-style transmon target with directed CX couplings on a 4-qubit linear chain:
/// p[0] -> p[1] -> p[2] -> p[3]
fn create_synthetic_cx_directed_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut target = Target::new("synthetic_cx_directed", 4);

    // 1Q operations: rz (parameterized Angle4Pi), sx (fixed), x (fixed)
    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0) // Virtual Z gate has 0 duration
        .error_estimate(0.0001)
        .description("Virtual Z rotation")
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(35.5) // ns
        .error_estimate(0.0005)
        .description("Square-root of X gate")
        .build();
    target.add_operation(sx_desc);

    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(35.5) // ns
        .error_estimate(0.0005)
        .description("Pauli-X gate")
        .build();
    target.add_operation(x_desc);

    // 2Q directed CX: (0, 1), (1, 2), (2, 3)
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![
            (p0, p1),
            (p1, p2),
            (p2, p3),
        ]))
        .duration(320.0) // ns
        .error_estimate(0.008)
        .description("Cross-resonance controlled-NOT gate")
        .build();
    target.add_operation(cx_desc);

    // Measurement on all single qubits
    let meas_desc = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(800.0)
        .error_estimate(0.015)
        .description("Dispersive readout")
        .build();
    target.add_operation(meas_desc);

    target.validate().expect("target must be valid");
    target
}

/// Creates a synthetic neutral-atom or silicon target with undirected CZ couplings on a 4-qubit square ring:
/// {0, 1}, {1, 2}, {2, 3}, {3, 0}
fn create_synthetic_cz_undirected_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut target = Target::new("synthetic_cz_undirected", 4);

    // 1Q operations: rx, rz
    let rx_desc = TargetOperationDescriptor::builder("rx", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(50.0)
        .error_estimate(0.0003)
        .build();
    target.add_operation(rx_desc);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0)
        .error_estimate(0.0001)
        .build();
    target.add_operation(rz_desc);

    // 2Q symmetric CZ on ring: {0, 1}, {1, 2}, {2, 3}, {3, 0}
    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(vec![
            (p0, p1),
            (p1, p2),
            (p2, p3),
            (p3, p0),
        ]))
        .duration(150.0)
        .error_estimate(0.004)
        .description("Symmetric controlled-phase gate")
        .build();
    target.add_operation(cz_desc);

    target.validate().expect("target must be valid");
    target
}

/// Creates a synthetic ECR native target (e.g. IBM Eagle native basis: rz, sx, x, ecr).
fn create_synthetic_ecr_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let mut target = Target::new("synthetic_ecr_target", 3);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(35.0)
        .error_estimate(0.0004)
        .build();
    target.add_operation(sx_desc);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .build();
    target.add_operation(rz_desc);

    // ECR directed on (0, 1) and (1, 2)
    let ecr_desc = TargetOperationDescriptor::builder("ecr", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![
            (p0, p1),
            (p1, p2),
        ]))
        .duration(280.0)
        .error_estimate(0.006)
        .description("Echoed cross-resonance gate")
        .build();
    target.add_operation(ecr_desc);

    target.validate().expect("target must be valid");
    target
}

/// Creates a synthetic mixed target supporting multiple entangling families (CX, CZ, ECR, RZZ) with distinct connectivities.
fn create_synthetic_mixed_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut target = Target::new("synthetic_mixed_multi_family", 4);

    // 1Q primitives
    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .build();
    target.add_operation(sx_desc);

    // 2Q family 1: CZ undirected on {0, 1} and {2, 3}
    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(vec![
            (p0, p1),
            (p2, p3),
        ]))
        .build();
    target.add_operation(cz_desc);

    // 2Q family 2: CX directed ONLY on (1, 2)
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![(p1, p2)]))
        .build();
    target.add_operation(cx_desc);

    // 2Q family 3: ECR directed ONLY on (2, 1) [reverse of CX!]
    let ecr_desc = TargetOperationDescriptor::builder("ecr", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![(p2, p1)]))
        .build();
    target.add_operation(ecr_desc);

    // 2Q family 4: Continuous RZZ rotation with Angle4Pi domain on {0, 3}
    let rzz_desc = TargetOperationDescriptor::builder("rzz", 2)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::undirected_two_qubit(vec![(p0, p3)]))
        .build();
    target.add_operation(rzz_desc);

    target.validate().expect("mixed target must be valid");
    target
}

// ============================================================================
// 1. Synthetic Fixture Verification
// ============================================================================

#[test]
fn test_synthetic_cx_directed_target_connectivity() {
    let target = create_synthetic_cx_directed_target();
    assert_eq!(target.num_qubits(), 4);
    assert_eq!(target.name(), "synthetic_cx_directed");
    assert!(target.is_operation_supported("cx"));
    assert!(target.is_operation_supported("rz"));
    assert!(target.is_operation_supported("sx"));
    assert!(target.is_operation_supported("measure"));

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    // Valid directed couplings
    assert!(target.is_valid_operand_tuple("cx", &[p0, p1]));
    assert!(target.is_valid_operand_tuple("cx", &[p1, p2]));
    assert!(target.is_valid_operand_tuple("cx", &[p2, p3]));

    // Reverse orientation must be REJECTED for directed CX!
    assert!(
        !target.is_valid_operand_tuple("cx", &[p1, p0]),
        "directed CX(1, 0) must be rejected when only (0, 1) is supported"
    );
    assert!(
        !target.is_valid_operand_tuple("cx", &[p2, p1]),
        "directed CX(2, 1) must be rejected when only (1, 2) is supported"
    );
    assert!(
        !target.is_valid_operand_tuple("cx", &[p3, p2]),
        "directed CX(3, 2) must be rejected when only (2, 3) is supported"
    );

    // Non-adjacent qubits must be REJECTED
    assert!(!target.is_valid_operand_tuple("cx", &[p0, p2]));
    assert!(!target.is_valid_operand_tuple("cx", &[p0, p3]));
    assert!(!target.is_valid_operand_tuple("cx", &[p1, p3]));
}

#[test]
fn test_synthetic_cz_undirected_target_connectivity() {
    let target = create_synthetic_cz_undirected_target();
    assert_eq!(target.num_qubits(), 4);
    assert!(target.operation("cz").unwrap().is_two_qubit());
    assert!(!target.operation("cz").unwrap().is_directed());

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    // Undirected ring edges permit BOTH orientations
    assert!(target.is_valid_operand_tuple("cz", &[p0, p1]));
    assert!(
        target.is_valid_operand_tuple("cz", &[p1, p0]),
        "undirected CZ must permit reverse orientation (1, 0)"
    );

    assert!(target.is_valid_operand_tuple("cz", &[p1, p2]));
    assert!(target.is_valid_operand_tuple("cz", &[p2, p1]));

    assert!(target.is_valid_operand_tuple("cz", &[p2, p3]));
    assert!(target.is_valid_operand_tuple("cz", &[p3, p2]));

    assert!(target.is_valid_operand_tuple("cz", &[p3, p0]));
    assert!(target.is_valid_operand_tuple("cz", &[p0, p3]));

    // Diagonal/non-edge pairs must be rejected
    assert!(!target.is_valid_operand_tuple("cz", &[p0, p2]));
    assert!(!target.is_valid_operand_tuple("cz", &[p2, p0]));
    assert!(!target.is_valid_operand_tuple("cz", &[p1, p3]));
    assert!(!target.is_valid_operand_tuple("cz", &[p3, p1]));
}

#[test]
fn test_synthetic_ecr_native_target() {
    let target = create_synthetic_ecr_target();
    assert_eq!(target.num_qubits(), 3);
    assert!(target.is_operation_supported("ecr"));
    assert!(!target.is_operation_supported("cx"));
    assert!(!target.is_operation_supported("cz"));

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let ecr_desc = target.operation("ecr").unwrap();
    assert_eq!(ecr_desc.quantum_arity, 2);
    assert!(ecr_desc.is_directed());
    assert_eq!(ecr_desc.parameter_count(), 0);

    assert!(target.is_valid_operand_tuple("ecr", &[p0, p1]));
    assert!(target.is_valid_operand_tuple("ecr", &[p1, p2]));
    assert!(!target.is_valid_operand_tuple("ecr", &[p1, p0]));
    assert!(!target.is_valid_operand_tuple("ecr", &[p2, p1]));
    assert!(!target.is_valid_operand_tuple("ecr", &[p0, p2]));
}

#[test]
fn test_synthetic_mixed_multi_family_target() {
    let target = create_synthetic_mixed_target();
    assert_eq!(target.num_qubits(), 4);

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    // 1. CZ on {0, 1} symmetric
    assert!(target.is_valid_operand_tuple("cz", &[p0, p1]));
    assert!(target.is_valid_operand_tuple("cz", &[p1, p0]));
    assert!(!target.is_valid_operand_tuple("cz", &[p1, p2]));

    // 2. CX directed on (1, 2)
    assert!(target.is_valid_operand_tuple("cx", &[p1, p2]));
    assert!(
        !target.is_valid_operand_tuple("cx", &[p2, p1]),
        "CX is directed only on (1, 2)"
    );

    // 3. ECR directed on (2, 1)
    assert!(target.is_valid_operand_tuple("ecr", &[p2, p1]));
    assert!(
        !target.is_valid_operand_tuple("ecr", &[p1, p2]),
        "ECR is directed only on (2, 1)"
    );

    // 4. RZZ on {0, 3} with ParameterDomain::Angle4Pi
    let rzz_desc = target.operation("rzz").unwrap();
    assert_eq!(rzz_desc.parameter_domains, vec![ParameterDomain::Angle4Pi]);
    assert!(target.is_valid_operand_tuple("rzz", &[p0, p3]));
    assert!(target.is_valid_operand_tuple("rzz", &[p3, p0]));
    assert!(!target.is_valid_operand_tuple("rzz", &[p0, p1]));
}

// ============================================================================
// 2. Physical Operand Tuple Legality & Edge Cases
// ============================================================================

#[test]
fn test_operand_tuple_arity_mismatch() {
    let target = create_synthetic_cx_directed_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    // 1Q operation passed 2 operands
    assert!(!target.is_valid_operand_tuple("sx", &[p0, p1]));
    // 1Q operation passed 0 operands
    assert!(!target.is_valid_operand_tuple("sx", &[]));

    // 2Q operation passed 1 operand
    assert!(!target.is_valid_operand_tuple("cx", &[p0]));
    // 2Q operation passed 3 operands
    assert!(!target.is_valid_operand_tuple("cx", &[p0, p1, p2]));
}

#[test]
fn test_operand_tuple_duplicate_physical_qubits_rejected() {
    let target = create_synthetic_cx_directed_target();
    let p0 = PhysicalQubitId::new(0);

    // 2Q operation on duplicate qubit (0, 0)
    assert!(
        !target.is_valid_operand_tuple("cx", &[p0, p0]),
        "operand tuple containing duplicate physical qubits must be rejected"
    );
}

#[test]
fn test_operand_tuple_out_of_bounds_physical_qubit() {
    let target = create_synthetic_cx_directed_target();
    let p0 = PhysicalQubitId::new(0);
    let p99 = PhysicalQubitId::new(99);

    assert!(!target.is_valid_operand_tuple("sx", &[p99]));
    assert!(!target.is_valid_operand_tuple("cx", &[p0, p99]));
    assert!(!target.is_valid_operand_tuple("cx", &[p99, p0]));
}

// ============================================================================
// 3. Validation Rules & Structural Invariants
// ============================================================================

#[test]
fn test_validation_rejects_zero_physical_qubits() {
    let target = Target::new("zero_qubit_target", 0);
    let err = target.validate().unwrap_err();
    assert_eq!(err, TargetError::ZeroQubits);
}

#[test]
fn test_validation_rejects_empty_operations() {
    let target = Target::new("empty_ops_target", 4);
    let err = target.validate().unwrap_err();
    assert!(matches!(err, TargetError::NoOperations(name) if name == "empty_ops_target"));
}

#[test]
fn test_validation_rejects_arity_zero() {
    let mut target = Target::new("bad_arity_target", 4);
    let bad_desc = TargetOperationDescriptor::builder("bad_op", 0).build();
    target.add_operation(bad_desc);

    let err = target.validate().unwrap_err();
    assert!(matches!(
        err,
        TargetError::InvalidArity {
            name,
            arity: 0,
            ..
        } if name == "bad_op"
    ));
}

#[test]
fn test_validation_rejects_arity_exceeding_capacity() {
    let mut target = Target::new("small_target", 2);
    // 3-qubit gate on a 2-qubit device
    let bad_desc = TargetOperationDescriptor::builder("toffoli", 3).build();
    target.add_operation(bad_desc);

    let err = target.validate().unwrap_err();
    assert!(matches!(
        err,
        TargetError::InvalidArity {
            name,
            arity: 3,
            capacity: 2,
            ..
        } if name == "toffoli"
    ));
}

#[test]
fn test_validation_rejects_out_of_bounds_coupling_reference() {
    let mut target = Target::new("bad_coupling_target", 3);
    let p0 = PhysicalQubitId::new(0);
    let p5 = PhysicalQubitId::new(5); // out of bounds for capacity 3

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![(p0, p5)]))
        .build();
    target.add_operation(cx_desc);

    let err = target.validate().unwrap_err();
    assert!(matches!(
        err,
        TargetError::QubitOutOfBounds {
            qubit,
            capacity: 3,
            ..
        } if qubit == p5
    ));
}

#[test]
fn test_validation_rejects_self_loop_coupling() {
    let mut target = Target::new("self_loop_target", 4);
    let p1 = PhysicalQubitId::new(1);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![(p1, p1)]))
        .build();
    target.add_operation(cx_desc);

    let err = target.validate().unwrap_err();
    assert!(matches!(
        err,
        TargetError::SelfLoopEdge {
            operation,
            qubit,
        } if operation == "cx" && qubit == p1
    ));
}

#[test]
fn test_validation_rejects_invalid_error_estimate() {
    let mut target = Target::new("bad_error_target", 2);
    // Error rate > 1.0
    let desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .error_estimate(1.25)
        .build();
    target.add_operation(desc);

    let err = target.validate().unwrap_err();
    assert!(matches!(err, TargetError::InvalidErrorEstimate { .. }));

    // Error rate < 0.0
    let mut target_neg = Target::new("neg_error_target", 2);
    let desc_neg = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .error_estimate(-0.01)
        .build();
    target_neg.add_operation(desc_neg);
    assert!(matches!(
        target_neg.validate().unwrap_err(),
        TargetError::InvalidErrorEstimate { .. }
    ));
}

#[test]
fn test_validation_rejects_negative_or_nan_duration() {
    let mut target = Target::new("bad_dur_target", 2);
    let desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(-10.0)
        .build();
    target.add_operation(desc);

    assert!(matches!(
        target.validate().unwrap_err(),
        TargetError::InvalidDuration { .. }
    ));

    let mut target_nan = Target::new("nan_dur_target", 2);
    let desc_nan = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(f64::NAN)
        .build();
    target_nan.add_operation(desc_nan);
    assert!(matches!(
        target_nan.validate().unwrap_err(),
        TargetError::InvalidDuration { .. }
    ));
}

#[test]
fn test_validation_rejects_calibration_referencing_unknown_operation() {
    let mut target = Target::new("cal_test_target", 2);
    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .build();
    target.add_operation(x_desc);

    let mut cal = TargetCalibration::new();
    let p0 = PhysicalQubitId::new(0);
    // Calibration for "cz", but target only declares "x"
    cal.set_tuple(
        "cz",
        vec![p0, PhysicalQubitId::new(1)],
        TupleCalibration::new().with_duration(120.0),
    );
    target.calibration = Some(cal);

    let err = target.validate().unwrap_err();
    assert!(matches!(
        err,
        TargetError::UnknownOperation { operation, .. } if operation == "cz"
    ));
}

// ============================================================================
// 4. The Explicit Unknown Principle
// ============================================================================

#[test]
fn test_missing_timing_and_calibration_remains_explicitly_unknown() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Target with NO calibration or timing metrics whatsoever
    let mut uncalibrated = Target::new("uncalibrated_target", 2);
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![(p0, p1)]))
        .build();
    uncalibrated.add_operation(cx_desc);
    uncalibrated.validate().unwrap();

    // Invariant: Duration must be None, NEVER substituted with 0.0!
    assert_eq!(
        uncalibrated.tuple_duration("cx", &[p0, p1]),
        None,
        "uncalibrated gate duration must be explicitly None, NOT 0.0"
    );

    // Invariant: Error estimate must be None, NEVER substituted with 0.0!
    assert_eq!(
        uncalibrated.tuple_error("cx", &[p0, p1]),
        None,
        "uncalibrated gate error estimate must be explicitly None, NOT 0.0"
    );

    // Invariant: Target with no QubitCalibration returns None for T1, T2
    assert!(uncalibrated.calibration.is_none());
}

#[test]
fn test_partial_calibration_returns_none_for_unmeasured_tuples() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let mut target = Target::new("partial_cal_target", 3);
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![
            (p0, p1),
            (p1, p2),
        ]))
        .build();
    target.add_operation(cx_desc);

    let mut cal = TargetCalibration::new();
    // Only calibrate (0, 1), leave (1, 2) uncalibrated
    cal.set_tuple(
        "cx",
        vec![p0, p1],
        TupleCalibration::new()
            .with_duration(250.0)
            .with_error(0.005),
    );
    target.calibration = Some(cal);
    target.validate().unwrap();

    // Calibrated tuple (0, 1) returns Some
    assert_eq!(target.tuple_duration("cx", &[p0, p1]), Some(250.0));
    assert_eq!(target.tuple_error("cx", &[p0, p1]), Some(0.005));

    // Uncalibrated tuple (1, 2) returns None (Unknown), NOT 0.0!
    assert_eq!(target.tuple_duration("cx", &[p1, p2]), None);
    assert_eq!(target.tuple_error("cx", &[p1, p2]), None);
}

// ============================================================================
// 5. Calibration & Provenance Tracking Across Serialization
// ============================================================================

#[test]
fn test_provenance_metadata_survives_round_trip() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    let prov = Provenance::new("cryogenic_fridge_snapshot_cal_v3")
        .with_timestamp("2026-11-04T05:30:00Z")
        .with_method("interleaved_randomized_benchmarking");

    let mut cal = TargetCalibration::new();
    let q0_cal = QubitCalibration {
        t1: Some(112.5), // us
        t2: Some(85.2),  // us
        readout_error: Some(0.012),
        readout_duration: Some(650.0), // ns
        frequency: Some(5.123),        // GHz
        provenance: Some(
            Provenance::new("qubit_spectroscopy").with_timestamp("2026-11-04T04:00:00Z"),
        ),
    };
    cal.set_qubit(p0, q0_cal);

    let tuple_cal = TupleCalibration::new()
        .with_duration(320.0)
        .with_error(0.0075)
        .with_provenance(prov.clone());
    cal.set_tuple("cx", vec![p0, p1], tuple_cal);

    let target = TargetBuilder::new("provenance_target", 2)
        .add_operation(
            TargetOperationDescriptor::builder("cx", 2)
                .connectivity(OperationConnectivity::directed_two_qubit(vec![(p0, p1)]))
                .build(),
        )
        .calibration(cal)
        .provenance(Provenance::new("target_manifest_v1"))
        .build()
        .unwrap();

    // Serialize to JSON
    let json_str = target.to_json().unwrap();

    // Deserialize back
    let round_trip = Target::from_json(&json_str).unwrap();

    // Verify target-level provenance
    assert_eq!(
        round_trip.provenance.as_ref().unwrap().source,
        "target_manifest_v1"
    );

    // Verify qubit-level calibration and provenance
    let rt_cal = round_trip.calibration.as_ref().unwrap();
    let rt_q0 = rt_cal.get_qubit(p0).unwrap();
    assert_eq!(rt_q0.t1, Some(112.5));
    assert_eq!(rt_q0.t2, Some(85.2));
    assert_eq!(rt_q0.readout_error, Some(0.012));
    assert_eq!(
        rt_q0.provenance.as_ref().unwrap().source,
        "qubit_spectroscopy"
    );

    // Verify tuple-level calibration and provenance
    let rt_tuple = rt_cal.get_tuple(&TargetOpId::new("cx"), &[p0, p1]).unwrap();
    assert_eq!(rt_tuple.duration, Some(320.0));
    assert_eq!(rt_tuple.error_estimate, Some(0.0075));
    let rt_prov = rt_tuple.provenance.as_ref().unwrap();
    assert_eq!(rt_prov.source, "cryogenic_fridge_snapshot_cal_v3");
    assert_eq!(rt_prov.timestamp.as_deref(), Some("2026-11-04T05:30:00Z"));
    assert_eq!(
        rt_prov.method.as_deref(),
        Some("interleaved_randomized_benchmarking")
    );
}

// ============================================================================
// 6. Deterministic Serialization & Deserialization
// ============================================================================

#[test]
fn test_deterministic_serialization_reproducibility() {
    let target = create_synthetic_mixed_target();

    let json1 = target.to_json().unwrap();
    let json2 = target.to_json().unwrap();
    assert_eq!(
        json1, json2,
        "serialization must be byte-for-byte deterministic"
    );

    let pretty1 = target.to_json_pretty().unwrap();
    let pretty2 = target.to_json_pretty().unwrap();
    assert_eq!(
        pretty1, pretty2,
        "pretty serialization must be byte-for-byte deterministic"
    );
}

#[test]
fn test_round_trip_serialization_fidelity() {
    let targets = [
        create_synthetic_cx_directed_target(),
        create_synthetic_cz_undirected_target(),
        create_synthetic_ecr_target(),
        create_synthetic_mixed_target(),
    ];

    for original in targets {
        let json = original.to_json_pretty().unwrap();
        let deserialized = Target::from_json(&json).expect("deserialization must succeed");
        assert_eq!(original, deserialized);
    }
}

#[test]
fn test_deserialization_rejects_invalid_target_json() {
    // 1. JSON with 0 qubits
    let bad_json_zero_qubits = r#"{
        "name": "bad_target",
        "num_qubits": 0,
        "operations": {}
    }"#;
    assert!(matches!(
        Target::from_json(bad_json_zero_qubits).unwrap_err(),
        TargetError::ZeroQubits
    ));

    // 2. JSON with out-of-bounds qubit reference
    let bad_json_oob = r#"{
        "name": "bad_oob",
        "num_qubits": 2,
        "operations": {
            "cx": {
                "name": "cx",
                "quantum_arity": 2,
                "connectivity": {
                    "DirectedTwoQubit": {
                        "edges": [[0, 5]]
                    }
                }
            }
        }
    }"#;
    assert!(matches!(
        Target::from_json(bad_json_oob).unwrap_err(),
        TargetError::QubitOutOfBounds { .. }
    ));

    // 3. JSON with self-loop edge
    let bad_json_self_loop = r#"{
        "name": "bad_self_loop",
        "num_qubits": 3,
        "operations": {
            "cx": {
                "name": "cx",
                "quantum_arity": 2,
                "connectivity": {
                    "DirectedTwoQubit": {
                        "edges": [[1, 1]]
                    }
                }
            }
        }
    }"#;
    assert!(matches!(
        Target::from_json(bad_json_self_loop).unwrap_err(),
        TargetError::SelfLoopEdge { .. }
    ));

    // 4. JSON with invalid error rate > 1.0
    let bad_json_error = r#"{
        "name": "bad_err",
        "num_qubits": 2,
        "operations": {
            "x": {
                "name": "x",
                "quantum_arity": 1,
                "error_estimate": 1.5,
                "connectivity": {
                    "SingleQubit": {
                        "qubits": [0]
                    }
                }
            }
        }
    }"#;
    assert!(matches!(
        Target::from_json(bad_json_error).unwrap_err(),
        TargetError::InvalidErrorEstimate { .. }
    ));
}

// ============================================================================
// 7. Coupling Graph Topologies & Algorithms
// ============================================================================

#[test]
fn test_coupling_graph_linear_chain() {
    let graph = CouplingGraph::linear_chain(4, Directionality::Undirected);
    assert_eq!(graph.num_qubits, 4);
    assert!(graph.is_connected());

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    assert_eq!(graph.degree(p0), 1);
    assert_eq!(graph.degree(p1), 2);
    assert_eq!(graph.degree(p2), 2);
    assert_eq!(graph.degree(p3), 1);

    assert_eq!(graph.neighbors(p0), vec![p1]);
    assert_eq!(graph.neighbors(p1), vec![p0, p2]);
}

#[test]
fn test_coupling_graph_grid_2d() {
    let graph = CouplingGraph::grid_2d(2, 2, Directionality::Undirected);
    assert_eq!(graph.num_qubits, 4);
    assert!(graph.is_connected());

    let p0 = PhysicalQubitId::new(0); // (0, 0)
    let p1 = PhysicalQubitId::new(1); // (0, 1)
    let p2 = PhysicalQubitId::new(2); // (1, 0)
    let p3 = PhysicalQubitId::new(3); // (1, 1)

    // Corner qubits in 2x2 grid have degree 2
    assert_eq!(graph.degree(p0), 2);
    assert_eq!(graph.degree(p1), 2);
    assert_eq!(graph.degree(p2), 2);
    assert_eq!(graph.degree(p3), 2);

    assert!(graph.has_edge(p0, p1));
    assert!(graph.has_edge(p0, p2));
    assert!(!graph.has_edge(p0, p3)); // diagonal
}

#[test]
fn test_target_combined_coupling_graph() {
    let target = create_synthetic_cx_directed_target();
    let graph = target.combined_coupling_graph();
    assert_eq!(graph.num_qubits, 4);

    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    assert!(graph.has_edge(p0, p1));
    assert!(graph.has_edge(p1, p2));
    assert!(graph.has_edge(p2, p3));
    assert!(!graph.has_edge(p1, p0)); // Directed!
}

// ============================================================================
// 8. Decoupling from Compiler-Core Logical GateId Catalog
// ============================================================================

#[test]
fn test_target_native_primitives_require_zero_logical_catalog_changes() {
    // Describe exotic / vendor-native operations not in GateId:
    // ecr, fsim, rzz, sx, sy, cz_pulse
    let mut exotic_target = Target::new("exotic_vendor_chip", 2);

    let fsim_desc = TargetOperationDescriptor::builder("fsim", 2)
        .add_parameter_domain(ParameterDomain::Angle4Pi) // theta
        .add_parameter_domain(ParameterDomain::Angle2Pi) // phi
        .connectivity(OperationConnectivity::all_to_all())
        .description("Google Sycamore native fermionic simulation gate")
        .build();
    exotic_target.add_operation(fsim_desc);

    let rzz_desc = TargetOperationDescriptor::builder("rzz", 2)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_to_all())
        .build();
    exotic_target.add_operation(rzz_desc);

    assert!(exotic_target.validate().is_ok());
    assert!(exotic_target.is_operation_supported("fsim"));
    assert!(exotic_target.is_operation_supported("rzz"));

    // Invariant: Core logical GateCatalog has exactly 16 universal gates
    assert_eq!(
        qcc::gate_catalog::GateCatalog::all_gates().len(),
        16,
        "logical gate catalog must remain untouched and strictly target-independent"
    );
    assert!(
        qcc::gate_catalog::GateCatalog::lookup("fsim").is_none(),
        "fsim is a target-native instruction, not in universal logical catalog"
    );
    assert!(
        qcc::gate_catalog::GateCatalog::lookup("rzz").is_none(),
        "rzz is a target-native instruction, not in universal logical catalog"
    );
}
