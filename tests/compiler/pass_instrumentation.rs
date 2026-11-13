//! Integration tests for Phase 72: Pass-Manager Instrumentation and Verification Hooks.
//!
//! Validates:
//! 1. Execution order sequencing.
//! 2. Fingerprint boundary continuity.
//! 3. Pass-local metrics vs cumulative pipeline metrics.
//! 4. Skipped pass truthful status reporting.
//! 5. Failed pass diagnostic preservation.
//! 6. Metric availability states for missing/unmeasured metrics.
//! 7. Verification hooks disabled by default.
//! 8. Verification hook provenance and status reporting when enabled.
//! 9. Unsupported verification not reported as passed.
//! 10. Invariance of final compiler output under instrumentation.
//! 11. Invariance of compiler error behavior under instrumentation.
//! 12. Full schema invariant validity and JSON round-trips.
//! 13. Overhead measurement across representative workloads.

use std::time::Instant;

use qcc::builder::CircuitBuilder;
use qcc::compiler::config::CompilerConfig;
use qcc::compiler::pass_diagnostic::{
    execute_pass_verification_hook, MeasurementAvailability, PassDiagnosticConfig,
    PassDiagnosticReport, PassStatus, PassVerificationHookConfig, PassVerificationStatus,
    VerifierProvenance,
};
use qcc::compiler::pipeline::CompilerPipeline;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::rewrite::CircuitTransaction;
use qcc::ir::wire::QubitId;
use qcc::opt::context::PassContext;
use qcc::opt::errors::{PassError, PassManagerError};
use qcc::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use qcc::opt::pass_manager::PassManager;
use qcc::optimizer::config::{OptimizationLevel, OptimizerConfig};
use qcc::optimizer::passes::identity::IdentityEliminationPass;
use qcc::optimizer::pipeline::OptimizerPipeline;
use qcc::quantum::gate::Gate;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::OperationConnectivity;

// ----------------------------------------------------------------------------
// Target Helper
// ----------------------------------------------------------------------------

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

fn make_linear_target(name: &str, num_qubits: usize) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let z_desc = TargetOperationBuilder::new("z", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let measure_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(500.0)
        .build();

    builder = builder
        .add_operation(h_desc)
        .add_operation(x_desc)
        .add_operation(z_desc)
        .add_operation(rz_desc)
        .add_operation(sx_desc)
        .add_operation(measure_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((q(i as u32), q((i + 1) as u32)));
    }

    if !edges.is_empty() {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx_desc);

        let swap_desc = TargetOperationBuilder::new("swap", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(300.0)
            .build();
        builder = builder.add_operation(swap_desc);
    }

    builder.build().expect("valid target")
}

// ----------------------------------------------------------------------------
// Test Mock Passes
// ----------------------------------------------------------------------------

struct AddGatesPass {
    count: usize,
}

impl AddGatesPass {
    fn new(count: usize) -> Self {
        Self { count }
    }
}

impl CompilerPass for AddGatesPass {
    fn id(&self) -> PassId {
        PassId::new("add_gates_pass")
    }

    fn description(&self) -> &'static str {
        "Test pass that adds CX gates"
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::UnitaryOnly
    }

    fn mutation_scope(&self) -> PassMutation {
        PassMutation::CircuitTopology
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        if circuit.num_qubits() < 2 {
            return Err(PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: "requires at least 2 qubits".to_string(),
            });
        }
        for _ in 0..self.count {
            let op = Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)]);
            circuit.add_operation(op).map_err(|source| PassError::Ir {
                pass_id: self.id(),
                source,
            })?;
        }
        Ok(PassResult::modified(format!(
            "added {} CX gates",
            self.count
        )))
    }
}

struct RemoveGatesPass {
    count: usize,
}

impl RemoveGatesPass {
    fn new(count: usize) -> Self {
        Self { count }
    }
}

impl CompilerPass for RemoveGatesPass {
    fn id(&self) -> PassId {
        PassId::new("remove_gates_pass")
    }

    fn description(&self) -> &'static str {
        "Test pass that removes trailing gates"
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::UnitaryOnly
    }

    fn mutation_scope(&self) -> PassMutation {
        PassMutation::CircuitTopology
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let node_ids: Vec<_> = circuit.graph.nodes().map(|n| n.id).collect();
        let to_remove = self.count.min(node_ids.len());
        for id in node_ids.into_iter().rev().take(to_remove) {
            let mut tx = CircuitTransaction::begin(circuit);
            tx.remove_operation(id)
                .map_err(|source| PassError::ExecutionFailed {
                    pass_id: self.id(),
                    reason: source.to_string(),
                })?;
            tx.commit().map_err(|source| PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: source.to_string(),
            })?;
        }
        Ok(PassResult::modified(format!("removed {} gates", to_remove)))
    }
}

struct DeliberateFailingPass;

impl CompilerPass for DeliberateFailingPass {
    fn id(&self) -> PassId {
        PassId::new("deliberate_failing_pass")
    }

    fn description(&self) -> &'static str {
        "Test pass that intentionally returns an error"
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::GeneralStructural
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        Err(PassError::ExecutionFailed {
            pass_id: self.id(),
            reason: "deliberate test failure".to_string(),
        })
    }
}

#[allow(dead_code)]
struct SemanticCorruptingPass;

impl CompilerPass for SemanticCorruptingPass {
    fn id(&self) -> PassId {
        PassId::new("semantic_corrupting_pass")
    }

    fn description(&self) -> &'static str {
        "Pass that inserts an unauthorized Z gate violating unitary equivalence"
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::UnitaryOnly
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let q = QubitId(0);
        let op = Operation::gate(&Gate::Z, vec![q]);
        circuit.add_operation(op).map_err(|source| PassError::Ir {
            pass_id: self.id(),
            source,
        })?;
        Ok(PassResult::modified("inserted unauthorized Z gate"))
    }
}

fn build_bell_circuit() -> Circuit {
    let mut b = CircuitBuilder::new("bell");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.finish().unwrap()
}

// ----------------------------------------------------------------------------
// Test 1: Multiple passes produce records in correct execution order
// ----------------------------------------------------------------------------
#[test]
fn test_01_multiple_passes_execution_order() {
    let mut circuit = build_bell_circuit();
    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm.add_pass(AddGatesPass::new(2));
    pm.add_pass(AddGatesPass::new(1));
    pm.add_pass(RemoveGatesPass::new(1));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    assert_eq!(report.pass_diagnostics.len(), 3);
    for (idx, diag) in report.pass_diagnostics.iter().enumerate() {
        assert_eq!(diag.execution_order, idx);
        assert_eq!(diag.status, PassStatus::Ran);
        diag.validate_invariants().unwrap();
    }
}

// ----------------------------------------------------------------------------
// Test 2: Input and output fingerprints correspond to actual pass boundaries
// ----------------------------------------------------------------------------
#[test]
fn test_02_fingerprints_boundary_continuity() {
    let mut circuit = build_bell_circuit();
    let initial_json = qcc::ir::circuit_to_json(&circuit).unwrap();
    let expected_initial_fp = qcc::compiler::manifest::sha256_hex(initial_json.as_bytes());

    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm.add_pass(AddGatesPass::new(2));
    pm.add_pass(AddGatesPass::new(1));
    pm.add_pass(RemoveGatesPass::new(2));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    assert_eq!(
        report.pass_diagnostics[0].fingerprints.input_fingerprint,
        Some(expected_initial_fp)
    );

    for i in 0..(report.pass_diagnostics.len() - 1) {
        let out_fp = &report.pass_diagnostics[i].fingerprints.output_fingerprint;
        let next_in_fp = &report.pass_diagnostics[i + 1]
            .fingerprints
            .input_fingerprint;
        assert!(out_fp.is_some());
        assert_eq!(
            out_fp,
            next_in_fp,
            "Boundary continuity failed between step {} and step {}",
            i,
            i + 1
        );
    }
}

// ----------------------------------------------------------------------------
// Test 3: Pass-local metrics are not confused with cumulative pipeline metrics
// ----------------------------------------------------------------------------
#[test]
fn test_03_pass_local_vs_cumulative_metrics() {
    let mut circuit = build_bell_circuit();
    let initial_ops = circuit.num_operations(); // 2

    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    // Pass 1: Adds 3 CX gates
    pm.add_pass(AddGatesPass::new(3));
    // Pass 2: Removes 2 gates
    pm.add_pass(RemoveGatesPass::new(2));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    let diag1 = &report.pass_diagnostics[0];
    let diag2 = &report.pass_diagnostics[1];

    // Pass 1 local accounting: added 3, removed 0
    assert_eq!(diag1.transformation.operations_added, 3);
    assert_eq!(diag1.transformation.operations_removed, 0);
    assert_eq!(
        diag1.measurements.before.as_ref().unwrap().cx_count,
        Some(1)
    );
    assert_eq!(diag1.measurements.after.as_ref().unwrap().cx_count, Some(4));

    // Pass 2 local accounting: added 0, removed 2
    assert_eq!(diag2.transformation.operations_added, 0);
    assert_eq!(diag2.transformation.operations_removed, 2);
    assert_eq!(
        diag2.measurements.before.as_ref().unwrap().cx_count,
        Some(4)
    );
    assert_eq!(diag2.measurements.after.as_ref().unwrap().cx_count, Some(2));

    // Net pipeline delta is +1, but neither pass locally claimed +1!
    assert_eq!(circuit.num_operations(), initial_ops + 1);
}

// ----------------------------------------------------------------------------
// Test 4: Skipped passes produce truthful statuses
// ----------------------------------------------------------------------------
#[test]
fn test_04_skipped_passes_truthful_status() {
    let mut circuit = build_bell_circuit();
    let mut opt_cfg = OptimizerConfig::new(OptimizationLevel::O0);
    opt_cfg.diagnostics = PassDiagnosticConfig::standard();

    let optimizer = OptimizerPipeline::new(opt_cfg);
    let mut context = PassContext::new();
    let report = optimizer
        .optimize_with_context(&mut circuit, &mut context)
        .unwrap();

    assert!(!report.pass_diagnostics.is_empty());
    for diag in &report.pass_diagnostics {
        assert_eq!(diag.status, PassStatus::Skipped);
        assert!(diag.fingerprints.input_fingerprint.is_some());
        assert!(diag.fingerprints.output_fingerprint.is_none());
        assert!(diag.measurements.before.is_some());
        assert!(diag.measurements.after.is_none());
        assert_eq!(
            diag.measurements.availability.get("after"),
            Some(&MeasurementAvailability::NotApplicable)
        );
        assert_eq!(diag.verification.status, PassVerificationStatus::NotRun);
        diag.validate_invariants().unwrap();
    }
}

// ----------------------------------------------------------------------------
// Test 5: Failed passes preserve earlier diagnostic records
// ----------------------------------------------------------------------------
#[test]
fn test_05_failed_passes_preserve_earlier_records() {
    let mut circuit = build_bell_circuit();
    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm.add_pass(AddGatesPass::new(1)); // Step 0: succeeds
    pm.add_pass(AddGatesPass::new(2)); // Step 1: succeeds
    pm.add_pass(DeliberateFailingPass); // Step 2: fails
    pm.add_pass(AddGatesPass::new(1)); // Step 3: should not run

    let mut context = PassContext::new();
    let err = pm.run_with_context(&mut circuit, &mut context).unwrap_err();

    match err {
        PassManagerError::PassFailed { step_index, .. } => {
            assert_eq!(step_index, 2);
        }
        other => panic!("expected PassFailed error, got {:?}", other),
    }

    // Both pass manager and context preserve the diagnostic records
    let reports = pm.diagnostic_reports();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].status, PassStatus::Ran);
    assert_eq!(reports[1].status, PassStatus::Ran);
    assert_eq!(reports[2].status, PassStatus::Failed);

    assert!(reports[2].measurements.before.is_some());
    assert!(reports[2].measurements.after.is_none());
    assert_eq!(
        reports[2].measurements.availability.get("after"),
        Some(&MeasurementAvailability::Unavailable)
    );

    assert_eq!(context.diagnostic_reports().len(), 3);
}

// ----------------------------------------------------------------------------
// Test 6: Missing metrics use correct availability states
// ----------------------------------------------------------------------------
#[test]
fn test_06_missing_metrics_availability_states() {
    let mut circuit = build_bell_circuit();
    let mut diag_cfg = PassDiagnosticConfig::standard();
    diag_cfg.collect_metrics = false;
    diag_cfg.collect_fingerprints = false;

    let mut pm = PassManager::new().with_diagnostic_config(diag_cfg);
    pm.add_pass(AddGatesPass::new(1));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    let diag = &report.pass_diagnostics[0];
    assert!(diag.measurements.before.is_none());
    assert!(diag.measurements.after.is_none());
    assert_eq!(
        diag.measurements.availability.get("before"),
        Some(&MeasurementAvailability::NotMeasured)
    );
    assert_eq!(
        diag.measurements.availability.get("after"),
        Some(&MeasurementAvailability::NotMeasured)
    );
    assert!(diag.fingerprints.input_fingerprint.is_none());
    assert!(diag.fingerprints.output_fingerprint.is_none());
    diag.validate_invariants().unwrap();
}

// ----------------------------------------------------------------------------
// Test 7: Verification hooks are disabled by default
// ----------------------------------------------------------------------------
#[test]
fn test_07_verification_hooks_disabled_by_default() {
    let default_cfg = PassDiagnosticConfig::default();
    assert!(!default_cfg.verification_hook.enabled);

    let mut circuit = build_bell_circuit();
    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm.add_pass(AddGatesPass::new(1));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    let diag = &report.pass_diagnostics[0];
    assert_eq!(diag.verification.status, PassVerificationStatus::NotRun);
    assert!(diag.verification.verifier_name.is_none());
}

// ----------------------------------------------------------------------------
// Test 8: Enabled verification records accurate provenance and status
// ----------------------------------------------------------------------------
#[test]
fn test_08_enabled_verification_records_accurate_provenance() {
    let mut b = CircuitBuilder::new("id_circ");
    let q = b.add_quantum_register("q", 1).unwrap();
    b.id(q[0]).unwrap();
    let mut circuit = b.finish().unwrap();

    let diag_cfg = PassDiagnosticConfig::standard().with_verification(true);
    let mut pm = PassManager::new().with_diagnostic_config(diag_cfg);
    pm.add_pass(IdentityEliminationPass::default());

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    let diag = &report.pass_diagnostics[0];
    assert_eq!(diag.verification.status, PassVerificationStatus::Passed);
    assert_eq!(
        diag.verification.provenance,
        Some(VerifierProvenance::Internal)
    );
    assert_eq!(
        diag.verification.verifier_name,
        Some("qcc_unitary_equivalence".to_string())
    );
    assert!(diag.verification.verifier_version.is_some());
    assert!(diag.verification.verifier_mode.is_some());
    assert!(diag.verification.tolerances.is_some());
    diag.validate_invariants().unwrap();
}

// ----------------------------------------------------------------------------
// Test 9: Unsupported verification is not reported as passed
// ----------------------------------------------------------------------------
#[test]
fn test_09_unsupported_verification_not_reported_as_passed() {
    // 1. Non-unitary circuit (contains measurement)
    let mut b = CircuitBuilder::new("meas");
    let q = b.add_quantum_register("q", 1).unwrap();
    let c = b.add_classical_register("c", 1).unwrap();
    b.h(q[0]).unwrap();
    b.measure(q[0], c[0]).unwrap();
    let non_unitary_c = b.finish().unwrap();

    let hook_cfg = PassVerificationHookConfig {
        enabled: true,
        ..Default::default()
    };

    let ver = execute_pass_verification_hook(&non_unitary_c, &non_unitary_c, &hook_cfg).unwrap();
    assert_eq!(ver.status, PassVerificationStatus::Unsupported);
    assert!(ver.detail.as_ref().unwrap().contains("non-unitary"));

    // 2. Zero-qubit circuit
    let empty_c = Circuit::new("empty");
    let ver_zero = execute_pass_verification_hook(&empty_c, &empty_c, &hook_cfg).unwrap();
    assert_eq!(ver_zero.status, PassVerificationStatus::Unsupported);

    // 3. Exceeds max_qubits
    let bell = build_bell_circuit();
    let hook_tight = PassVerificationHookConfig {
        enabled: true,
        max_qubits: 1, // bell has 2 qubits
        ..Default::default()
    };
    let ver_exceeds = execute_pass_verification_hook(&bell, &bell, &hook_tight).unwrap();
    assert_eq!(ver_exceeds.status, PassVerificationStatus::Unsupported);
}

// ----------------------------------------------------------------------------
// Test 10: Invariance of final compiler output under instrumentation
// ----------------------------------------------------------------------------
#[test]
fn test_10_instrumentation_does_not_change_compiler_output() {
    let target = make_linear_target("target_5q", 5);
    let circuit = build_bell_circuit();

    let cfg_no_diag = CompilerConfig::o2();
    let pipeline_no_diag = CompilerPipeline::new(cfg_no_diag);
    let res_no_diag = pipeline_no_diag.compile(&circuit, &target).unwrap();

    let cfg_diag = CompilerConfig::o2().with_diagnostics(PassDiagnosticConfig::standard());
    let pipeline_diag = CompilerPipeline::new(cfg_diag);
    let res_diag = pipeline_diag.compile(&circuit, &target).unwrap();

    assert_eq!(
        res_no_diag.physical_circuit.num_operations(),
        res_diag.physical_circuit.num_operations()
    );
    assert_eq!(
        res_no_diag.report.metrics.final_gate_count,
        res_diag.report.metrics.final_gate_count
    );
    assert_eq!(
        res_no_diag.report.metrics.final_depth,
        res_diag.report.metrics.final_depth
    );
    assert_eq!(
        res_no_diag.layout.final_layout(),
        res_diag.layout.final_layout()
    );

    // Physical circuits serialize bit-for-bit identically
    let json_no_diag = qcc::ir::circuit_to_json(&res_no_diag.physical_circuit).unwrap();
    let json_diag = qcc::ir::circuit_to_json(&res_diag.physical_circuit).unwrap();
    assert_eq!(json_no_diag, json_diag);

    // And pass_diagnostics was populated on the instrumented result!
    assert!(!res_diag.pass_diagnostics.is_empty());
}

// ----------------------------------------------------------------------------
// Test 11: Invariance of compiler error behavior under instrumentation
// ----------------------------------------------------------------------------
#[test]
fn test_11_instrumentation_preserves_error_behavior() {
    let mut circuit = build_bell_circuit();
    let mut pm_no_diag = PassManager::new();
    pm_no_diag.add_pass(DeliberateFailingPass);
    let err_no_diag = pm_no_diag.run(&mut circuit.clone()).unwrap_err();

    let mut pm_diag = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm_diag.add_pass(DeliberateFailingPass);
    let err_diag = pm_diag.run(&mut circuit).unwrap_err();

    assert_eq!(err_no_diag.to_string(), err_diag.to_string());
}

// ----------------------------------------------------------------------------
// Test 12: Serialization validates against schema invariants
// ----------------------------------------------------------------------------
#[test]
fn test_12_serialization_validates_against_json_schema() {
    let mut circuit = build_bell_circuit();
    let diag_cfg = PassDiagnosticConfig::standard().with_verification(true);
    let mut pm = PassManager::new().with_diagnostic_config(diag_cfg);
    pm.add_pass(IdentityEliminationPass::default());
    pm.add_pass(AddGatesPass::new(2));

    let mut context = PassContext::new();
    let report = pm.run_with_context(&mut circuit, &mut context).unwrap();

    for diag in &report.pass_diagnostics {
        diag.validate_invariants().unwrap();
        let json_str = diag.to_json().unwrap();
        let parsed = PassDiagnosticReport::from_json(&json_str).unwrap();
        assert_eq!(diag, &parsed);
    }
}

// ----------------------------------------------------------------------------
// Test 13: Instrumentation overhead measurement
// ----------------------------------------------------------------------------
#[test]
fn test_13_instrumentation_overhead_measurement() {
    // Construct representative 100-gate circuit
    let mut b = CircuitBuilder::new("benchmark_100");
    let q = b.add_quantum_register("q", 4).unwrap();
    for _ in 0..25 {
        b.h(q[0]).unwrap();
        b.rz(q[1], ParameterExpr::from(0.125)).unwrap();
        b.cx(q[0], q[1]).unwrap();
        b.cx(q[2], q[3]).unwrap();
    }
    let circuit = b.finish().unwrap();
    let target = make_linear_target("benchmark_target_4q", 4);

    // 1. Measure disabled
    let cfg_disabled = CompilerConfig::o2();
    let pipe_disabled = CompilerPipeline::new(cfg_disabled);
    let t0 = Instant::now();
    for _ in 0..5 {
        pipe_disabled.compile(&circuit, &target).unwrap();
    }
    let elapsed_disabled = t0.elapsed();

    // 2. Measure standard diagnostics enabled (metrics + fingerprints)
    let cfg_enabled = CompilerConfig::o2().with_diagnostics(PassDiagnosticConfig::standard());
    let pipe_enabled = CompilerPipeline::new(cfg_enabled);
    let t1 = Instant::now();
    for _ in 0..5 {
        pipe_enabled.compile(&circuit, &target).unwrap();
    }
    let elapsed_enabled = t1.elapsed();

    println!(
        "Phase 72 Overhead Measurement (100 gates, 5 iterations): disabled={:?}, enabled={:?}",
        elapsed_disabled, elapsed_enabled
    );
}
