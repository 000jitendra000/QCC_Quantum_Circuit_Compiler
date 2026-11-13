//! Stable, versioned per-pass diagnostic report schema and data models.
//!
//! Provides a truthful, machine-readable record of what each compiler pass did,
//! its before-and-after circuit metrics, DAG transformation accounting,
//! input/output circuit fingerprints, and explicit verification provenance
//! (Roadmap v3.1, Milestone M8, Phase 71).
//!
//! # Architecture Invariants
//!
//! 1. **Deterministic Representation Fingerprints (Not Semantic Proof)**:
//!    Input and output SHA-256 fingerprints capture a deterministic representation fingerprint
//!    of a specific in-memory QccIR circuit DAG.
//!    - **Guaranteed**: Repeated serialization of the same circuit representation produces bit-for-bit
//!      identical bytes and SHA-256 digests.
//!    - **Representation-Dependent**: Serialization explicitly includes allocated [`crate::ir::node::OperationId`]
//!      values and uses them for tie-breaking in Kahn's topological sort. Consequently, circuits with
//!      different operation IDs or differing insertion orders of concurrent gates produce different
//!      fingerprints.
//!    - **Non-Equivalence**: Identical fingerprints do **NOT** prove unitary equivalence, and different
//!      fingerprints do **NOT** imply semantic inequivalence. Equivalent circuits with different
//!      representations (e.g. $X \cdot X = I$) produce distinct fingerprints.
//!
//! 2. **Truthful Verification Status**:
//!    The verification status must explicitly be one of:
//!    - [`PassVerificationStatus::Passed`]: Equivalence criterion satisfied within declared
//!      verifier scope and tolerances (numerical verification, not an axiomatic formal mathematical proof).
//!    - [`PassVerificationStatus::Failed`]: Equivalence check ran and detected a violation.
//!    - [`PassVerificationStatus::NotRun`]: No verification check was executed.
//!    - [`PassVerificationStatus::Unsupported`]: Circuit contains features (e.g. non-unitary operations) not supported by the verifier.
//!
//!    **Invariant**: [`PassVerificationStatus::NotRun`] and [`PassVerificationStatus::Unsupported`]
//!    must **never** be collapsed into or reported as [`PassVerificationStatus::Passed`].
//!
//! 3. **Provenance Separation**:
//!    Internal QCC verifiers ([`VerifierProvenance::Internal`]) and independent
//!    black-box oracle verifiers ([`VerifierProvenance::IndependentOracle`]) are strictly
//!    distinguished. Normal Rust compiler pipelines never claim independent oracle verification.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::compiler::manifest::CompilerIdentityManifest;
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::optimizer::report::CircuitMetrics;
use crate::verification::tolerance::VerificationTolerances;
use crate::verification::unitary::{EquivalenceChecker, VerificationError, VerificationMethod};

/// Canonical schema identifier for QCC per-pass diagnostic reports.
pub const PASS_DIAGNOSTIC_SCHEMA_ID: &str = "qcc.pass_diagnostic";

/// Current semantic schema version for [`PassDiagnosticReport`].
pub const PASS_DIAGNOSTIC_SCHEMA_VERSION: &str = "1.0.0";

/// Execution status of a compiler pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassStatus {
    /// Pass ran to completion.
    Ran,
    /// Pass was bypassed or skipped (e.g. disabled, redundant, or prerequisites not met).
    Skipped,
    /// Pass execution encountered a fatal error and aborted.
    Failed,
}

impl fmt::Display for PassStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ran => write!(f, "ran"),
            Self::Skipped => write!(f, "skipped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Availability classification for individual measurements and metrics.
///
/// Enables consumers to distinguish whether a missing measurement was deliberately
/// omitted (`NotMeasured`), could not be computed (`Unavailable`), or is conceptually
/// meaningless for the pass/stage (`NotApplicable`), without fabricating default zero values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementAvailability {
    /// Metric was explicitly measured and populated.
    Available,
    /// Metric measurement was omitted or bypassed (e.g. telemetry disabled for performance).
    NotMeasured,
    /// Metric computation was attempted but failed or data was missing (e.g. target calibration missing, or pass failed).
    Unavailable,
    /// Metric is conceptually not applicable (e.g. routing metrics in pre-routing passes, or post-metrics for skipped passes).
    NotApplicable,
}

impl fmt::Display for MeasurementAvailability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::NotMeasured => write!(f, "not_measured"),
            Self::Unavailable => write!(f, "unavailable"),
            Self::NotApplicable => write!(f, "not_applicable"),
        }
    }
}

/// Verification status of a compiler pass.
///
/// **Verification Semantics**:
/// - [`PassVerificationStatus::Passed`]: Indicates that the specified verifier satisfied
///   its equivalence criterion within its declared operational scope and numerical tolerances.
///   This records empirical numerical agreement (e.g. statevector amplitude discrepancy $\le \text{atol}$);
///   it does **not** constitute an axiomatic formal mathematical proof.
/// - [`PassVerificationStatus::Failed`]: Verification check ran and detected a discrepancy exceeding tolerances.
/// - [`PassVerificationStatus::NotRun`]: No verification check was executed for this pass.
/// - [`PassVerificationStatus::Unsupported`]: Circuit contains constructs (e.g. mid-circuit measurements, resets)
///   outside the verifier's operational domain.
///
/// **Non-Negotiable Invariant**: `NotRun` and `Unsupported` must **never** be collapsed
/// into `Passed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassVerificationStatus {
    /// Equivalence criterion satisfied within declared verifier scope and tolerances (numerical verification, not an axiomatic formal mathematical proof).
    Passed,
    /// Verification failed: a discrepancy exceeding tolerances or invariant violation was detected.
    Failed,
    /// Verification was not executed for this pass.
    NotRun,
    /// Verification could not run because the circuit contains features outside the verifier's domain.
    Unsupported,
}

impl fmt::Display for PassVerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::NotRun => write!(f, "not_run"),
            Self::Unsupported => write!(f, "unsupported"),
        }
    }
}

/// Provenance origin of verification evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierProvenance {
    /// Verification performed by in-tree QCC Rust verifier (e.g. statevector unitary checker).
    Internal,
    /// Verification performed by external independent oracle (e.g. Qiskit oracle tooling).
    IndependentOracle,
}

impl fmt::Display for VerifierProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal => write!(f, "internal"),
            Self::IndependentOracle => write!(f, "independent_oracle"),
        }
    }
}

/// Deterministic representation fingerprints before and after pass execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitFingerprints {
    /// Cryptographic hash algorithm used for fingerprinting (`"sha256"`).
    pub algorithm: String,
    /// SHA-256 digest of canonical input circuit representation, or `None` if unavailable.
    pub input_fingerprint: Option<String>,
    /// SHA-256 digest of canonical output circuit representation, or `None` if unavailable/skipped.
    pub output_fingerprint: Option<String>,
    /// Invariant assertion: always `false`. Fingerprint equivalence != semantic equivalence.
    pub semantic_equivalence_guaranteed: bool,
}

impl CircuitFingerprints {
    /// Creates a new SHA-256 fingerprint pair with `semantic_equivalence_guaranteed = false`.
    pub fn sha256(input: Option<String>, output: Option<String>) -> Self {
        Self {
            algorithm: "sha256".to_string(),
            input_fingerprint: input,
            output_fingerprint: output,
            semantic_equivalence_guaranteed: false,
        }
    }

    /// Creates an uncomputed fingerprint record with `None` for both input and output.
    pub fn uncomputed() -> Self {
        Self::sha256(None, None)
    }

    /// Computes deterministic representation fingerprints directly from circuit DAG references.
    ///
    /// # Established Guarantees
    ///
    /// 1. **Repeatability**: Repeated serialization of the exact same in-memory [`crate::ir::circuit::Circuit`]
    ///    instance produces bit-for-bit identical JSON text and SHA-256 digests across calls, threads, and runs.
    /// 2. **Pointer & Iteration Independence**: Serialized order uses Kahn's algorithm with a
    ///    `BTreeSet<OperationId>` ready queue and sorted dependency edges, eliminating any `HashMap`
    ///    iteration nondeterminism or memory-address dependencies for identical `OperationId` values.
    /// 3. **Exact Value Preservation**: Preserves exact [`crate::ir::parameter::ParameterExpr`] ASTs,
    ///    circuit-level scalar global phase, register declaration ordering, and sorted metadata.
    ///
    /// # Explicit Limitations (Representation-Dependent)
    ///
    /// 1. **OperationId Sensitivity**: QccIR allocates [`crate::ir::node::OperationId`] sequentially upon node
    ///    creation (`next_id += 1`). Serialized output explicitly includes these operation IDs.
    /// 2. **Insertion Order for Concurrent Operations**: When operations have no dependency between them,
    ///    the ready queue breaks ties in ascending `OperationId` order. If concurrent operations were
    ///    inserted in different orders, their assigned IDs are reversed, altering both the topological
    ///    emission sequence and the serialized ID values.
    /// 3. **No Isomorphic Canonicalization Across Different Operation IDs**: Different `OperationId`
    ///    allocations will produce different fingerprints even if two circuits have identical graph topology.
    /// 4. **No Semantic Equivalence Proof**: Matching fingerprints prove representation identity only.
    ///    Different representations of equivalent circuits (e.g. $X \cdot X = I$, or decomposed rotations)
    ///    produce different fingerprints. `semantic_equivalence_guaranteed` is unconditionally `false`.
    pub fn from_circuits(
        input: Option<&crate::ir::circuit::Circuit>,
        output: Option<&crate::ir::circuit::Circuit>,
    ) -> Result<Self, crate::ir::error::IrError> {
        let input_fingerprint = match input {
            Some(c) => {
                let json = crate::ir::circuit_to_json(c)?;
                Some(crate::compiler::manifest::sha256_hex(json.as_bytes()))
            }
            None => None,
        };
        let output_fingerprint = match output {
            Some(c) => {
                let json = crate::ir::circuit_to_json(c)?;
                Some(crate::compiler::manifest::sha256_hex(json.as_bytes()))
            }
            None => None,
        };
        Ok(Self {
            algorithm: "sha256".to_string(),
            input_fingerprint,
            output_fingerprint,
            semantic_equivalence_guaranteed: false,
        })
    }
}

/// Quantitative snapshot of circuit metrics at a pass boundary.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CircuitSnapshotMetrics {
    /// Total unitary quantum gate count.
    pub total_gate_count: Option<usize>,
    /// Histogram mapping lower-case gate mnemonic to frequency count.
    pub per_gate_counts: Option<BTreeMap<String, usize>>,
    /// Number of CX (controlled-NOT) entangling gates.
    pub cx_count: Option<usize>,
    /// Number of logical SWAP gates present in the input circuit specification.
    pub logical_swaps: Option<usize>,
    /// Number of physical SWAP gates inserted by the router.
    pub router_swaps_inserted: Option<usize>,
    /// Number of final physical SWAP gates emitted.
    pub final_emitted_swaps: Option<usize>,
    /// Critical path dependency depth in the circuit DAG.
    pub circuit_depth: Option<usize>,
    /// Target-dependent cost metrics (e.g. latency, error, duration), or `None` if uncomputed.
    pub cost_metrics: Option<BTreeMap<String, f64>>,
}

impl CircuitSnapshotMetrics {
    /// Constructs snapshot metrics from an existing [`CircuitMetrics`] instance.
    pub fn from_circuit_metrics(
        metrics: &CircuitMetrics,
        logical_swaps: Option<usize>,
        router_swaps_inserted: Option<usize>,
        final_emitted_swaps: Option<usize>,
        cost_metrics: Option<BTreeMap<String, f64>>,
    ) -> Self {
        let cx_count = metrics.gate_histogram.get("cx").copied();
        Self {
            total_gate_count: Some(metrics.gate_count),
            per_gate_counts: Some(metrics.gate_histogram.clone()),
            cx_count,
            logical_swaps,
            router_swaps_inserted,
            final_emitted_swaps,
            circuit_depth: Some(metrics.circuit_depth),
            cost_metrics,
        }
    }

    /// Computes quantitative snapshot metrics directly from a QccIR [`Circuit`].
    pub fn from_circuit(circuit: &Circuit) -> Self {
        let metrics = CircuitMetrics::compute(circuit);
        let logical_swaps = metrics.gate_histogram.get("swap").copied();
        Self::from_circuit_metrics(&metrics, logical_swaps, None, None, None)
    }
}

/// Measurements recorded before and after a pass execution.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassMeasurements {
    /// Circuit snapshot metrics before pass execution, or `None` if skipped/unmeasured.
    pub before: Option<CircuitSnapshotMetrics>,
    /// Circuit snapshot metrics after pass execution, or `None` if skipped/failed/unmeasured.
    pub after: Option<CircuitSnapshotMetrics>,
    /// Nondeterministic runtime telemetry: wall-clock duration of pass in milliseconds.
    pub duration_ms: Option<f64>,
    /// Explicit availability mapping distinguishing 'available', 'not_measured',
    /// 'unavailable', and 'not_applicable' for individual metrics or snapshots.
    pub availability: BTreeMap<String, MeasurementAvailability>,
}

impl PassMeasurements {
    /// Creates a new measurements bundle with an explicit availability map.
    pub fn new(
        before: Option<CircuitSnapshotMetrics>,
        after: Option<CircuitSnapshotMetrics>,
        duration_ms: Option<f64>,
        availability: BTreeMap<String, MeasurementAvailability>,
    ) -> Self {
        Self {
            before,
            after,
            duration_ms,
            availability,
        }
    }

    /// Creates a measurements bundle with default availability classifications derived from snapshot presence.
    pub fn with_snapshots(
        before: Option<CircuitSnapshotMetrics>,
        after: Option<CircuitSnapshotMetrics>,
        duration_ms: Option<f64>,
    ) -> Self {
        let mut availability = BTreeMap::new();
        availability.insert(
            "before".to_string(),
            if before.is_some() {
                MeasurementAvailability::Available
            } else {
                MeasurementAvailability::NotApplicable
            },
        );
        availability.insert(
            "after".to_string(),
            if after.is_some() {
                MeasurementAvailability::Available
            } else {
                MeasurementAvailability::NotApplicable
            },
        );
        availability.insert(
            "duration_ms".to_string(),
            if duration_ms.is_some() {
                MeasurementAvailability::Available
            } else {
                MeasurementAvailability::NotMeasured
            },
        );
        Self {
            before,
            after,
            duration_ms,
            availability,
        }
    }

    /// Queries the availability status for a named measurement or metric.
    pub fn get_availability(&self, metric_name: &str) -> MeasurementAvailability {
        if let Some(&status) = self.availability.get(metric_name) {
            return status;
        }
        match metric_name {
            "before" => {
                if self.before.is_some() {
                    MeasurementAvailability::Available
                } else {
                    MeasurementAvailability::NotApplicable
                }
            }
            "after" => {
                if self.after.is_some() {
                    MeasurementAvailability::Available
                } else {
                    MeasurementAvailability::NotApplicable
                }
            }
            "duration_ms" => {
                if self.duration_ms.is_some() {
                    MeasurementAvailability::Available
                } else {
                    MeasurementAvailability::NotMeasured
                }
            }
            _ => MeasurementAvailability::Unavailable,
        }
    }

    /// Records or updates the availability status for a named metric.
    pub fn set_availability(
        &mut self,
        metric_name: impl Into<String>,
        status: MeasurementAvailability,
    ) {
        self.availability.insert(metric_name.into(), status);
    }
}

/// Global phase adjustment introduced by a compiler pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalPhaseAdjustment {
    /// Numerical angle shift in radians.
    pub radians: f64,
    /// Exact symbolic or algebraic expression for the phase shift.
    pub expression: String,
}

impl GlobalPhaseAdjustment {
    /// Creates a new global phase adjustment record.
    pub fn new(radians: f64, expression: impl Into<String>) -> Self {
        Self {
            radians,
            expression: expression.into(),
        }
    }
}

/// Transformation accounting recording mutations performed by a compiler pass.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TransformationAccounting {
    /// Number of new operations inserted into the circuit DAG.
    pub operations_added: usize,
    /// Number of operations removed from the circuit DAG.
    pub operations_removed: usize,
    /// Number of operations rewritten in-place or replaced.
    pub operations_rewritten: usize,
    /// Global phase adjustment introduced by this pass, if any.
    pub global_phase_adjustment: Option<GlobalPhaseAdjustment>,
    /// List of compiler analyses or cached data structures invalidated by this pass.
    pub analysis_invalidations: Vec<String>,
}

impl TransformationAccounting {
    /// Constructs a clean accounting record for a pass that performed zero mutations.
    pub fn no_op() -> Self {
        Self::default()
    }

    /// Computes DAG transformation accounting between pre-pass and post-pass circuits.
    ///
    /// Evaluates exact operation ID set differences and in-place operation mutations
    /// without guessing or inferring from aggregate gate-count changes.
    pub fn from_circuit_transition(
        before: &Circuit,
        after: &Circuit,
        invalidated_analyses: Vec<String>,
    ) -> Self {
        let before_ops: HashMap<OperationId, &Operation> =
            before.graph.nodes().map(|n| (n.id, &n.operation)).collect();
        let after_ops: HashMap<OperationId, &Operation> =
            after.graph.nodes().map(|n| (n.id, &n.operation)).collect();

        let mut operations_added = 0;
        let mut operations_removed = 0;
        let mut operations_rewritten = 0;

        for (id, op_after) in &after_ops {
            match before_ops.get(id) {
                Some(op_before) => {
                    if op_before != op_after {
                        operations_rewritten += 1;
                    }
                }
                None => {
                    operations_added += 1;
                }
            }
        }

        for id in before_ops.keys() {
            if !after_ops.contains_key(id) {
                operations_removed += 1;
            }
        }

        let global_phase_adjustment = if before.global_phase() != after.global_phase() {
            let empty_env = HashMap::new();
            let rad_before = before.global_phase().eval(&empty_env).unwrap_or(0.0);
            let rad_after = after.global_phase().eval(&empty_env).unwrap_or(0.0);
            let delta = rad_after - rad_before;
            Some(GlobalPhaseAdjustment::new(
                delta,
                format!("({} - {})", after.global_phase(), before.global_phase()),
            ))
        } else {
            None
        };

        Self {
            operations_added,
            operations_removed,
            operations_rewritten,
            global_phase_adjustment,
            analysis_invalidations: invalidated_analyses,
        }
    }
}

/// Verification evidence and provenance for a compiler pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PassVerification {
    /// Explicit verification status.
    pub status: PassVerificationStatus,
    /// Identifier of the verifier engine, or `None` if not run.
    pub verifier_name: Option<String>,
    /// Version string of the verifier engine, or `None` if not run.
    pub verifier_version: Option<String>,
    /// Verification method mode (e.g. `"exhaustive_unitary"`, `"sampled_statevector"`).
    pub verifier_mode: Option<String>,
    /// Provenance origin: internal vs independent oracle, or `None` if not run.
    pub provenance: Option<VerifierProvenance>,
    /// Tolerances applied during equivalence checking, or `None` if not run.
    pub tolerances: Option<BTreeMap<String, f64>>,
    /// Detailed diagnostic report or failure explanation.
    pub detail: Option<String>,
}

impl PassVerification {
    /// Constructs an unverified status record ([`PassVerificationStatus::NotRun`]).
    pub fn not_run() -> Self {
        Self {
            status: PassVerificationStatus::NotRun,
            verifier_name: None,
            verifier_version: None,
            verifier_mode: None,
            provenance: None,
            tolerances: None,
            detail: None,
        }
    }

    /// Constructs an unsupported status record ([`PassVerificationStatus::Unsupported`]).
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            status: PassVerificationStatus::Unsupported,
            verifier_name: None,
            verifier_version: None,
            verifier_mode: None,
            provenance: None,
            tolerances: None,
            detail: Some(reason.into()),
        }
    }

    /// Constructs a successful verification record from an internal QCC verifier.
    pub fn internal_passed(
        verifier_name: impl Into<String>,
        verifier_version: impl Into<String>,
        mode: impl Into<String>,
        tolerances: Option<BTreeMap<String, f64>>,
    ) -> Self {
        Self {
            status: PassVerificationStatus::Passed,
            verifier_name: Some(verifier_name.into()),
            verifier_version: Some(verifier_version.into()),
            verifier_mode: Some(mode.into()),
            provenance: Some(VerifierProvenance::Internal),
            tolerances,
            detail: None,
        }
    }

    /// Constructs a successful verification record from an independent oracle.
    pub fn oracle_passed(
        verifier_name: impl Into<String>,
        verifier_version: impl Into<String>,
        mode: impl Into<String>,
        tolerances: Option<BTreeMap<String, f64>>,
    ) -> Self {
        Self {
            status: PassVerificationStatus::Passed,
            verifier_name: Some(verifier_name.into()),
            verifier_version: Some(verifier_version.into()),
            verifier_mode: Some(mode.into()),
            provenance: Some(VerifierProvenance::IndependentOracle),
            tolerances,
            detail: None,
        }
    }

    /// Constructs a failed verification record.
    pub fn failed(
        verifier_name: impl Into<String>,
        detail: impl Into<String>,
        provenance: VerifierProvenance,
    ) -> Self {
        Self {
            status: PassVerificationStatus::Failed,
            verifier_name: Some(verifier_name.into()),
            verifier_version: None,
            verifier_mode: None,
            provenance: Some(provenance),
            tolerances: None,
            detail: Some(detail.into()),
        }
    }
}

/// Complete versioned per-pass diagnostic report.
///
/// Satisfies JSON Schema `qcc.pass_diagnostic` v1.0.0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PassDiagnosticReport {
    /// Canonical schema identifier (`"qcc.pass_diagnostic"`).
    pub schema_id: String,
    /// Semantic schema version (`"1.0.0"`).
    pub schema_version: String,
    /// Stable machine-readable identifier of the pass.
    pub pass_id: String,
    /// Human-readable display name of the pass.
    pub pass_name: String,
    /// 0-based execution sequence index within the compiler/pipeline run.
    pub execution_order: usize,
    /// Execution status of the pass (`"ran"`, `"skipped"`, or `"failed"`).
    pub status: PassStatus,
    /// Compiler identity and build metadata.
    pub compiler: CompilerIdentityManifest,
    /// Pass-specific configuration parameters and thresholds.
    pub pass_config: BTreeMap<String, serde_json::Value>,
    /// Input and output circuit fingerprints.
    pub fingerprints: CircuitFingerprints,
    /// Pre-pass and post-pass measurements and execution timing telemetry.
    pub measurements: PassMeasurements,
    /// DAG mutation accounting and analysis invalidations.
    pub transformation: TransformationAccounting,
    /// Semantic verification outcome and provenance.
    pub verification: PassVerification,
}

impl PassDiagnosticReport {
    /// Creates a new per-pass diagnostic report with standard schema headers and compiler identity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pass_id: impl Into<String>,
        pass_name: impl Into<String>,
        execution_order: usize,
        status: PassStatus,
        pass_config: BTreeMap<String, serde_json::Value>,
        fingerprints: CircuitFingerprints,
        measurements: PassMeasurements,
        transformation: TransformationAccounting,
        verification: PassVerification,
    ) -> Self {
        Self {
            schema_id: PASS_DIAGNOSTIC_SCHEMA_ID.to_string(),
            schema_version: PASS_DIAGNOSTIC_SCHEMA_VERSION.to_string(),
            pass_id: pass_id.into(),
            pass_name: pass_name.into(),
            execution_order,
            status,
            compiler: CompilerIdentityManifest::current(),
            pass_config,
            fingerprints,
            measurements,
            transformation,
            verification,
        }
    }

    /// Serializes this report to a deterministic, formatted JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a report from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Enforces architectural invariants on the diagnostic report.
    ///
    /// # Errors
    ///
    /// Returns [`PassDiagnosticValidationError`] if:
    /// - `schema_id` does not match [`PASS_DIAGNOSTIC_SCHEMA_ID`].
    /// - `schema_version` does not match [`PASS_DIAGNOSTIC_SCHEMA_VERSION`].
    /// - `semantic_equivalence_guaranteed` is `true`.
    /// - `status == Skipped` but `verification.status == Passed`.
    /// - `verification.status == Passed` but verifier provenance is missing.
    pub fn validate_invariants(&self) -> Result<(), PassDiagnosticValidationError> {
        if self.schema_id != PASS_DIAGNOSTIC_SCHEMA_ID {
            return Err(PassDiagnosticValidationError::InvalidSchemaId(
                self.schema_id.clone(),
            ));
        }
        if self.schema_version != PASS_DIAGNOSTIC_SCHEMA_VERSION {
            return Err(PassDiagnosticValidationError::InvalidSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        if self.fingerprints.semantic_equivalence_guaranteed {
            return Err(PassDiagnosticValidationError::SemanticEquivalenceGuaranteedTrue);
        }
        if self.status == PassStatus::Skipped
            && self.verification.status == PassVerificationStatus::Passed
        {
            return Err(PassDiagnosticValidationError::SkippedPassWithRunVerification);
        }
        if self.verification.status == PassVerificationStatus::Passed
            && self.verification.provenance.is_none()
        {
            return Err(PassDiagnosticValidationError::PassedWithoutProvenance);
        }
        Ok(())
    }
}

/// Serializes a sequence of per-pass diagnostic reports to a formatted JSON array string.
pub fn pass_diagnostics_to_json_pretty(
    reports: &[PassDiagnosticReport],
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(reports)
}

/// Deserializes a sequence of per-pass diagnostic reports from a JSON array string.
pub fn pass_diagnostics_from_json(
    json_str: &str,
) -> Result<Vec<PassDiagnosticReport>, serde_json::Error> {
    serde_json::from_str(json_str)
}

/// Validation errors for [`PassDiagnosticReport`] invariant enforcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassDiagnosticValidationError {
    /// `schema_id` is unexpected.
    InvalidSchemaId(String),
    /// `schema_version` is unexpected.
    InvalidSchemaVersion(String),
    /// Invariant violation: `semantic_equivalence_guaranteed` was set to `true`.
    SemanticEquivalenceGuaranteedTrue,
    /// Invariant violation: a skipped pass was marked with `verification.status == Passed`.
    SkippedPassWithRunVerification,
    /// Invariant violation: verification marked as `Passed` without specifying `provenance`.
    PassedWithoutProvenance,
}

impl fmt::Display for PassDiagnosticValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchemaId(id) => write!(f, "Invalid schema_id: '{id}'"),
            Self::InvalidSchemaVersion(v) => write!(f, "Invalid schema_version: '{v}'"),
            Self::SemanticEquivalenceGuaranteedTrue => {
                write!(f, "semantic_equivalence_guaranteed must always be false (fingerprint equivalence != semantic equivalence)")
            }
            Self::SkippedPassWithRunVerification => {
                write!(f, "Skipped pass cannot have verification status 'passed'")
            }
            Self::PassedWithoutProvenance => {
                write!(f, "Verification marked 'passed' requires explicit provenance ('internal' or 'independent_oracle')")
            }
        }
    }
}

impl std::error::Error for PassDiagnosticValidationError {}

/// Returns `true` if the circuit contains only unitary quantum operations and barriers (and has $\ge 1$ declared qubit).
pub fn is_circuit_unitary_eligible(circuit: &Circuit) -> bool {
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

/// Configuration for optional development and testing verification hooks at pass boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct PassVerificationHookConfig {
    /// Whether verification hooks are enabled at pass boundaries.
    ///
    /// Default: `false`. Verification is strictly opt-in to avoid imposing mandatory
    /// statevector simulation overhead during standard compilation runs.
    pub enabled: bool,
    /// Numerical tolerances applied during equivalence checking.
    pub tolerances: Option<VerificationTolerances>,
    /// Maximum qubit count for statevector simulation verification (default: 16).
    pub max_qubits: usize,
    /// Whether equivalence verification failure immediately aborts pass execution with an error.
    ///
    /// Default: `true`.
    pub fail_on_mismatch: bool,
}

impl Default for PassVerificationHookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tolerances: None,
            max_qubits: 16,
            fail_on_mismatch: true,
        }
    }
}

/// Structured error produced when a per-pass verification hook detects a violation.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum PassVerificationHookError {
    /// Semantic equivalence check failed (candidate circuit is not unitarily equivalent to reference).
    #[error("per-pass verification hook detected semantic mismatch: {detail}")]
    Mismatch {
        /// Diagnostic description of the mismatch.
        detail: String,
        /// Recorded verification evidence.
        verification: PassVerification,
    },

    /// Verification engine encountered an operational error.
    #[error("per-pass verification hook engine error: {source}")]
    EngineError {
        /// Underlying verification engine error.
        #[source]
        source: VerificationError,
        /// Recorded verification evidence.
        verification: PassVerification,
    },
}

/// Configuration for compiler pass diagnostic instrumentation and audit reporting.
#[derive(Clone, Debug, PartialEq)]
pub struct PassDiagnosticConfig {
    /// Whether pass diagnostic record collection is enabled.
    ///
    /// Default: `false`. When disabled, passes execute with zero diagnostic overhead
    /// and zero extra memory allocations.
    pub enabled: bool,
    /// Whether to compute before-and-after quantitative circuit snapshot metrics.
    ///
    /// Default: `true` (when enabled).
    pub collect_metrics: bool,
    /// Whether to compute cryptographic representation fingerprints (SHA-256) of circuits.
    ///
    /// Default: `true` (when enabled).
    pub collect_fingerprints: bool,
    /// Configuration for optional development/testing verification hooks.
    pub verification_hook: PassVerificationHookConfig,
}

impl Default for PassDiagnosticConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            collect_metrics: true,
            collect_fingerprints: true,
            verification_hook: PassVerificationHookConfig::default(),
        }
    }
}

impl PassDiagnosticConfig {
    /// Creates a disabled diagnostic configuration (zero overhead).
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Creates an enabled configuration for basic pass execution records only
    /// (status, pass identity, execution order, configuration, transformation accounting, and timing),
    /// completely omitting detailed circuit snapshot metrics and cryptographic fingerprints.
    pub fn basic() -> Self {
        Self {
            enabled: true,
            collect_metrics: false,
            collect_fingerprints: false,
            verification_hook: PassVerificationHookConfig::default(),
        }
    }

    /// Creates a standard diagnostic configuration collecting metrics and fingerprints,
    /// with verification hooks disabled by default.
    pub fn standard() -> Self {
        Self {
            enabled: true,
            collect_metrics: true,
            collect_fingerprints: true,
            verification_hook: PassVerificationHookConfig::default(),
        }
    }

    /// Creates a complete diagnostic configuration with metrics, fingerprints,
    /// and verification hooks enabled.
    pub fn full() -> Self {
        Self {
            enabled: true,
            collect_metrics: true,
            collect_fingerprints: true,
            verification_hook: PassVerificationHookConfig {
                enabled: true,
                ..Default::default()
            },
        }
    }

    /// Enables or disables collection of detailed before/after circuit snapshot metrics.
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.collect_metrics = enabled;
        self
    }

    /// Enables or disables cryptographic SHA-256 representation fingerprint computation.
    pub fn with_fingerprints(mut self, enabled: bool) -> Self {
        self.collect_fingerprints = enabled;
        self
    }

    /// Enables or disables verification hooks.
    pub fn with_verification(mut self, enabled: bool) -> Self {
        self.enabled = true;
        self.verification_hook.enabled = enabled;
        self
    }

    /// Sets custom tolerances for verification hooks.
    pub fn with_tolerances(mut self, tolerances: VerificationTolerances) -> Self {
        self.verification_hook.tolerances = Some(tolerances);
        self
    }

    /// Sets whether verification mismatch causes an immediate fatal error.
    pub fn with_fail_on_mismatch(mut self, fail_on_mismatch: bool) -> Self {
        self.verification_hook.fail_on_mismatch = fail_on_mismatch;
        self
    }
}

/// Executes an optional development/testing verification hook comparing circuits before and after a pass.
#[allow(clippy::result_large_err)]
pub fn execute_pass_verification_hook(
    before: &Circuit,
    after: &Circuit,
    config: &PassVerificationHookConfig,
) -> Result<PassVerification, PassVerificationHookError> {
    if !config.enabled {
        return Ok(PassVerification::not_run());
    }

    if before.num_qubits() == 0 || after.num_qubits() == 0 {
        return Ok(PassVerification::unsupported(
            "circuit contains 0 declared qubits",
        ));
    }

    if !is_circuit_unitary_eligible(before) || !is_circuit_unitary_eligible(after) {
        return Ok(PassVerification::unsupported(
            "circuit contains non-unitary operations (measurement, reset, or classical)",
        ));
    }

    if before.num_qubits() != after.num_qubits() {
        let detail = format!(
            "qubit count mismatch across pass: before={}, after={}",
            before.num_qubits(),
            after.num_qubits()
        );
        let verification = PassVerification::failed(
            "qcc_unitary_equivalence",
            detail.clone(),
            VerifierProvenance::Internal,
        );
        if config.fail_on_mismatch {
            return Err(PassVerificationHookError::Mismatch {
                detail,
                verification,
            });
        } else {
            return Ok(verification);
        }
    }

    if before.num_qubits() > config.max_qubits {
        return Ok(PassVerification::unsupported(format!(
            "circuit qubit count ({}) exceeds verification maximum ({})",
            before.num_qubits(),
            config.max_qubits
        )));
    }

    let tolerances = config.tolerances.unwrap_or_default();
    let checker = EquivalenceChecker::new().with_tolerances(tolerances);

    let mut tol_map = BTreeMap::new();
    tol_map.insert("state_tolerance".to_string(), tolerances.state_tolerance);
    tol_map.insert("matrix_tolerance".to_string(), tolerances.matrix_tolerance);
    tol_map.insert("phase_tolerance".to_string(), tolerances.phase_tolerance);
    tol_map.insert(
        "amplitude_threshold".to_string(),
        tolerances.amplitude_threshold,
    );
    tol_map.insert("norm_tolerance".to_string(), tolerances.norm_tolerance);

    match checker.check_equivalence(before, after) {
        Ok(verdict) => {
            let mode_str = match verdict.method {
                VerificationMethod::ComputationalBasis { num_basis_states } => {
                    format!("exhaustive_unitary_{}_states", num_basis_states)
                }
                VerificationMethod::RandomizedStates { num_states } => {
                    format!("randomized_{}_states", num_states)
                }
            };

            if verdict.is_equivalent {
                Ok(PassVerification::internal_passed(
                    "qcc_unitary_equivalence",
                    env!("CARGO_PKG_VERSION"),
                    mode_str,
                    Some(tol_map),
                ))
            } else {
                let detail = verdict
                    .worst_mismatch
                    .as_ref()
                    .map(|m| {
                        format!(
                            "amplitude discrepancy at output index {} (diff: {:.2e})",
                            m.output_amplitude_index, m.amplitude_difference
                        )
                    })
                    .unwrap_or_else(|| "unitary equivalence check failed".to_string());

                let mut failed_ver = PassVerification::failed(
                    "qcc_unitary_equivalence",
                    detail.clone(),
                    VerifierProvenance::Internal,
                );
                failed_ver.verifier_version = Some(env!("CARGO_PKG_VERSION").to_string());
                failed_ver.verifier_mode = Some(mode_str);
                failed_ver.tolerances = Some(tol_map);

                if config.fail_on_mismatch {
                    Err(PassVerificationHookError::Mismatch {
                        detail,
                        verification: failed_ver,
                    })
                } else {
                    Ok(failed_ver)
                }
            }
        }
        Err(err) => match &err {
            VerificationError::NonUnitaryOperation { reason, .. } => {
                Ok(PassVerification::unsupported(reason.clone()))
            }
            VerificationError::SystemTooLargeForMatrix { qubits, max_qubits } => {
                Ok(PassVerification::unsupported(format!(
                    "system size of {qubits} qubits exceeds matrix limit of {max_qubits} qubits"
                )))
            }
            _ => {
                let detail = err.to_string();
                let failed_ver = PassVerification::failed(
                    "qcc_unitary_equivalence",
                    detail.clone(),
                    VerifierProvenance::Internal,
                );
                if config.fail_on_mismatch {
                    Err(PassVerificationHookError::EngineError {
                        source: err,
                        verification: failed_ver,
                    })
                } else {
                    Ok(failed_ver)
                }
            }
        },
    }
}

/// Helper building a diagnostic report for an executed pass ([`PassStatus::Ran`]).
#[allow(clippy::too_many_arguments)]
pub fn build_ran_pass_report(
    pass_id: impl Into<String>,
    pass_name: impl Into<String>,
    execution_order: usize,
    pass_config: BTreeMap<String, serde_json::Value>,
    before: &Circuit,
    after: &Circuit,
    duration_ms: Option<f64>,
    invalidated_analyses: Vec<String>,
    verification: PassVerification,
    config: &PassDiagnosticConfig,
) -> PassDiagnosticReport {
    let fingerprints = if config.collect_fingerprints {
        CircuitFingerprints::from_circuits(Some(before), Some(after))
            .unwrap_or_else(|_| CircuitFingerprints::uncomputed())
    } else {
        CircuitFingerprints::uncomputed()
    };

    let (before_snap, after_snap) = if config.collect_metrics {
        (
            Some(CircuitSnapshotMetrics::from_circuit(before)),
            Some(CircuitSnapshotMetrics::from_circuit(after)),
        )
    } else {
        (None, None)
    };

    let mut availability = BTreeMap::new();
    availability.insert(
        "before".to_string(),
        if before_snap.is_some() {
            MeasurementAvailability::Available
        } else {
            MeasurementAvailability::NotMeasured
        },
    );
    availability.insert(
        "after".to_string(),
        if after_snap.is_some() {
            MeasurementAvailability::Available
        } else {
            MeasurementAvailability::NotMeasured
        },
    );
    availability.insert(
        "duration_ms".to_string(),
        if duration_ms.is_some() {
            MeasurementAvailability::Available
        } else {
            MeasurementAvailability::NotMeasured
        },
    );

    let measurements = PassMeasurements::new(before_snap, after_snap, duration_ms, availability);
    let transformation =
        TransformationAccounting::from_circuit_transition(before, after, invalidated_analyses);

    PassDiagnosticReport::new(
        pass_id,
        pass_name,
        execution_order,
        PassStatus::Ran,
        pass_config,
        fingerprints,
        measurements,
        transformation,
        verification,
    )
}

/// Helper building a diagnostic report for a skipped pass ([`PassStatus::Skipped`]).
pub fn build_skipped_pass_report(
    pass_id: impl Into<String>,
    pass_name: impl Into<String>,
    execution_order: usize,
    pass_config: BTreeMap<String, serde_json::Value>,
    circuit: Option<&Circuit>,
    config: &PassDiagnosticConfig,
) -> PassDiagnosticReport {
    let input_fp = if config.collect_fingerprints {
        circuit.and_then(|c| {
            crate::ir::circuit_to_json(c)
                .ok()
                .map(|json| crate::compiler::manifest::sha256_hex(json.as_bytes()))
        })
    } else {
        None
    };

    let fingerprints = CircuitFingerprints {
        algorithm: "sha256".to_string(),
        input_fingerprint: input_fp,
        output_fingerprint: None,
        semantic_equivalence_guaranteed: false,
    };

    let before_snap = if config.collect_metrics {
        circuit.map(CircuitSnapshotMetrics::from_circuit)
    } else {
        None
    };

    let mut availability = BTreeMap::new();
    availability.insert(
        "before".to_string(),
        if before_snap.is_some() {
            MeasurementAvailability::Available
        } else if circuit.is_some() {
            MeasurementAvailability::NotMeasured
        } else {
            MeasurementAvailability::NotApplicable
        },
    );
    availability.insert("after".to_string(), MeasurementAvailability::NotApplicable);
    availability.insert(
        "duration_ms".to_string(),
        MeasurementAvailability::NotApplicable,
    );

    let measurements = PassMeasurements::new(before_snap, None, None, availability);
    let transformation = TransformationAccounting::no_op();
    let verification = PassVerification::not_run();

    PassDiagnosticReport::new(
        pass_id,
        pass_name,
        execution_order,
        PassStatus::Skipped,
        pass_config,
        fingerprints,
        measurements,
        transformation,
        verification,
    )
}

/// Helper building a diagnostic report for a failed pass ([`PassStatus::Failed`]).
#[allow(clippy::too_many_arguments)]
pub fn build_failed_pass_report(
    pass_id: impl Into<String>,
    pass_name: impl Into<String>,
    execution_order: usize,
    pass_config: BTreeMap<String, serde_json::Value>,
    circuit_before: Option<&Circuit>,
    duration_ms: Option<f64>,
    verification: Option<PassVerification>,
    config: &PassDiagnosticConfig,
) -> PassDiagnosticReport {
    let input_fp = if config.collect_fingerprints {
        circuit_before.and_then(|c| {
            crate::ir::circuit_to_json(c)
                .ok()
                .map(|json| crate::compiler::manifest::sha256_hex(json.as_bytes()))
        })
    } else {
        None
    };

    let fingerprints = CircuitFingerprints {
        algorithm: "sha256".to_string(),
        input_fingerprint: input_fp,
        output_fingerprint: None,
        semantic_equivalence_guaranteed: false,
    };

    let before_snap = if config.collect_metrics {
        circuit_before.map(CircuitSnapshotMetrics::from_circuit)
    } else {
        None
    };

    let mut availability = BTreeMap::new();
    availability.insert(
        "before".to_string(),
        if before_snap.is_some() {
            MeasurementAvailability::Available
        } else {
            MeasurementAvailability::NotMeasured
        },
    );
    availability.insert("after".to_string(), MeasurementAvailability::Unavailable);
    availability.insert(
        "duration_ms".to_string(),
        if duration_ms.is_some() {
            MeasurementAvailability::Available
        } else {
            MeasurementAvailability::NotMeasured
        },
    );

    let measurements = PassMeasurements::new(before_snap, None, duration_ms, availability);
    let transformation = TransformationAccounting::no_op();
    let verification = verification.unwrap_or_else(PassVerification::not_run);

    PassDiagnosticReport::new(
        pass_id,
        pass_name,
        execution_order,
        PassStatus::Failed,
        pass_config,
        fingerprints,
        measurements,
        transformation,
        verification,
    )
}
