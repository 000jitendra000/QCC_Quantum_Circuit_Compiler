//! Phase 12 — Pass Manager Interface Integration Tests
//!
//! Validates the deterministic compiler-pass execution contract, analysis dependency
//! and invalidation lifecycles, semantic domains, post-pass debug verification,
//! and structured error propagation.

use std::sync::{Arc, Mutex};

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::opt::{
    AnalysisId, CompilerPass, PassContext, PassError, PassExecutionStatus, PassId, PassManager,
    PassManagerError, PassMutation, PassResult, PreservedAnalyses, SemanticDomain,
};
use qcc::quantum::Gate;

// ---------------------------------------------------------------------------
// Synthetic Test Analyses
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct OperationCountAnalysis {
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QubitActivityAnalysis {
    pub active_qubits: Vec<usize>,
}

const OP_COUNT_ANALYSIS: AnalysisId = AnalysisId::new("operation_count_analysis");
const QUBIT_ACTIVITY_ANALYSIS: AnalysisId = AnalysisId::new("qubit_activity_analysis");

// ---------------------------------------------------------------------------
// Synthetic Test Passes
// ---------------------------------------------------------------------------

/// No-op pass that makes no modifications and preserves all analyses.
struct NoOpPass {
    id: PassId,
}

impl NoOpPass {
    fn new(name: &'static str) -> Self {
        Self {
            id: PassId::new(name),
        }
    }
}

impl CompilerPass for NoOpPass {
    fn id(&self) -> PassId {
        self.id
    }

    fn description(&self) -> &'static str {
        "Synthetic no-op pass that leaves the circuit unchanged"
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::AnalysisOnly
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        Ok(PassResult::unmodified())
    }
}

/// Order-tracking pass that logs its execution into a shared vector.
struct OrderTrackingPass {
    id: PassId,
    log: Arc<Mutex<Vec<&'static str>>>,
}

impl OrderTrackingPass {
    fn new(name: &'static str, log: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            id: PassId::new(name),
            log,
        }
    }
}

impl CompilerPass for OrderTrackingPass {
    fn id(&self) -> PassId {
        self.id
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::AnalysisOnly
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        self.log.lock().unwrap().push(self.id.0);
        Ok(PassResult::unmodified())
    }
}

/// Pass that computes and caches `OperationCountAnalysis`.
struct CountAnalysisProducerPass;

impl CompilerPass for CountAnalysisProducerPass {
    fn id(&self) -> PassId {
        PassId::new("count_analysis_producer")
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::AnalysisOnly
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let count = circuit.num_operations();
        context.insert_analysis(OP_COUNT_ANALYSIS, OperationCountAnalysis { count });
        Ok(PassResult::unmodified())
    }
}

/// Pass that requires `OperationCountAnalysis` to be present and valid.
struct CountAnalysisConsumerPass {
    expected_count: Option<usize>,
}

impl CountAnalysisConsumerPass {
    fn new(expected: Option<usize>) -> Self {
        Self {
            expected_count: expected,
        }
    }
}

impl CompilerPass for CountAnalysisConsumerPass {
    fn id(&self) -> PassId {
        PassId::new("count_analysis_consumer")
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::AnalysisOnly
    }

    fn required_analyses(&self) -> Vec<AnalysisId> {
        vec![OP_COUNT_ANALYSIS]
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let analysis = context
            .get_analysis::<OperationCountAnalysis>(OP_COUNT_ANALYSIS)
            .expect("required analysis guaranteed by pass manager");

        if let Some(expected) = self.expected_count {
            assert_eq!(analysis.count, expected);
        }

        Ok(PassResult::unmodified())
    }
}

/// Mutating pass that adds a gate and declares all analyses invalidated.
struct MutatingPessimisticPass {
    id: PassId,
}

impl MutatingPessimisticPass {
    fn new(name: &'static str) -> Self {
        Self {
            id: PassId::new(name),
        }
    }
}

impl CompilerPass for MutatingPessimisticPass {
    fn id(&self) -> PassId {
        self.id
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::GeneralStructural
    }

    fn mutation_scope(&self) -> PassMutation {
        PassMutation::CircuitTopology
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let q0 = circuit.quantum_registers[0].qubit(0).unwrap();
        circuit
            .add_operation(Operation::gate(&Gate::H, vec![q0]))
            .map_err(|source| PassError::Ir {
                pass_id: self.id(),
                source,
            })?;

        Ok(PassResult::modified(
            "appended H gate, invalidating all analyses",
        ))
    }
}

/// Mutating pass that preserves a specific analysis.
struct MutatingSelectivePreservePass {
    preserve: AnalysisId,
}

impl CompilerPass for MutatingSelectivePreservePass {
    fn id(&self) -> PassId {
        PassId::new("mutating_selective_preserve")
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::GeneralStructural
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let q0 = circuit.quantum_registers[0].qubit(0).unwrap();
        circuit
            .add_operation(Operation::gate(&Gate::X, vec![q0]))
            .map_err(|source| PassError::Ir {
                pass_id: self.id(),
                source,
            })?;

        Ok(PassResult::new(
            true,
            PreservedAnalyses::preserve_one(self.preserve),
            format!(
                "mutated circuit while explicitly preserving {}",
                self.preserve
            ),
        ))
    }
}

/// Pass with custom failing precondition.
struct FailingPreconditionPass {
    reason: &'static str,
}

impl CompilerPass for FailingPreconditionPass {
    fn id(&self) -> PassId {
        PassId::new("failing_precondition")
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::GeneralStructural
    }

    fn check_preconditions(
        &self,
        _circuit: &Circuit,
        _context: &PassContext,
    ) -> Result<(), PassError> {
        Err(PassError::PreconditionViolation {
            pass_id: self.id(),
            reason: self.reason.to_string(),
        })
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        panic!("run() should never be reached when precondition fails");
    }
}

/// Pass that fails inside `run()`.
struct FailingExecutionPass {
    reason: &'static str,
}

impl CompilerPass for FailingExecutionPass {
    fn id(&self) -> PassId {
        PassId::new("failing_execution")
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
            reason: self.reason.to_string(),
        })
    }
}

/// Pass that violates declared postcondition.
struct ViolatingPostconditionPass;

impl CompilerPass for ViolatingPostconditionPass {
    fn id(&self) -> PassId {
        PassId::new("violating_postcondition")
    }

    fn semantic_domain(&self) -> SemanticDomain {
        SemanticDomain::GeneralStructural
    }

    fn run(
        &mut self,
        _circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        Ok(PassResult::unmodified())
    }

    fn check_postconditions(
        &self,
        _circuit: &Circuit,
        _context: &PassContext,
    ) -> Result<(), PassError> {
        Err(PassError::PostconditionViolation {
            pass_id: self.id(),
            reason: "synthetic postcondition assertion failed".to_string(),
        })
    }
}

/// Helper to build a clean 2-qubit Bell circuit.
fn create_test_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("test_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.finish().unwrap()
}

// ---------------------------------------------------------------------------
// 14 Mandatory Test Cases
// ---------------------------------------------------------------------------

#[test]
fn test_01_noop_pass_executes_successfully() {
    let mut circuit = create_test_circuit();
    let initial_ops = circuit.num_operations();

    let mut pm = PassManager::new();
    pm.add_pass(NoOpPass::new("noop_test"));

    let report = pm.run(&mut circuit).expect("pass execution should succeed");

    assert!(report.is_success());
    assert_eq!(report.total_passes_executed, 1);
    assert_eq!(report.total_passes_modified, 0);
    assert_eq!(report.total_delta_operations, 0);
    assert_eq!(circuit.num_operations(), initial_ops);

    let step = &report.trace.steps[0];
    assert_eq!(step.pass_id, PassId::new("noop_test"));
    assert_eq!(step.status, PassExecutionStatus::Success);
    assert!(!step.modified);
}

#[test]
fn test_02_multiple_passes_execute_in_registered_order() {
    let mut circuit = create_test_circuit();
    let log = Arc::new(Mutex::new(Vec::new()));

    let mut pm = PassManager::new();
    pm.add_pass(OrderTrackingPass::new("pass_alpha", Arc::clone(&log)));
    pm.add_pass(OrderTrackingPass::new("pass_beta", Arc::clone(&log)));
    pm.add_pass(OrderTrackingPass::new("pass_gamma", Arc::clone(&log)));

    let report = pm.run(&mut circuit).expect("pipeline should succeed");

    assert_eq!(report.total_passes_executed, 3);
    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded, vec!["pass_alpha", "pass_beta", "pass_gamma"]);

    assert_eq!(report.trace.steps[0].pass_id, PassId::new("pass_alpha"));
    assert_eq!(report.trace.steps[1].pass_id, PassId::new("pass_beta"));
    assert_eq!(report.trace.steps[2].pass_id, PassId::new("pass_gamma"));
}

#[test]
fn test_03_pass_identity_and_trace_deterministic_across_runs() {
    let mut circuit1 = create_test_circuit();
    let mut circuit2 = create_test_circuit();

    let mut pm1 = PassManager::new();
    pm1.add_pass(NoOpPass::new("step1"));
    pm1.add_pass(MutatingPessimisticPass::new("step2"));
    pm1.add_pass(NoOpPass::new("step3"));

    let mut pm2 = PassManager::new();
    pm2.add_pass(NoOpPass::new("step1"));
    pm2.add_pass(MutatingPessimisticPass::new("step2"));
    pm2.add_pass(NoOpPass::new("step3"));

    let report1 = pm1.run(&mut circuit1).unwrap();
    let report2 = pm2.run(&mut circuit2).unwrap();

    assert!(report1.trace.is_identical(&report2.trace));
    assert_eq!(circuit1.num_operations(), circuit2.num_operations());
}

#[test]
fn test_04_required_analysis_availability_enforced() {
    let mut circuit = create_test_circuit();
    let mut pm = PassManager::new();

    // Pass requires OP_COUNT_ANALYSIS, but context is empty
    pm.add_pass(CountAnalysisConsumerPass::new(None));

    let result = pm.run(&mut circuit);
    match result {
        Err(PassManagerError::MissingRequiredAnalysis {
            pass_id,
            analysis_id,
        }) => {
            assert_eq!(pass_id, PassId::new("count_analysis_consumer"));
            assert_eq!(analysis_id, OP_COUNT_ANALYSIS);
        }
        res => panic!("Expected MissingRequiredAnalysis, got: {res:?}"),
    }
}

#[test]
fn test_05_preserved_analyses_remain_available() {
    let mut circuit = create_test_circuit();
    let mut context = PassContext::new();

    context.insert_analysis(
        QUBIT_ACTIVITY_ANALYSIS,
        QubitActivityAnalysis {
            active_qubits: vec![0, 1],
        },
    );

    let mut pm = PassManager::new();
    // Pass explicitly preserves QUBIT_ACTIVITY_ANALYSIS while mutating
    pm.add_pass(MutatingSelectivePreservePass {
        preserve: QUBIT_ACTIVITY_ANALYSIS,
    });

    pm.run_with_context(&mut circuit, &mut context)
        .expect("pipeline should succeed");

    assert!(context.has_analysis(&QUBIT_ACTIVITY_ANALYSIS));
    let analysis = context
        .get_analysis::<QubitActivityAnalysis>(QUBIT_ACTIVITY_ANALYSIS)
        .expect("analysis should still be cached");
    assert_eq!(analysis.active_qubits, vec![0, 1]);
}

#[test]
fn test_06_invalidated_analyses_not_reused() {
    let mut circuit = create_test_circuit();
    let mut context = PassContext::new();

    context.insert_analysis(OP_COUNT_ANALYSIS, OperationCountAnalysis { count: 2 });
    assert!(context.has_analysis(&OP_COUNT_ANALYSIS));

    let mut pm = PassManager::new();
    // Pessimistic pass purges all analyses
    pm.add_pass(MutatingPessimisticPass::new("mutator"));

    pm.run_with_context(&mut circuit, &mut context).unwrap();

    assert!(
        !context.has_analysis(&OP_COUNT_ANALYSIS),
        "Pessimistic mutating pass must purge non-preserved analysis"
    );
}

#[test]
fn test_07_later_pass_cannot_consume_stale_cached_analysis() {
    let mut circuit = create_test_circuit();
    let mut pm = PassManager::new();

    // 1. Producer caches OP_COUNT_ANALYSIS = 2
    pm.add_pass(CountAnalysisProducerPass);
    // 2. Mutator mutates circuit and purges all analyses
    pm.add_pass(MutatingPessimisticPass::new("mutator"));
    // 3. Consumer requires OP_COUNT_ANALYSIS -> should fail because step 2 invalidated it
    pm.add_pass(CountAnalysisConsumerPass::new(Some(2)));

    let result = pm.run(&mut circuit);
    match result {
        Err(PassManagerError::MissingRequiredAnalysis {
            pass_id,
            analysis_id,
        }) => {
            assert_eq!(pass_id, PassId::new("count_analysis_consumer"));
            assert_eq!(analysis_id, OP_COUNT_ANALYSIS);
        }
        res => panic!("Expected MissingRequiredAnalysis for stale analysis, got: {res:?}"),
    }
}

#[test]
fn test_08_preconditions_checked() {
    let mut circuit = create_test_circuit();
    let mut pm = PassManager::new();

    pm.add_pass(FailingPreconditionPass {
        reason: "hardware connectivity violated",
    });

    let result = pm.run(&mut circuit);
    match result {
        Err(PassManagerError::PreconditionFailed {
            step_index,
            pass_id,
            error,
        }) => {
            assert_eq!(step_index, 0);
            assert_eq!(pass_id, PassId::new("failing_precondition"));
            assert!(error.to_string().contains("hardware connectivity violated"));
        }
        res => panic!("Expected PreconditionFailed, got: {res:?}"),
    }

    // Also test UnitaryOnly domain precondition rejection on circuit with measurements
    let mut builder = CircuitBuilder::new("measured_circuit");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    let mut measured_circuit = builder.finish().unwrap();

    struct UnitaryOnlyPass;
    impl CompilerPass for UnitaryOnlyPass {
        fn id(&self) -> PassId {
            PassId::new("unitary_only_pass")
        }
        fn semantic_domain(&self) -> SemanticDomain {
            SemanticDomain::UnitaryOnly
        }
        fn run(&mut self, _: &mut Circuit, _: &mut PassContext) -> Result<PassResult, PassError> {
            Ok(PassResult::unmodified())
        }
    }

    let mut pm_unitary = PassManager::new();
    pm_unitary.add_pass(UnitaryOnlyPass);
    let result_domain = pm_unitary.run(&mut measured_circuit);
    assert!(matches!(
        result_domain,
        Err(PassManagerError::PreconditionFailed { .. })
    ));
}

#[test]
fn test_09_postconditions_and_debug_verification_checked() {
    let mut circuit = create_test_circuit();
    let mut pm = PassManager::new().with_debug_verification(true);

    pm.add_pass(ViolatingPostconditionPass);

    let result = pm.run(&mut circuit);
    match result {
        Err(PassManagerError::DebugVerificationFailed {
            step_index,
            pass_id,
            error,
        }) => {
            assert_eq!(step_index, 0);
            assert_eq!(pass_id, PassId::new("violating_postcondition"));
            assert!(error
                .to_string()
                .contains("synthetic postcondition assertion failed"));
        }
        res => panic!("Expected DebugVerificationFailed, got: {res:?}"),
    }
}

#[test]
fn test_10_failing_pass_propagates_structured_error() {
    let mut circuit = create_test_circuit();
    let mut pm = PassManager::new();

    pm.add_pass(FailingExecutionPass {
        reason: "matrix inversion singular",
    });

    let result = pm.run(&mut circuit);
    match result {
        Err(PassManagerError::PassFailed {
            step_index,
            pass_id,
            error,
        }) => {
            assert_eq!(step_index, 0);
            assert_eq!(pass_id, PassId::new("failing_execution"));
            assert!(error.to_string().contains("matrix inversion singular"));
        }
        res => panic!("Expected PassFailed error, got: {res:?}"),
    }
}

#[test]
fn test_11_pipeline_execution_stops_after_failed_pass() {
    let mut circuit = create_test_circuit();
    let log = Arc::new(Mutex::new(Vec::new()));

    let mut pm = PassManager::new();
    pm.add_pass(OrderTrackingPass::new("before_failure", Arc::clone(&log)));
    pm.add_pass(FailingExecutionPass {
        reason: "intentional abort",
    });
    pm.add_pass(OrderTrackingPass::new("after_failure", Arc::clone(&log)));

    let result = pm.run(&mut circuit);
    assert!(result.is_err());

    let executed = log.lock().unwrap().clone();
    assert_eq!(executed, vec!["before_failure"]);
    assert!(
        !executed.contains(&"after_failure"),
        "Pipeline must halt immediately upon pass failure"
    );
}

#[test]
fn test_12_statistics_correctly_describe_pass_effects() {
    let mut circuit = create_test_circuit();
    let initial_ops = circuit.num_operations();

    let mut pm = PassManager::new();
    pm.add_pass(NoOpPass::new("pass_noop"));
    pm.add_pass(MutatingPessimisticPass::new("pass_add_h"));

    let report = pm.run(&mut circuit).unwrap();

    assert_eq!(report.statistics.len(), 2);

    let stat0 = &report.statistics[0];
    assert_eq!(stat0.pass_id, PassId::new("pass_noop"));
    assert_eq!(stat0.execution_order, 0);
    assert!(!stat0.modified);
    assert_eq!(stat0.initial_operations, initial_ops);
    assert_eq!(stat0.final_operations, initial_ops);
    assert_eq!(stat0.delta_operations, 0);

    let stat1 = &report.statistics[1];
    assert_eq!(stat1.pass_id, PassId::new("pass_add_h"));
    assert_eq!(stat1.execution_order, 1);
    assert!(stat1.modified);
    assert_eq!(stat1.initial_operations, initial_ops);
    assert_eq!(stat1.final_operations, initial_ops + 1);
    assert_eq!(stat1.delta_operations, 1);
    assert_eq!(report.total_delta_operations, 1);
    assert_eq!(report.total_passes_modified, 1);
}

#[test]
fn test_13_analysis_only_passes_do_not_invalidate_unrelated_analyses() {
    let mut circuit = create_test_circuit();
    let mut context = PassContext::new();

    context.insert_analysis(OP_COUNT_ANALYSIS, OperationCountAnalysis { count: 2 });
    context.insert_analysis(
        QUBIT_ACTIVITY_ANALYSIS,
        QubitActivityAnalysis {
            active_qubits: vec![0, 1],
        },
    );

    let mut pm = PassManager::new();
    pm.add_pass(NoOpPass::new("analysis_only_pass"));

    pm.run_with_context(&mut circuit, &mut context).unwrap();

    assert!(context.has_analysis(&OP_COUNT_ANALYSIS));
    assert!(context.has_analysis(&QUBIT_ACTIVITY_ANALYSIS));
}

#[test]
fn test_14_repeated_executions_produce_consistent_traces_and_results() {
    let base_circuit = create_test_circuit();

    let mut traces = Vec::new();

    for _ in 0..10 {
        let mut circuit = base_circuit.clone();
        let mut pm = PassManager::new();
        pm.add_pass(CountAnalysisProducerPass);
        pm.add_pass(CountAnalysisConsumerPass::new(Some(2)));
        pm.add_pass(MutatingPessimisticPass::new("mutate_1"));
        pm.add_pass(NoOpPass::new("noop_end"));

        let report = pm.run(&mut circuit).unwrap();
        traces.push(report.trace);
    }

    for window in traces.windows(2) {
        assert!(
            window[0].is_identical(&window[1]),
            "Repeated pass pipeline runs must produce bitwise-identical deterministic traces"
        );
    }
}
