//! Integration tests for Phase 35: Optimizer Integration and Reports.
//!
//! Validates:
//! 1. Repeatability: identical inputs yield identical optimized circuits and deterministic reports.
//! 2. Per-pass equivalence: verified against Phase 10 checker under common global phase.
//! 3. No-op pipeline: already-simplified circuits remain unchanged.
//! 4. Metric consistency: operations, gates, depth, and global phase agree with resulting circuits.
//! 5. Optimization levels: O0, O1, O2, O3 differences in enabled passes and behavior.
//! 6. Analysis invalidation: cache invalidation and recomputation of critical path depth.
//! 7. Graph validation: post-pass QccIR structural integrity assertions.
//! 8. Termination policy: fixed-point convergence vs iteration budget exhaustion.
//! 9. Regression corpus: interactions among identity elimination, inverse cancellation,
//!    rotation merging, Euler decomposition/fusion, and commutation reordering.
//! 10. Non-unitary circuits: preservation of measurement, reset, barrier, and classical dependencies.

use std::collections::HashMap;

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::ClassicalBitId;
use qcc::opt::context::{AnalysisId, PassContext};
use qcc::optimizer::config::{OptimizationLevel, OptimizerConfig};
use qcc::optimizer::pipeline::OptimizerPipeline;
use qcc::optimizer::report::{compute_circuit_depth, PipelineTermination};
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Repeatability and Determinism Tests
// ============================================================================

#[test]
fn test_repeatability_identical_output_and_report() {
    let make_circuit = || {
        let mut builder = CircuitBuilder::new("repeatability_circ");
        let q = builder.add_quantum_register("q", 3).unwrap();

        let pi_4 = ParameterExpr::from(std::f64::consts::PI / 4.0);
        let neg_pi_4 = ParameterExpr::from(-std::f64::consts::PI / 4.0);

        builder.rz(q[0], pi_4.clone()).unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder.rz(q[0], neg_pi_4).unwrap();
        builder.h(q[2]).unwrap();
        builder.h(q[2]).unwrap(); // Cancelling pair on q2
        builder.finish().unwrap()
    };

    let pipeline = OptimizerPipeline::for_level(OptimizationLevel::O3);

    let mut circuit1 = make_circuit();
    let report1 = pipeline.optimize(&mut circuit1).unwrap();

    let mut circuit2 = make_circuit();
    let report2 = pipeline.optimize(&mut circuit2).unwrap();

    // 1. Resulting circuit operations must match exactly
    let ops1: Vec<String> = circuit1
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation.to_string())
        .collect();
    let ops2: Vec<String> = circuit2
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation.to_string())
        .collect();
    assert_eq!(ops1, ops2);

    // 2. Deterministic string outputs must be bit-exact
    assert_eq!(
        report1.to_deterministic_string(),
        report2.to_deterministic_string()
    );
    assert_eq!(report1.iterations_executed, report2.iterations_executed);
    assert_eq!(report1.termination, report2.termination);
    assert_eq!(report1.total_passes_modified, report2.total_passes_modified);
    assert_eq!(report1.total_passes_executed, report2.total_passes_executed);
}

// ============================================================================
// 2. Per-Pass & Overall Unitary Equivalence Tests
// ============================================================================

#[test]
fn test_per_pass_and_overall_unitary_equivalence() {
    let mut builder = CircuitBuilder::new("equiv_test");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_6 = ParameterExpr::from(std::f64::consts::PI / 6.0);
    let pi_3 = ParameterExpr::from(std::f64::consts::PI / 3.0);

    // Rotation merge + inverse cancellation + commutation reorder
    builder.rz(q[0], pi_6.clone()).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], pi_3).unwrap();
    builder.h(q[1]).unwrap();
    builder.h(q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    // Run pipeline with equivalence verification enabled
    let config = OptimizerConfig::new(OptimizationLevel::O3)
        .with_equivalence_verification(true)
        .with_graph_verification(true);
    let pipeline = OptimizerPipeline::new(config);

    let report = pipeline.optimize(&mut circuit).unwrap();

    assert_eq!(report.unitary_equivalence_verified, Some(true));
    for pass_rep in &report.passes {
        if pass_rep.modified {
            assert_eq!(
                pass_rep.equivalence_verified,
                Some(true),
                "Pass {} modified circuit but equivalence was not verified",
                pass_rep.pass_id
            );
        }
    }

    // Verify against Phase 10 checker independently
    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. No-Op Pipeline Behavior
// ============================================================================

#[test]
fn test_noop_pipeline_on_already_simplified_circuit() {
    let mut builder = CircuitBuilder::new("already_simplified");
    let q = builder.add_quantum_register("q", 2).unwrap();

    // Circuit cannot be simplified further by logical passes
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    let pipeline = OptimizerPipeline::for_level(OptimizationLevel::O2);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Circuit should be unmodified
    assert_eq!(circuit.num_operations(), original.num_operations());
    assert_eq!(report.total_passes_modified, 0);
    assert_eq!(report.delta_operations(), 0);
    assert_eq!(report.delta_depth(), 0);
    assert_eq!(report.termination, PipelineTermination::Converged);
    assert_eq!(report.iterations_executed, 1);
}

// ============================================================================
// 4. Metric Consistency Tests
// ============================================================================

#[test]
fn test_metric_consistency_agrees_with_circuit() {
    let mut builder = CircuitBuilder::new("metric_check");
    let q = builder.add_quantum_register("q", 3).unwrap();

    let pi_2 = ParameterExpr::from(std::f64::consts::PI / 2.0);

    // Initial: 6 gates, depth: 3
    builder.h(q[0]).unwrap();
    builder.h(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], pi_2.clone()).unwrap();
    builder.rz(q[0], pi_2).unwrap(); // merges to Rz(pi)
    builder.cx(q[1], q[2]).unwrap();

    let mut circuit = builder.finish().unwrap();

    let config = OptimizerConfig::new(OptimizationLevel::O2);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Check final metrics match circuit directly
    let direct_metrics = qcc::optimizer::CircuitMetrics::compute(&circuit);
    assert_eq!(report.final_metrics, direct_metrics);

    // Check circuit depth definition consistency
    let direct_depth = compute_circuit_depth(&circuit);
    assert_eq!(report.final_metrics.circuit_depth, direct_depth);

    // Check operation count consistency
    assert_eq!(
        report.final_metrics.total_operations,
        circuit.num_operations()
    );
    assert_eq!(
        report.final_metrics.gate_count,
        circuit
            .graph
            .nodes()
            .filter(|n| matches!(n.operation, Operation::Gate { .. }))
            .count()
    );
}

// ============================================================================
// 5. Optimization Levels Configuration (O0, O1, O2, O3)
// ============================================================================

#[test]
fn test_optimization_levels_differentiation() {
    let make_circuit = || {
        let mut builder = CircuitBuilder::new("levels_test");
        let q = builder.add_quantum_register("q", 2).unwrap();

        let pi_4 = ParameterExpr::from(std::f64::consts::PI / 4.0);

        // Cancellation opportunity
        builder.h(q[0]).unwrap();
        builder.h(q[0]).unwrap();

        // Rotation merge opportunity
        builder.rz(q[1], pi_4.clone()).unwrap();
        builder.rz(q[1], pi_4).unwrap();

        // Commutation reorder opportunity: Rz on control across CX
        builder
            .rz(q[0], ParameterExpr::from(std::f64::consts::PI / 3.0))
            .unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder
            .rz(q[0], ParameterExpr::from(-std::f64::consts::PI / 3.0))
            .unwrap();

        builder.finish().unwrap()
    };

    // 1. Level O0: Zero modifications
    {
        let mut c0 = make_circuit();
        let report0 = OptimizerPipeline::for_level(OptimizationLevel::O0)
            .optimize(&mut c0)
            .unwrap();
        assert_eq!(report0.total_passes_modified, 0);
        assert_eq!(c0.num_operations(), 7);
        assert_eq!(report0.termination, PipelineTermination::NoOp);
    }

    // 2. Level O1: Cancels H-H, but does not merge Rz-Rz or reorder across CX
    {
        let mut c1 = make_circuit();
        let report1 = OptimizerPipeline::for_level(OptimizationLevel::O1)
            .optimize(&mut c1)
            .unwrap();
        assert_eq!(report1.total_passes_modified, 1);
        assert_eq!(c1.num_operations(), 5); // 7 - 2 (H-H) = 5
        assert_eq!(
            report1.termination,
            PipelineTermination::SinglePassCompleted
        );
    }

    // 3. Level O2: Cancels H-H, merges Rz-Rz, but does NOT reorder Rz across CX
    {
        let mut c2 = make_circuit();
        let report2 = OptimizerPipeline::for_level(OptimizationLevel::O2)
            .optimize(&mut c2)
            .unwrap();
        // H-H cancelled (-2), Rz-Rz merged (-1) -> 4 ops remaining
        assert_eq!(c2.num_operations(), 4);
        assert_eq!(report2.termination, PipelineTermination::Converged);
    }

    // 4. Level O3: Reorders Rz across CX to expose cancellation, cancels H-H, merges Rz-Rz
    {
        let mut c3 = make_circuit();
        let report3 = OptimizerPipeline::for_level(OptimizationLevel::O3)
            .optimize(&mut c3)
            .unwrap();
        // H-H cancelled (-2), Rz-Rz merged (-1), Rz-CX-Rz reordered and cancelled (-2) -> 2 ops remaining: CX and Rz
        assert_eq!(c3.num_operations(), 2);
        assert_eq!(report3.termination, PipelineTermination::Converged);
    }
}

// ============================================================================
// 6. Analysis Invalidation Across Modifying Passes
// ============================================================================

#[test]
fn test_analysis_invalidation_lifecycle() {
    let mut builder = CircuitBuilder::new("analysis_inv");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap(); // will be eliminated by inverse cancellation

    let mut circuit = builder.finish().unwrap();

    let synthetic_analysis = AnalysisId::new("synthetic_analysis");

    let mut context = PassContext::new();
    context.insert_analysis(synthetic_analysis, 42usize);
    assert!(context.has_analysis(&synthetic_analysis));

    let pipeline = OptimizerPipeline::for_level(OptimizationLevel::O1);
    let report = pipeline
        .optimize_with_context(&mut circuit, &mut context)
        .unwrap();

    assert!(report.total_passes_modified > 0);
    // After a modifying pass, unpreserved analyses must be invalidated
    assert!(
        !context.has_analysis(&synthetic_analysis),
        "Modifying pass must invalidate unpreserved analyses"
    );
    // But CIRCUIT_DEPTH is re-inserted and maintained by the pipeline
    assert!(context.has_analysis(&AnalysisId::CIRCUIT_DEPTH));
}

// ============================================================================
// 7. Graph Validation After Every Pass
// ============================================================================

#[test]
fn test_graph_validation_after_every_pass() {
    let mut builder = CircuitBuilder::new("graph_val");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi = ParameterExpr::from(std::f64::consts::PI);
    builder.rx(q[0], pi.clone()).unwrap();
    builder.rx(q[0], pi).unwrap(); // merges to 2pi -> eliminated with +pi global phase

    let mut circuit = builder.finish().unwrap();

    let config = OptimizerConfig::new(OptimizationLevel::O2).with_graph_verification(true);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    assert!(report.total_passes_modified > 0);
    assert!(circuit.validate().is_ok());
    assert!(!circuit.graph.is_cyclic());

    let nodes = circuit.graph.ordered_nodes().unwrap();
    for (i, node) in nodes.iter().enumerate() {
        assert_eq!(node.sequence_number, (i + 1) as u64);
    }
}

// ============================================================================
// 8. Termination and Iteration-Budget Behavior
// ============================================================================

#[test]
fn test_iteration_budget_exhaustion() {
    let mut builder = CircuitBuilder::new("iter_budget");
    let q = builder.add_quantum_register("q", 3).unwrap();

    // Create circuit with multi-step cascades
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();

    // Set max_iterations = 1 on O2
    let config = OptimizerConfig::new(OptimizationLevel::O3).with_max_iterations(1);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    assert_eq!(report.iterations_executed, 1);
    assert_eq!(
        report.termination,
        PipelineTermination::IterationLimitReached
    );
}

// ============================================================================
// 9. Regression Corpus
// ============================================================================

#[test]
fn test_regression_corpus_euler_fusion_and_cancellation() {
    // Sequence of single-qubit gates that fuse into an identity or near-identity
    let mut builder = CircuitBuilder::new("fusion_cancellation");
    let q = builder.add_quantum_register("q", 1).unwrap();

    builder.x(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.y(q[0]).unwrap(); // X * Z * Y = -i I (global phase only)

    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    let config = OptimizerConfig::new(OptimizationLevel::O2)
        .with_equivalence_verification(true)
        .with_one_qubit_fusion(true);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Fused and identity eliminated: zero gates remain
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(report.final_metrics.gate_count, 0);

    // Global phase must match
    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_regression_corpus_two_pi_rotation_merging_and_elimination() {
    // Two pi rotations on Rx merge to 2pi, which eliminates to identity with +pi phase
    let mut builder = CircuitBuilder::new("two_pi_merge_elim");
    let q = builder.add_quantum_register("q", 1).unwrap();

    let pi = ParameterExpr::from(std::f64::consts::PI);
    builder.rx(q[0], pi.clone()).unwrap();
    builder.rx(q[0], pi).unwrap();

    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    let config = OptimizerConfig::new(OptimizationLevel::O2).with_equivalence_verification(true);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    assert_eq!(circuit.num_operations(), 0);
    assert!(report.phase_changed());

    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_regression_corpus_symbolic_expressions() {
    let mut builder = CircuitBuilder::new("symbolic_pipeline");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let theta = ParameterExpr::symbol("theta");
    let neg_theta = -theta.clone();

    builder.rz(q[0], theta).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], neg_theta).unwrap();

    let mut circuit = builder.finish().unwrap();

    // With symbol_env binding theta = 1.0
    let mut env = HashMap::new();
    env.insert("theta".to_string(), 1.0);

    let config = OptimizerConfig::new(OptimizationLevel::O3)
        .with_symbol_env(env)
        .with_equivalence_verification(false);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Under O3 with symbol env, the symbolic rotations reorder and cancel
    assert_eq!(circuit.num_operations(), 1);
    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "cx");
    assert_eq!(report.termination, PipelineTermination::Converged);
}

// ============================================================================
// 10. Non-Unitary Circuits Preservation
// ============================================================================

#[test]
fn test_non_unitary_circuit_bell_state_with_measurement() {
    // Milestone M1 Bell state with terminal measurement
    let mut builder = CircuitBuilder::new("bell_meas");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap(); // Cancels to identity
    builder.h(q[0]).unwrap(); // Leaves single H
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 6);

    let config = OptimizerConfig::new(OptimizationLevel::O2).with_equivalence_verification(true);
    let pipeline = OptimizerPipeline::new(config);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // H-H cancelled, leaving H, CX, and 2 measurements
    assert_eq!(circuit.num_operations(), 4);
    assert_eq!(report.final_metrics.non_unitary_operations, 2);
    // Unitary equivalence is cleanly skipped and reported as None for non-unitary circuit
    assert_eq!(report.unitary_equivalence_verified, None);

    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "h");
    assert_eq!(nodes[1].operation.name(), "cx");
    assert_eq!(nodes[2].operation.name(), "measure");
    assert_eq!(nodes[3].operation.name(), "measure");
}

#[test]
fn test_non_unitary_circuit_with_mid_circuit_reset_and_barrier() {
    let mut builder = CircuitBuilder::new("mid_reset_barrier");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_4 = ParameterExpr::from(std::f64::consts::PI / 4.0);

    builder.rz(q[0], pi_4.clone()).unwrap();
    builder.rz(q[0], pi_4).unwrap(); // merges to Rz(pi/2)
    builder.reset(q[0]).unwrap(); // reset must block further merging
    builder.barrier(&[q[0], q[1]]).unwrap(); // barrier must block
    builder.x(q[1]).unwrap();
    builder.x(q[1]).unwrap(); // cancels to identity

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 6);

    let pipeline = OptimizerPipeline::for_level(OptimizationLevel::O2);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Rz merged (-1), X-X cancelled (-2) -> 3 operations remain: Rz, Reset, Barrier
    assert_eq!(circuit.num_operations(), 3);
    assert_eq!(report.final_metrics.non_unitary_operations, 2);

    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "rz");
    assert_eq!(nodes[1].operation.name(), "reset");
    assert_eq!(nodes[2].operation.name(), "barrier");
}

#[test]
fn test_non_unitary_circuit_with_classical_hazard_ordering() {
    let mut circuit = Circuit::new("classical_hazard_pipeline");
    circuit.add_classical_register("c", 2).unwrap();
    let c0 = ClassicalBitId(0);

    let op1 = Operation::classical("write_c0", vec![], vec![c0]);
    let op2 = Operation::classical("read_c0", vec![c0], vec![]);
    circuit.add_operation(op1).unwrap();
    circuit.add_operation(op2).unwrap();

    let pipeline = OptimizerPipeline::for_level(OptimizationLevel::O3);
    let report = pipeline.optimize(&mut circuit).unwrap();

    // Classical hazard operations must remain untouched
    assert_eq!(circuit.num_operations(), 2);
    assert_eq!(report.total_passes_modified, 0);
    assert_eq!(report.final_metrics.non_unitary_operations, 2);
}
