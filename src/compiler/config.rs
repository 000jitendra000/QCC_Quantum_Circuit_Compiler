//! Compiler configuration model and preset options.
//!
//! Provides comprehensive, reproducible configuration across all compiler pipeline stages:
//! logical optimization, basis translation, placement, routing, post-routing scheduling,
//! and layout-aware verification.

use crate::optimizer::config::{OptimizationLevel, OptimizerConfig};
use crate::routing::greedy::BaselineRouterConfig;
use crate::routing::layout_map::LayoutMap;
use crate::routing::placement::PlacementStrategy;
use crate::routing::sabre::SabreRouterConfig;
use crate::scheduler::alap::AlapMakespanPolicy;
use crate::scheduler::asap::ScheduleConfig;
use crate::verification::VerificationTolerances;

/// Selection of routing algorithm and its configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum RouterChoice {
    /// Bidirectional SABRE router with front-layer analysis, extended lookahead, and decay.
    Sabre(SabreRouterConfig),
    /// Baseline shortest-path router using deterministic Dijkstra routing.
    Baseline(BaselineRouterConfig),
}

impl Default for RouterChoice {
    fn default() -> Self {
        Self::Sabre(SabreRouterConfig::default())
    }
}

/// Selection of post-routing physical circuit scheduler.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SchedulerChoice {
    /// Post-routing scheduling is disabled (no `Schedule` or timeline generated).
    #[default]
    Disabled,
    /// As Soon As Possible (ASAP) scheduler.
    Asap(ScheduleConfig),
    /// As Late As Possible (ALAP) scheduler with configurable makespan policy.
    Alap(ScheduleConfig, AlapMakespanPolicy),
    /// Resource-constrained scheduler with timeline calendar and placement blocker explainability.
    ResourceConstrained,
}

/// Configuration options for layout-aware semantic verification.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationConfig {
    /// Whether layout-aware unitary verification is enabled for unitary circuits.
    pub enabled: bool,
    /// Maximum physical qubit count for full statevector verification (default: 16).
    pub max_qubits: usize,
    /// Numerical tolerances for statevector amplitude and global phase comparison.
    pub tolerances: VerificationTolerances,
    /// Whether verification mismatch immediately fails compilation with an error.
    pub fail_on_mismatch: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_qubits: 16,
            tolerances: VerificationTolerances::default(),
            fail_on_mismatch: true,
        }
    }
}

/// Configuration options for pre- and post-routing basis translation.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslationConfig {
    /// Maximum recursive decomposition depth.
    pub max_depth: usize,
    /// Whether local unitary equivalence is verified after each rule decomposition.
    pub verify_local_equivalence: bool,
    /// Whether post-routing two-qubit direction correction is enabled.
    pub enable_direction_correction: bool,
    /// Whether post-routing final basis lowering is enabled for any non-native gates.
    pub enable_final_lowering: bool,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            verify_local_equivalence: true,
            enable_direction_correction: true,
            enable_final_lowering: true,
        }
    }
}

/// Comprehensive, reproducible configuration for the end-to-end compilation pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct CompilerConfig {
    /// Logical optimization configuration.
    pub optimization: OptimizerConfig,
    /// Basis translation configuration.
    pub translation: TranslationConfig,
    /// Initial placement strategy (if explicit initial layout is not provided).
    pub placement_strategy: PlacementStrategy,
    /// Optional user-specified explicit initial layout.
    pub explicit_initial_layout: Option<LayoutMap>,
    /// Router selection and parameters.
    pub router: RouterChoice,
    /// Optional post-routing scheduling selection.
    pub scheduler: SchedulerChoice,
    /// Layout-aware semantic verification options.
    pub verification: VerificationConfig,
    /// Whether to enforce strict target legality validation fail-closed (default: true).
    pub validate_target_legality: bool,
    /// Whether to run debug graph validation after each individual stage.
    pub verify_each_stage: bool,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            optimization: OptimizerConfig::new(OptimizationLevel::O2),
            translation: TranslationConfig::default(),
            placement_strategy: PlacementStrategy::Weighted,
            explicit_initial_layout: None,
            router: RouterChoice::default(),
            scheduler: SchedulerChoice::Disabled,
            verification: VerificationConfig::default(),
            validate_target_legality: true,
            verify_each_stage: false,
        }
    }
}

impl CompilerConfig {
    /// Creates a compiler configuration targeting a specific optimization level.
    pub fn for_level(level: OptimizationLevel) -> Self {
        Self {
            optimization: OptimizerConfig::new(level),
            ..Default::default()
        }
    }

    /// Creates a zero-optimization pipeline configuration (O0: pure structural translation).
    pub fn o0() -> Self {
        Self::for_level(OptimizationLevel::O0)
    }

    /// Creates a basic peephole optimization configuration (O1).
    pub fn o1() -> Self {
        Self::for_level(OptimizationLevel::O1)
    }

    /// Creates a standard logical optimization configuration (O2, default).
    pub fn o2() -> Self {
        Self::for_level(OptimizationLevel::O2)
    }

    /// Creates an aggressive logical optimization configuration (O3: includes commutation reordering).
    pub fn o3() -> Self {
        Self::for_level(OptimizationLevel::O3)
    }

    /// Sets the logical optimizer configuration.
    pub fn with_optimizer_config(mut self, config: OptimizerConfig) -> Self {
        self.optimization = config;
        self
    }

    /// Sets the optimization level.
    pub fn with_optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization = OptimizerConfig::new(level);
        self
    }

    /// Sets the initial placement strategy.
    pub fn with_placement_strategy(mut self, strategy: PlacementStrategy) -> Self {
        self.placement_strategy = strategy;
        self
    }

    /// Sets an explicit initial layout mapping logical qubits to physical device qubits.
    pub fn with_initial_layout(mut self, layout: LayoutMap) -> Self {
        self.explicit_initial_layout = Some(layout);
        self
    }

    /// Sets the circuit router choice.
    pub fn with_router(mut self, router: RouterChoice) -> Self {
        self.router = router;
        self
    }

    /// Sets the post-routing scheduler choice.
    pub fn with_scheduler(mut self, scheduler: SchedulerChoice) -> Self {
        self.scheduler = scheduler;
        self
    }

    /// Configures the pipeline to run ASAP post-routing scheduling with default parameters.
    pub fn with_asap_scheduling(mut self) -> Self {
        self.scheduler = SchedulerChoice::Asap(ScheduleConfig::default());
        self
    }

    /// Configures the pipeline to run ALAP post-routing scheduling with ASAP-derived makespan.
    pub fn with_alap_scheduling(mut self) -> Self {
        self.scheduler = SchedulerChoice::Alap(
            ScheduleConfig::default(),
            AlapMakespanPolicy::DerivedFromAsap,
        );
        self
    }

    /// Configures the pipeline to run resource-constrained scheduling.
    pub fn with_resource_constrained_scheduling(mut self) -> Self {
        self.scheduler = SchedulerChoice::ResourceConstrained;
        self
    }

    /// Configures whether layout-aware semantic verification is enabled.
    pub fn with_verification(mut self, enabled: bool) -> Self {
        self.verification.enabled = enabled;
        self
    }

    /// Sets custom verification numerical tolerances.
    pub fn with_verification_tolerances(mut self, tolerances: VerificationTolerances) -> Self {
        self.verification.tolerances = tolerances;
        self
    }

    /// Configures whether debug graph validation runs after each stage.
    pub fn with_verify_each_stage(mut self, verify: bool) -> Self {
        self.verify_each_stage = verify;
        self
    }
}
