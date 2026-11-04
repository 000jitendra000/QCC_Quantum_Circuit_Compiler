//! Integration tests for Phase 38: Target Capability Queries and Legality Validation.
//!
//! Validates:
//! 1. Native operation support and logical `GateId` bridging.
//! 2. Operand tuple legality, directionality enforcement, arity, and duplicate qubit detection.
//! 3. Continuous parameter legality, count checks, and non-finite value rejection.
//! 4. Tri-state metric lookup (Measured vs Estimated vs Absent) enforcing the Explicit Unknown Principle.

use qcc::gate_signature::{GateId, ParameterDomain};
use qcc::target::{
    LegalityError, OperationConnectivity, PhysicalQubitId, Provenance, QubitCalibration, Target,
    TargetCalibration, TargetOperationDescriptor, TupleCalibration,
};

/// Constructs a synthetic 4-qubit target device with directed CX, undirected CZ, and single-qubit gates.
fn create_test_target() -> Target {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut target = Target::new("synthetic_target_p38", 4);
    target.provenance = Some(Provenance::new("target_data_sheet_v1"));

    // 1Q: RZ rotation with Angle4Pi domain, nominal duration 0.0 (virtual Z), nominal error 0.0
    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0)
        .error_estimate(0.0)
        .build();
    target.add_operation(rz_desc);

    // 1Q: SX pulse with 0 parameters, nominal duration 35.0 ns, nominal error 0.0005
    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(35.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(sx_desc);

    // 1Q: X pulse with 0 parameters, nominal duration 35.0 ns, nominal error 0.0005
    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(35.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(x_desc);

    // 2Q: CX directed on linear chain (0->1, 1->2, 2->3)
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![
            (p0, p1),
            (p1, p2),
            (p2, p3),
        ]))
        .duration(300.0) // nominal descriptor duration
        .error_estimate(0.01) // nominal descriptor error
        .build();
    target.add_operation(cx_desc);

    // 2Q: CZ undirected on (0, 2)
    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(vec![(p0, p2)]))
        .duration(200.0)
        .error_estimate(0.008)
        .build();
    target.add_operation(cz_desc);

    // Measure operation on all qubits
    let meas_desc = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(800.0)
        .error_estimate(0.02)
        .build();
    target.add_operation(meas_desc);

    // Setup partial calibration
    let mut cal = TargetCalibration::new();

    // Empirically measured CX(0, 1) calibration
    let cx_01_prov =
        Provenance::new("randomized_benchmarking").with_timestamp("2026-11-04T06:00:00Z");
    let cx_01_cal = TupleCalibration::new()
        .with_duration(280.0)
        .with_error(0.0065)
        .with_provenance(cx_01_prov);
    cal.set_tuple("cx", vec![p0, p1], cx_01_cal);

    // Empirically measured Qubit 0 physical metrics
    let q0_prov = Provenance::new("spectroscopy").with_timestamp("2026-11-04T05:30:00Z");
    let mut q0_cal = QubitCalibration::new();
    q0_cal.t1 = Some(120.5); // microseconds
    q0_cal.t2 = Some(95.2); // microseconds
    q0_cal.readout_error = Some(0.015);
    q0_cal.readout_duration = Some(750.0); // nanoseconds
    q0_cal.frequency = Some(5.125); // GHz
    q0_cal.provenance = Some(q0_prov);
    cal.set_qubit(p0, q0_cal);

    // Qubit 1 has partial calibration (only T1, no T2) without tuple-level provenance (inherits target provenance)
    let mut q1_cal = QubitCalibration::new();
    q1_cal.t1 = Some(110.0);
    cal.set_qubit(p1, q1_cal);

    target.calibration = Some(cal);
    target.validate().expect("test target must be valid");
    target
}

// ============================================================================
// 1. Operation Support Queries
// ============================================================================

#[test]
fn test_supports_operation_native_and_unknown() {
    let target = create_test_target();

    assert!(target.supports_operation("cx"));
    assert!(target.supports_operation("cz"));
    assert!(target.supports_operation("sx"));
    assert!(target.supports_operation("rz"));
    assert!(target.supports_operation("x"));
    assert!(target.supports_operation("measure"));

    // Unsupported operations
    assert!(!target.supports_operation("ecr"));
    assert!(!target.supports_operation("iswap"));
    assert!(!target.supports_operation("fsim"));
    assert!(!target.supports_operation("nonexistent_gate"));
}

#[test]
fn test_supports_logical_gate_bridging() {
    let target = create_test_target();

    // Logical gates that are supported natively
    assert!(target.supports_logical_gate(GateId::CX));
    assert!(target.supports_logical_gate(GateId::CZ));
    assert!(target.supports_logical_gate(GateId::X));
    assert!(target.supports_logical_gate(GateId::Rz));

    // Logical gates not natively supported on this target (require decomposition)
    assert!(!target.supports_logical_gate(GateId::H));
    assert!(!target.supports_logical_gate(GateId::Y));
    assert!(!target.supports_logical_gate(GateId::Z));
    assert!(!target.supports_logical_gate(GateId::S));
    assert!(!target.supports_logical_gate(GateId::T));
    assert!(!target.supports_logical_gate(GateId::SWAP));
    assert!(!target.supports_logical_gate(GateId::Rx));
    assert!(!target.supports_logical_gate(GateId::Ry));
}

// ============================================================================
// 2. Operand Tuple Legality & Directionality
// ============================================================================

#[test]
fn test_check_operand_legality_directed_vs_undirected() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    // CX directed: (0, 1) is valid
    assert!(target.check_operand_legality("cx", &[p0, p1]).is_ok());

    // CX directed: (1, 0) is ILLEGAL (asymmetric directionality)
    let err = target.check_operand_legality("cx", &[p1, p0]).unwrap_err();
    assert_eq!(
        err,
        LegalityError::IllegalCoupling {
            operation: "cx".to_string(),
            operands: vec![p1, p0],
        }
    );

    // CZ undirected: both (0, 2) and (2, 0) are valid
    assert!(target.check_operand_legality("cz", &[p0, p2]).is_ok());
    assert!(target.check_operand_legality("cz", &[p2, p0]).is_ok());

    // CZ uncoupled pair: (0, 1) is not an edge in the CZ coupling graph
    let err_cz = target.check_operand_legality("cz", &[p0, p1]).unwrap_err();
    assert!(matches!(err_cz, LegalityError::IllegalCoupling { .. }));
}

#[test]
fn test_check_operand_legality_arity_and_duplicates() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Arity mismatch: 2Q op passed 1 operand
    let err_arity_1 = target.check_operand_legality("cx", &[p0]).unwrap_err();
    assert_eq!(
        err_arity_1,
        LegalityError::ArityMismatch {
            operation: "cx".to_string(),
            expected: 2,
            actual: 1,
        }
    );

    // Arity mismatch: 1Q op passed 2 operands
    let err_arity_2 = target.check_operand_legality("sx", &[p0, p1]).unwrap_err();
    assert_eq!(
        err_arity_2,
        LegalityError::ArityMismatch {
            operation: "sx".to_string(),
            expected: 1,
            actual: 2,
        }
    );

    // Duplicate physical qubit in operand tuple: (0, 0)
    let err_dup = target.check_operand_legality("cx", &[p0, p0]).unwrap_err();
    assert_eq!(
        err_dup,
        LegalityError::DuplicateOperandQubit {
            qubit: p0,
            operands: vec![p0, p0],
        }
    );

    // Qubit out of bounds: p[99] exceeds capacity 4
    let p99 = PhysicalQubitId::new(99);
    let err_oob = target.check_operand_legality("cx", &[p0, p99]).unwrap_err();
    assert_eq!(
        err_oob,
        LegalityError::QubitOutOfBounds {
            qubit: p99,
            capacity: 4,
        }
    );

    // Unsupported operation
    let err_unsupported = target
        .check_operand_legality("magic_op", &[p0, p1])
        .unwrap_err();
    assert_eq!(
        err_unsupported,
        LegalityError::UnsupportedOperation("magic_op".to_string())
    );
}

// ============================================================================
// 3. Parameter Legality & Domain Constraints
// ============================================================================

#[test]
fn test_check_parameter_legality() {
    let target = create_test_target();

    // RZ expects 1 parameter (Angle4Pi)
    assert!(target
        .check_parameter_legality("rz", &[std::f64::consts::PI])
        .is_ok());
    assert!(target.check_parameter_legality("rz", &[0.0]).is_ok());
    assert!(target.check_parameter_legality("rz", &[-10.5]).is_ok());

    // Parameter count mismatch: RZ with 0 parameters
    let err_cnt_0 = target.check_parameter_legality("rz", &[]).unwrap_err();
    assert_eq!(
        err_cnt_0,
        LegalityError::ParameterCountMismatch {
            operation: "rz".to_string(),
            expected: 1,
            actual: 0,
        }
    );

    // Parameter count mismatch: SX with 1 parameter (expected 0)
    let err_cnt_sx = target.check_parameter_legality("sx", &[1.0]).unwrap_err();
    assert_eq!(
        err_cnt_sx,
        LegalityError::ParameterCountMismatch {
            operation: "sx".to_string(),
            expected: 0,
            actual: 1,
        }
    );

    // Non-finite parameter: NaN
    let err_nan = target
        .check_parameter_legality("rz", &[f64::NAN])
        .unwrap_err();
    assert!(matches!(
        err_nan,
        LegalityError::NonFiniteParameter { index: 0, .. }
    ));

    // Non-finite parameter: Infinity
    let err_inf = target
        .check_parameter_legality("rz", &[f64::INFINITY])
        .unwrap_err();
    assert!(matches!(
        err_inf,
        LegalityError::NonFiniteParameter { index: 0, .. }
    ));
}

#[test]
fn test_is_legal_convenience_predicate() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Valid directed CX
    assert!(target.is_legal("cx", &[p0, p1], &[]));

    // Reverse directed CX -> illegal
    assert!(!target.is_legal("cx", &[p1, p0], &[]));

    // Valid parameterized RZ
    assert!(target.is_legal("rz", &[p0], &[1.57]));

    // RZ with wrong param count -> illegal
    assert!(!target.is_legal("rz", &[p0], &[]));

    // RZ with NaN -> illegal
    assert!(!target.is_legal("rz", &[p0], &[f64::NAN]));
}

// ============================================================================
// 4. Tri-State Metric Lookup (Explicit Unknown Principle)
// ============================================================================

#[test]
fn test_query_duration_measured_estimated_absent() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    // 1. CX(0, 1) has explicit empirical calibration with provenance -> Measured
    let dur_01 = target.query_duration("cx", &[p0, p1]);
    assert!(dur_01.is_measured());
    assert_eq!(dur_01.value(), Some(&280.0));
    let prov = dur_01.provenance().unwrap();
    assert_eq!(prov.source, "randomized_benchmarking");
    assert_eq!(prov.timestamp.as_deref(), Some("2026-11-04T06:00:00Z"));

    // 2. CX(1, 2) has NO tuple-specific calibration, falls back to descriptor nominal duration (300.0) -> Estimated
    let dur_12 = target.query_duration("cx", &[p1, p2]);
    assert!(dur_12.is_estimated());
    assert_eq!(dur_12.value(), Some(&300.0));
    assert_eq!(dur_12.provenance().unwrap().source, "target_data_sheet_v1");

    // 3. Target with no duration descriptor nor calibration -> Absent (never 0.0)
    let mut uncal_target = Target::new("uncalibrated", 2);
    let no_dur_desc = TargetOperationDescriptor::builder("dummy", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .build();
    uncal_target.add_operation(no_dur_desc);

    let dur_absent = uncal_target.query_duration("dummy", &[p0]);
    assert!(dur_absent.is_absent());
    assert_eq!(dur_absent.value(), None);

    // Unsupported op query -> Absent
    assert!(target.query_duration("magic_op", &[p0]).is_absent());
}

#[test]
fn test_query_error_measured_estimated_absent() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    // 1. CX(0, 1) calibrated error -> Measured
    let err_01 = target.query_error("cx", &[p0, p1]);
    assert!(err_01.is_measured());
    assert_eq!(err_01.value(), Some(&0.0065));

    // 2. CX(1, 2) nominal error from descriptor -> Estimated
    let err_12 = target.query_error("cx", &[p1, p2]);
    assert!(err_12.is_estimated());
    assert_eq!(err_12.value(), Some(&0.01));

    // 3. Uncalibrated operation -> Absent
    let mut uncal = Target::new("uncal", 2);
    uncal.add_operation(
        TargetOperationDescriptor::builder("op", 1)
            .connectivity(OperationConnectivity::all_single_qubits(2))
            .build(),
    );
    assert!(uncal.query_error("op", &[p0]).is_absent());
}

#[test]
fn test_query_qubit_level_metrics() {
    let target = create_test_target();
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p99 = PhysicalQubitId::new(99);

    // Qubit 0: fully calibrated with provenance
    let t1_q0 = target.query_qubit_t1(p0);
    assert!(t1_q0.is_measured());
    assert_eq!(t1_q0.value(), Some(&120.5));

    let t2_q0 = target.query_qubit_t2(p0);
    assert!(t2_q0.is_measured());
    assert_eq!(t2_q0.value(), Some(&95.2));

    let ro_err_q0 = target.query_readout_error(p0);
    assert!(ro_err_q0.is_measured());
    assert_eq!(ro_err_q0.value(), Some(&0.015));

    let ro_dur_q0 = target.query_readout_duration(p0);
    assert!(ro_dur_q0.is_measured());
    assert_eq!(ro_dur_q0.value(), Some(&750.0));

    let freq_q0 = target.query_qubit_frequency(p0);
    assert!(freq_q0.is_measured());
    assert_eq!(freq_q0.value(), Some(&5.125));

    // Qubit 1: T1 calibrated with target provenance (Measured), T2 absent
    let t1_q1 = target.query_qubit_t1(p1);
    assert!(t1_q1.is_measured());
    assert_eq!(t1_q1.value(), Some(&110.0));
    assert_eq!(t1_q1.provenance().unwrap().source, "target_data_sheet_v1");

    let t2_q1 = target.query_qubit_t2(p1);
    assert!(
        t2_q1.is_absent(),
        "T2 was not provided for qubit 1 and must be Absent"
    );

    // Qubit 2: no calibration record at all -> all Absent
    assert!(target.query_qubit_t1(p2).is_absent());
    assert!(target.query_qubit_t2(p2).is_absent());
    assert!(target.query_readout_error(p2).is_absent());

    // Out-of-bounds qubit -> Absent
    assert!(target.query_qubit_t1(p99).is_absent());
}
