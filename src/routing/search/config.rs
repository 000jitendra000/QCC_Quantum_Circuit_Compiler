//! Configuration, budgets, and builder patterns for multi-candidate routing search.
//!
//! Governs candidate budgets, seed derivation, layout strategies, and evaluation objectives.

use serde::{Deserialize, Serialize};

use crate::routing::layout_map::LayoutMap;
use crate::routing::placement::PlacementStrategy;
use crate::routing::sabre::SabreRouterConfig;
use crate::routing::search::candidate::{
    compute_sabre_config_hash, compute_target_hash, CandidateDescriptor, CandidateIdentity,
};
use crate::routing::search::error::RoutingSearchConfigError;
use crate::routing::search::objective::RoutingObjective;
use crate::target::model::Target;

/// Maximum allowable candidate count upper bound to strictly prevent combinatorial explosion.
pub const HARD_MAX_CANDIDATE_LIMIT: usize = 256;

/// Default candidate budget limit.
pub const DEFAULT_MAX_CANDIDATES: usize = 64;

/// Deterministic seed selection or derivation strategy for candidate evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedStrategy {
    /// Single baseline seed (or `None` for unseeded/default router).
    Single(Option<u64>),
    /// Explicit deterministic sequence of seeds.
    Explicit(Vec<u64>),
    /// Deterministically derive N seeds from a base seed using a stable splitmix64 expansion.
    Derived {
        /// Base seed for derivation.
        base_seed: u64,
        /// Number of seeds to generate.
        count: usize,
    },
    /// Deterministic arithmetic sequence: `start + i * step`.
    Range {
        /// Sequence start value.
        start: u64,
        /// Sequence step delta (must be > 0).
        step: u64,
        /// Number of seeds to generate.
        count: usize,
    },
}

impl Default for SeedStrategy {
    fn default() -> Self {
        Self::Single(None)
    }
}

impl SeedStrategy {
    /// Validates the seed strategy parameters.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        match self {
            Self::Single(_) => Ok(()),
            Self::Explicit(seeds) => {
                if seeds.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidSeedConfiguration(
                        "explicit seed list cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }
            Self::Derived { count, .. } => {
                if *count == 0 {
                    return Err(RoutingSearchConfigError::InvalidSeedConfiguration(
                        "derived seed count must be at least 1".to_string(),
                    ));
                }
                if *count > HARD_MAX_CANDIDATE_LIMIT {
                    return Err(RoutingSearchConfigError::InvalidCandidateCount {
                        count: *count,
                        min: 1,
                        max: HARD_MAX_CANDIDATE_LIMIT,
                    });
                }
                Ok(())
            }
            Self::Range { count, step, .. } => {
                if *count == 0 {
                    return Err(RoutingSearchConfigError::InvalidSeedConfiguration(
                        "range seed count must be at least 1".to_string(),
                    ));
                }
                if *step == 0 {
                    return Err(RoutingSearchConfigError::InvalidSeedConfiguration(
                        "range seed step must be non-zero".to_string(),
                    ));
                }
                if *count > HARD_MAX_CANDIDATE_LIMIT {
                    return Err(RoutingSearchConfigError::InvalidCandidateCount {
                        count: *count,
                        min: 1,
                        max: HARD_MAX_CANDIDATE_LIMIT,
                    });
                }
                Ok(())
            }
        }
    }

    /// Generates a deterministic sequence of seeds matching the requested candidate count.
    pub fn generate_seeds(
        &self,
        count: usize,
    ) -> Result<Vec<Option<u64>>, RoutingSearchConfigError> {
        self.validate()?;
        let mut seeds = Vec::with_capacity(count);
        match self {
            Self::Single(s) => {
                for _ in 0..count {
                    seeds.push(*s);
                }
            }
            Self::Explicit(explicit) => {
                for i in 0..count {
                    seeds.push(Some(explicit[i % explicit.len()]));
                }
            }
            Self::Derived { base_seed, .. } => {
                for i in 0..count {
                    seeds.push(Some(splitmix64(base_seed.wrapping_add(i as u64))));
                }
            }
            Self::Range { start, step, .. } => {
                for i in 0..count {
                    seeds.push(Some(start.wrapping_add((i as u64).wrapping_mul(*step))));
                }
            }
        }
        Ok(seeds)
    }
}

/// Initial placement strategy selection for candidate exploration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayoutStrategy {
    /// Single placement strategy applied across all candidates.
    Single(PlacementStrategy),
    /// Multiple placement strategies to explore across candidates.
    Strategies(Vec<PlacementStrategy>),
    /// Explicit set of user-supplied initial layout maps.
    Explicit(Vec<LayoutMap>),
    /// Combined placement strategies and explicit layout maps.
    Combined {
        /// Placement strategies to explore.
        strategies: Vec<PlacementStrategy>,
        /// Explicit layouts to explore.
        explicit: Vec<LayoutMap>,
    },
}

impl Default for LayoutStrategy {
    fn default() -> Self {
        Self::Single(PlacementStrategy::Weighted)
    }
}

impl LayoutStrategy {
    /// Validates the layout strategy parameters.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        match self {
            Self::Single(_) => Ok(()),
            Self::Strategies(strats) => {
                if strats.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidLayoutConfiguration(
                        "placement strategies list cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }
            Self::Explicit(layouts) => {
                if layouts.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidLayoutConfiguration(
                        "explicit layout list cannot be empty".to_string(),
                    ));
                }
                for (i, l) in layouts.iter().enumerate() {
                    l.validate().map_err(|e| {
                        RoutingSearchConfigError::InvalidLayoutConfiguration(format!(
                            "explicit layout at index {i} failed validation: {e}"
                        ))
                    })?;
                }
                Ok(())
            }
            Self::Combined {
                strategies,
                explicit,
            } => {
                if strategies.is_empty() && explicit.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidLayoutConfiguration(
                        "combined layout strategy must specify at least one strategy or explicit layout"
                            .to_string(),
                    ));
                }
                for (i, l) in explicit.iter().enumerate() {
                    l.validate().map_err(|e| {
                        RoutingSearchConfigError::InvalidLayoutConfiguration(format!(
                            "explicit layout at index {i} failed validation: {e}"
                        ))
                    })?;
                }
                Ok(())
            }
        }
    }

    /// Validates the layout strategy against a specific hardware target model.
    ///
    /// Rejects explicit or combined layout strategies where any layout has a physical
    /// capacity incompatible with the target device's physical capacity (`target.num_qubits`).
    pub fn validate_with_target(&self, target: &Target) -> Result<(), RoutingSearchConfigError> {
        self.validate()?;
        let target_phys = target.num_qubits;

        let check_layout = |l: &LayoutMap, i: usize| -> Result<(), RoutingSearchConfigError> {
            if l.num_physical() != target_phys {
                return Err(RoutingSearchConfigError::InvalidLayoutConfiguration(format!(
                    "explicit layout at index {i} has physical capacity {} which is incompatible with target '{}' physical capacity {}",
                    l.num_physical(),
                    target.name,
                    target_phys,
                )));
            }
            for p in l.assigned_physical_qubits() {
                if p.0 as usize >= target_phys {
                    return Err(RoutingSearchConfigError::InvalidLayoutConfiguration(format!(
                        "explicit layout at index {i} assigns physical qubit {} which exceeds target '{}' physical capacity {}",
                        p.0,
                        target.name,
                        target_phys,
                    )));
                }
            }
            Ok(())
        };

        match self {
            Self::Single(_) | Self::Strategies(_) => Ok(()),
            Self::Explicit(layouts) => {
                for (i, l) in layouts.iter().enumerate() {
                    check_layout(l, i)?;
                }
                Ok(())
            }
            Self::Combined { explicit, .. } => {
                for (i, l) in explicit.iter().enumerate() {
                    check_layout(l, i)?;
                }
                Ok(())
            }
        }
    }
}

/// Operational resource budget and bounds for multi-candidate routing search.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchBudget {
    /// Upper bound on candidate count evaluated in a single compilation run.
    pub max_candidates: usize,
    /// Cumulative maximum physical SWAP insertions permitted across all candidates combined.
    pub max_total_swaps: Option<usize>,
    /// Cumulative maximum scheduling loop iterations permitted across all candidates combined.
    pub max_total_iterations: Option<usize>,
    /// Wall-clock timeout budget in milliseconds (if applicable).
    pub timeout_ms: Option<u64>,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            max_candidates: DEFAULT_MAX_CANDIDATES,
            max_total_swaps: None,
            max_total_iterations: None,
            timeout_ms: None,
        }
    }
}

impl SearchBudget {
    /// Validates the search budget constraints.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        if self.max_candidates == 0 || self.max_candidates > HARD_MAX_CANDIDATE_LIMIT {
            return Err(RoutingSearchConfigError::InvalidCandidateCount {
                count: self.max_candidates,
                min: 1,
                max: HARD_MAX_CANDIDATE_LIMIT,
            });
        }
        if let Some(swaps) = self.max_total_swaps {
            if swaps == 0 {
                return Err(RoutingSearchConfigError::InvalidBudget(
                    "max_total_swaps must be strictly positive".to_string(),
                ));
            }
        }
        if let Some(iters) = self.max_total_iterations {
            if iters == 0 {
                return Err(RoutingSearchConfigError::InvalidBudget(
                    "max_total_iterations must be strictly positive".to_string(),
                ));
            }
        }
        if let Some(t) = self.timeout_ms {
            if t == 0 {
                return Err(RoutingSearchConfigError::InvalidBudget(
                    "timeout_ms must be strictly positive".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Fallback policy if multi-candidate search encounters exhaustion, failure, or budget limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FallbackPolicy {
    /// Return the best valid candidate discovered before the budget/error was hit.
    #[default]
    BestValidSoFar,
    /// Fall back strictly to the deterministic baseline candidate (Candidate 0).
    DeterministicBaseline,
    /// Abort compilation fail-closed with a structured error.
    Abort,
}

/// Comprehensive configuration for multi-candidate routing search (Phase 74).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiCandidateRoutingConfig {
    /// Number of routing candidates to evaluate (default: 1).
    pub candidate_count: usize,
    /// Base SABRE router configuration (decay parameters, lookahead weight, refinement rounds).
    pub sabre_base_config: SabreRouterConfig,
    /// Random seed selection or derivation strategy.
    pub seed_strategy: SeedStrategy,
    /// Initial placement exploration strategy.
    pub layout_strategy: LayoutStrategy,
    /// Candidate evaluation objective function and hard constraints.
    pub objective: RoutingObjective,
    /// Execution budget and candidate search upper bounds.
    pub budget: SearchBudget,
    /// Fallback behavior on budget exhaustion or partial candidate failure.
    pub fallback_policy: FallbackPolicy,
}

impl Default for MultiCandidateRoutingConfig {
    fn default() -> Self {
        Self {
            candidate_count: 1,
            sabre_base_config: SabreRouterConfig::default(),
            seed_strategy: SeedStrategy::default(),
            layout_strategy: LayoutStrategy::default(),
            objective: RoutingObjective::default(),
            budget: SearchBudget::default(),
            fallback_policy: FallbackPolicy::default(),
        }
    }
}

impl MultiCandidateRoutingConfig {
    /// Creates a builder for fluently constructing a multi-candidate routing configuration.
    pub fn builder() -> MultiCandidateRoutingConfigBuilder {
        MultiCandidateRoutingConfigBuilder::default()
    }

    /// Creates a configuration wrapping an existing SABRE router config in single-candidate mode.
    pub fn single_candidate(config: SabreRouterConfig) -> Self {
        Self {
            candidate_count: 1,
            sabre_base_config: config,
            seed_strategy: SeedStrategy::Single(None),
            layout_strategy: LayoutStrategy::Single(PlacementStrategy::Weighted),
            objective: RoutingObjective::default(),
            budget: SearchBudget::default(),
            fallback_policy: FallbackPolicy::DeterministicBaseline,
        }
    }

    /// Returns `true` if this configuration represents single-candidate baseline execution.
    pub fn is_single_candidate(&self) -> bool {
        self.candidate_count == 1
    }

    /// Extracts the baseline single-candidate SABRE configuration for compatibility fallback.
    pub fn baseline_sabre_config(&self) -> SabreRouterConfig {
        let mut cfg = self.sabre_base_config.clone();
        if cfg.random_seed.is_none() {
            if let SeedStrategy::Single(Some(s)) = self.seed_strategy {
                cfg.random_seed = Some(s);
            }
        }
        match &self.layout_strategy {
            LayoutStrategy::Single(strat) => {
                cfg.placement_strategy = *strat;
            }
            LayoutStrategy::Strategies(strats) => {
                if let Some(&first) = strats.first() {
                    cfg.placement_strategy = first;
                }
            }
            LayoutStrategy::Combined { strategies, .. } => {
                if let Some(&first) = strategies.first() {
                    cfg.placement_strategy = first;
                }
            }
            LayoutStrategy::Explicit(_) => {}
        }
        cfg
    }

    /// Validates the multi-candidate routing configuration invariants.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        self.budget.validate()?;

        if self.candidate_count == 0 {
            return Err(RoutingSearchConfigError::InvalidCandidateCount {
                count: 0,
                min: 1,
                max: self.budget.max_candidates,
            });
        }

        if self.candidate_count > self.budget.max_candidates {
            return Err(RoutingSearchConfigError::InvalidCandidateCount {
                count: self.candidate_count,
                min: 1,
                max: self.budget.max_candidates,
            });
        }

        self.seed_strategy.validate()?;
        self.layout_strategy.validate()?;
        self.objective.validate()?;

        Ok(())
    }

    /// Validates the configuration against a specific hardware target model.
    ///
    /// Enforces the "Explicit Unknown Principle": if an objective references
    /// `EstimatedDuration` but the target lacks a hardware timing model,
    /// validation fails closed with a structured error.
    pub fn validate_with_target(&self, target: &Target) -> Result<(), RoutingSearchConfigError> {
        self.validate()?;
        self.layout_strategy.validate_with_target(target)?;

        for metric in self.objective.function.referenced_metrics() {
            if metric.requires_timing_model() && target.timing.is_none() {
                return Err(RoutingSearchConfigError::MissingTimingModel {
                    metric,
                    target: target.name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Derives the deterministic list of [`CandidateDescriptor`]s for this configuration.
    ///
    /// Candidate generation is 100% deterministic, bounded, and produces identical
    /// candidate IDs and configurations across independent process runs.
    pub fn derive_candidate_descriptors(
        &self,
        target: &Target,
    ) -> Result<Vec<CandidateDescriptor>, RoutingSearchConfigError> {
        self.validate_with_target(target)?;

        let target_hash = compute_target_hash(target);
        let router_options_hash = compute_sabre_config_hash(&self.sabre_base_config);
        let seeds = self.seed_strategy.generate_seeds(self.candidate_count)?;
        let baseline_cfg = self.baseline_sabre_config();

        let mut descriptors = Vec::with_capacity(self.candidate_count);
        let mut baseline_designated = false;

        for i in 0..self.candidate_count {
            let seed = if i == 0 {
                baseline_cfg.random_seed
            } else {
                seeds[i]
            };
            let (placement_strategy, explicit_layout, explicit_layout_index) =
                match &self.layout_strategy {
                    LayoutStrategy::Single(strat) => (*strat, None, None),
                    LayoutStrategy::Strategies(strats) => (strats[i % strats.len()], None, None),
                    LayoutStrategy::Explicit(layouts) => {
                        let idx = i % layouts.len();
                        (
                            PlacementStrategy::Identity,
                            Some(layouts[idx].clone()),
                            Some(idx),
                        )
                    }
                    LayoutStrategy::Combined {
                        strategies,
                        explicit,
                    } => {
                        let total_options = strategies.len() + explicit.len();
                        let opt_idx = i % total_options;
                        if opt_idx < strategies.len() {
                            (strategies[opt_idx], None, None)
                        } else {
                            let e_idx = opt_idx - strategies.len();
                            (
                                PlacementStrategy::Identity,
                                Some(explicit[e_idx].clone()),
                                Some(e_idx),
                            )
                        }
                    }
                };

            let identity = CandidateIdentity::new(
                i,
                seed,
                placement_strategy,
                explicit_layout_index,
                &router_options_hash,
                &target_hash,
            );

            let mut candidate_config = self.sabre_base_config.clone();
            candidate_config.random_seed = seed;
            candidate_config.placement_strategy = placement_strategy;

            // Explicit baseline designation: Candidate is designated as baseline strictly by its
            // configuration/provenance matching the deterministic baseline router options, seed,
            // placement strategy, and absence of explicit layout overrides. Candidate index is
            // never used as part of baseline identity.
            let matches_baseline = candidate_config.random_seed == baseline_cfg.random_seed
                && candidate_config.placement_strategy == baseline_cfg.placement_strategy
                && explicit_layout.is_none();

            let is_baseline = if !baseline_designated && matches_baseline {
                baseline_designated = true;
                true
            } else {
                false
            };

            descriptors.push(CandidateDescriptor {
                identity,
                is_baseline,
                config: candidate_config,
                explicit_initial_layout: explicit_layout,
            });
        }

        Ok(descriptors)
    }
}

/// Fluent builder for constructing [`MultiCandidateRoutingConfig`].
#[derive(Clone, Debug, Default)]
pub struct MultiCandidateRoutingConfigBuilder {
    config: MultiCandidateRoutingConfig,
}

impl MultiCandidateRoutingConfigBuilder {
    /// Sets the target number of candidates to evaluate.
    pub fn candidate_count(mut self, count: usize) -> Self {
        self.config.candidate_count = count;
        self
    }

    /// Sets the base SABRE router configuration.
    pub fn sabre_base_config(mut self, config: SabreRouterConfig) -> Self {
        self.config.sabre_base_config = config;
        self
    }

    /// Sets the random seed selection or derivation strategy.
    pub fn seed_strategy(mut self, strategy: SeedStrategy) -> Self {
        self.config.seed_strategy = strategy;
        self
    }

    /// Sets the initial layout placement strategy.
    pub fn layout_strategy(mut self, strategy: LayoutStrategy) -> Self {
        self.config.layout_strategy = strategy;
        self
    }

    /// Sets the candidate evaluation objective.
    pub fn objective(mut self, objective: RoutingObjective) -> Self {
        self.config.objective = objective;
        self
    }

    /// Sets the execution search budget.
    pub fn budget(mut self, budget: SearchBudget) -> Self {
        self.config.budget = budget;
        self
    }

    /// Sets the maximum allowable candidate count budget.
    pub fn max_candidates(mut self, max: usize) -> Self {
        self.config.budget.max_candidates = max;
        self
    }

    /// Sets the search fallback policy.
    pub fn fallback_policy(mut self, policy: FallbackPolicy) -> Self {
        self.config.fallback_policy = policy;
        self
    }

    /// Builds and validates the [`MultiCandidateRoutingConfig`].
    pub fn build(self) -> Result<MultiCandidateRoutingConfig, RoutingSearchConfigError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

/// Deterministic 64-bit splitmix expansion for pseudo-random seed generation.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}
