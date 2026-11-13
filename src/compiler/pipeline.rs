//! The integrated end-to-end compiler pipeline implementation.
//!
//! Connects logical circuit optimization, target basis translation, initial placement,
//! circuit routing, final target-specific translation, final target legality validation,
//! post-routing physical scheduling, and layout-aware semantic verification.

use std::collections::BTreeMap;
use std::time::Instant;

use thiserror::Error;

use crate::compiler::config::{CompilerConfig, RouterChoice, SchedulerChoice};
use crate::compiler::result::{
    CompilationReport, CompilationResult, CompilerConfigSummary, LegalityStageReport,
    OptimizationStageReport, PipelineMetricsSummary, PostRoutingTranslationReport,
    RoutingStageReport, SchedulingStageReport, VerificationStageReport, VerificationStatus,
};
use crate::ir::circuit::Circuit;
use crate::opt::context::PassContext;
use crate::opt::pass::{CompilerPass, PassId};
use crate::optimizer::config::OptimizationLevel;
use crate::optimizer::passes::RotationMergePass;
use crate::optimizer::pipeline::{is_unitary_eligible, OptimizerError, OptimizerPipeline};
use crate::optimizer::report::CircuitMetrics;
use crate::routing::greedy::BaselineRouter;
use crate::routing::layout_map::LayoutError;
use crate::routing::pathfinding::RoutingError;
use crate::routing::placement::PlacementError;
use crate::routing::sabre::SabreBidirectionalRouter;
use crate::scheduler::alap::AlapScheduler;
use crate::scheduler::asap::AsapScheduler;
use crate::scheduler::resource_constraints::{
    ResourceConstrainedScheduler, ResourceConstraintManager,
};
use crate::scheduler::validator::ScheduleValidator;
use crate::scheduler::ScheduleError;
use crate::target::connectivity::OperationConnectivity;
use crate::target::direction::DirectionError;
use crate::target::legality::{TargetLegalityError, TargetLegalityValidator};
use crate::target::model::Target;
use crate::translation::direction::TwoQubitDirectionCorrector;
use crate::translation::engine::BasisTranslationEngine;
use crate::translation::TranslationError;
use crate::verification::layout_equivalence::LayoutEquivalenceChecker;
use crate::verification::VerificationError;

/// Structured error produced during end-to-end compilation.
#[derive(Debug, Error)]
pub enum CompilerError {
    /// Input circuit or target model failed structural invariant validation.
    #[error("input validation failed: {0}")]
    InvalidInput(String),

    /// Logical optimization stage failed.
    #[error("logical optimization failed: {0}")]
    Optimization(#[from] OptimizerError),

    /// Pre-routing or post-routing basis translation failed.
    #[error("basis translation failed: {0}")]
    BasisTranslation(#[from] TranslationError),

    /// Initial placement failed.
    #[error("initial placement failed: {0}")]
    Placement(#[from] PlacementError),

    /// Circuit routing failed.
    #[error("circuit routing failed: {0}")]
    Routing(#[from] RoutingError),

    /// Layout representation or invariant failure.
    #[error("layout error: {0}")]
    Layout(#[from] LayoutError),

    /// Post-routing two-qubit direction correction failed.
    #[error("post-routing direction correction failed: {0}")]
    DirectionCorrection(#[from] DirectionError),

    /// Final target legality validation failed.
    #[error("final target legality validation failed: {0}")]
    TargetLegality(#[from] TargetLegalityError),

    /// Post-routing physical scheduling failed.
    #[error("post-routing scheduling failed: {0}")]
    Scheduling(#[from] ScheduleError),

    /// Post-routing schedule validation failed against hardware constraints.
    #[error("schedule validation failed: {0}")]
    ScheduleValidation(String),

    /// Layout-aware semantic verification engine failed.
    #[error("semantic verification engine error: {0}")]
    Verification(#[from] VerificationError),

    /// Circuit failed layout-aware unitary equivalence against the logical reference.
    #[error("semantic equivalence verification failed: {reason}")]
    SemanticEquivalenceFailed {
        /// Detailed mismatch reason.
        reason: String,
    },
}

/// The integrated QCC end-to-end compilation pipeline.
#[derive(Clone, Debug)]
pub struct CompilerPipeline {
    config: CompilerConfig,
}

impl Default for CompilerPipeline {
    fn default() -> Self {
        Self::new(CompilerConfig::default())
    }
}

impl CompilerPipeline {
    /// Creates a new compilation pipeline with the given configuration.
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    /// Creates a compilation pipeline for a given optimization level with standard settings.
    pub fn for_level(level: OptimizationLevel) -> Self {
        Self::new(CompilerConfig::for_level(level))
    }

    /// Returns a reference to the active pipeline configuration.
    #[inline]
    pub fn config(&self) -> &CompilerConfig {
        &self.config
    }

    /// Returns a mutable reference to the active pipeline configuration.
    #[inline]
    pub fn config_mut(&mut self) -> &mut CompilerConfig {
        &mut self.config
    }

    /// Compiles a logical [`Circuit`] for a hardware [`Target`].
    ///
    /// Executes the full sequence of compilation stages:
    /// 1. Logical circuit optimization.
    /// 2. Target basis translation into the device's native entangling and single-qubit basis.
    /// 3. Physical placement and routing along coupling edges with layout tracking.
    /// 4. Post-routing direction correction and final target translation.
    /// 5. Final target legality validation.
    /// 6. Optional post-routing physical scheduling (ASAP, ALAP, or resource-constrained).
    /// 7. Layout-aware semantic verification enforcing ADR-0001 strict ancilla zero-state policies.
    pub fn compile(
        &self,
        circuit: &Circuit,
        target: &Target,
    ) -> Result<CompilationResult, CompilerError> {
        let wall_start = Instant::now();

        // --------------------------------------------------------------------
        // 0. Pre-Flight Validation
        // --------------------------------------------------------------------
        circuit
            .validate()
            .map_err(|e| CompilerError::InvalidInput(format!("input circuit invalid: {e}")))?;
        target
            .validate()
            .map_err(|e| CompilerError::InvalidInput(format!("target model invalid: {e}")))?;

        let num_logical = circuit.num_qubits();
        let num_physical = target.num_qubits();

        if num_physical < num_logical {
            return Err(CompilerError::Routing(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            }));
        }

        let initial_gate_count = circuit.num_operations();
        let initial_depth = CircuitMetrics::compute(circuit).circuit_depth;
        let original_swaps = count_circuit_swaps(circuit);

        // --------------------------------------------------------------------
        // 1. Logical Circuit Optimization
        // --------------------------------------------------------------------
        let (opt_circuit, opt_report) = if self.config.optimization.level == OptimizationLevel::O0 {
            if self.config.diagnostics.enabled {
                let mut opt_cfg = self.config.optimization.clone();
                opt_cfg.diagnostics = self.config.diagnostics.clone();
                let opt_pipeline = OptimizerPipeline::new(opt_cfg);
                let mut opt_c = circuit.clone();
                let report = opt_pipeline.optimize(&mut opt_c)?;
                (opt_c, Some(report))
            } else {
                (circuit.clone(), None)
            }
        } else {
            let mut opt_cfg = self.config.optimization.clone();
            if self.config.diagnostics.enabled {
                opt_cfg.diagnostics = self.config.diagnostics.clone();
            }
            let opt_pipeline = OptimizerPipeline::new(opt_cfg);
            let mut opt_c = circuit.clone();
            let report = opt_pipeline.optimize(&mut opt_c)?;
            (opt_c, Some(report))
        };

        if self.config.verify_each_stage {
            opt_circuit.validate().map_err(|e| {
                CompilerError::InvalidInput(format!("graph invalid after optimization: {e}"))
            })?;
        }

        let optimized_gate_count = opt_circuit.num_operations();

        // --------------------------------------------------------------------
        // 2. Target Basis Translation (Pre-Routing)
        // --------------------------------------------------------------------
        // Translate high-level and unsupported operations into the target's native
        // routing basis (e.g. CX or CZ + native 1Q rotations) with relaxed all-to-all connectivity.
        let routing_target = make_routing_basis_target(target);
        let basis_engine = BasisTranslationEngine::new(routing_target)
            .with_max_depth(self.config.translation.max_depth)
            .with_verify_equivalence(self.config.translation.verify_local_equivalence);

        let (mut translated_circuit, basis_report) =
            basis_engine.translate_circuit(&opt_circuit)?;
        cleanup_consecutive_rotations(&mut translated_circuit)?;

        if self.config.verify_each_stage {
            translated_circuit.validate().map_err(|e| {
                CompilerError::InvalidInput(format!("graph invalid after basis translation: {e}"))
            })?;
        }

        let translated_gate_count = translated_circuit.num_operations();

        // --------------------------------------------------------------------
        // 3. Placement & Circuit Routing
        // --------------------------------------------------------------------
        let initial_layout_input = self.config.explicit_initial_layout.clone();
        let routing_target = make_routing_connectivity_target(
            target,
            self.config.translation.enable_direction_correction,
        );

        let (routed_circuit, circuit_layout, routing_report) = match &self.config.router {
            RouterChoice::Sabre(sabre_config) => {
                let mut cfg = sabre_config.clone();
                cfg.placement_strategy = self.config.placement_strategy;
                let router = SabreBidirectionalRouter::with_config(cfg);
                let (routed, rep) = router.route_circuit(
                    &translated_circuit,
                    &routing_target,
                    initial_layout_input,
                )?;

                let stage_rep = RoutingStageReport {
                    router_name: "SabreBidirectionalRouter".to_string(),
                    placement_strategy: format!("{:?}", self.config.placement_strategy),
                    swaps_inserted: rep.swaps_inserted,
                    original_operations: rep.original_operations,
                    routed_operations: rep.routed_operations,
                    metrics: rep.metrics,
                };

                (routed, rep.layout, stage_rep)
            }
            RouterChoice::Baseline(baseline_config) => {
                let mut cfg = baseline_config.clone();
                cfg.placement_strategy = self.config.placement_strategy;
                let router = BaselineRouter::with_config(cfg);
                let (routed, rep) = router.route_circuit(
                    &translated_circuit,
                    &routing_target,
                    initial_layout_input,
                )?;

                let stage_rep = RoutingStageReport {
                    router_name: "BaselineRouter".to_string(),
                    placement_strategy: format!("{:?}", self.config.placement_strategy),
                    swaps_inserted: rep.swaps_inserted,
                    original_operations: rep.original_operations,
                    routed_operations: rep.routed_operations,
                    metrics: rep.metrics,
                };

                (routed, rep.layout, stage_rep)
            }
        };

        if self.config.verify_each_stage {
            routed_circuit.validate().map_err(|e| {
                CompilerError::InvalidInput(format!("graph invalid after routing: {e}"))
            })?;
        }

        let routed_gate_count = routed_circuit.num_operations();

        // --------------------------------------------------------------------
        // 4. Final Target-Specific Translation (Post-Routing)
        // --------------------------------------------------------------------
        // Correct any direction-sensitive physical entangling gates (e.g. reverse CX
        // along directed coupling edges using Hadamard conjugation) and lower any
        // non-native single-qubit operations in-place on the same physical operands.
        let (dir_circuit, dir_report) = if self.config.translation.enable_direction_correction {
            let corrector = TwoQubitDirectionCorrector::new(target.clone());
            let (corrected, rep) = corrector.correct_circuit(&routed_circuit)?;
            (corrected, Some(rep))
        } else {
            (routed_circuit, None)
        };

        let (mut final_phys_circuit, post_trans_report) =
            if self.config.translation.enable_final_lowering {
                let phys_engine = BasisTranslationEngine::new(target.clone())
                    .with_max_depth(self.config.translation.max_depth)
                    .with_verify_equivalence(self.config.translation.verify_local_equivalence);
                let (phys_c, rep) = phys_engine.translate_circuit(&dir_circuit)?;
                (phys_c, Some(rep))
            } else {
                (dir_circuit, None)
            };
        cleanup_consecutive_rotations(&mut final_phys_circuit)?;

        if self.config.verify_each_stage {
            final_phys_circuit.validate().map_err(|e| {
                CompilerError::InvalidInput(format!(
                    "graph invalid after post-routing translation: {e}"
                ))
            })?;
        }

        let post_routing_report = PostRoutingTranslationReport {
            directions_examined: dir_report
                .as_ref()
                .map(|r| r.operations_examined)
                .unwrap_or(0),
            directions_corrected: dir_report
                .as_ref()
                .map(|r| r.operations_corrected)
                .unwrap_or(0),
            lowered_operations: post_trans_report
                .as_ref()
                .map(|r| r.operations_decomposed)
                .unwrap_or(0),
        };

        let final_gate_count = final_phys_circuit.num_operations();
        let final_depth = CircuitMetrics::compute(&final_phys_circuit).circuit_depth;

        // --------------------------------------------------------------------
        // 5. Final Target Legality Validation
        // --------------------------------------------------------------------
        // Validate the final physical circuit against the selected target.
        // Fail closed immediately if any physical gate, operand tuple, or parameter violates hardware constraints.
        let legality_validator = TargetLegalityValidator::new(target.clone());
        let legality_report = legality_validator.validate_circuit(&final_phys_circuit)?;

        if self.config.validate_target_legality && !legality_report.is_valid() {
            let details = legality_report.summary();
            return Err(CompilerError::TargetLegality(
                TargetLegalityError::ValidationFailed {
                    circuit: final_phys_circuit.name.clone(),
                    target: target.name.clone(),
                    violation_count: legality_report.violations.len(),
                    details,
                    violations: legality_report.violations,
                },
            ));
        }

        // --------------------------------------------------------------------
        // 6. Post-Routing Scheduling
        // --------------------------------------------------------------------
        // Operates strictly and exclusively on the final routed, translated physical DAG.
        let (schedule, schedule_output, timeline, scheduling_report) = match &self.config.scheduler
        {
            SchedulerChoice::Disabled => (None, None, None, None),
            SchedulerChoice::Asap(sched_cfg) => {
                let scheduler = AsapScheduler::with_config(sched_cfg.clone());
                let sched = scheduler.schedule(&final_phys_circuit, target)?;

                let val_rep =
                    ScheduleValidator::new().validate(&sched, &final_phys_circuit, target);
                if !val_rep.is_valid() {
                    return Err(CompilerError::ScheduleValidation(val_rep.summary()));
                }

                let out = sched.to_output(Some(target));
                let tl = sched.generate_timeline();
                let rep = SchedulingStageReport {
                    scheduler_name: "AsapScheduler".to_string(),
                    makespan_ns: sched.makespan,
                    total_scheduled_operations: sched.operations.len(),
                    critical_path_operations: out.summary.critical_path_operations.len(),
                    peak_concurrency: out.summary.peak_concurrency,
                };

                (Some(sched), Some(out), Some(tl), Some(rep))
            }
            SchedulerChoice::Alap(sched_cfg, policy) => {
                let scheduler = AlapScheduler::new()
                    .with_config(sched_cfg.clone())
                    .with_policy(*policy);
                let sched = scheduler.schedule(&final_phys_circuit, target)?;

                let val_rep =
                    ScheduleValidator::new().validate(&sched, &final_phys_circuit, target);
                if !val_rep.is_valid() {
                    return Err(CompilerError::ScheduleValidation(val_rep.summary()));
                }

                let out = sched.to_output(Some(target));
                let tl = sched.generate_timeline();
                let rep = SchedulingStageReport {
                    scheduler_name: "AlapScheduler".to_string(),
                    makespan_ns: sched.makespan,
                    total_scheduled_operations: sched.operations.len(),
                    critical_path_operations: out.summary.critical_path_operations.len(),
                    peak_concurrency: out.summary.peak_concurrency,
                };

                (Some(sched), Some(out), Some(tl), Some(rep))
            }
            SchedulerChoice::ResourceConstrained => {
                let mgr = ResourceConstraintManager::from_target(target)?;
                let scheduler = ResourceConstrainedScheduler::new(mgr);
                let res = scheduler.schedule(&final_phys_circuit, target)?;

                let val_rep =
                    ScheduleValidator::new().validate(&res.schedule, &final_phys_circuit, target);
                if !val_rep.is_valid() {
                    return Err(CompilerError::ScheduleValidation(val_rep.summary()));
                }

                let out = res.schedule.to_output(Some(target));
                let tl = res.schedule.generate_timeline();
                let rep = SchedulingStageReport {
                    scheduler_name: "ResourceConstrainedScheduler".to_string(),
                    makespan_ns: res.schedule.makespan,
                    total_scheduled_operations: res.schedule.operations.len(),
                    critical_path_operations: out.summary.critical_path_operations.len(),
                    peak_concurrency: out.summary.peak_concurrency,
                };

                (Some(res.schedule), Some(out), Some(tl), Some(rep))
            }
        };

        // --------------------------------------------------------------------
        // 7. Layout-Aware Semantic Verification
        // --------------------------------------------------------------------
        // Verifies the final physical result against the original logical reference
        // using LayoutEquivalenceChecker under initial and final layouts.
        // Enforces ADR-0001: all unused physical qubits must start and end in |0⟩.
        let verification_report = if !self.config.verification.enabled {
            VerificationStageReport {
                status: VerificationStatus::Disabled,
                is_equivalent: true,
                phase_difference: None,
                max_amplitude_diff: None,
            }
        } else if !is_unitary_eligible(circuit) || !is_unitary_eligible(&final_phys_circuit) {
            // Milestone M7 scope: dynamic/measurement verification
            VerificationStageReport {
                status: VerificationStatus::SkippedNonUnitary,
                is_equivalent: true,
                phase_difference: None,
                max_amplitude_diff: None,
            }
        } else if num_physical > self.config.verification.max_qubits {
            VerificationStageReport {
                status: VerificationStatus::SkippedExceedsQubitLimit {
                    qubit_count: num_physical,
                    max_allowed: self.config.verification.max_qubits,
                },
                is_equivalent: true,
                phase_difference: None,
                max_amplitude_diff: None,
            }
        } else {
            let checker = LayoutEquivalenceChecker::new()
                .with_tolerances(self.config.verification.tolerances);

            let verdict =
                checker.check_layout_equivalence(circuit, &final_phys_circuit, &circuit_layout)?;

            if verdict.is_equivalent {
                VerificationStageReport {
                    status: VerificationStatus::VerifiedEquivalent,
                    is_equivalent: true,
                    phase_difference: verdict.estimated_phase,
                    max_amplitude_diff: Some(verdict.max_amplitude_difference),
                }
            } else {
                let reason = verdict
                    .worst_mismatch
                    .as_ref()
                    .map(|m| {
                        format!(
                            "amplitude discrepancy at output index {} (diff: {:.2e})",
                            m.output_amplitude_index, m.amplitude_difference
                        )
                    })
                    .unwrap_or_else(|| "unitary equivalence check failed".to_string());

                if self.config.verification.fail_on_mismatch {
                    return Err(CompilerError::SemanticEquivalenceFailed { reason });
                } else {
                    VerificationStageReport {
                        status: VerificationStatus::VerifiedNotEquivalent { reason },
                        is_equivalent: false,
                        phase_difference: verdict.estimated_phase,
                        max_amplitude_diff: Some(verdict.max_amplitude_difference),
                    }
                }
            }
        };

        let total_duration_ms = wall_start.elapsed().as_secs_f64() * 1000.0;

        // --------------------------------------------------------------------
        // 8. Compile Comprehensive Audit Report
        // --------------------------------------------------------------------
        let config_summary = CompilerConfigSummary {
            optimization_level: format!("{:?}", self.config.optimization.level),
            router: match &self.config.router {
                RouterChoice::Sabre(_) => "Sabre".to_string(),
                RouterChoice::Baseline(_) => "Baseline".to_string(),
            },
            placement_strategy: format!("{:?}", self.config.placement_strategy),
            scheduler: match &self.config.scheduler {
                SchedulerChoice::Disabled => "Disabled".to_string(),
                SchedulerChoice::Asap(_) => "Asap".to_string(),
                SchedulerChoice::Alap(_, _) => "Alap".to_string(),
                SchedulerChoice::ResourceConstrained => "ResourceConstrained".to_string(),
            },
            verification_enabled: self.config.verification.enabled,
            validate_target_legality: self.config.validate_target_legality,
        };

        let final_swaps = count_circuit_swaps(&final_phys_circuit);

        let metrics_summary = PipelineMetricsSummary {
            initial_gate_count,
            optimized_gate_count,
            translated_gate_count,
            routed_gate_count,
            final_gate_count,
            swaps_inserted: routing_report.swaps_inserted,
            original_swaps,
            router_swaps_inserted: routing_report.swaps_inserted,
            final_swaps,
            initial_depth,
            final_depth,
            makespan_ns: scheduling_report.as_ref().map(|s| s.makespan_ns),
            is_legal: legality_report.is_valid(),
            is_verified: verification_report.is_equivalent,
        };

        let opt_stage_report = opt_report.as_ref().map(|r| OptimizationStageReport {
            initial_gate_count: r.initial_metrics.total_operations,
            optimized_gate_count: r.final_metrics.total_operations,
            passes_executed: r.passes.len(),
            iterations: r.iterations_executed,
        });

        let legality_stage_report = LegalityStageReport {
            is_legal: legality_report.is_valid(),
            operations_examined: legality_report.operations_examined,
            violation_count: legality_report.violations.len(),
            summary: legality_report.summary(),
        };

        let report = CompilationReport {
            circuit_name: circuit.name.clone(),
            target_name: target.name.clone(),
            logical_qubits: num_logical,
            physical_qubits: num_physical,
            config: config_summary,
            initial_layout: circuit_layout.initial().clone(),
            final_layout: circuit_layout.final_layout().clone(),
            optimization: opt_stage_report,
            basis_translation: Some(basis_report),
            routing: routing_report,
            post_routing_translation: Some(post_routing_report),
            legality: legality_stage_report,
            scheduling: scheduling_report,
            verification: verification_report,
            metrics: metrics_summary,
            total_duration_ms,
        };

        let pass_diagnostics = opt_report
            .as_ref()
            .map(|r| r.pass_diagnostics.clone())
            .unwrap_or_default();

        Ok(CompilationResult {
            physical_circuit: final_phys_circuit,
            layout: circuit_layout,
            target: target.clone(),
            input_global_phase: circuit.global_phase.clone(),
            schedule,
            schedule_output,
            timeline,
            legality_report,
            optimization_report: opt_report,
            report,
            pass_diagnostics,
        })
    }
}

/// Helper deriving a pre-routing basis target with relaxed AllToAll connectivity.
fn make_routing_basis_target(target: &Target) -> Target {
    let mut routing_target = target.clone();
    routing_target.name = format!("{}_routing_basis", target.name());

    let mut relaxed_ops = BTreeMap::new();
    for (op_id, desc) in target.operations() {
        let mut relaxed = desc.clone();
        if desc.quantum_arity == 1 {
            relaxed.connectivity = OperationConnectivity::all_single_qubits(target.num_qubits());
        } else if desc.quantum_arity == 2 {
            relaxed.connectivity = OperationConnectivity::AllToAll;
        }
        relaxed_ops.insert(op_id.clone(), relaxed);
    }
    routing_target.operations = relaxed_ops;

    routing_target
}

/// Helper deriving a routing target where 2-qubit operations that can be direction-corrected
/// have undirected connectivity along their physical coupling edges.
fn make_routing_connectivity_target(target: &Target, enable_direction_correction: bool) -> Target {
    if !enable_direction_correction {
        return target.clone();
    }
    let mut routing_target = target.clone();
    routing_target.name = format!("{}_undirected_routing", target.name());

    let mut updated_ops = BTreeMap::new();
    for (op_id, desc) in target.operations() {
        let mut desc = desc.clone();
        if desc.is_two_qubit() {
            let tuples = desc.connectivity.allowed_tuples(target.num_qubits(), 2);
            let mut undirected_edges = Vec::new();
            for t in tuples {
                if t.len() == 2 {
                    undirected_edges.push((t[0], t[1]));
                }
            }
            if !undirected_edges.is_empty() {
                desc.connectivity = OperationConnectivity::undirected_two_qubit(undirected_edges);
            }
        }
        updated_ops.insert(op_id.clone(), desc);
    }
    routing_target.operations = updated_ops;

    routing_target
}

/// Helper counting total `swap` operations in a circuit.
fn count_circuit_swaps(circuit: &Circuit) -> usize {
    circuit
        .graph
        .ordered_nodes()
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| n.operation.name().eq_ignore_ascii_case("swap"))
                .count()
        })
        .unwrap_or(0)
}

/// Helper executing a cleanup pass to merge consecutive same-wire rotations
/// and eliminate zero-rotation identities while preserving exact global phase.
fn cleanup_consecutive_rotations(circuit: &mut Circuit) -> Result<(), CompilerError> {
    let mut pass = RotationMergePass::new();
    let mut context = PassContext::new();
    pass.run(circuit, &mut context).map_err(|e| {
        CompilerError::Optimization(OptimizerError::PassExecution {
            pass_id: PassId::new("rotation_merge"),
            reason: e.to_string(),
        })
    })?;
    Ok(())
}
