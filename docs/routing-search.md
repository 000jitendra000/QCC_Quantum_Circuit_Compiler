# QCC Multi-Candidate Routing Configuration and Evaluation Objectives

```text
Document: docs/routing-search.md
Roadmap: QCC Engineering Roadmap v3.1 (Milestone M9 — Phase 74)
Module: qcc::routing::search
```

---

## 1. Overview and Motivation

In quantum circuit routing, heuristic algorithms such as SABRE (ASPLOS 2019, Li et al.) are sensitive to pseudo-random tie-breaking seeds and initial qubit placements. A single seed or heuristic layout can become trapped in local minima, producing excessive physical SWAP gates or suboptimal circuit depth.

**Milestone M9 (Phases 74–77)** introduces bounded multi-candidate routing search to QCC:
- **Phase 74 (Current)**: Multi-candidate routing configuration, budgets, candidate descriptors, and explicit evaluation objectives.
- **Phase 75**: Seeded multi-run SABRE candidate generation and execution with candidate isolation.
- **Phase 76**: Deterministic candidate ranking, Pareto/lexicographic selection, and selection explainability reporting.
- **Phase 77**: Empirical validation and comparative benchmarking on the canonical verified corpus against the single-candidate baseline.

**Phase 74** establishes the strict configuration and objective contracts before any candidate generation or selection is performed, guaranteeing bounded execution, reproducibility, and backward compatibility.

---

## 2. Core Architectural Invariants

### Invariant 1: Hard Constraints vs Soft Ranking Objectives
Multi-candidate routing strictly separates mandatory correctness filters from soft ranking metrics:
1. **Hard Constraints (Mandatory Filters)**:
   - **Target Legality**: All physical gates must be target-native and satisfy hardware connectivity and directionality (`TargetLegalityValidator`).
   - **Semantic Equivalence**: Layout-aware unitary equivalence against the logical reference circuit must hold.
   - **ADR-0001 Ancilla Zero-State Preservation**: Every physical qubit not occupied by a logical qubit must start in $|0\rangle$ and return to $|0\rangle$ at circuit termination.
   > [!IMPORTANT]
   > Any candidate that violates a hard constraint is **immediately disqualified**. Hard constraints are never relaxed or traded off against soft cost functions.

2. **Soft Ranking Objectives**:
   Evaluated strictly among candidates that satisfy *all* hard constraints.

### Invariant 2: The Explicit Unknown Principle
Following QCC architectural rules ([`docs/05-target-routing-and-layout.md`](./docs/05-target-routing-and-layout.md)):
- Missing hardware timing, calibration, or duration metrics are modeled as `None` (`Unknown`), **never assumed to be zero**.
- If a user configures an objective requiring `EstimatedDuration` or makespan, but the target device provides no `TimingModel`, configuration validation fails closed with a structured error (`RoutingSearchConfigError::MissingTimingModel`).

### Invariant 3: Bounded Search and Anti-Explosion Bounds
- Candidate search space is strictly bounded by `budget.max_candidates` (default: 64, hard maximum: 256).
- Unbounded combinatorial searches (e.g. all permutations of 100 qubits) are disallowed.
- Optional cumulative swap, iteration, and timeout limits prevent infinite loops or runaway execution.

### Invariant 4: Bit-for-Bit Determinism and Candidate Identity
- Given identical configuration, circuit, and target, candidate generation is bit-for-bit identical across runs.
- Every candidate receives a canonical `CandidateIdentity` incorporating its sequence index, seed, placement strategy, layout index, router configuration hash, and target hash.

### Invariant 5: Single-Candidate Backward Compatibility
- Default configuration (`candidate_count: 1`) preserves existing single-candidate SABRE routing behavior identically.
- Fallback policies allow safe rollback to the deterministic baseline candidate if search bounds are exceeded.

---

## 3. Configuration Model

### 3.1 Data Structures (`MultiCandidateRoutingConfig`)

```rust
pub struct MultiCandidateRoutingConfig {
    pub candidate_count: usize,
    pub sabre_base_config: SabreRouterConfig,
    pub seed_strategy: SeedStrategy,
    pub layout_strategy: LayoutStrategy,
    pub objective: RoutingObjective,
    pub budget: SearchBudget,
    pub fallback_policy: FallbackPolicy,
}
```

### 3.2 Seed Strategies (`SeedStrategy`)

| Strategy | Description | Behavior |
|---|---|---|
| `Single(Option<u64>)` | Baseline seed | Uses the single provided seed (or `None` for unseeded) across all runs. |
| `Explicit(Vec<u64>)` | Enumerated seeds | Explores a user-supplied deterministic list of seeds. |
| `Derived { base_seed, count }` | Deterministic derivation | Generates $N$ distinct seeds from `base_seed` using a 64-bit splitmix expansion. |
| `Range { start, step, count }` | Arithmetic sequence | Generates $N$ seeds as `start + i * step`. |

### 3.3 Initial-Layout Strategies (`LayoutStrategy`)

| Strategy | Description |
|---|---|
| `Single(PlacementStrategy)` | Applies a single placement strategy (e.g. `PlacementStrategy::Weighted`). |
| `Strategies(Vec<PlacementStrategy>)` | Explores multiple placement heuristics (e.g. `[Weighted, Degree, Random]`). |
| `Explicit(Vec<LayoutMap>)` | Evaluates a user-supplied set of explicit injective initial layout maps. |
| `Combined { strategies, explicit }` | Combines heuristic placement strategies and explicit layout maps. |

### 3.4 Fallback Policies (`FallbackPolicy`)

| Policy | Behavior on Error / Budget Exhaustion |
|---|---|
| `BestValidSoFar` | Returns the highest-scoring candidate among all evaluated candidates that passed all hard constraints. |
| `DeterministicBaseline` | Strictly rolls back to Candidate 0 (the baseline SABRE configuration). |
| `Abort` | Fails closed, aborting compilation with a structured `RoutingError`. |

---

## 4. Evaluation Objectives

### 4.1 Supported Quality Metrics (`ObjectiveMetric`)

QCC strictly categorizes metrics into topological, hardware-dependent, and unsupported:

1. **Topological QccIR Metrics** (Directly measurable without hardware timing models):
   - `SwapCount`: Total count of physical SWAP gates inserted during routing.
   - `CxCount`: Total count of physical CX operations in the routed circuit.
   - `TwoQubitGateCount`: Total count of physical 2-qubit operations.
   - `TotalGateCount`: Total operations (1Q + 2Q) in the routed physical DAG.
   - `CircuitDepth`: Critical-path topological depth of the routed DAG.

2. **Hardware-Dependent Metrics** (Measurable only when target provides calibration/timing):
   - `EstimatedDuration`: Circuit makespan in nanoseconds calculated by post-routing physical scheduling. If target lacks timing, reported as `Unknown` or rejected during validation.

3. **Unsupported / Deferred Metrics**:
   - `Unsupported(reason)`: Explicitly captures deferred metrics (e.g. noise-aware gate fidelity, crosstalk penalty). Rejects configuration fail-closed if configured.

### 4.2 Objective Functions (`ObjectiveFunction`)

```rust
pub enum ObjectiveFunction {
    /// Minimize a single metric (e.g. SwapCount).
    Single(ObjectiveMetric),
    /// Lexicographic priority order (primary, secondary tie-breaker, etc.).
    Lexicographic(Vec<ObjectiveMetric>),
    /// Weighted linear sum: score = sum(weight_i * metric_i).
    Weighted(Vec<(ObjectiveMetric, f64)>),
}
```

---

## 5. Candidate Identity and Derivation

Every candidate generated by `derive_candidate_descriptors(&self, target: &Target)` receives a canonical deterministic descriptor:

```text
cand_<index:03>_<seed>_<placement>[_<layout_idx>]_<target_hash:8>
```

Examples:
- `cand_000_s000000000000002a_weight_a1b2c3d4`
- `cand_001_s0000000000000034_deg_a1b2c3d4`
- `cand_002_s000000000000003e_ident_l0_a1b2c3d4`

This canonical naming guarantees complete provenance tracking in compilation manifests and diagnostic reports.

---

## 6. Compiler Pipeline Integration

In `src/compiler/config.rs`, `RouterChoice` integrates multi-candidate routing:

```rust
pub enum RouterChoice {
    Sabre(SabreRouterConfig),
    Baseline(BaselineRouterConfig),
    MultiCandidate(MultiCandidateRoutingConfig),
}
```

In Phase 74:
- `RouterChoice::MultiCandidate` validates the full search configuration against the target.
- For single-candidate baseline or fallback execution, it executes the base SABRE configuration with zero regressions.
- Milestone M9 Phase 75 connects this configuration model to multi-run execution.
