//! Analysis dependency tracking, invalidation models, and pass execution context.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::compiler::pass_diagnostic::PassDiagnosticReport;

/// Strongly-typed identifier for static and dynamic compiler analyses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnalysisId(pub &'static str);

impl AnalysisId {
    /// Creates a new static analysis identifier.
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Built-in analysis: Gate count and operation summary.
    pub const GATE_COUNT: Self = Self::new("gate_count");

    /// Built-in analysis: Deterministic topological operation ordering.
    pub const TOPOLOGICAL_ORDER: Self = Self::new("topological_order");

    /// Built-in analysis: Qubit participation and wire activity.
    pub const QUBIT_USAGE: Self = Self::new("qubit_usage");

    /// Built-in analysis: Estimated circuit critical path depth.
    pub const CIRCUIT_DEPTH: Self = Self::new("circuit_depth");
}

impl fmt::Display for AnalysisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Explicit set of analyses guaranteed to remain valid after pass execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreservedAnalyses {
    /// All cached analyses remain valid (typical for analysis-only or no-op passes).
    All,
    /// No cached analyses remain valid; all existing analyses are invalidated.
    None,
    /// An explicit deterministic subset of analyses that remain valid.
    Preserve(BTreeSet<AnalysisId>),
}

impl PreservedAnalyses {
    /// Preserves all cached analyses.
    #[inline]
    pub fn all() -> Self {
        Self::All
    }

    /// Preserves no analyses (conservative default for general mutating passes).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Preserves a single specific analysis.
    pub fn preserve_one(id: AnalysisId) -> Self {
        let mut set = BTreeSet::new();
        set.insert(id);
        Self::Preserve(set)
    }

    /// Preserves an explicit set of analyses.
    pub fn preserve_set(ids: impl IntoIterator<Item = AnalysisId>) -> Self {
        Self::Preserve(ids.into_iter().collect())
    }

    /// Returns `true` if the given analysis is preserved under this declaration.
    pub fn is_preserved(&self, id: &AnalysisId) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::Preserve(set) => set.contains(id),
        }
    }

    /// Returns `true` if all analyses are preserved.
    #[inline]
    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    /// Returns `true` if no analyses are preserved.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Execution context passed to compiler passes, providing immutable access to valid analyses
/// and tracking analysis life-cycles.
#[derive(Default)]
pub struct PassContext {
    /// Cached analysis entries keyed by analysis identifier.
    analyses: BTreeMap<AnalysisId, Box<dyn Any + Send + Sync>>,
    /// Configurable debug verification switch.
    debug_verification: bool,
    /// Chronological log of per-pass diagnostic reports.
    diagnostics: Vec<PassDiagnosticReport>,
}

impl PassContext {
    /// Creates a new empty `PassContext`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables or disables post-pass debug verification.
    pub fn with_debug_verification(mut self, enabled: bool) -> Self {
        self.debug_verification = enabled;
        self
    }

    /// Returns an immutable slice of all per-pass diagnostic reports recorded in this context.
    #[inline]
    pub fn diagnostic_reports(&self) -> &[PassDiagnosticReport] {
        &self.diagnostics
    }

    /// Appends a per-pass diagnostic report to the context.
    #[inline]
    pub fn push_diagnostic_report(&mut self, report: PassDiagnosticReport) {
        self.diagnostics.push(report);
    }

    /// Clears any recorded diagnostic reports from this context.
    #[inline]
    pub fn clear_diagnostic_reports(&mut self) {
        self.diagnostics.clear();
    }

    /// Returns `true` if debug verification is active in this context.
    #[inline]
    pub fn is_debug_verification_enabled(&self) -> bool {
        self.debug_verification
    }

    /// Sets the debug verification flag.
    pub fn set_debug_verification(&mut self, enabled: bool) {
        self.debug_verification = enabled;
    }

    /// Checks if a valid cached result exists for the specified analysis.
    pub fn has_analysis(&self, id: &AnalysisId) -> bool {
        self.analyses.contains_key(id)
    }

    /// Retrieves an immutable reference to a cached analysis result.
    ///
    /// Returns `None` if the analysis is not present or if the downcast to `T` fails.
    pub fn get_analysis<T: 'static>(&self, id: AnalysisId) -> Option<&T> {
        self.analyses
            .get(&id)
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Inserts or updates a cached analysis result in the context.
    pub fn insert_analysis<T: 'static + Send + Sync>(&mut self, id: AnalysisId, result: T) {
        self.analyses.insert(id, Box::new(result));
    }

    /// Returns a sorted list of all currently valid analysis identifiers.
    pub fn available_analyses(&self) -> Vec<AnalysisId> {
        self.analyses.keys().copied().collect()
    }

    /// Invalidates all analyses not explicitly preserved by the provided declaration.
    ///
    /// Returns the list of analysis IDs that were purged.
    pub fn invalidate(&mut self, preserved: &PreservedAnalyses) -> Vec<AnalysisId> {
        match preserved {
            PreservedAnalyses::All => Vec::new(),
            PreservedAnalyses::None => self.invalidate_all(),
            PreservedAnalyses::Preserve(preserved_set) => {
                let to_remove: Vec<AnalysisId> = self
                    .analyses
                    .keys()
                    .copied()
                    .filter(|id| !preserved_set.contains(id))
                    .collect();

                for id in &to_remove {
                    self.analyses.remove(id);
                }
                to_remove
            }
        }
    }

    /// Purges all cached analyses from the context.
    ///
    /// Returns the list of all purged analysis IDs.
    pub fn invalidate_all(&mut self) -> Vec<AnalysisId> {
        let keys: Vec<AnalysisId> = self.analyses.keys().copied().collect();
        self.analyses.clear();
        keys
    }

    /// Invalidates a single specific analysis if present.
    ///
    /// Returns `true` if the analysis was present and removed.
    pub fn invalidate_one(&mut self, id: &AnalysisId) -> bool {
        self.analyses.remove(id).is_some()
    }
}
