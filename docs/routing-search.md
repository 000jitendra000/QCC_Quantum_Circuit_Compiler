# QCC Multi-Candidate Routing Configuration, Search Engine, and Objectives

```text
Document: docs/routing-search.md
Roadmap: QCC Engineering Roadmap v3.1 (Milestone M9 — Phases 74 & 75)
Module: qcc::routing::search
```

---

## 1. Overview and Motivation

In quantum circuit routing, heuristic algorithms such as SABRE (ASPLOS 2019, Li et al.) are sensitive to pseudo-random tie-breaking seeds and initial qubit placements. A single seed or heuristic layout can become trapped in local minima, producing excessive physical SWAP gates or suboptimal circuit depth.

**Milestone M9 (Phases 74–77)** introduces bounded multi-candidate routing search to QCC:
- **Phase 74 (Completed)**: Multi-candidate routing configuration, budgets, candidate descriptors, and explicit evaluation objectives.
- **Phase 75 (Completed)**: Multi-run SABRE candidate generation and execution engine with candidate isolation, cumulative resource budgeting, and truth-preserving validation.
- **Phase 76**: Deterministic candidate ranking, Pareto/lexicographic selection, and selection explainability reporting.
- **Phase 77**: Empirical validation and comparative benchmarking on the canonical verified corpus against the single-candidate baseline.

**Phase 74** established configuration and objective contracts, and **Phase 75** implements the execution engine that runs multiple SABRE instances over varied seeds and layout heuristics under strict cumulative resource limits and independent verification.

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

During target validation (`validate_with_target`), every explicit layout in `Explicit` or `Combined` is validated against the target device's physical capacity: layouts whose physical capacity does not equal `target.num_qubits` or that map logical qubits to out-of-bounds physical qubits are rejected fail-closed upfront.

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

### 4.3 Mandatory Hard Constraints (`HardConstraint`)

All valid routing candidates must strictly satisfy three mandatory hard constraints:
1. `TargetLegality`: Routed physical circuits must satisfy target physical gate legality and coupling directionality.
2. `SemanticEquivalence`: Routed physical circuits must preserve layout-aware unitary equivalence against the logical reference.
3. `AncillaZeroState`: Unassigned physical ancilla qubits must return to $|0\rangle$ at circuit termination (ADR-0001).

These constraints act as non-negotiable boolean filters: an invalid candidate is immediately disqualified regardless of its objective score. Soft ranking objectives (single, lexicographic, or weighted) cannot omit, weaken, or trade off any of these constraints.

---

## 5. Candidate Identity and Derivation

Every candidate generated by `derive_candidate_descriptors(&self, target: &Target)` receives a canonical deterministic descriptor and collision-resistant candidate ID:

```text
cand_<index:03>_<seed>_<placement>[_<layout_idx>]_r<router_hash:64>_t<target_hash:64>
```

### Identity Fields
- `index`: 0-based candidate evaluation index within the search batch (`%03d`).
- `seed`: 16-hex lower-case seed (`s0123456789abcdef`) or `snone` when unseeded.
- `strategy`: placement identifier (`ident`, `rand`, `deg`, `weight`).
- `explicit_layout_index`: optional explicit layout index (`_l{idx}`).
- `router_options_hash`: complete 64-hex SHA-256 hash of the SABRE router configuration (`r{hash}`).
- `target_hash`: complete 64-hex SHA-256 hash of the target device model (`t{hash}`).

Embedding the complete 64-hex SHA-256 hashes guarantees collision resistance and deterministic reproducibility:
1. Identical candidate-defining inputs produce identical IDs.
2. Different router configuration hashes produce distinct candidate IDs even if all other fields are equal.
3. Different target device hashes produce distinct candidate IDs even if all other fields are equal.

---

## 6. Compiler Pipeline Integration

In `src/compiler/config.rs`, `RouterChoice` integrates multi-candidate routing with boxed indirection:

```rust
pub enum RouterChoice {
    Sabre(SabreRouterConfig),
    Baseline(BaselineRouterConfig),
    MultiCandidate(Box<MultiCandidateRoutingConfig>),
}
```

In the compilation pipeline (`src/compiler/pipeline.rs`):
- When `config.is_single_candidate()` is true ($N=1$), execution takes a zero-overhead fast path directly into `SabreRouter`, preserving 100% backward compatibility with single-run SABRE behavior and metrics.
- When $N > 1$, `MultiCandidateSearchRunner` executes the multi-run search batch, logs per-candidate metrics, enforces cumulative resource limits, validates candidates, and selects a provisional candidate via `select_provisional_candidate()`, recording the chosen candidate ID and search metrics in the stage report.

---

## 7. Multi-Run Search Engine Architecture (Phase 75)

### 7.1 Candidate Isolation and Execution Lifecycle

`MultiCandidateSearchRunner` executes derived candidate descriptors sequentially or iteratively while maintaining strict candidate isolation:
1. **Input Circuit and Target Immutability**: Each candidate operates on an isolated clone of the input `Circuit` and references the immutable `Target`. A routing failure, panic, or mutation in one candidate cannot corrupt other candidates or the caller's state.
2. **Deterministic Layout Generation**: When an explicit layout map is not provided, the candidate's initial layout is generated via the configured `PlacementStrategy` (`Weighted`, `Degree`, `Random`, or `Identity`) using the candidate's seed for deterministic tie-breaking.
3. **Execution State Machine**: Each candidate transitions through:
   - `Pending`: Queued for execution.
   - `Skipped`: Skipped prior to routing if search budget (swaps, iterations, or timeout) was already exhausted.
   - `Success`: Routing succeeded; produces a routed physical `Circuit`, `CircuitLayout`, and `SabreRouterReport`.
   - `RoutingFailed`: Routing terminated with an error (e.g. routing budget exceeded or dead end). Partial work is recorded.
   - `ValidationFailed`: Routing produced a circuit, but one or more mandatory hard constraints failed.

### 7.2 Cumulative Search Resource Accounting

Single-candidate SABRE limits (`SabreRouterConfig.max_swaps`, `max_iterations`) bound a single pass or refinement run. In multi-candidate search, cumulative limits (`SearchBudget`) govern the entire search batch:
- **Exact Iteration Tracking**: `SabreRouterReport` tracks `total_iterations`, accumulating exact iteration counts across forward, reverse, and refinement passes.
- **Exact SWAP Tracking**: `SabreRouterReport` tracks `total_swaps_executed`, recording all physical SWAP insertions.
- **Partial Work Preservation**: When a candidate fails, its partial work (iterations and swaps consumed up to the point of failure) is captured in the error and credited to cumulative accounting (`total_swaps_consumed`, `total_iterations_consumed`).
- **Dynamic Budget Clamping**: Before each candidate run, remaining search budgets are computed:
  $$\text{remaining\_swaps} = \text{max\_total\_swaps} - \text{cumulative\_swaps}$$
  $$\text{remaining\_iterations} = \text{max\_total\_iterations} - \text{cumulative\_iterations}$$
  The candidate's `SabreRouterConfig` is clamped so that an individual candidate cannot exceed the remaining global search budget.
- **Cooperative Timeout Semantics**: Search deadlines (`timeout_ms`) are evaluated cooperatively between candidate executions. If the elapsed search time exceeds the deadline before a candidate starts, remaining candidates are skipped with `BudgetExhaustionReason::Timeout`. Running candidates are not asynchronously interrupted.

### 7.3 Independent Hard Constraint Validation

In accordance with ADR-0001 and QCC invariants, validation results are captured in `CandidateValidationRecord` with three independent statuses (`CheckStatus::Passed`, `CheckStatus::Failed`, `CheckStatus::Unexecuted`):
1. **Target Legality**: Verified via `TargetLegalityValidator::validate_circuit()`. Checks physical gate set support, 2-qubit coupling graph adjacency, and gate directionality.
2. **Logical Equivalence**: Verified via `LayoutEquivalenceChecker::check_layout_equivalence_detailed()`. Checks unitary equivalence up to a single common global phase. For physical qubit counts $n_{\text{phys}} > 16$, simulation is marked `CheckStatus::Unexecuted` to prevent exponential host memory allocation.
3. **Ancilla Zero-State Preservation**: Verified independently by `LayoutEquivalenceChecker` (ADR-0001). Confirms that all physical qubits not in the final logical layout return strictly to $|0\rangle$. If $n_{\text{phys}} > 16$, marked `CheckStatus::Unexecuted`.

A candidate is marked `is_fully_validated() == true` if and only if all executed checks returned `Passed` and no check was `Unexecuted` or `Failed`.

### 7.4 Preserved Execution Reports on Failure

If multi-candidate search fails to produce a satisfactory candidate, `MultiCandidateSearchRunner` returns a structured `RoutingSearchExecutionError` that embeds the complete `Box<MultiCandidateExecutionReport>`:
- `execution_report.candidates` contains full records for all candidates executed or skipped.
- Total swaps, iterations, and search duration are preserved for post-mortem diagnostics and CLI reporting.
- Error variants distinguish `Aborted` (hard budget reached under `Abort` policy), `InvalidBaselineFallback` (baseline candidate failed hard constraints), `AllCandidatesFailed` (routing errors in all candidates), and `NoValidCandidates` (all routed candidates failed validation).

### 7.5 Fallback Policies and Provisional Selection

When all candidates fail or budgets are exhausted, `FallbackPolicy` dictates the resolution:
- `Abort`: Halts compilation immediately and returns `RoutingSearchExecutionError::Aborted`.
- `DeterministicBaseline`: Selects the deterministic baseline candidate (Candidate 0, corresponding to baseline SABRE configuration). If Candidate 0 failed routing or hard constraints, fails closed with `RoutingSearchExecutionError::InvalidBaselineFallback`.
- `BestValidSoFar`: Selects the highest-scoring candidate among candidates that passed all mandatory hard constraints. If no candidates passed validation, fails closed with `RoutingSearchExecutionError::NoValidCandidates`.

In Phase 75, before Phase 76 ranking is integrated, `select_provisional_candidate()` provides the provisional selection API:
1. Prefers the validated baseline candidate if valid.
2. Otherwise selects the first candidate with `CandidateExecutionStatus::Success` that passed all hard constraints (`is_fully_validated() == true`).
3. If no fully validated candidate exists, falls back to the first candidate that produced a successful routing without validation failures.
4. Returns a `ProvisionalCandidateSelection` with the stable, collision-resistant `candidate_id`.
