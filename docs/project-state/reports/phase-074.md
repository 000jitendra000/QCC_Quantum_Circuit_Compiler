# Phase 74 Completion Report: Multi-Candidate Routing Configuration and Objectives

```text
Phase: Phase 74
Milestone: Milestone M9 — Multi-Candidate Routing Improvements
Roadmap: QCC Engineering Roadmap v3.1 (Phases 60–85)
Date: 2026-11-13
Status: COMPLETED & ACCEPTED
```

---

## 1. Executive Summary

**Phase 74** initiates **Milestone M9 (Multi-Candidate Routing Improvements)** by establishing the configuration, evaluation objective, candidate provenance, and deterministic fallback contracts for multi-candidate quantum circuit routing search.

In previous phases (Phases 44–53), QCC executed routing via single-run deterministic algorithms (`BaselineRouter` or `SabreBidirectionalRouter`) using a fixed placement strategy and seed. Phase 74 establishes the foundational contracts allowing the compiler to explore multiple distinct routing trajectories over bounded SABRE seeds and initial layout candidates, evaluate them against explicit objectives, and deterministically fall back to a validated baseline when necessary.

All configuration data structures and validation methods strictly enforce core project invariants:
1. **Explicit Unknown Principle**: `ObjectiveMetric::EstimatedDuration` strictly requires hardware timing information from the target model (`target.timing.is_some()`). If evaluated against an uncalibrated target, configuration validation fails fail-closed with `RoutingSearchConfigError::MissingTimingModel` rather than fabricating zero-cost defaults.
2. **Hard Constraints vs. Soft Objectives**: Mandatory physical legality (coupling/directionality), layout-aware unitary equivalence, and ADR-0001 clean ancilla zero-state preservation are modeled as non-negotiable `HardConstraint` filters. They are strictly decoupled from soft ranking objectives (`ObjectiveFunction`).
3. **Anti-Explosion Search Budgets**: Combinatorial search spaces are strictly bounded with `candidate_count` capped at `HARD_MAX_CANDIDATE_LIMIT = 256` (defaulting to 1 for backward compatibility), maximum swap insertion caps, and execution timeouts.
4. **Deterministic Candidate Identity**: Each candidate is tracked via `CandidateIdentity` and `CandidateDescriptor` with a deterministic 16-hex-digit SHA-256 candidate ID derived from circuit hash, target hash, SABRE config hash, seed, and placement strategy.
5. **100% Backward Compatibility**: `MultiCandidateRoutingConfig::default()` represents a single-candidate baseline configuration (`candidate_count: 1`). Integrated into `CompilerPipeline::compile`, it produces bit-for-bit identical routed circuits and metric reports to standard `RouterChoice::Sabre`.

---

## 2. Architecture and Implementation Details

### 2.1 Multi-Candidate Search Module Layout

All Phase 74 implementation resides in the newly created module `src/routing/search/` (registered in `src/routing/mod.rs` and re-exported in `src/lib.rs`):

- **`src/routing/search/mod.rs`**: Module declarations and public API re-exports.
- **`src/routing/search/error.rs`**: `RoutingSearchConfigError` enum with structured variants for zero/exceeded candidate counts, invalid seed/layout configurations, invalid objective weights, missing timing models, and search budget exhaustion.
- **`src/routing/search/objective.rs`**: `ObjectiveMetric`, `ObjectiveFunction` (`Single`, `Lexicographic`, `Weighted`), `HardConstraint` (`TargetLegality`, `SemanticEquivalence`, `AncillaZeroState`), and `RoutingObjective`.
- **`src/routing/search/candidate.rs`**: `CandidateIdentity`, `CandidateDescriptor`, `derive_canonical_candidate_id()`, `compute_sabre_config_hash()`, and `compute_target_hash()`.
- **`src/routing/search/config.rs`**: `MultiCandidateRoutingConfig`, `MultiCandidateRoutingConfigBuilder`, `SeedStrategy`, `LayoutStrategy`, `SearchBudget`, and `FallbackPolicy`.

### 2.2 Objective Formulation and Validation

Evaluation objectives allow flexible ranking of routing candidates according to measurable circuit properties:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectiveMetric {
    SwapCount,
    CxCount,
    TwoQubitGateCount,
    TotalGateCount,
    CircuitDepth,
    EstimatedDuration,
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectiveFunction {
    Single(ObjectiveMetric),
    Lexicographic(Vec<ObjectiveMetric>),
    Weighted(Vec<(ObjectiveMetric, f64)>),
}
```

Validation ensures:
- Unsupported metrics (such as deferred noise or hardware-profile parameters) fail fail-closed with `RoutingSearchConfigError::UnsupportedMetric`.
- Lexicographic objectives must contain at least one metric.
- Weighted objectives require finite, non-negative weights ($\ge 0.0$) with at least one strictly positive weight ($> 0.0$).
- Target compatibility validation (`validate_with_target`) inspects target hardware features: if `EstimatedDuration` is specified but the target lacks timing metadata (`target.timing.is_none()`), validation fails with `RoutingSearchConfigError::MissingTimingModel`.
- Mandatory hard constraints: all three hard constraints (`TargetLegality`, `SemanticEquivalence`, and `AncillaZeroState`) are strictly mandatory. Omitting any of them causes `validate()` to fail closed. Soft ranking objectives cannot weaken or override them.

### 2.3 Seed and Layout Generation Strategies

1. **`SeedStrategy`**:
   - `Single(Option<u64>)`: Single fixed seed.
   - `Explicit(Vec<u64>)`: User-supplied sequence of seeds.
   - `Derived { base_seed, count }`: Deterministic 64-bit SplitMix PRNG expansion generating `count` pseudo-random seeds without external runtime dependencies.
   - `Range { start, step, count }`: Bounded linear arithmetic progression of seeds.
2. **`LayoutStrategy`**:
   - `Single(PlacementStrategy)`: Single placement strategy across all candidates.
   - `Strategies(Vec<PlacementStrategy>)`: Round-robin cycling across multiple placement strategies (`Identity`, `Degree`, `Weighted`, `Random`).
   - `Explicit(Vec<LayoutMap>)`: Explicit pre-computed initial layout mappings.
   - `Combined { strategies, explicit }`: Unified search over algorithmic placers and explicit layouts.
   - Target physical capacity validation: `validate_with_target()` validates explicit and combined layouts against `target.num_qubits`, rejecting layouts whose physical capacity does not equal `target.num_qubits` or that map logical qubits to physical qubits beyond target bounds.

### 2.4 Candidate Descriptors and Provenance

Before executing routing search (Phase 75), each planned candidate has a deterministic `CandidateDescriptor`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateDescriptor {
    pub candidate_id: String,
    pub index: usize,
    pub identity: CandidateIdentity,
    pub target_hash: String,
    pub sabre_config_hash: String,
}
```

`CandidateIdentity` records:
- Candidate index ($0 \le i < \text{candidate\_count}$).
- Derived random seed (`Option<u64>`).
- Placement strategy (`PlacementStrategy`).
- Explicit layout mapping and index (`Option<LayoutMap>`, `Option<usize>`).
- Base router configuration SHA-256 hash (`router_options_hash`).
- Target device model SHA-256 hash (`target_hash`).

Candidate IDs are generated deterministically and directly embed both the complete 64-hex router options hash and target hash:
```rust
pub fn derive_canonical_candidate_id(
    index: usize,
    seed: Option<u64>,
    strategy: PlacementStrategy,
    explicit_layout_index: Option<usize>,
    router_options_hash: &str,
    target_hash: &str,
) -> String
```
Format: `cand_{index:03}_{seed_str}_{strat_str}{layout_suffix}_r{router_options_hash}_t{target_hash}`. Embedding the full 64-hex SHA-256 hashes guarantees collision resistance: modifying any candidate-defining input—including router options or target model—produces a distinct candidate ID.

### 2.5 Pipeline Integration and Backward Compatibility

`RouterChoice` in `src/compiler/config.rs` was extended with boxed indirection to maintain memory efficiency:
```rust
pub enum RouterChoice {
    Sabre(SabreRouterConfig),
    Baseline(BaselineRouterConfig),
    MultiCandidate(Box<MultiCandidateRoutingConfig>),
}
```

`CompilerPipeline::compile` in `src/compiler/pipeline.rs` was integrated:
- Validates search configuration against the target using `search_config.validate_with_target(target)?`.
- In default single-candidate mode (`candidate_count: 1`), routes the circuit using `search_config.baseline_sabre_config()`.
- Produces `RoutingStageReport` indicating `router_name: "SabreBidirectionalRouter [MultiCandidate Baseline]"`.
- Verified in `tests/routing/search_config.rs` to produce identical final gate count, circuit depth, and SWAP count as standard `RouterChoice::Sabre`.

---

## 3. Verification and Acceptance Evidence

### 3.1 Verification Commands and Results

| Command | Status | Details |
|---|:---:|---|
| `cargo fmt --check` | **PASS** | Strict rustfmt compliance with zero diffs |
| `cargo clippy --all-targets -- -D warnings` | **PASS** | Zero warnings; boxed large enum variant; derived defaults |
| `cargo doc --no-deps` | **PASS** | All public items fully documented with rustdoc |
| `cargo test --test routing` | **PASS** | 148/148 routing tests passed (including 14 Phase 74 tests) |
| `PATH=".venv/bin:$PATH" cargo test --no-fail-fast` | **PASS** | 1,458 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out across 40 test targets (plus 1 Doc-tests target) |
| `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v` | **PASS** | All 61 Python oracle tests passed (38.7s) |

### 3.2 Automated Test Coverage

The test suite [`tests/routing/search_config.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/routing/search_config.rs) contains 14 unit and integration tests:

1. `test_default_multi_candidate_config_is_single_candidate`: Verifies default configuration defaults to 1 candidate, `FallbackPolicy::BestValidSoFar`, all three mandatory hard constraints enabled, and `is_single_candidate() == true`.
2. `test_builder_fluency_and_custom_configuration`: Tests fluent builder pattern across all fields and methods.
3. `test_seed_strategies`: Verifies deterministic seed generation across `Single`, `Explicit`, `Derived` (SplitMix PRNG), and `Range` strategies.
4. `test_layout_strategies`: Verifies validation and candidate descriptor derivation for `Single`, `Strategies`, `Explicit`, and `Combined` layout strategies.
5. `test_budget_bounds_validation`: Validates rejection of `candidate_count == 0`, `candidate_count > HARD_MAX_CANDIDATE_LIMIT (256)`, and `max_candidates < candidate_count`.
6. `test_objective_validation`: Tests rejection of empty lexicographic lists, empty weighted lists, negative weights, all-zero weights, unsupported metrics, and empty hard constraints.
7. `test_mandatory_hard_constraints_cannot_be_omitted_or_weakened`: Asserts that omitting `TargetLegality`, `SemanticEquivalence`, or `AncillaZeroState` fails validation, and that weighted or lexicographic objectives cannot weaken or omit any mandatory constraint.
8. `test_target_timing_compatibility_and_explicit_unknown`: Proves that topological metrics succeed without timing models, while `EstimatedDuration` fails on targets without `TimingModel` and succeeds on targets with `TimingModel`.
9. `test_candidate_descriptor_derivation_determinism`: Tests repeatable, deterministic derivation of candidate descriptors and formatted candidate IDs across identical configurations.
10. `test_candidate_id_distinguishes_router_configurations`: Proves that altering SABRE router parameters (e.g. refinement rounds) directly alters `CandidateIdentity.id` and `router_options_hash` even when seeds and placers are identical, while identical configurations produce identical IDs embedding the full 64-hex hash.
11. `test_candidate_id_distinguishes_target_configurations`: Proves that differing target devices directly produce distinct `target_hash`es and distinct candidate IDs embedding the full 64-hex target hash.
12. `test_explicit_layout_target_capacity_validation`: Asserts that explicit and combined layouts incompatible with the target device's physical capacity (too large or too small) are rejected upfront during `validate_with_target()`.
13. `test_compiler_pipeline_single_candidate_backward_compatibility`: Executes full end-to-end circuit compilation on a linear topology comparing `RouterChoice::Sabre` and `RouterChoice::MultiCandidate(Box::default())`, verifying exact gate count, depth, and SWAP counts.
14. `test_serde_roundtrip_config`: Validates full JSON serialization and deserialization roundtrip fidelity for `MultiCandidateRoutingConfig`.

---

## 4. Documentation

- **Architectural Reference**: Created [`docs/routing-search.md`](file:///home/ronanki_jitendra/rust/QCC/docs/routing-search.md) documenting design principles, configuration schema, objective specifications, deterministic seed generation, candidate descriptors, and error handling.
- **Progress Ledger**: Updated [`docs/project-state/progress.md`](file:///home/ronanki_jitendra/rust/QCC/docs/project-state/progress.md) with Phase 74 completion records and Milestone M9 progress (1 / 4 phases complete).

---

## 5. Scope Boundary Compliance and Deferred Work

In strict accordance with Roadmap v3.1 and project invariants:
- **Phase 74 Only**: Only configuration and evaluation objective contracts were implemented.
- **Phase 75 (Seeded multi-run SABRE candidate generation)**: Deferred to Phase 75.
- **Phase 76 (Candidate evaluation and objective-based selection)**: Deferred to Phase 76.
- **Phase 77 (Routing search diagnostics, inspection and CLI exposure)**: Deferred to Phase 77.
- **Noise / Hardware Profiles**: No noise simulation, KAK, MPS, or hardware-profile parameters were introduced.
- **SABRE Router Core**: Underlying SABRE pathfinding and bidirectional refinement algorithms were untouched.

---

## 6. Exit Gate Conclusion

> **ROADMAP v3.1 EXIT GATE**: "A tested search configuration supports bounded seeds/layouts, declared objectives and deterministic fallback."

**Exit Gate Status: MET & ACCEPTED.**
All requirements have been fully verified through comprehensive Rust and Python test suites. Phase 74 is complete and ready for commit.
