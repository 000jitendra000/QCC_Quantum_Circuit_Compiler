//! Compilation result, stage reports, telemetry, and pipeline diagnostics.
//!
//! Provides comprehensive reporting of all decisions, layout transformations,
//! routing metrics, scheduling intervals, and correctness proofs produced during
//! end-to-end compilation.

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::optimizer::report::OptimizerReport;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::LayoutMap;
use crate::routing::metrics::RoutingMetrics;
use crate::scheduler::output::ScheduleOutput;
use crate::scheduler::schedule::Schedule;
use crate::scheduler::timeline::TimelineData;
use crate::target::legality::TargetLegalityReport;
use crate::translation::TranslationReport;

/// Concise summary of compiler configuration options for reporting and reproducibility.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilerConfigSummary {
    /// Active optimization level (e.g. `"O2"`).
    pub optimization_level: String,
    /// Router selected (e.g. `"Sabre"` or `"Baseline"`).
    pub router: String,
    /// Placement strategy used (e.g. `"Weighted"`).
    pub placement_strategy: String,
    /// Post-routing scheduler choice (e.g. `"Asap"`, `"Alap"`, `"Disabled"`).
    pub scheduler: String,
    /// Whether layout-aware semantic verification was requested.
    pub verification_enabled: bool,
    /// Whether strict target legality validation was enforced.
    pub validate_target_legality: bool,
}

/// Logical optimization stage summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptimizationStageReport {
    /// Total gates before optimization.
    pub initial_gate_count: usize,
    /// Total gates after optimization.
    pub optimized_gate_count: usize,
    /// Number of passes executed.
    pub passes_executed: usize,
    /// Number of fixed-point iterations.
    pub iterations: usize,
}

/// Routing stage summary and metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutingStageReport {
    /// Name of the router invoked.
    pub router_name: String,
    /// Initial placement strategy used.
    pub placement_strategy: String,
    /// Number of physical SWAP gates inserted during routing.
    pub swaps_inserted: usize,
    /// Operation count of the pre-routing circuit.
    pub original_operations: usize,
    /// Operation count of the routed physical circuit.
    pub routed_operations: usize,
    /// Comprehensive routing quality metrics.
    pub metrics: RoutingMetrics,
}

/// Post-routing target-specific translation and direction correction report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PostRoutingTranslationReport {
    /// Number of entangling operations inspected for target coupling direction.
    pub directions_examined: usize,
    /// Number of direction-sensitive operations corrected (e.g. via Hadamard conjugation).
    pub directions_corrected: usize,
    /// Additional single-qubit gates decomposed to native target basis after direction correction.
    pub lowered_operations: usize,
}

/// Target legality stage summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalityStageReport {
    /// True if the physical circuit is 100% legal on the target.
    pub is_legal: bool,
    /// Total operations examined.
    pub operations_examined: usize,
    /// Number of violations detected (0 for valid circuits).
    pub violation_count: usize,
    /// Formatted human-readable summary of violations or confirmation.
    pub summary: String,
}

/// Post-routing physical scheduling summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchedulingStageReport {
    /// Scheduler algorithm invoked.
    pub scheduler_name: String,
    /// Total circuit makespan in nanoseconds.
    pub makespan_ns: f64,
    /// Total physical operations scheduled.
    pub total_scheduled_operations: usize,
    /// Number of operations on the critical execution path (zero slack).
    pub critical_path_operations: usize,
    /// Peak concurrent hardware operations.
    pub peak_concurrency: usize,
}

/// Status classification of layout-aware semantic verification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Verified equivalent up to a single common scalar global phase.
    VerifiedEquivalent,
    /// Verification failed: semantic mismatch detected.
    VerifiedNotEquivalent {
        /// Explanation of failure or largest amplitude discrepancy.
        reason: String,
    },
    /// Verification bypassed because the circuit contains non-unitary operations (measure/reset).
    SkippedNonUnitary,
    /// Verification bypassed because physical qubit count exceeds simulation memory limit.
    SkippedExceedsQubitLimit {
        /// Physical qubit count.
        qubit_count: usize,
        /// Maximum allowed verification qubit limit.
        max_allowed: usize,
    },
    /// Verification was explicitly disabled in configuration.
    Disabled,
}

/// Layout-aware semantic verification summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationStageReport {
    /// Outcome of the verification check.
    pub status: VerificationStatus,
    /// Whether the candidate circuit is semantically equivalent to the reference.
    pub is_equivalent: bool,
    /// Global phase difference in radians $[0, 2\pi)$, if verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_difference: Option<f64>,
    /// Maximum pointwise amplitude difference across all test basis states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_amplitude_diff: Option<f64>,
}

/// Summary metrics comparing circuit properties before and after compilation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PipelineMetricsSummary {
    /// Gate count in the original input logical circuit.
    pub initial_gate_count: usize,
    /// Gate count after logical optimization.
    pub optimized_gate_count: usize,
    /// Gate count after pre-routing target basis translation.
    pub translated_gate_count: usize,
    /// Gate count after placement and routing.
    pub routed_gate_count: usize,
    /// Final gate count of the physical legalized circuit.
    pub final_gate_count: usize,
    /// Total routing SWAPs inserted (alias for router_swaps_inserted).
    pub swaps_inserted: usize,
    /// Original SWAP operations in the input logical circuit.
    pub original_swaps: usize,
    /// Routing SWAP operations inserted during placement/routing.
    pub router_swaps_inserted: usize,
    /// Total SWAP operations remaining in the final physical circuit.
    pub final_swaps: usize,
    /// Original DAG depth.
    pub initial_depth: usize,
    /// Final physical DAG depth.
    pub final_depth: usize,
    /// Scheduled execution makespan in nanoseconds, if scheduled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub makespan_ns: Option<f64>,
    /// Whether the output circuit passed target legality validation.
    pub is_legal: bool,
    /// Whether the output circuit was semantically verified against the logical reference.
    pub is_verified: bool,
}

/// Comprehensive, structured audit report detailing every stage of the compilation pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilationReport {
    /// Name of the input logical circuit.
    pub circuit_name: String,
    /// Name of the compilation target hardware.
    pub target_name: String,
    /// Number of logical qubits in the input circuit.
    pub logical_qubits: usize,
    /// Number of physical device qubits on the target.
    pub physical_qubits: usize,
    /// Summary of compiler configuration options.
    pub config: CompilerConfigSummary,
    /// Initial layout mapping ($L_{\text{initial}}$ at $t=0$).
    pub initial_layout: LayoutMap,
    /// Final layout mapping ($L_{\text{final}}$ after all routing SWAPs).
    pub final_layout: LayoutMap,
    /// Logical optimization pass report, if optimization was executed.
    pub optimization: Option<OptimizationStageReport>,
    /// Pre-routing target basis translation report.
    pub basis_translation: Option<TranslationReport>,
    /// Circuit routing report and metrics.
    pub routing: RoutingStageReport,
    /// Post-routing target direction correction and final translation report.
    pub post_routing_translation: Option<PostRoutingTranslationReport>,
    /// Final target legality validation report.
    pub legality: LegalityStageReport,
    /// Post-routing physical scheduling report, if scheduling was enabled.
    pub scheduling: Option<SchedulingStageReport>,
    /// Layout-aware semantic verification report.
    pub verification: VerificationStageReport,
    /// Quantitative metrics summary tracking circuit evolution across stages.
    pub metrics: PipelineMetricsSummary,
    /// Total wall-clock compilation duration in milliseconds.
    pub total_duration_ms: f64,
}

impl CompilationReport {
    /// Returns `true` if compilation succeeded, target legality passed, and any requested
    /// verification confirmed semantic equivalence.
    pub fn is_successful(&self) -> bool {
        self.legality.is_legal && self.verification.is_equivalent
    }

    /// Serializes this report to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this report to formatted JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a compilation report from JSON.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

/// The complete structured result of compiling a logical quantum circuit for a physical target.
#[derive(Clone, Debug)]
pub struct CompilationResult {
    /// Final physical quantum circuit mapped, routed, and legalized for the target.
    pub physical_circuit: Circuit,
    /// Multi-stage layout tracking $L_{\text{initial}}$, $L_{\text{current}}$, and $L_{\text{final}}$.
    pub layout: CircuitLayout,
    /// Post-routing physical execution schedule, if scheduling was enabled.
    pub schedule: Option<Schedule>,
    /// Serialized schedule output representation, if scheduled.
    pub schedule_output: Option<ScheduleOutput>,
    /// Structured timeline visualization data, if scheduled.
    pub timeline: Option<TimelineData>,
    /// Full in-memory target legality validation report.
    pub legality_report: TargetLegalityReport,
    /// Full in-memory optimizer execution report, if optimization was executed.
    pub optimization_report: Option<OptimizerReport>,
    /// Comprehensive audit report detailing all stage decisions and metrics.
    pub report: CompilationReport,
}

impl CompilationResult {
    /// Returns a reference to the compiled physical circuit.
    #[inline]
    pub fn circuit(&self) -> &Circuit {
        &self.physical_circuit
    }

    /// Returns a reference to the multi-stage circuit layout.
    #[inline]
    pub fn layout(&self) -> &CircuitLayout {
        &self.layout
    }

    /// Returns `true` if post-routing scheduling was executed.
    #[inline]
    pub fn is_scheduled(&self) -> bool {
        self.schedule.is_some()
    }

    /// Returns a reference to the physical schedule, if present.
    #[inline]
    pub fn schedule(&self) -> Option<&Schedule> {
        self.schedule.as_ref()
    }

    /// Returns a reference to the compilation report.
    #[inline]
    pub fn report(&self) -> &CompilationReport {
        &self.report
    }
}
