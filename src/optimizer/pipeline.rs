//! Integrated optimization pipeline composition, pass orchestration, and audit reporting.

use std::time::Instant;

use thiserror::Error;

use crate::compiler::pass_diagnostic::{
    build_failed_pass_report, build_ran_pass_report, build_skipped_pass_report,
    execute_pass_verification_hook, PassDiagnosticReport, PassVerification,
    PassVerificationHookError,
};
use crate::ir::circuit::Circuit;
use crate::ir::error::IrValidationError;
use crate::ir::operation::Operation;
use crate::opt::context::{AnalysisId, PassContext};
use crate::opt::errors::PassManagerError;
use crate::opt::pass::{CompilerPass, PassId};
use crate::optimizer::config::{OptimizationLevel, OptimizerConfig};
use crate::optimizer::passes::cancellation::{InverseCancellationConfig, InverseCancellationPass};
use crate::optimizer::passes::commutation_reorder::{
    CommutationReorderConfig, CommutationReorderPass, ReorderStrategy,
};
use crate::optimizer::passes::identity::{IdentityEliminationConfig, IdentityEliminationPass};
use crate::optimizer::passes::one_qubit_fusion::{OneQubitFusionConfig, OneQubitFusionPass};
use crate::optimizer::passes::rotation_merge::{
    RotationMergeConfig, RotationMergePass, SymbolicMergePolicy,
};
use crate::optimizer::report::{
    CircuitMetrics, OptimizerReport, PassExecutionReport, PipelineTermination,
};
use crate::verification::{EquivalenceChecker, VerificationError, VerificationTolerances};

/// Structured error produced during compiler optimization pipeline execution.
#[derive(Debug, Error)]
pub enum OptimizerError {
    /// Underlying compiler pass infrastructure error.
    #[error("Pass manager error during optimization: {0}")]
    PassManager(#[from] PassManagerError),

    /// Circuit failed QccIR structural graph validation after pass execution.
    #[error("Post-pass QccIR graph validation failed after pass '{pass_id}': {source}")]
    GraphValidation {
        /// Pass that produced the invalid graph.
        pass_id: PassId,
        /// Underlying structural validation error.
        #[source]
        source: IrValidationError,
    },

    /// Pass violated unitary equivalence semantics in test/debug mode.
    #[error("Unitary equivalence verification failed after pass '{pass_id}': {reason}")]
    EquivalenceFailed {
        /// Pass that caused the semantic divergence.
        pass_id: PassId,
        /// Detailed description of the semantic mismatch.
        reason: String,
    },

    /// Verification engine error encountered during equivalence checking.
    #[error("Verification error during equivalence check after pass '{pass_id}': {source}")]
    Verification {
        /// Pass under verification.
        pass_id: PassId,
        /// Underlying verification engine error.
        #[source]
        source: VerificationError,
    },

    /// Pass execution failed.
    #[error("Pass execution failed for '{pass_id}': {reason}")]
    PassExecution {
        /// Pass that failed.
        pass_id: PassId,
        /// Reason for failure.
        reason: String,
    },
}

/// Returns `true` if the circuit contains only unitary quantum operations and barriers (and has $\ge 1$ declared qubit).
///
/// Under QCC formal semantics, `Operation::Barrier` acts as the identity operator $I$ on quantum statevectors
/// and does not collapse or project amplitudes. Only `Measure`, `Reset`, and `Classical` are non-unitary.
pub fn is_unitary_eligible(circuit: &Circuit) -> bool {
    if circuit.num_qubits() == 0 {
        return false;
    }
    for node in circuit.graph.nodes() {
        match &node.operation {
            Operation::Measure { .. } | Operation::Reset { .. } | Operation::Classical { .. } => {
                return false
            }
            Operation::Gate { .. } | Operation::Barrier { .. } => {}
        }
    }
    true
}

/// Integrated compiler optimization pipeline orchestrating passes, metrics, and audit reports.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerPipeline {
    config: OptimizerConfig,
}

impl Default for OptimizerPipeline {
    fn default() -> Self {
        Self::new(OptimizerConfig::default())
    }
}

impl OptimizerPipeline {
    /// Creates a new optimization pipeline with the specified configuration.
    pub fn new(config: OptimizerConfig) -> Self {
        Self { config }
    }

    /// Creates an optimization pipeline for a given optimization level with default settings.
    pub fn for_level(level: OptimizationLevel) -> Self {
        Self::new(OptimizerConfig::new(level))
    }

    /// Returns a reference to the active pipeline configuration.
    #[inline]
    pub fn config(&self) -> &OptimizerConfig {
        &self.config
    }

    /// Returns a mutable reference to the active pipeline configuration.
    #[inline]
    pub fn config_mut(&mut self) -> &mut OptimizerConfig {
        &mut self.config
    }

    /// Builds an [`EquivalenceChecker`] configured with the pipeline's tolerance and symbolic environment.
    pub fn equivalence_checker(&self) -> EquivalenceChecker {
        let tolerances = VerificationTolerances {
            state_tolerance: self.config.tolerance,
            matrix_tolerance: self.config.tolerance,
            phase_tolerance: self.config.tolerance * 10.0,
            amplitude_threshold: self.config.tolerance * 10.0,
            norm_tolerance: self.config.tolerance * 100.0,
        };
        EquivalenceChecker::new()
            .with_tolerances(tolerances)
            .with_environment(self.config.symbol_env.clone())
    }

    /// Executes the optimization pipeline on the given circuit with a fresh context.
    pub fn optimize(&self, circuit: &mut Circuit) -> Result<OptimizerReport, OptimizerError> {
        let mut context = PassContext::new().with_debug_verification(self.config.verify_graph);
        self.optimize_with_context(circuit, &mut context)
    }

    /// Executes a custom sequence of compiler passes through the pipeline's transactional
    /// execution, validation, and equivalence verification loop using a fresh context.
    pub fn optimize_with_passes(
        &self,
        circuit: &mut Circuit,
        passes: &mut [Box<dyn CompilerPass>],
    ) -> Result<OptimizerReport, OptimizerError> {
        let mut context = PassContext::new().with_debug_verification(self.config.verify_graph);
        self.optimize_with_passes_and_context(circuit, &mut context, passes)
    }

    /// Executes a custom sequence of passes through the pipeline using an existing context.
    pub fn optimize_with_passes_and_context(
        &self,
        circuit: &mut Circuit,
        context: &mut PassContext,
        passes: &mut [Box<dyn CompilerPass>],
    ) -> Result<OptimizerReport, OptimizerError> {
        let circuit_name = circuit.name.clone();
        let initial_metrics = CircuitMetrics::compute(circuit);

        // Prime the context with initial circuit depth analysis
        context.insert_analysis(AnalysisId::CIRCUIT_DEPTH, initial_metrics.circuit_depth);

        let initial_snapshot = if self.config.verify_equivalence && is_unitary_eligible(circuit) {
            Some(circuit.clone())
        } else {
            None
        };

        let mut reports = Vec::new();
        let mut diagnostic_reports = Vec::new();
        let mut total_passes_modified = 0;
        let mut total_passes_executed = 0;
        let mut total_duration_ns = 0u128;
        let mut step_index = 0;
        let mut iteration = 0;
        let mut termination = PipelineTermination::IterationLimitReached;

        let total_timer_start = if self.config.measure_durations {
            Some(Instant::now())
        } else {
            None
        };

        while iteration < self.config.max_iterations {
            iteration += 1;
            let mut any_modified_in_iteration = false;

            for pass in passes.iter_mut() {
                total_passes_executed += 1;
                let report = self.execute_single_pass(
                    pass.as_mut(),
                    circuit,
                    context,
                    iteration,
                    step_index,
                    &mut total_duration_ns,
                    &mut diagnostic_reports,
                )?;
                if report.modified {
                    any_modified_in_iteration = true;
                    total_passes_modified += 1;
                }
                reports.push(report);
                step_index += 1;
            }

            if !any_modified_in_iteration {
                termination = PipelineTermination::Converged;
                break;
            }
        }

        let overall_duration_ns = total_timer_start.map(|s| s.elapsed().as_nanos());
        let final_metrics = CircuitMetrics::compute(circuit);

        // Overall pipeline unitary equivalence verification
        let mut overall_equivalence_verified = None;
        if let Some(initial) = &initial_snapshot {
            if is_unitary_eligible(circuit) {
                let checker = self.equivalence_checker();
                let verdict = checker
                    .check_equivalence(initial, circuit)
                    .map_err(|source| OptimizerError::Verification {
                        pass_id: PassId::new("pipeline_summary"),
                        source,
                    })?;

                if !verdict.is_equivalent {
                    return Err(OptimizerError::EquivalenceFailed {
                        pass_id: PassId::new("pipeline_summary"),
                        reason: format!(
                            "Overall pipeline unitary equivalence failed: max amplitude diff = {:.2e}",
                            verdict.max_amplitude_difference
                        ),
                    });
                }
                overall_equivalence_verified = Some(true);
            }
        }

        Ok(OptimizerReport {
            circuit_name,
            level: self.config.level,
            initial_metrics,
            final_metrics,
            passes: reports,
            iterations_executed: iteration,
            termination,
            total_passes_modified,
            total_passes_executed,
            unitary_equivalence_verified: overall_equivalence_verified,
            total_duration_ns: overall_duration_ns,
            pass_diagnostics: diagnostic_reports,
        })
    }

    /// Executes the optimization pipeline on the given circuit using an existing [`PassContext`].
    pub fn optimize_with_context(
        &self,
        circuit: &mut Circuit,
        context: &mut PassContext,
    ) -> Result<OptimizerReport, OptimizerError> {
        let circuit_name = circuit.name.clone();
        let initial_metrics = CircuitMetrics::compute(circuit);

        // Prime the context with initial circuit depth analysis
        context.insert_analysis(AnalysisId::CIRCUIT_DEPTH, initial_metrics.circuit_depth);

        let initial_snapshot = if self.config.verify_equivalence && is_unitary_eligible(circuit) {
            Some(circuit.clone())
        } else {
            None
        };

        // Handle O0: No-op / validation only
        if self.config.level == OptimizationLevel::O0 {
            if self.config.verify_graph {
                circuit
                    .validate()
                    .map_err(|source| OptimizerError::GraphValidation {
                        pass_id: PassId::new("o0_validation"),
                        source,
                    })?;
            }

            let pass_diagnostics = if self.config.diagnostics.enabled {
                let passes = self.build_canonical_passes();
                passes
                    .iter()
                    .enumerate()
                    .map(|(i, pass)| {
                        let pass_id = pass.id();
                        let pass_name = if pass.description().is_empty() {
                            pass_id.to_string()
                        } else {
                            pass.description().to_string()
                        };
                        build_skipped_pass_report(
                            pass_id.0,
                            pass_name,
                            i,
                            pass.config_summary(),
                            Some(circuit),
                            &self.config.diagnostics,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };

            for d in &pass_diagnostics {
                context.push_diagnostic_report(d.clone());
            }

            return Ok(OptimizerReport {
                circuit_name,
                level: self.config.level,
                initial_metrics: initial_metrics.clone(),
                final_metrics: initial_metrics,
                passes: Vec::new(),
                iterations_executed: 0,
                termination: PipelineTermination::NoOp,
                total_passes_modified: 0,
                total_passes_executed: 0,
                unitary_equivalence_verified: if initial_snapshot.is_some() {
                    Some(true)
                } else {
                    None
                },
                total_duration_ns: if self.config.measure_durations {
                    Some(0)
                } else {
                    None
                },
                pass_diagnostics,
            });
        }

        let mut reports = Vec::new();
        let mut diagnostic_reports = Vec::new();
        let mut total_passes_modified = 0;
        let mut total_passes_executed = 0;
        let mut total_duration_ns = 0u128;
        let mut step_index = 0;
        let mut iteration = 0;
        let mut termination = PipelineTermination::IterationLimitReached;

        let total_timer_start = if self.config.measure_durations {
            Some(Instant::now())
        } else {
            None
        };

        while iteration < self.config.max_iterations {
            iteration += 1;
            let mut any_modified_in_iteration = false;

            // Construct fresh pass instances for this iteration
            let mut passes = self.build_passes_for_level();

            for pass in passes.iter_mut() {
                total_passes_executed += 1;
                let report = self.execute_single_pass(
                    pass.as_mut(),
                    circuit,
                    context,
                    iteration,
                    step_index,
                    &mut total_duration_ns,
                    &mut diagnostic_reports,
                )?;
                if report.modified {
                    any_modified_in_iteration = true;
                    total_passes_modified += 1;
                }
                reports.push(report);
                step_index += 1;
            }

            // Check loop termination policy
            if self.config.level == OptimizationLevel::O1 {
                termination = PipelineTermination::SinglePassCompleted;
                break;
            }

            if !any_modified_in_iteration {
                termination = PipelineTermination::Converged;
                break;
            }
        }

        let overall_duration_ns = total_timer_start.map(|s| s.elapsed().as_nanos());

        let final_metrics = CircuitMetrics::compute(circuit);

        // Overall pipeline unitary equivalence verification
        let mut overall_equivalence_verified = None;
        if let Some(initial) = &initial_snapshot {
            if is_unitary_eligible(circuit) {
                let checker = self.equivalence_checker();
                let verdict = checker
                    .check_equivalence(initial, circuit)
                    .map_err(|source| OptimizerError::Verification {
                        pass_id: PassId::new("pipeline_summary"),
                        source,
                    })?;

                if !verdict.is_equivalent {
                    return Err(OptimizerError::EquivalenceFailed {
                        pass_id: PassId::new("pipeline_summary"),
                        reason: format!(
                            "Overall pipeline unitary equivalence failed: max amplitude diff = {:.2e}",
                            verdict.max_amplitude_difference
                        ),
                    });
                }
                overall_equivalence_verified = Some(true);
            }
        }

        Ok(OptimizerReport {
            circuit_name,
            level: self.config.level,
            initial_metrics,
            final_metrics,
            passes: reports,
            iterations_executed: iteration,
            termination,
            total_passes_modified,
            total_passes_executed,
            unitary_equivalence_verified: overall_equivalence_verified,
            total_duration_ns: overall_duration_ns,
            pass_diagnostics: diagnostic_reports,
        })
    }

    /// Transactionally executes a single pass on a candidate circuit.
    ///
    /// Preserves the pre-pass circuit if graph validation or equivalence verification fails.
    #[allow(clippy::too_many_arguments)]
    fn execute_single_pass(
        &self,
        pass: &mut dyn CompilerPass,
        circuit: &mut Circuit,
        context: &mut PassContext,
        iteration: usize,
        step_index: usize,
        total_duration_ns: &mut u128,
        diagnostic_reports: &mut Vec<PassDiagnosticReport>,
    ) -> Result<PassExecutionReport, OptimizerError> {
        let pass_id = pass.id();
        let pass_name = if pass.description().is_empty() {
            pass_id.to_string()
        } else {
            pass.description().to_string()
        };
        let pass_config = pass.config_summary();

        let metrics_before = CircuitMetrics::compute(circuit);
        let snapshot_before = if self.config.diagnostics.enabled
            || (self.config.verify_equivalence && is_unitary_eligible(circuit))
        {
            Some(circuit.clone())
        } else {
            None
        };

        // 1. Required analyses check
        for required in pass.required_analyses() {
            if !context.has_analysis(&required) {
                if self.config.diagnostics.enabled {
                    let diag = build_failed_pass_report(
                        pass_id.0,
                        &pass_name,
                        step_index,
                        pass_config,
                        snapshot_before.as_ref(),
                        None,
                        None,
                        &self.config.diagnostics,
                    );
                    diagnostic_reports.push(diag.clone());
                    context.push_diagnostic_report(diag);
                }
                return Err(OptimizerError::PassManager(
                    PassManagerError::MissingRequiredAnalysis {
                        pass_id,
                        analysis_id: required,
                    },
                ));
            }
        }

        // 2. Preconditions check on current valid circuit
        if let Err(err) = pass.check_preconditions(circuit, context) {
            if self.config.diagnostics.enabled {
                let diag = build_failed_pass_report(
                    pass_id.0,
                    &pass_name,
                    step_index,
                    pass_config,
                    snapshot_before.as_ref(),
                    None,
                    None,
                    &self.config.diagnostics,
                );
                diagnostic_reports.push(diag.clone());
                context.push_diagnostic_report(diag);
            }
            return Err(OptimizerError::PassExecution {
                pass_id,
                reason: err.to_string(),
            });
        }

        // 3. Create candidate circuit for this pass execution (transactional isolation)
        let mut candidate = circuit.clone();

        let pass_start = if self.config.measure_durations || self.config.diagnostics.enabled {
            Some(Instant::now())
        } else {
            None
        };

        let run_result = pass.run(&mut candidate, context);

        let duration_ns = pass_start.map(|s| s.elapsed().as_nanos());
        let duration_ms = duration_ns.map(|ns| ns as f64 / 1_000_000.0);
        if let Some(d) = duration_ns {
            *total_duration_ns = total_duration_ns.saturating_add(d);
        }

        let pass_result = match run_result {
            Ok(res) => res,
            Err(err) => {
                if self.config.diagnostics.enabled {
                    let diag = build_failed_pass_report(
                        pass_id.0,
                        &pass_name,
                        step_index,
                        pass_config,
                        snapshot_before.as_ref(),
                        duration_ms,
                        None,
                        &self.config.diagnostics,
                    );
                    diagnostic_reports.push(diag.clone());
                    context.push_diagnostic_report(diag);
                }
                return Err(OptimizerError::PassExecution {
                    pass_id,
                    reason: err.to_string(),
                });
            }
        };

        // 4. Invalidation handling: purge analyses not explicitly preserved
        let invalidated = context.invalidate(&pass_result.preserved_analyses);
        let invalidated_strs: Vec<String> = invalidated.iter().map(|id| id.to_string()).collect();

        // 5. Post-pass QccIR graph validation on candidate
        if self.config.verify_graph {
            if let Err(source) = candidate.validate() {
                if self.config.diagnostics.enabled {
                    let diag = build_failed_pass_report(
                        pass_id.0,
                        &pass_name,
                        step_index,
                        pass_config,
                        snapshot_before.as_ref(),
                        duration_ms,
                        None,
                        &self.config.diagnostics,
                    );
                    diagnostic_reports.push(diag.clone());
                    context.push_diagnostic_report(diag);
                }
                return Err(OptimizerError::GraphValidation { pass_id, source });
            }
        }

        // 6. Metrics after pass execution on candidate
        let metrics_after = CircuitMetrics::compute(&candidate);

        // Update context with freshly computed circuit depth
        context.insert_analysis(AnalysisId::CIRCUIT_DEPTH, metrics_after.circuit_depth);

        // 7. Verification hook and/or historical equivalence check
        let mut equivalence_verified = None;
        let mut hook_verification = None;

        if self.config.diagnostics.enabled && self.config.diagnostics.verification_hook.enabled {
            match execute_pass_verification_hook(
                snapshot_before.as_ref().unwrap(),
                &candidate,
                &self.config.diagnostics.verification_hook,
            ) {
                Ok(ver) => {
                    if ver.status
                        == crate::compiler::pass_diagnostic::PassVerificationStatus::Passed
                    {
                        equivalence_verified = Some(true);
                    }
                    hook_verification = Some(ver);
                }
                Err(hook_err) => {
                    let (detail, failed_ver) = match hook_err {
                        PassVerificationHookError::Mismatch {
                            detail,
                            verification,
                        } => (detail, verification),
                        PassVerificationHookError::EngineError {
                            source,
                            verification,
                        } => (source.to_string(), verification),
                    };
                    let diag = build_failed_pass_report(
                        pass_id.0,
                        &pass_name,
                        step_index,
                        pass_config,
                        snapshot_before.as_ref(),
                        duration_ms,
                        Some(failed_ver),
                        &self.config.diagnostics,
                    );
                    diagnostic_reports.push(diag.clone());
                    context.push_diagnostic_report(diag);

                    return Err(OptimizerError::EquivalenceFailed {
                        pass_id,
                        reason: detail,
                    });
                }
            }
        } else if let Some(before) = &snapshot_before {
            if self.config.verify_equivalence && is_unitary_eligible(&candidate) {
                let checker = self.equivalence_checker();
                let verdict = checker
                    .check_equivalence(before, &candidate)
                    .map_err(|source| OptimizerError::Verification { pass_id, source })?;

                if !verdict.is_equivalent {
                    return Err(OptimizerError::EquivalenceFailed {
                        pass_id,
                        reason: format!(
                            "Unitary equivalence check failed: max amplitude diff = {:.2e}",
                            verdict.max_amplitude_difference
                        ),
                    });
                }
                equivalence_verified = Some(true);
            }
        }

        if self.config.diagnostics.enabled {
            let ver = hook_verification.unwrap_or_else(PassVerification::not_run);
            let diag = build_ran_pass_report(
                pass_id.0,
                &pass_name,
                step_index,
                pass_config,
                snapshot_before.as_ref().unwrap(),
                &candidate,
                duration_ms,
                invalidated_strs,
                ver,
                &self.config.diagnostics,
            );
            diagnostic_reports.push(diag.clone());
            context.push_diagnostic_report(diag);
        }

        // 8. Commit candidate: now circuit advances to this verified state
        *circuit = candidate;

        Ok(PassExecutionReport {
            pass_id,
            iteration,
            step_index,
            modified: pass_result.modified,
            summary: pass_result.summary,
            initial_metrics: metrics_before,
            final_metrics: metrics_after,
            duration_ns,
            equivalence_verified,
        })
    }

    /// Constructs the deterministic sequence of pass instances for a single iteration.
    /// Returns the canonical sequence of standard optimization passes (O2 level)
    /// used for skipped pass diagnostic reporting at O0.
    pub fn build_canonical_passes(&self) -> Vec<Box<dyn CompilerPass>> {
        self.build_passes_for(OptimizationLevel::O2)
    }

    fn build_passes_for_level(&self) -> Vec<Box<dyn CompilerPass>> {
        self.build_passes_for(self.config.level)
    }

    fn build_passes_for(&self, level: OptimizationLevel) -> Vec<Box<dyn CompilerPass>> {
        let mut passes: Vec<Box<dyn CompilerPass>> = Vec::new();

        let identity_config = IdentityEliminationConfig {
            tolerance: self.config.tolerance,
            eliminate_two_pi_rotations: true,
            symbol_env: self.config.symbol_env.clone(),
        };

        let cancel_config = InverseCancellationConfig {
            tolerance: self.config.tolerance,
            symbol_env: self.config.symbol_env.clone(),
            ..Default::default()
        };

        let rotation_config = RotationMergeConfig {
            tolerance: self.config.tolerance,
            symbol_env: self.config.symbol_env.clone(),
            symbolic_policy: SymbolicMergePolicy::PreserveUnresolved,
            ..Default::default()
        };

        let fusion_config = OneQubitFusionConfig {
            tolerance: self.config.tolerance,
            symbol_env: self.config.symbol_env.clone(),
            min_window_size: 2,
            eliminate_identities: true,
            ..Default::default()
        };

        let reorder_config = CommutationReorderConfig {
            window_size: self.config.reorder_window,
            max_rewrites: self.config.reorder_max_rewrites,
            tolerance: self.config.tolerance,
            strategy: ReorderStrategy::ExposeOpportunities,
            symbol_env: self.config.symbol_env.clone(),
            ..Default::default()
        };

        match level {
            OptimizationLevel::O0 => {
                // Empty pass sequence
            }
            OptimizationLevel::O1 => {
                // Peephole reductions
                passes.push(Box::new(IdentityEliminationPass::with_config(
                    identity_config.clone(),
                )));
                passes.push(Box::new(InverseCancellationPass::with_config(
                    cancel_config.clone(),
                )));
            }
            OptimizationLevel::O2 => {
                // Standard logical optimization
                passes.push(Box::new(IdentityEliminationPass::with_config(
                    identity_config.clone(),
                )));
                passes.push(Box::new(InverseCancellationPass::with_config(
                    cancel_config.clone(),
                )));
                passes.push(Box::new(RotationMergePass::with_config(
                    rotation_config.clone(),
                )));
                if self.config.one_qubit_fusion {
                    passes.push(Box::new(OneQubitFusionPass::with_config(
                        fusion_config.clone(),
                    )));
                }
                passes.push(Box::new(IdentityEliminationPass::with_config(
                    identity_config.clone(),
                )));
                passes.push(Box::new(InverseCancellationPass::with_config(
                    cancel_config.clone(),
                )));
            }
            OptimizationLevel::O3 => {
                // Aggressive optimization with DAG commutation reordering
                passes.push(Box::new(CommutationReorderPass::with_config(
                    reorder_config,
                )));
                passes.push(Box::new(IdentityEliminationPass::with_config(
                    identity_config.clone(),
                )));
                passes.push(Box::new(InverseCancellationPass::with_config(
                    cancel_config.clone(),
                )));
                passes.push(Box::new(RotationMergePass::with_config(
                    rotation_config.clone(),
                )));
                if self.config.one_qubit_fusion {
                    passes.push(Box::new(OneQubitFusionPass::with_config(
                        fusion_config.clone(),
                    )));
                }
                passes.push(Box::new(IdentityEliminationPass::with_config(
                    identity_config.clone(),
                )));
                passes.push(Box::new(InverseCancellationPass::with_config(
                    cancel_config.clone(),
                )));
            }
        }

        passes
    }
}
