//! Integration tests for Phase 43: Final Target Legality Validator.

use qcc::builder::CircuitBuilder;
use qcc::diagnostics::TargetDiagnosticCode;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::opt::pass_manager::PassManager;
use qcc::target::legality::{TargetLegalityError, TargetLegalityPass, TargetLegalityValidator};
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::target::OperationConnectivity;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper building a 3-qubit test target with native RZ, SX, and directed CX along (1, 0) and (1, 2).
fn make_test_target() -> Target {
    let mut target = Target::new("test_target_3q", 3);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(0.0)
        .error_estimate(0.0001)
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(sx_desc);

    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(x_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([
            (q(1), q(0)),
            (q(1), q(2)),
        ]))
        .duration(200.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

/// Helper building a target where an operation has NO timing or calibration metadata.
fn make_target_missing_timing_metadata() -> Target {
    let mut target = Target::new("uncalibrated_target", 2);

    // Bare descriptor with NO duration and NO error estimate
    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .build();
    target.add_operation(x_desc);

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Valid Target Circuit
// ============================================================================

#[test]
fn test_valid_circuit_passes_validation() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("valid_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.rz(q[0], 0.5).unwrap();
    builder.custom_gate("sx", &[q[1]], vec![]).unwrap();
    // CX(1, 0) and CX(1, 2) are both valid directed edges on target
    builder.cx(q[1], q[0]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let report = validator.validate_circuit(&circuit).unwrap();
    assert!(report.is_valid());
    assert_eq!(report.operations_examined, 4);
    assert_eq!(report.violation_count(), 0);
    assert!(validator.is_circuit_legal(&circuit));
}

// ============================================================================
// 2. Unsupported Operation
// ============================================================================

#[test]
fn test_unsupported_operation_detected_with_diagnostics() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("unsupported_op_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.rz(q[0], 1.0).unwrap();
    // 'toffoli' / 'ccx' is not in target basis
    builder
        .custom_gate("toffoli", &[q[0], q[1], q[2]], vec![])
        .unwrap();
    let circuit = builder.finish().unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed {
            violation_count,
            violations,
            ..
        } => {
            assert_eq!(violation_count, 1);
            let v = &violations[0];
            assert_eq!(v.operation_name, "toffoli");
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::UnsupportedOperation.as_str())
            );
            assert!(v.diagnostic.message.contains("toffoli"));
            assert!(v
                .diagnostic
                .notes
                .iter()
                .any(|n| n.contains("supports operations")));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 3. Arity Mismatch
// ============================================================================

#[test]
fn test_arity_mismatch_detected() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    let mut circuit = qcc::ir::circuit::Circuit::new("arity_mismatch_circuit");
    circuit.add_quantum_register("q", 3).unwrap();
    // 'x' gate is 1-qubit, but given 2 operands
    let q0 = qcc::ir::wire::QubitId(0);
    let q1 = qcc::ir::wire::QubitId(1);
    circuit
        .add_operation(Operation::custom_gate("x", vec![q0, q1], vec![]))
        .unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(v.operation_name, "x");
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::ArityMismatch.as_str())
            );
            assert!(v
                .diagnostic
                .message
                .contains("expected 1 physical operands, got 2"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 4. Physical Qubit Range / Out of Bounds
// ============================================================================

#[test]
fn test_qubit_out_of_bounds_detected() {
    let target = make_test_target(); // capacity = 3 (valid indices: 0..=2)
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("out_of_bounds_circuit");
    let q = builder.add_quantum_register("q", 5).unwrap();
    // Qubit 4 exceeds physical capacity of 3
    builder.rz(q[4], 0.25).unwrap();
    let circuit = builder.finish().unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::QubitOutOfBounds.as_str())
            );
            assert!(v
                .diagnostic
                .message
                .contains("physical qubit p[4] is out of bounds"));
            assert!(v.diagnostic.notes[0].contains("capacity is 3 qubits"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 5. Duplicate Qubit Operands
// ============================================================================

#[test]
fn test_duplicate_qubit_operand_detected() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    // Test direct node validation for duplicate physical operands
    let node = qcc::ir::node::NodeData::new(
        qcc::ir::node::OperationId(42),
        1,
        Operation::custom_gate(
            "cx",
            vec![qcc::ir::wire::QubitId(1), qcc::ir::wire::QubitId(1)],
            vec![],
        ),
    );

    let violation = validator.validate_node(&node).unwrap_err();
    assert_eq!(
        violation.diagnostic.code.as_deref(),
        Some(TargetDiagnosticCode::DuplicateOperandQubit.as_str())
    );
    assert!(violation
        .diagnostic
        .message
        .contains("duplicate physical qubit p[1]"));
}

// ============================================================================
// 6. Illegal Coupling & Directionality
// ============================================================================

#[test]
fn test_illegal_direction_detected_with_reversal_hint() {
    let target = make_test_target(); // CX only allowed along (1, 0) and (1, 2)
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("wrong_direction_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    // CX(0, 1) is illegal (only (1, 0) is permitted)
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::IllegalCoupling.as_str())
            );
            // Note should explicitly indicate that reverse edge (1, 0) is available on target
            assert!(v
                .diagnostic
                .notes
                .iter()
                .any(|n| n.contains("direction correction or reversing operands is required")));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

#[test]
fn test_unconnected_qubit_pair_detected() {
    let target = make_test_target(); // (0, 2) has no coupling in either direction
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("unconnected_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::IllegalCoupling.as_str())
            );
            assert!(v
                .diagnostic
                .notes
                .iter()
                .any(|n| n.contains("no coupling edge exists")));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 7. Parameter Count & Domain Violations
// ============================================================================

#[test]
fn test_parameter_count_mismatch_detected() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    let mut circuit = qcc::ir::circuit::Circuit::new("param_count_circuit");
    circuit.add_quantum_register("q", 3).unwrap();
    // 'rz' expects 1 parameter, but 0 are supplied
    let q0 = qcc::ir::wire::QubitId(0);
    circuit
        .add_operation(Operation::custom_gate("rz", vec![q0], vec![]))
        .unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::ParameterCountMismatch.as_str())
            );
            assert!(v.diagnostic.message.contains("expected 1, got 0"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

#[test]
fn test_non_finite_parameter_detected() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("nan_param_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder
        .custom_gate("rz", &[q[0]], vec![ParameterExpr::constant(f64::NAN)])
        .unwrap();
    let circuit = builder.finish().unwrap();

    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations.len(), 1);
            let v = &violations[0];
            assert_eq!(
                v.diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::NonFiniteParameter.as_str())
            );
            assert!(v.diagnostic.message.contains("non-finite parameter"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 8. Missing Timing or Calibration Metadata is Legal
// ============================================================================

#[test]
fn test_missing_timing_metadata_remains_legal() {
    let target = make_target_missing_timing_metadata();
    let validator = TargetLegalityValidator::new(target);

    let mut builder = CircuitBuilder::new("uncalibrated_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.custom_gate("x", &[q[0]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    // Must be completely legal despite having None for duration and error
    let report = validator.validate_circuit(&circuit).unwrap();
    assert!(report.is_valid());
    assert_eq!(report.violation_count(), 0);
}

// ============================================================================
// 9. Revalidation After Circuit Rewrite
// ============================================================================

#[test]
fn test_revalidation_after_circuit_rewrite() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target);

    // Initial valid circuit
    let mut builder = CircuitBuilder::new("rewrite_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.rz(q[1], 0.5).unwrap();
    builder.cx(q[1], q[0]).unwrap(); // valid direction (1, 0)
    let mut circuit = builder.finish().unwrap();

    // 1. Initial validation passes
    assert!(validator.validate_circuit(&circuit).is_ok());

    // 2. Perform a DAG rewrite: replace valid CX(1, 0) with illegal CX(0, 1)
    let nodes = circuit.graph.ordered_nodes().unwrap();
    let cx_node = nodes.iter().find(|n| n.operation.name() == "cx").unwrap();

    let illegal_op = Operation::custom_gate("cx", vec![q[0].qubit_id(), q[1].qubit_id()], vec![]);
    qcc::ir::rewrite::replace_operation(&mut circuit, cx_node.id, illegal_op).unwrap();

    // 3. Revalidation after rewrite catches the violation immediately!
    let err = validator.validate_circuit(&circuit).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed {
            violation_count,
            violations,
            ..
        } => {
            assert_eq!(violation_count, 1);
            assert_eq!(violations[0].operation_name, "cx");
            assert_eq!(
                violations[0].diagnostic.code.as_deref(),
                Some(TargetDiagnosticCode::IllegalCoupling.as_str())
            );
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }
}

// ============================================================================
// 10. PassManager Integration
// ============================================================================

#[test]
fn test_target_legality_pass_in_pass_manager() {
    let target = make_test_target();
    let pass = TargetLegalityPass::new(target.clone());

    let mut pm = PassManager::new();
    pm.add_pass(pass);

    // 1. Valid circuit succeeds in pipeline
    let mut builder = CircuitBuilder::new("pm_valid");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.rz(q[0], 0.1).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let mut valid_circuit = builder.finish().unwrap();

    let res = pm.run(&mut valid_circuit);
    assert!(res.is_ok());

    // 2. Invalid circuit fails pipeline with descriptive error
    let mut builder2 = CircuitBuilder::new("pm_invalid");
    let q2 = builder2.add_quantum_register("q", 3).unwrap();
    builder2
        .custom_gate("unsupported_gate", &[q2[0]], vec![])
        .unwrap();
    let mut invalid_circuit = builder2.finish().unwrap();

    let err = pm.run(&mut invalid_circuit).unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("unsupported_gate"));
    assert!(err_str.contains("T0001"));
}

// ============================================================================
// 11. Direction Correction & Basis Translation Integration
// ============================================================================

#[test]
fn test_legality_validator_with_direction_correction() {
    let target = make_test_target(); // only CX(1, 0) and CX(1, 2) allowed
    let validator = TargetLegalityValidator::new(target.clone());

    // Circuit has CX(0, 1) which has illegal direction
    let mut builder = CircuitBuilder::new("dir_legality_test");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let original = builder.finish().unwrap();

    // 1. Validator rejects before direction correction
    let err = validator.validate_circuit(&original).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations[0].diagnostic.code.as_deref(), Some("T0005"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }

    // 2. Apply direction correction
    let corrector = qcc::translation::direction::TwoQubitDirectionCorrector::new(target);
    let (corrected, _) = corrector.correct_circuit(&original).unwrap();

    // 3. Validator accepts corrected circuit
    let report = validator.validate_circuit(&corrected).unwrap();
    assert!(report.is_valid());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn test_legality_validator_with_basis_translation() {
    let target = make_test_target();
    let validator = TargetLegalityValidator::new(target.clone());

    // Circuit has SWAP, which is not in target basis
    let mut builder = CircuitBuilder::new("basis_legality_test");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.swap(q[1], q[0]).unwrap();
    let original = builder.finish().unwrap();

    // 1. Validator rejects un-translated SWAP
    let err = validator.validate_circuit(&original).unwrap_err();
    match err {
        TargetLegalityError::ValidationFailed { violations, .. } => {
            assert_eq!(violations[0].diagnostic.code.as_deref(), Some("T0001"));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }

    // 2. Translate through BasisTranslationEngine
    let engine = qcc::translation::engine::BasisTranslationEngine::new(target);
    let (translated, _) = engine.translate_circuit(&original).unwrap();

    // 3. Validator accepts translated circuit
    let report = validator.validate_circuit(&translated).unwrap();
    assert!(report.is_valid());
    assert_eq!(report.violation_count(), 0);
}
