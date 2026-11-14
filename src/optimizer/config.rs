//! Configuration and optimization levels for the integrated compiler optimizer pipeline.

use std::collections::HashMap;
use std::fmt;

use crate::compiler::pass_diagnostic::PassDiagnosticConfig;

/// Optimization level controlling the aggressiveness and scope of transformations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OptimizationLevel {
    /// O0: No optimization. Pure structural validation and metric collection without mutations.
    O0,
    /// O1: Basic peephole optimization. Executes identity elimination and inverse cancellation in a single sweep.
    O1,
    /// O2 (Default): Standard logical optimization. Fixed-point loop of identity elimination, inverse cancellation,
    /// same-axis rotation merging, and single-qubit Euler gate fusion.
    #[default]
    O2,
    /// O3: Aggressive logical optimization. Fixed-point loop including DAG-aware commutation reordering
    /// (`ReorderStrategy::ExposeOpportunities`), identity elimination, inverse cancellation,
    /// rotation merging, and single-qubit fusion.
    O3,
}

impl fmt::Display for OptimizationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::O0 => write!(f, "O0"),
            Self::O1 => write!(f, "O1"),
            Self::O2 => write!(f, "O2"),
            Self::O3 => write!(f, "O3"),
        }
    }
}

/// Default maximum number of fixed-point optimization loop iterations.
pub const DEFAULT_MAX_OPTIMIZATION_ITERATIONS: usize = 10;

/// Default numerical tolerance for floating-point comparisons in rotation algebra and matrix equivalence.
pub const DEFAULT_OPTIMIZATION_TOLERANCE: f64 = 1e-9;

/// Default lookahead window size for commutation reordering in O3.
pub const DEFAULT_PIPELINE_REORDER_WINDOW: usize = 8;

/// Default rewrite budget per iteration for commutation reordering in O3.
pub const DEFAULT_PIPELINE_REORDER_MAX_REWRITES: usize = 50;

/// Configuration for the integrated compiler optimizer pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerConfig {
    /// Optimization aggressiveness level.
    pub level: OptimizationLevel,
    /// Maximum number of outer fixed-point iterations before forcing termination.
    pub max_iterations: usize,
    /// Whether QccIR structural graph validation is unconditionally run after every pass.
    pub verify_graph: bool,
    /// Whether Phase 10 unitary equivalence is verified after every pass for pure unitary circuits.
    pub verify_equivalence: bool,
    /// Evaluation environment for symbolic rotation parameters.
    pub symbol_env: HashMap<String, f64>,
    /// Floating-point comparison tolerance for rotation angle algebra and matrix equivalence.
    pub tolerance: f64,
    /// Maximum lookahead/lookbehind window size for commutation reordering (O3).
    pub reorder_window: usize,
    /// Maximum number of adjacent swaps per iteration for commutation reordering (O3).
    pub reorder_max_rewrites: usize,
    /// Whether wall-clock duration measurement is enabled for performance telemetry.
    pub measure_durations: bool,
    /// Whether single-qubit Euler gate fusion is enabled in O2/O3.
    pub one_qubit_fusion: bool,
    /// Whether commutative two-qubit (CX/CZ) cancellation is enabled.
    /// In Phase 80, this remains opt-in (default: `false`) pending Phase 81 independent oracle benchmark validation.
    pub enable_commutative_cancellation: bool,
    /// Whether logical SWAP operations are elided into output wire permutations (ADR-0006).
    /// Remains opt-in (default: `false`) pending WP3 benchmark evaluation.
    pub elide_swaps: bool,
    /// Configuration for per-pass diagnostic instrumentation and audit reporting.
    pub diagnostics: PassDiagnosticConfig,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            level: OptimizationLevel::O2,
            max_iterations: DEFAULT_MAX_OPTIMIZATION_ITERATIONS,
            verify_graph: true,
            verify_equivalence: false,
            symbol_env: HashMap::new(),
            tolerance: DEFAULT_OPTIMIZATION_TOLERANCE,
            reorder_window: DEFAULT_PIPELINE_REORDER_WINDOW,
            reorder_max_rewrites: DEFAULT_PIPELINE_REORDER_MAX_REWRITES,
            measure_durations: true,
            one_qubit_fusion: true,
            enable_commutative_cancellation: false,
            elide_swaps: false,
            diagnostics: PassDiagnosticConfig::default(),
        }
    }
}

impl OptimizerConfig {
    /// Creates a new optimizer configuration with the specified optimization level.
    pub fn new(level: OptimizationLevel) -> Self {
        let max_iterations = match level {
            OptimizationLevel::O0 => 0,
            OptimizationLevel::O1 => 1,
            OptimizationLevel::O2 | OptimizationLevel::O3 => DEFAULT_MAX_OPTIMIZATION_ITERATIONS,
        };
        Self {
            level,
            max_iterations,
            ..Default::default()
        }
    }

    /// Sets the optimization level.
    pub fn with_level(mut self, level: OptimizationLevel) -> Self {
        self.level = level;
        if level == OptimizationLevel::O0 {
            self.max_iterations = 0;
        } else if level == OptimizationLevel::O1 {
            self.max_iterations = 1;
        }
        self
    }

    /// Sets the maximum number of fixed-point iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Enables or disables post-pass QccIR graph validation.
    pub fn with_graph_verification(mut self, enabled: bool) -> Self {
        self.verify_graph = enabled;
        self
    }

    /// Enables or disables post-pass unitary equivalence verification.
    pub fn with_equivalence_verification(mut self, enabled: bool) -> Self {
        self.verify_equivalence = enabled;
        self
    }

    /// Sets the runtime symbol evaluation environment.
    pub fn with_symbol_env(mut self, env: HashMap<String, f64>) -> Self {
        self.symbol_env = env;
        self
    }

    /// Sets the numerical comparison tolerance.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Sets the commutation reorder lookahead window size.
    pub fn with_reorder_window(mut self, window: usize) -> Self {
        self.reorder_window = window;
        self
    }

    /// Sets the commutation reorder maximum rewrites budget.
    pub fn with_reorder_max_rewrites(mut self, max_rewrites: usize) -> Self {
        self.reorder_max_rewrites = max_rewrites;
        self
    }

    /// Enables or disables telemetry duration measurement.
    pub fn with_duration_measurement(mut self, enabled: bool) -> Self {
        self.measure_durations = enabled;
        self
    }

    /// Enables or disables single-qubit Euler gate fusion.
    pub fn with_one_qubit_fusion(mut self, enabled: bool) -> Self {
        self.one_qubit_fusion = enabled;
        self
    }

    /// Enables or disables commutative two-qubit (CX/CZ) cancellation.
    pub fn with_commutative_cancellation(mut self, enabled: bool) -> Self {
        self.enable_commutative_cancellation = enabled;
        self
    }

    /// Enables or disables explicit logical SWAP elision into output wire permutations (ADR-0006).
    pub fn with_elide_swaps(mut self, enabled: bool) -> Self {
        self.elide_swaps = enabled;
        self
    }

    /// Sets the per-pass diagnostic instrumentation configuration.
    pub fn with_diagnostics(mut self, diagnostics: PassDiagnosticConfig) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}
