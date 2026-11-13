//! Pass manager orchestrating deterministic compiler-pass execution pipelines.

use std::time::Instant;

use crate::compiler::pass_diagnostic::{
    build_failed_pass_report, build_ran_pass_report, build_skipped_pass_report,
    execute_pass_verification_hook, PassDiagnosticConfig, PassDiagnosticReport, PassVerification,
    PassVerificationHookError,
};
use crate::ir::circuit::Circuit;
use crate::opt::context::{PassContext, PreservedAnalyses};
use crate::opt::errors::{PassError, PassManagerError};
use crate::opt::pass::CompilerPass;
use crate::opt::statistics::{
    PassExecutionStatus, PassExecutionTrace, PassManagerReport, PassStatistics, PassTraceStep,
};

/// Deterministic compiler pass pipeline manager.
///
/// Executes an ordered sequence of [`CompilerPass`] implementations over a QccIR [`Circuit`],
/// enforcing analysis requirements, managing preservation and invalidation lifecycles,
/// running post-pass debug verifications, and collecting execution telemetry.
pub struct PassManager {
    /// Ordered sequence of passes to execute.
    passes: Vec<Box<dyn CompilerPass>>,
    /// Whether debug verification is unconditionally executed after every pass.
    debug_verification: bool,
    /// Whether wall-clock execution durations are measured for telemetry.
    measure_durations: bool,
    /// Configuration for per-pass diagnostic instrumentation.
    diagnostics: PassDiagnosticConfig,
    /// Chronological log of per-pass diagnostic reports from the last execution run.
    diagnostic_records: Vec<PassDiagnosticReport>,
}

impl Default for PassManager {
    fn default() -> Self {
        Self {
            passes: Vec::new(),
            debug_verification: false,
            measure_durations: true,
            diagnostics: PassDiagnosticConfig::default(),
            diagnostic_records: Vec::new(),
        }
    }
}

impl PassManager {
    /// Creates a new empty `PassManager`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables or disables debug verification after each individual pass.
    ///
    /// When enabled, runs circuit structural validation and pass-specific postcondition
    /// assertions after every step.
    pub fn with_debug_verification(mut self, enabled: bool) -> Self {
        self.debug_verification = enabled;
        self
    }

    /// Enables or disables telemetry duration measurement.
    pub fn with_duration_measurement(mut self, enabled: bool) -> Self {
        self.measure_durations = enabled;
        self
    }

    /// Configures per-pass diagnostic instrumentation.
    pub fn with_diagnostic_config(mut self, config: PassDiagnosticConfig) -> Self {
        self.diagnostics = config;
        self
    }

    /// Returns an immutable slice of all per-pass diagnostic reports recorded during the last run.
    #[inline]
    pub fn diagnostic_reports(&self) -> &[PassDiagnosticReport] {
        &self.diagnostic_records
    }

    /// Appends a pass to the execution pipeline.
    ///
    /// Passes execute strictly in the order they are added.
    pub fn add_pass<P: CompilerPass + 'static>(&mut self, pass: P) -> &mut Self {
        self.passes.push(Box::new(pass));
        self
    }

    /// Appends a boxed pass to the execution pipeline.
    pub fn add_boxed_pass(&mut self, pass: Box<dyn CompilerPass>) -> &mut Self {
        self.passes.push(pass);
        self
    }

    /// Returns the number of registered passes in the pipeline.
    #[inline]
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    /// Returns `true` if no passes are registered.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Returns an immutable slice of all registered passes.
    #[inline]
    pub fn passes(&self) -> &[Box<dyn CompilerPass>] {
        &self.passes
    }

    /// Executes the pass pipeline on the given circuit with a freshly initialized context.
    pub fn run(&mut self, circuit: &mut Circuit) -> Result<PassManagerReport, PassManagerError> {
        let mut context = PassContext::new().with_debug_verification(self.debug_verification);
        self.run_with_context(circuit, &mut context)
    }

    /// Executes the pass pipeline on the given circuit using an existing [`PassContext`].
    pub fn run_with_context(
        &mut self,
        circuit: &mut Circuit,
        context: &mut PassContext,
    ) -> Result<PassManagerReport, PassManagerError> {
        let mut trace = PassExecutionTrace::new(&circuit.name);
        let mut statistics = Vec::with_capacity(self.passes.len());
        self.diagnostic_records.clear();

        let mut total_modified = 0;
        let mut total_duration_ns = 0u128;
        let initial_circuit_ops = circuit.num_operations();

        for (step_index, pass) in self.passes.iter_mut().enumerate() {
            let pass_id = pass.id();
            let pass_name = if pass.description().is_empty() {
                pass_id.to_string()
            } else {
                pass.description().to_string()
            };
            let pass_config = pass.config_summary();
            let semantic_domain = pass.semantic_domain();
            let initial_ops = circuit.num_operations();

            // 0. Check conditional enablement
            if !pass.is_enabled(circuit, context) {
                if self.diagnostics.enabled {
                    let diag = build_skipped_pass_report(
                        pass_id.0,
                        pass_name,
                        step_index,
                        pass_config,
                        Some(circuit),
                        &self.diagnostics,
                    );
                    self.diagnostic_records.push(diag.clone());
                    context.push_diagnostic_report(diag);
                }
                continue;
            }

            let circuit_before = if self.diagnostics.enabled {
                Some(circuit.clone())
            } else {
                None
            };

            // 1. Enforce required analyses before execution
            for required in pass.required_analyses() {
                if !context.has_analysis(&required) {
                    if self.diagnostics.enabled {
                        let diag = build_failed_pass_report(
                            pass_id.0,
                            &pass_name,
                            step_index,
                            pass_config,
                            circuit_before.as_ref(),
                            None,
                            None,
                            &self.diagnostics,
                        );
                        self.diagnostic_records.push(diag.clone());
                        context.push_diagnostic_report(diag);
                    }
                    return Err(PassManagerError::MissingRequiredAnalysis {
                        pass_id,
                        analysis_id: required,
                    });
                }
            }

            // 2. Check pass preconditions
            if let Err(error) = pass.check_preconditions(circuit, context) {
                if self.diagnostics.enabled {
                    let diag = build_failed_pass_report(
                        pass_id.0,
                        &pass_name,
                        step_index,
                        pass_config,
                        circuit_before.as_ref(),
                        None,
                        None,
                        &self.diagnostics,
                    );
                    self.diagnostic_records.push(diag.clone());
                    context.push_diagnostic_report(diag);
                }
                return Err(PassManagerError::PreconditionFailed {
                    step_index,
                    pass_id,
                    error,
                });
            }

            // 3. Execute the pass
            let start = if self.measure_durations || self.diagnostics.enabled {
                Some(Instant::now())
            } else {
                None
            };

            let run_result = pass.run(circuit, context);

            let duration_ns = start.map(|s| s.elapsed().as_nanos());
            let duration_ms = duration_ns.map(|ns| ns as f64 / 1_000_000.0);
            if let Some(d) = duration_ns {
                total_duration_ns = total_duration_ns.saturating_add(d);
            }

            let pass_result = match run_result {
                Ok(res) => res,
                Err(error) => {
                    // Pessimistic invalidation: on failure, purge all analyses to avoid stale reuse
                    context.invalidate_all();

                    trace.record_step(PassTraceStep {
                        step_index,
                        pass_id,
                        semantic_domain,
                        status: PassExecutionStatus::Failed,
                        modified: false,
                        required_analyses: pass.required_analyses(),
                        invalidated_analyses: Vec::new(),
                        initial_operations: initial_ops,
                        final_operations: circuit.num_operations(),
                    });

                    if self.diagnostics.enabled {
                        let diag = build_failed_pass_report(
                            pass_id.0,
                            &pass_name,
                            step_index,
                            pass_config,
                            circuit_before.as_ref(),
                            duration_ms,
                            None,
                            &self.diagnostics,
                        );
                        self.diagnostic_records.push(diag.clone());
                        context.push_diagnostic_report(diag);
                    }

                    return Err(PassManagerError::PassFailed {
                        step_index,
                        pass_id,
                        error,
                    });
                }
            };

            if pass_result.modified {
                total_modified += 1;
            }

            // 4. Invalidation handling: purge analyses not explicitly preserved
            let invalidated_analyses = context.invalidate(&pass_result.preserved_analyses);
            let invalidated_strs: Vec<String> = invalidated_analyses
                .iter()
                .map(|id| id.to_string())
                .collect();

            let final_ops = circuit.num_operations();
            let delta_operations = final_ops as isize - initial_ops as isize;

            // 5. Postconditions & Debug Verification
            let should_verify = self.debug_verification || context.is_debug_verification_enabled();
            if should_verify {
                if let Err(error) = pass.check_postconditions(circuit, context) {
                    // Invalidate all analyses on postcondition failure
                    context.invalidate_all();

                    trace.record_step(PassTraceStep {
                        step_index,
                        pass_id,
                        semantic_domain,
                        status: PassExecutionStatus::Failed,
                        modified: pass_result.modified,
                        required_analyses: pass.required_analyses(),
                        invalidated_analyses,
                        initial_operations: initial_ops,
                        final_operations: final_ops,
                    });

                    if self.diagnostics.enabled {
                        let diag = build_failed_pass_report(
                            pass_id.0,
                            &pass_name,
                            step_index,
                            pass_config,
                            circuit_before.as_ref(),
                            duration_ms,
                            None,
                            &self.diagnostics,
                        );
                        self.diagnostic_records.push(diag.clone());
                        context.push_diagnostic_report(diag);
                    }

                    return Err(PassManagerError::DebugVerificationFailed {
                        step_index,
                        pass_id,
                        error,
                    });
                }
            }

            // 6. Verification hook (if enabled)
            let verification_result = if self.diagnostics.enabled {
                if self.diagnostics.verification_hook.enabled {
                    match execute_pass_verification_hook(
                        circuit_before.as_ref().unwrap(),
                        circuit,
                        &self.diagnostics.verification_hook,
                    ) {
                        Ok(ver) => ver,
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
                                circuit_before.as_ref(),
                                duration_ms,
                                Some(failed_ver),
                                &self.diagnostics,
                            );
                            self.diagnostic_records.push(diag.clone());
                            context.push_diagnostic_report(diag);

                            return Err(PassManagerError::DebugVerificationFailed {
                                step_index,
                                pass_id,
                                error: PassError::ExecutionFailed {
                                    pass_id,
                                    reason: detail,
                                },
                            });
                        }
                    }
                } else {
                    PassVerification::not_run()
                }
            } else {
                PassVerification::not_run()
            };

            // 7. Record deterministic step and telemetry statistics
            trace.record_step(PassTraceStep {
                step_index,
                pass_id,
                semantic_domain,
                status: PassExecutionStatus::Success,
                modified: pass_result.modified,
                required_analyses: pass.required_analyses(),
                invalidated_analyses: invalidated_analyses.clone(),
                initial_operations: initial_ops,
                final_operations: final_ops,
            });

            statistics.push(PassStatistics {
                pass_id,
                execution_order: step_index,
                semantic_domain,
                modified: pass_result.modified,
                status: PassExecutionStatus::Success,
                duration_ns,
                required_analyses: pass.required_analyses(),
                preserved_analyses: match &pass_result.preserved_analyses {
                    PreservedAnalyses::All => context.available_analyses(),
                    PreservedAnalyses::None => Vec::new(),
                    PreservedAnalyses::Preserve(set) => set.iter().copied().collect(),
                },
                invalidated_analyses,
                initial_operations: initial_ops,
                final_operations: final_ops,
                delta_operations,
                summary: pass_result.summary,
            });

            if self.diagnostics.enabled {
                let diag = build_ran_pass_report(
                    pass_id.0,
                    &pass_name,
                    step_index,
                    pass_config,
                    circuit_before.as_ref().unwrap(),
                    circuit,
                    duration_ms,
                    invalidated_strs,
                    verification_result,
                    &self.diagnostics,
                );
                self.diagnostic_records.push(diag.clone());
                context.push_diagnostic_report(diag);
            }
        }

        let total_delta_operations =
            circuit.num_operations() as isize - initial_circuit_ops as isize;

        Ok(PassManagerReport {
            circuit_name: circuit.name.clone(),
            trace,
            statistics,
            total_passes_executed: self.passes.len(),
            total_passes_modified: total_modified,
            total_delta_operations,
            total_duration_ns: if self.measure_durations {
                Some(total_duration_ns)
            } else {
                None
            },
            pass_diagnostics: self.diagnostic_records.clone(),
        })
    }
}
