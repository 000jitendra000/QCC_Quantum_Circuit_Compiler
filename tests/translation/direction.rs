//! Integration tests for Phase 42: Two-Qubit Direction Correction.

use std::sync::Arc;

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::operation::Operation;
use qcc::opt::pass_manager::PassManager;
use qcc::target::cost::CostModel;
use qcc::target::direction::{DirectionError, DirectionStatus};
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::target::OperationConnectivity;
use qcc::translation::direction::{
    verify_cx_reversal_matrix_c0_t1, verify_cx_reversal_matrix_c1_t0, DirectionCorrectionRegistry,
    TargetDirectionRule, TwoQubitDirectionCorrector, TwoQubitDirectionPass,
};
use qcc::translation::engine::BasisTranslationEngine;
use qcc::translation::rules::DecompositionCandidate;
use qcc::translation::TranslationError;
use qcc::verification::check_equivalence;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper building a target with only directed edge (1, 0) for CX, with native H and CX.
fn make_target_cx_one_way_1_to_0() -> Target {
    let mut target = Target::new("target_cx_1_to_0", 3);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(20.0)
        .error_estimate(0.0002)
        .build();
    target.add_operation(h_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([(q(1), q(0))]))
        .duration(200.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

/// Helper building a target with only directed edge (0, 1) for CX, with native H and CX.
fn make_target_cx_one_way_0_to_1() -> Target {
    let mut target = Target::new("target_cx_0_to_1", 3);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .duration(20.0)
        .error_estimate(0.0002)
        .build();
    target.add_operation(h_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([(q(0), q(1))]))
        .duration(200.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

/// Helper building a target with undirected CZ connectivity.
fn make_target_cz_undirected() -> Target {
    let mut target = Target::new("target_cz_undirected", 3);

    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit([(q(0), q(1))]))
        .duration(150.0)
        .error_estimate(0.003)
        .build();
    target.add_operation(cz_desc);

    target.validate().unwrap();
    target
}

/// Helper building a target with directed CZ connectivity (permits only (1, 0)).
fn make_target_cz_directed_1_to_0() -> Target {
    let mut target = Target::new("target_cz_directed_1_to_0", 3);

    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([(q(1), q(0))]))
        .duration(150.0)
        .error_estimate(0.003)
        .build();
    target.add_operation(cz_desc);

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Matrix Equality and Convention Verification
// ============================================================================

#[test]
fn test_matrix_reversal_c0_t1_equality() {
    // Verifies (H \otimes H) CX_{1,0} (H \otimes H) == CX_{0,1} to within 1e-12
    assert!(verify_cx_reversal_matrix_c0_t1(1e-12).is_ok());
}

#[test]
fn test_matrix_reversal_c1_t0_equality() {
    // Verifies (H \otimes H) CX_{0,1} (H \otimes H) == CX_{1,0} to within 1e-12
    assert!(verify_cx_reversal_matrix_c1_t0(1e-12).is_ok());
}

// ============================================================================
// 2. One-Way CX Target Fixtures & Both Orientations
// ============================================================================

#[test]
fn test_one_way_cx_reversal_forward_to_reversed() {
    let target = make_target_cx_one_way_1_to_0();

    // Circuit executes CX(0, 1), which is illegal on target (target only supports (1, 0))
    let mut builder = CircuitBuilder::new("test_cx_0_1");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let original = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target);
    let (corrected, report) = corrector.correct_circuit(&original).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_corrected, 1);
    assert_eq!(report.total_emitted_operations, 5); // H(0), H(1), CX(1, 0), H(0), H(1)

    // Verify operations in corrected circuit
    let ops: Vec<Operation> = corrected
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation)
        .collect();

    assert_eq!(ops[0].name(), "h");
    assert_eq!(ops[0].qubits()[0].0, 0);
    assert_eq!(ops[1].name(), "h");
    assert_eq!(ops[1].qubits()[0].0, 1);

    // The entangler must be CX(1, 0)
    assert_eq!(ops[2].name(), "cx");
    assert_eq!(ops[2].qubits()[0].0, 1); // control = 1
    assert_eq!(ops[2].qubits()[1].0, 0); // target = 0

    assert_eq!(ops[3].name(), "h");
    assert_eq!(ops[3].qubits()[0].0, 0);
    assert_eq!(ops[4].name(), "h");
    assert_eq!(ops[4].qubits()[0].0, 1);

    // Verify full unitary equivalence
    let verdict = check_equivalence(&original, &corrected).unwrap();
    assert!(
        verdict.is_equivalent,
        "Direction corrected circuit must be unitarily equivalent: max_diff={}",
        verdict.max_amplitude_difference
    );
}

#[test]
fn test_one_way_cx_already_legal_preserved() {
    let target = make_target_cx_one_way_1_to_0();

    // Circuit executes CX(1, 0), which is already legal on hardware
    let mut builder = CircuitBuilder::new("test_cx_1_0");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let original = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target);
    let (corrected, report) = corrector.correct_circuit(&original).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_preserved, 1);
    assert_eq!(report.operations_corrected, 0);
    assert_eq!(report.total_emitted_operations, 1);

    let ops: Vec<Operation> = corrected
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation)
        .collect();
    assert_eq!(ops[0].name(), "cx");
    assert_eq!(ops[0].qubits()[0].0, 1);
    assert_eq!(ops[0].qubits()[1].0, 0);

    let verdict = check_equivalence(&original, &corrected).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_one_way_cx_reversed_to_forward_orientation() {
    let target = make_target_cx_one_way_0_to_1();

    // Circuit executes CX(1, 0), while target only supports (0, 1)
    let mut builder = CircuitBuilder::new("test_reverse_1_0");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let original = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target);
    let (corrected, report) = corrector.correct_circuit(&original).unwrap();

    assert_eq!(report.operations_corrected, 1);
    assert_eq!(report.total_emitted_operations, 5);

    let ops: Vec<Operation> = corrected
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation)
        .collect();
    // Entangler must be CX(0, 1)
    assert_eq!(ops[2].name(), "cx");
    assert_eq!(ops[2].qubits()[0].0, 0);
    assert_eq!(ops[2].qubits()[1].0, 1);

    let verdict = check_equivalence(&original, &corrected).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. Symmetric Operations (CZ)
// ============================================================================

#[test]
fn test_symmetric_cz_undirected_no_unnecessary_correction() {
    let target = make_target_cz_undirected();

    let mut builder = CircuitBuilder::new("test_cz_undirected");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.cz(q[1], q[0]).unwrap();
    let original = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target);
    let (corrected, report) = corrector.correct_circuit(&original).unwrap();

    // Both CZ gates must be preserved without any Hadamards
    assert_eq!(report.operations_preserved, 2);
    assert_eq!(report.operations_corrected, 0);
    assert_eq!(report.total_emitted_operations, 2);

    let verdict = check_equivalence(&original, &corrected).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_symmetric_cz_directed_commutation_without_hadamards() {
    let target = make_target_cz_directed_1_to_0();

    // Circuit executes CZ(0, 1), but target only permits (1, 0)
    let mut builder = CircuitBuilder::new("test_cz_directed");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let original = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target);
    let (corrected, report) = corrector.correct_circuit(&original).unwrap();

    assert_eq!(report.operations_corrected, 1);
    assert_eq!(report.total_emitted_operations, 1); // Exact 1 gate, NO Hadamards!

    let ops: Vec<Operation> = corrected
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation)
        .collect();
    assert_eq!(ops[0].name(), "cz");
    assert_eq!(ops[0].qubits()[0].0, 1);
    assert_eq!(ops[0].qubits()[1].0, 0);

    let verdict = check_equivalence(&original, &corrected).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 4. Impossible Directions Fail Explicitly
// ============================================================================

#[test]
fn test_impossible_direction_rejected_explicitly() {
    let target = make_target_cx_one_way_1_to_0();

    // Target only connects (1, 0). Qubits 0 and 2 have NO connection in either direction.
    let mut builder = CircuitBuilder::new("test_impossible");
    let q_reg = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q_reg[0], q_reg[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let corrector = TwoQubitDirectionCorrector::new(target.clone());
    let err = corrector.correct_circuit(&circuit).unwrap_err();

    match err {
        DirectionError::NoViableDirection {
            operation, u, v, ..
        } => {
            assert_eq!(operation, "cx");
            assert_eq!(u, q(0));
            assert_eq!(v, q(2));
        }
        other => panic!("expected NoViableDirection error, got {other:?}"),
    }

    // Also assert that DirectionStatus confirms NoCoupling
    assert_eq!(
        target.check_direction("cx", q(0), q(2)),
        DirectionStatus::NoCoupling
    );
}

// ============================================================================
// 5. Target-Specific Correction Alternatives & Deterministic Cost Selection
// ============================================================================

#[derive(Clone)]
struct CustomExpensiveReversalRule;

impl TargetDirectionRule for CustomExpensiveReversalRule {
    fn name(&self) -> &'static str {
        "custom_expensive_cx_rule"
    }

    fn target_gate(&self) -> &'static str {
        "cx"
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("cx") || op.qubits().len() != 2 {
            return false;
        }
        let u = PhysicalQubitId(op.qubits()[0].0 as u32);
        let v = PhysicalQubitId(op.qubits()[1].0 as u32);
        target.check_direction("cx", u, v) == DirectionStatus::NeedsReversal
    }

    fn correct(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let u = op.qubits()[0];
        let v = op.qubits()[1];
        // Custom sequence inserting extra identity-like operations for higher cost
        let operations = vec![
            Operation::custom_gate("h", vec![u], vec![]),
            Operation::custom_gate("h", vec![v], vec![]),
            Operation::custom_gate("cx", vec![v, u], vec![]),
            Operation::custom_gate("h", vec![u], vec![]),
            Operation::custom_gate("h", vec![v], vec![]),
        ];
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: 0.0,
        })
    }
}

#[test]
fn test_target_specific_alternative_deterministic_selection() {
    let target = make_target_cx_one_way_1_to_0();

    let mut builder = CircuitBuilder::new("test_alternative");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let mut registry = DirectionCorrectionRegistry::with_default_rules();
    registry.register_alternative(Arc::new(CustomExpensiveReversalRule));

    let corrector = TwoQubitDirectionCorrector::new(target)
        .with_registry(registry)
        .with_cost_model(CostModel::default());

    let (corrected, report) = corrector.correct_circuit(&circuit).unwrap();
    assert_eq!(report.operations_corrected, 1);

    // Selected candidate must be deterministic and valid
    let verdict = check_equivalence(&circuit, &corrected).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 6. Integration with Compiler Pass & PassManager
// ============================================================================

#[test]
fn test_compiler_pass_integration() {
    let target = make_target_cx_one_way_1_to_0();

    let mut builder = CircuitBuilder::new("test_pass");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap(); // Needs reversal
    let original = builder.finish().unwrap();

    let pass = TwoQubitDirectionPass::new(target);
    let mut manager = PassManager::new();
    manager.add_pass(pass);

    let mut circuit = original.clone();
    let pass_report = manager.run(&mut circuit).unwrap();
    assert!(pass_report.total_passes_modified > 0);

    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 7. Integration with BasisTranslationEngine & Superconducting Targets
// ============================================================================

#[test]
fn test_basis_translation_engine_cx_reversal_on_superconducting_target() {
    // Superconducting target with native RZ, SX and directed CX (1, 0)
    let mut target = Target::new("ibm_style_directed", 2);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(0.0)
        .error_estimate(0.0001)
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(sx_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([(q(1), q(0))]))
        .duration(200.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();

    // Circuit has CX(0, 1) which needs reversal AND has no native H
    let mut builder = CircuitBuilder::new("test_sc_reversal");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let original = builder.finish().unwrap();

    let engine = BasisTranslationEngine::new(target);
    let (translated, report) = engine.translate_circuit(&original).unwrap();

    assert_eq!(report.operations_decomposed, 1);

    // Verify all emitted operations are strictly native to the target: rz, sx, or cx
    let ops: Vec<Operation> = translated
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation)
        .collect();

    for op in &ops {
        assert!(
            op.name() == "rz" || op.name() == "sx" || op.name() == "cx",
            "Non-native operation emitted: {}",
            op.name()
        );
        if op.name() == "cx" {
            // Must strictly use legal hardware orientation (1, 0)
            assert_eq!(op.qubits()[0].0, 1);
            assert_eq!(op.qubits()[1].0, 0);
        }
    }

    let verdict = check_equivalence(&original, &translated).unwrap();
    assert!(verdict.is_equivalent);
}
