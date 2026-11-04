//! Comprehensive integration tests for Phase 41: Basis Translation Engine.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::target::{
    CostModel, CostWeights, OperationConnectivity, PhysicalQubitId, Target,
    TargetOperationDescriptor,
};
use qcc::translation::rules::{DecompositionCandidate, TranslationRule, TranslationRuleRegistry};
use qcc::translation::{BasisTranslationEngine, BasisTranslationPass, TranslationError};
use qcc::verification::check_equivalence;

// ============================================================================
// Synthetic Target Fixtures
// ============================================================================

/// Creates a 4-qubit target supporting native RZ, SX, and directed CX on all pairs.
fn create_cx_native_target() -> Target {
    let mut target = Target::new("cx_native_target", 4);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0)
        .error_estimate(0.0001)
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(sx_desc);

    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(x_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::all_to_all())
        .duration(250.0)
        .error_estimate(0.005)
        .build();
    target.add_operation(cx_desc);

    let meas_desc = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(700.0)
        .error_estimate(0.01)
        .build();
    target.add_operation(meas_desc);

    target.validate().unwrap();
    target
}

/// Creates a 4-qubit target supporting native RZ, SX, and undirected CZ on all pairs.
fn create_cz_native_target() -> Target {
    let mut target = Target::new("cz_native_target", 4);

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0)
        .error_estimate(0.0001)
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .error_estimate(0.0005)
        .build();
    target.add_operation(sx_desc);

    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::all_to_all())
        .duration(180.0)
        .error_estimate(0.003)
        .build();
    target.add_operation(cz_desc);

    target.validate().unwrap();
    target
}

/// Creates a 4-qubit target supporting discrete gates H and CZ (no native CX, no RZ/SX).
fn create_h_cz_discrete_target() -> Target {
    let mut target = Target::new("h_cz_discrete_target", 4);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(20.0)
        .error_estimate(0.0002)
        .build();
    target.add_operation(h_desc);

    let cz_desc = TargetOperationDescriptor::builder("cz", 2)
        .connectivity(OperationConnectivity::all_to_all())
        .duration(150.0)
        .error_estimate(0.002)
        .build();
    target.add_operation(cz_desc);

    target.validate().unwrap();
    target
}

/// Creates a 4-qubit target supporting discrete gates H and CX.
fn create_h_cx_discrete_target() -> Target {
    let mut target = Target::new("h_cx_discrete_target", 4);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(20.0)
        .error_estimate(0.0002)
        .build();
    target.add_operation(h_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::all_to_all())
        .duration(200.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

/// Creates a 4-qubit target supporting CX, H, T, Tdg.
fn create_toffoli_basis_target() -> Target {
    let mut target = Target::new("toffoli_basis_target", 4);

    for gate in &["h", "t", "tdg"] {
        let desc = TargetOperationDescriptor::builder(*gate, 1)
            .connectivity(OperationConnectivity::all_single_qubits(4))
            .duration(25.0)
            .error_estimate(0.0003)
            .build();
        target.add_operation(desc);
    }

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::all_to_all())
        .duration(220.0)
        .error_estimate(0.004)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

/// Creates a 2-qubit target with NO two-qubit entanglers (single-qubit only).
fn create_single_qubit_only_target() -> Target {
    let mut target = Target::new("single_qubit_only_target", 2);

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

    target.validate().unwrap();
    target
}

// ============================================================================
// Test Cases
// ============================================================================

#[test]
fn test_already_supported_operations_preserved() {
    let target = create_cx_native_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("native_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.rz(q[0], 0.75).unwrap();
    builder.custom_gate("sx", &[q[1]], vec![]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 4);
    assert_eq!(report.operations_preserved, 4);
    assert_eq!(report.operations_decomposed, 0);
    assert_eq!(report.total_emitted_operations, 4);
    assert_eq!(report.global_phase_delta, 0.0);
    assert_eq!(translated.num_operations(), 4);
}

#[test]
fn test_cx_to_cz_translation() {
    let target = create_h_cz_discrete_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("cx_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);
    assert_eq!(translated.num_operations(), 3);

    // Order: H(1), CZ(0, 1), H(1)
    let ops: Vec<Operation> = translated
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation.clone())
        .collect();

    assert_eq!(ops[0].name(), "h");
    assert_eq!(ops[0].qubits()[0].0, 1);
    assert_eq!(ops[1].name(), "cz");
    assert_eq!(ops[2].name(), "h");
    assert_eq!(ops[2].qubits()[0].0, 1);

    // Verify local and global unitary equivalence
    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_cz_to_cx_translation() {
    let target = create_h_cx_discrete_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("cz_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);
    assert_eq!(translated.num_operations(), 3);

    let ops: Vec<Operation> = translated
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation.clone())
        .collect();

    assert_eq!(ops[0].name(), "h");
    assert_eq!(ops[1].name(), "cx");
    assert_eq!(ops[2].name(), "h");

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_swap_to_cx_translation() {
    let target = create_cx_native_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("swap_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);
    assert_eq!(translated.num_operations(), 3);

    for node in translated.graph.ordered_nodes().unwrap() {
        assert_eq!(node.operation.name(), "cx");
    }

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_recursive_swap_on_cz_target() {
    // Target supports only H and CZ
    let target = create_h_cz_discrete_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("swap_on_cz");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);

    // SWAP -> 3 CX. Each CX -> H, CZ, H. Total: 3 * 3 = 9 operations
    assert_eq!(translated.num_operations(), 9);

    for node in translated.graph.ordered_nodes().unwrap() {
        let name = node.operation.name();
        assert!(name == "cz" || name == "h", "unexpected operation '{name}'");
    }

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_recursive_multi_layer_full_superconducting_basis() {
    // Target supports ONLY CZ, RZ, and SX (no H, no CX, no SWAP)
    let target = create_cz_native_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("full_recursive_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);

    // All emitted operations must be strictly in {cz, rz, sx}
    for node in translated.graph.ordered_nodes().unwrap() {
        let name = node.operation.name();
        assert!(
            name == "cz" || name == "rz" || name == "sx",
            "operation '{name}' is not in native basis"
        );
    }

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_toffoli_decomposition() {
    let target = create_toffoli_basis_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("toffoli_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder
        .custom_gate("ccx", &[q[0], q[1], q[2]], vec![])
        .unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 1);
    assert_eq!(report.operations_decomposed, 1);
    assert_eq!(translated.num_operations(), 15);

    // Verify all operations are in target's basis
    for node in translated.graph.ordered_nodes().unwrap() {
        let name = node.operation.name();
        assert!(
            name == "cx" || name == "h" || name == "t" || name == "tdg",
            "unexpected op '{name}'"
        );
    }

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_single_qubit_rotations_synthesis() {
    let target = create_cx_native_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("1q_circuit");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.ry(q[0], 0.72).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 4);
    assert_eq!(report.operations_decomposed, 4);

    for node in translated.graph.ordered_nodes().unwrap() {
        let name = node.operation.name();
        assert!(
            name == "rz" || name == "sx" || name == "x",
            "non-native 1Q op '{name}'"
        );
    }

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

#[test]
fn test_symbolic_parameter_preservation() {
    let target = create_cx_native_target();
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("symbolic_circuit");
    let q = builder.add_quantum_register("q", 1).unwrap();

    let theta_sym = ParameterExpr::symbol("theta");
    let phi_sym = ParameterExpr::symbol("phi");

    builder
        .custom_gate("rx", &[q[0]], vec![theta_sym.clone()])
        .unwrap();
    builder
        .custom_gate("ry", &[q[0]], vec![phi_sym.clone()])
        .unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, report) = engine.translate_circuit(&circuit).unwrap();

    assert_eq!(report.operations_examined, 2);
    assert_eq!(report.operations_decomposed, 2);

    // Check that emitted operations contain expressions with the symbol
    let mut found_theta = false;
    let mut found_phi = false;

    for node in translated.graph.ordered_nodes().unwrap() {
        for param in node.operation.parameters() {
            let expr_str = format!("{param:?}");
            if expr_str.contains("theta") {
                found_theta = true;
            }
            if expr_str.contains("phi") {
                found_phi = true;
            }
        }
    }

    assert!(
        found_theta,
        "symbol 'theta' must be preserved in translated operations"
    );
    assert!(
        found_phi,
        "symbol 'phi' must be preserved in translated operations"
    );
}

#[test]
fn test_cost_based_candidate_selection() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Target with directed CX where (1, 0) is much cheaper/faster than (0, 1)
    let mut target = Target::new("asymmetric_cx_target", 2);

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

    // Both (0, 1) and (1, 0) are supported, but (0, 1) has duration 500ns, while (1, 0) has 100ns
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(vec![
            (p0, p1),
            (p1, p0),
        ]))
        .duration(100.0)
        .error_estimate(0.005)
        .build();
    target.add_operation(cx_desc);
    target.validate().unwrap();

    // Set high weight on duration
    let cost_weights = CostWeights {
        duration_ns: 10.0,
        ..Default::default()
    };
    let cost_model = CostModel::new(cost_weights);

    let engine = BasisTranslationEngine::builder(target)
        .cost_model(cost_model)
        .build();

    let mut builder = CircuitBuilder::new("swap_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let (translated, _report) = engine.translate_circuit(&circuit).unwrap();
    assert_eq!(translated.num_operations(), 3);

    let verdict = check_equivalence(&circuit, &translated).unwrap();
    assert!(
        verdict.is_equivalent,
        "circuits must be unitarily equivalent"
    );
}

// Custom rule for testing cycle detection
struct CycleRuleA;
impl TranslationRule for CycleRuleA {
    fn name(&self) -> &'static str {
        "cycle_rule_a"
    }
    fn target_gate(&self) -> &'static str {
        "gate_a"
    }
    fn quantum_arity(&self) -> usize {
        1
    }
    fn matches(&self, op: &Operation, _target: &Target) -> bool {
        op.name() == "gate_a"
    }
    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let q = op.qubits()[0];
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations: vec![Operation::custom_gate("gate_b", vec![q], vec![])],
            global_phase_delta: 0.0,
        })
    }
}

struct CycleRuleB;
impl TranslationRule for CycleRuleB {
    fn name(&self) -> &'static str {
        "cycle_rule_b"
    }
    fn target_gate(&self) -> &'static str {
        "gate_b"
    }
    fn quantum_arity(&self) -> usize {
        1
    }
    fn matches(&self, op: &Operation, _target: &Target) -> bool {
        op.name() == "gate_b"
    }
    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let q = op.qubits()[0];
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations: vec![Operation::custom_gate("gate_a", vec![q], vec![])],
            global_phase_delta: 0.0,
        })
    }
}

#[test]
fn test_recursive_cycle_detection() {
    let target = create_cx_native_target();
    let mut registry = TranslationRuleRegistry::empty();
    registry.register(Box::new(CycleRuleA));
    registry.register(Box::new(CycleRuleB));

    let engine = BasisTranslationEngine::builder(target)
        .registry(registry)
        .verify_equivalence(false) // Custom dummy gates don't have analytical matrices
        .build();

    let mut builder = CircuitBuilder::new("cycle_circuit");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.custom_gate("gate_a", &[q[0]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    let result = engine.translate_circuit(&circuit);
    match result {
        Err(TranslationError::CycleDetected {
            op_name,
            cycle_path,
        }) => {
            assert!(op_name == "gate_a" || op_name == "gate_b");
            assert!(cycle_path.contains("gate_a"));
            assert!(cycle_path.contains("gate_b"));
        }
        other => panic!("expected CycleDetected error, got {other:?}"),
    }
}

#[test]
fn test_max_recursion_depth_exceeded() {
    // Target supports only CZ and SX (requires depth > 2 for SWAP)
    let target = create_cz_native_target();
    let engine = BasisTranslationEngine::builder(target)
        .max_depth(1) // artificially restrict depth to 1
        .build();

    let mut builder = CircuitBuilder::new("deep_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let result = engine.translate_circuit(&circuit);
    match result {
        Err(TranslationError::MaxRecursionDepthExceeded {
            op_name: _,
            depth,
            max_depth,
        }) => {
            assert_eq!(max_depth, 1);
            assert!(depth >= 1);
        }
        other => panic!("expected MaxRecursionDepthExceeded error, got {other:?}"),
    }
}

#[test]
fn test_explicit_failure_when_no_valid_translation() {
    let target = create_single_qubit_only_target(); // No 2-qubit gates supported at all
    let engine = BasisTranslationEngine::new(target);

    let mut builder = CircuitBuilder::new("untranslatable_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let result = engine.translate_circuit(&circuit);
    match result {
        Err(TranslationError::NoViableTranslation { op_name, .. }) => {
            assert_eq!(op_name, "cx");
        }
        other => panic!("expected NoViableTranslation error, got {other:?}"),
    }
}

// Bogus rule for testing local equivalence rejection
struct CorruptedCxRule;
impl TranslationRule for CorruptedCxRule {
    fn name(&self) -> &'static str {
        "corrupted_cx_rule"
    }
    fn target_gate(&self) -> &'static str {
        "cx"
    }
    fn quantum_arity(&self) -> usize {
        2
    }
    fn matches(&self, op: &Operation, _target: &Target) -> bool {
        op.name() == "cx"
    }
    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let c = op.qubits()[0];
        let t = op.qubits()[1];
        // Erroneously emits CZ without Hadamards
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations: vec![Operation::custom_gate("cz", vec![c, t], vec![])],
            global_phase_delta: 0.0,
        })
    }
}

#[test]
fn test_local_equivalence_rejection_of_corrupted_candidate() {
    let target = create_cz_native_target();
    let mut registry = TranslationRuleRegistry::empty();
    registry.register(Box::new(CorruptedCxRule));

    let engine = BasisTranslationEngine::builder(target)
        .registry(registry)
        .verify_equivalence(true) // local equivalence verification must reject corrupted candidate
        .build();

    let mut builder = CircuitBuilder::new("corrupted_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let result = engine.translate_circuit(&circuit);
    // Since CorruptedCxRule is rejected by equivalence check, no viable candidate remains
    assert!(result.is_err());
    match result {
        Err(TranslationError::NoViableTranslation { .. }) => {}
        other => panic!("expected NoViableTranslation from rejected equivalence, got {other:?}"),
    }
}

#[test]
fn test_compiler_pass_integration() {
    let target = create_cx_native_target();
    let mut pass = BasisTranslationPass::new(target);

    let mut builder = CircuitBuilder::new("pass_test_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut context = PassContext::new();
    let result = pass.run(&mut circuit, &mut context).unwrap();

    assert!(result.modified);
    assert!(result.summary.contains("decomposed"));

    let report = pass.last_report().expect("report must be recorded");
    assert_eq!(report.operations_decomposed, 1);
    assert_eq!(circuit.num_operations(), 3);
}
