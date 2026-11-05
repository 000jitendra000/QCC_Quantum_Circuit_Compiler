//! Integrated End-to-End Compilation Pipeline.
//!
//! Connects logical circuit optimization, target basis translation, initial placement,
//! circuit routing, final target-specific translation, final target legality validation,
//! post-routing physical scheduling, and layout-aware semantic verification into an
//! ordered, reproducible, and verifiable compilation pipeline.
//!
//! # Architecture & Pipeline Ordering
//!
//! The pipeline executes in mathematically grounded, strictly ordered stages:
//!
//! 1. **Logical Optimization ([`crate::optimizer`])**:
//!    Fixed-point algebraic simplification (identity elimination, inverse cancellation,
//!    rotation merging, Euler gate fusion, and DAG-aware commutation reordering).
//!
//! 2. **Target Basis Translation ([`crate::translation`])**:
//!    Pre-routing decomposition of high-level and unsupported operations into the
//!    target's native routing basis (e.g., $CX$ or $CZ$ plus single-qubit rotations).
//!
//! 3. **Placement & Routing ([`crate::routing`])**:
//!    Initial physical qubit placement followed by shortest-path or bidirectional SABRE
//!    routing along physical hardware coupling edges with layout tracking ($L_{\text{initial}}$,
//!    $L_{\text{final}}$) and physical $SWAP$ insertion.
//!
//! 4. **Final Target Translation ([`crate::translation`])**:
//!    Post-routing two-qubit direction correction (e.g., reversing $CX$ on directed coupling edges
//!    via verified Hadamard conjugation) and lowering of introduced operations to native target operations.
//!
//! 5. **Final Target Legality Validation ([`crate::target::legality`])**:
//!    Strict fail-closed validation confirming that every emitted physical operation, operand tuple,
//!    and parameter is fully supported by the physical [`crate::target::model::Target`].
//!
//! 6. **Post-Routing Scheduling ([`crate::scheduler`])**:
//!    Optional physical timeline scheduling (ASAP, ALAP, or resource-constrained) operating
//!    exclusively on the final routed physical DAG.
//!
//! 7. **Layout-Aware Semantic Verification ([`crate::verification`])**:
//!    End-to-end unitary equivalence verification proving that the final physical circuit
//!    faithfully implements the logical circuit under $L_{\text{initial}}$ and $L_{\text{final}}$,
//!    strictly enforcing ADR-0001 (unused physical ancillae must start and end in $|0\rangle$).

pub mod config;
pub mod pipeline;
pub mod result;

pub use config::{
    CompilerConfig, RouterChoice, SchedulerChoice, TranslationConfig, VerificationConfig,
};
pub use pipeline::{CompilerError, CompilerPipeline};
pub use result::{
    CompilationReport, CompilationResult, CompilerConfigSummary, PipelineMetricsSummary,
    PostRoutingTranslationReport, RoutingStageReport, SchedulingStageReport,
    VerificationStageReport, VerificationStatus,
};
