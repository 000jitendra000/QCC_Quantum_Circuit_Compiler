//! Candidate identity, descriptor, and provenance models for multi-candidate routing search.
//!
//! Provides deterministic identification and parameter isolation for individual routing candidates.

use serde::{Deserialize, Serialize};

use crate::compiler::manifest::sha256_hex;
use crate::routing::layout_map::LayoutMap;
use crate::routing::placement::PlacementStrategy;
use crate::routing::sabre::SabreRouterConfig;
use crate::target::model::Target;

/// Deterministic identity and provenance for a single routing search candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateIdentity {
    /// 0-based sequence index within the candidate exploration batch.
    pub candidate_index: usize,
    /// Canonical deterministic candidate identifier string.
    pub id: String,
    /// Random seed supplied to the SABRE router for this candidate, if configured.
    pub seed: Option<u64>,
    /// Initial placement strategy used for this candidate.
    pub placement_strategy: PlacementStrategy,
    /// Index into explicit layouts list, if an explicit layout was supplied.
    pub explicit_layout_index: Option<usize>,
    /// Cryptographic SHA-256 hash of the base router configuration parameters.
    pub router_options_hash: String,
    /// Cryptographic SHA-256 hash of the target device model.
    pub target_hash: String,
}

impl CandidateIdentity {
    /// Constructs a new candidate identity with deterministic ID derivation.
    pub fn new(
        candidate_index: usize,
        seed: Option<u64>,
        placement_strategy: PlacementStrategy,
        explicit_layout_index: Option<usize>,
        router_options_hash: impl Into<String>,
        target_hash: impl Into<String>,
    ) -> Self {
        let r_hash = router_options_hash.into();
        let t_hash = target_hash.into();
        let id = derive_canonical_candidate_id(
            candidate_index,
            seed,
            placement_strategy,
            explicit_layout_index,
            &t_hash,
        );

        Self {
            candidate_index,
            id,
            seed,
            placement_strategy,
            explicit_layout_index,
            router_options_hash: r_hash,
            target_hash: t_hash,
        }
    }
}

/// Fully configured execution descriptor for an individual routing candidate.
///
/// Encapsulates the candidate's identity, specialized SABRE configuration,
/// and optional explicit initial layout for execution in Phase 75.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateDescriptor {
    /// Deterministic candidate identity and provenance.
    pub identity: CandidateIdentity,
    /// Specialized SABRE configuration containing this candidate's seed and placement strategy.
    pub config: SabreRouterConfig,
    /// Optional explicit initial layout map assigned to this candidate.
    pub explicit_initial_layout: Option<LayoutMap>,
}

/// Derives a deterministic candidate ID from its key parameters.
pub fn derive_canonical_candidate_id(
    index: usize,
    seed: Option<u64>,
    strategy: PlacementStrategy,
    explicit_layout_index: Option<usize>,
    target_hash: &str,
) -> String {
    let seed_str = match seed {
        Some(s) => format!("s{:016x}", s),
        None => "snone".to_string(),
    };
    let strat_str = match strategy {
        PlacementStrategy::Identity => "ident",
        PlacementStrategy::Random { .. } => "rand",
        PlacementStrategy::Degree => "deg",
        PlacementStrategy::Weighted => "weight",
    };
    let layout_suffix = match explicit_layout_index {
        Some(idx) => format!("_l{idx}"),
        None => String::new(),
    };
    let short_thash = if target_hash.len() >= 8 {
        &target_hash[..8]
    } else {
        target_hash
    };

    format!("cand_{index:03}_{seed_str}_{strat_str}{layout_suffix}_{short_thash}")
}

/// Computes a deterministic SHA-256 hash of a SABRE router configuration.
pub fn compute_sabre_config_hash(config: &SabreRouterConfig) -> String {
    let json =
        serde_json::to_string(config).unwrap_or_else(|_| "sabre_config_fallback".to_string());
    sha256_hex(json.as_bytes())
}

/// Computes a deterministic SHA-256 hash of a target device model.
pub fn compute_target_hash(target: &Target) -> String {
    let json = serde_json::to_string(target).unwrap_or_else(|_| target.name.clone());
    sha256_hex(json.as_bytes())
}
