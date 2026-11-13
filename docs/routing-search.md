# QCC Multi-Candidate Routing Configuration, Search Engine, and Objectives

```text
Document: docs/routing-search.md
Roadmap: QCC Engineering Roadmap v3.1 (Milestone M9 — Phases 74, 75, & 76)
Module: qcc::routing::search
```

---

## 1. Overview and Motivation

In quantum circuit routing, heuristic algorithms such as SABRE (ASPLOS 2019, Li et al.) are sensitive to pseudo-random tie-breaking seeds and initial qubit placements. A single seed or heuristic layout can become trapped in local minima, producing excessive physical SWAP gates or suboptimal circuit depth.

**Milestone M9 (Phases 74–77)** introduces bounded multi-candidate routing search to QCC:
- **Phase 74 (Completed)**: Multi-candidate routing configuration, budgets, candidate descriptors, and explicit evaluation objectives.
- **Phase 75 (Completed)**: Multi-run SABRE candidate generation and execution engine with candidate isolation, cumulative resource budgeting, and truth-preserving validation.
- **Phase 76 (Completed)**: Deterministic candidate ranking, Pareto dominance analysis, multi-tier tie-breaking, and explainability reporting.
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

### 7.5 Fallback Policies

When all candidates fail or budgets are exhausted, `FallbackPolicy` dictates the resolution:
- `Abort`: Halts compilation immediately and returns `RoutingSearchExecutionError::Aborted`.
- `DeterministicBaseline`: Selects the deterministic baseline candidate (Candidate 0, corresponding to baseline SABRE configuration). If Candidate 0 failed routing or hard constraints, fails closed with `RoutingSearchExecutionError::InvalidBaselineFallback`.
- `BestValidSoFar`: Selects the highest-scoring candidate among candidates that passed all mandatory hard constraints according to Phase 76 ranking. If no candidates passed validation, fails closed with `RoutingSearchExecutionError::NoValidCandidates`.

---

## 8. Deterministic Candidate Ranking and Selection Architecture (Phase 76)

### 8.1 Metric Extraction and Snapshotting (`CandidateMetricsSnapshot`)

When an individual candidate completes routing, `CandidateMetricsSnapshot::from_circuit_and_report()` extracts six quantitative metrics directly from the routed circuit DAG and `SabreRouterReport`:
1. `swap_count`: Number of physical SWAP gates inserted during routing.
2. `cx_count`: Total physical CX gates in the routed DAG.
3. `two_qubit_gate_count`: Total physical two-qubit operations in the routed DAG.
4. `total_gate_count`: Total physical operations (1Q + 2Q gates, measurements, etc.).
5. `circuit_depth`: Critical-path topological depth in the dependency DAG.
6. `estimated_duration_ns`: Post-routing physical makespan in nanoseconds under target `TimingModel`.

#### The Strict Duration Invariant
In strict accordance with the **Explicit Unknown Principle**:
- `estimated_duration_ns` is populated as `Some(f64)` **if and only if** the target device provides a calibrated hardware `TimingModel` AND all operations in the routed circuit have exact known durations (`DurationCost::Exact`).
- If target timing is absent, or if any physical gate duration is uncalibrated or missing, `estimated_duration_ns` is preserved as `None` (`Unknown`), **never fabricated or assumed to be zero**.
- Any objective requiring `EstimatedDuration` when candidate duration is `None` fails evaluation closed, marking the candidate `CandidateRankEligibility::Ineligible` with an explicit diagnostic explanation.

### 8.2 Objective Scoring (`ObjectiveEvaluator`)

All objectives operate in the **minimization direction** (lower score is better):
- **Single Metric (`ObjectiveFunction::Single`)**:
  $$\text{score} = m$$
- **Lexicographic Priority (`ObjectiveFunction::Lexicographic`)**:
  $$\text{vector} = [m_0, m_1, \dots, m_{k-1}]$$
  Evaluated element-by-element: candidate $A$ precedes $B$ at the first metric where $m_i(A) \neq m_i(B)$.
- **Weighted Linear Sum (`ObjectiveFunction::Weighted`)**:
  $$\text{composite\_score} = \sum_{i=1}^k w_i \cdot m_i$$
  where all $w_i \ge 0.0$ and $\sum w_i > 0.0$.
  
  **Contract on Metric Normalization and Numerical Safety**: In accordance with the Phase 74/76 specification, the composite score is evaluated as an unnormalized linear combination of raw metric values scaled by user weights $w_i$. Dynamic batch normalization (such as min-max scaling) is avoided because it destroys candidate evaluation isolation, creates zero-division hazards when candidate metric ranges are zero, and causes a candidate's score to shift dynamically depending on which other candidates are generated in the batch. Weights $w_i$ therefore serve as the authoritative unit-scaling factors (e.g., balancing 1 SWAP against $X$ depth or $Y$ nanoseconds).
  - All weights are validated to be finite and non-negative ($w_i \ge 0.0$), with $\sum w_i > 0.0$ and $\sum w_i < \infty$ (preventing validation overflow).
  - Term multiplications ($w_i \cdot m_i$) and running score accumulations are checked against IEEE-754 overflow. Any candidate whose evaluation results in a non-finite score (`Inf` or `NaN`) is disqualified fail-closed with structured error `MetricEvaluationError::NonFiniteScore`.

### 8.3 Pareto Dominance and Non-Dominated Sorting

To analyze multi-objective trade-offs across candidates, `RankingEngine` implements the standard Deb fast non-dominated sorting algorithm over active objective metrics:
- **Dominance Criterion**: Candidate $A$ dominates candidate $B$ ($A \succ B$) iff:
  $$\forall i \in \{1, \dots, k\}, \; A_i \le B_i \quad \land \quad \exists j \in \{1, \dots, k\}, \; A_j < B_j$$
- **Pareto Fronts**:
  - $F_0$: Non-dominated front (no candidate in the batch dominates any candidate in $F_0$).
  - $F_1$: Front of candidates dominated only by candidates in $F_0$.
  - $F_i$: Front $i$ in general.

### 8.4 Multi-Tier Deterministic Tie-Breaking Hierarchy

To guarantee bit-for-bit repeatability across executions and compilers, candidates are totally ordered by:
1. **Hard Constraint Validation & Eligibility**: Candidates passing all mandatory constraints precede ineligible candidates.
2. **Pareto Front Index**: Lower front index ($F_0 < F_1 < \dots$) strictly takes precedence, guaranteeing that non-dominated candidates are preferred over dominated candidates regardless of single-metric score biases.
3. **Objective Score**: Within the same Pareto front, lower scalar score or lexicographic vector takes precedence.
4. **Designated Baseline Preference**: The baseline candidate (`is_baseline == true`) wins on exact score and front tie.
5. **Canonical Candidate ID**: Lexicographical string comparison of `candidate_id` provides the final immutable tie-breaker.

### 8.5 Explainability and Compile Reports (`RankingReport`)

The full candidate table is preserved in `RankingReport`, recording for every candidate:
- Candidate ID, sequence index, seed, and placement strategy.
- Captured metrics snapshot (swaps, CX, depth, duration).
- Eligibility status (`Eligible` vs `Ineligible`).
- Evaluated objective score and Pareto front index.
- Overall 1-based rank.
- Clear human-readable justification of selection (e.g. `Selected: Rank #1 (Pareto Front 0, score 2.00) [designated baseline]`) or rejection reason (e.g. `Disqualified: Mandatory hard constraint 'SemanticEquivalence' failed: ...`).
- ASCII formatted inspection table via `report.ranking.format_table()`.

---

## 9. CLI Usage, Empirical Benchmarking, and Default-versus-Opt-In Policy (Phase 77)

### 9.1 CLI Options

Multi-candidate routing search is accessible directly from the `qcc compile` subcommand via two flags:

| CLI Option | Parameter | Description | Default |
|---|---|---|---|
| `--routing-candidates` | `<N>` | Number of routing candidates to explore ($N \ge 1$). Requires `--target <TARGET>`. | `1` |
| `--routing-objective` | `<OBJ>` | Candidate evaluation objective: `'min-swaps'` (default), `'min-depth'`, `'min-cx'`, `'min-gates'`. | `'min-swaps'` |

Example invocation:
```bash
# Compile using an 8-candidate multi-run search minimizing circuit depth
qcc compile input.qasm --target ring --routing-candidates 8 --routing-objective min-depth --emit-manifest manifest.json
```

When `--routing-candidates N` ($N > 1$) is supplied:
1. QCC derives $N$ deterministic random seeds via `SeedStrategy::Derived { base_seed: 42, count: N }`.
2. Candidate 0 is explicitly designated as the deterministic baseline candidate matching single-candidate execution.
3. All $N$ candidates are generated, executed under cumulative search budgets, and validated independently against target legality, unitary equivalence, and ADR-0001 clean-ancilla preservation.
4. The winner is selected using the configured objective and Pareto ranking hierarchy.

### 9.2 Seeded Deterministic Tie-Breaking and Candidate Diversification

To enable distinct candidate exploration while guaranteeing 100% bit-for-bit repeatability, QCC implements seeded deterministic tie-breaking within SABRE candidate scoring (`ScoreComponents::cmp_deterministic_with_seed`):
1. **5-Tier Preserved Comparison Hierarchy**:
   - Tier 1: `total_score` ascending (primary objective: smaller score preferred).
   - Tier 2: `front_cost` ascending (secondary tie-breaker: prefer immediate front-layer distance reduction).
   - Tier 3: `lookahead_cost` ascending (tertiary tie-breaker: prefer superior lookahead distance positioning).
   - Tier 4: `decay_factor` ascending (quaternary tie-breaker: prefer less frequently swapped physical qubits).
   - Tier 5: Tie-breaker:
     - When `seed == None`: physical candidate pair `(u, v)` compared lexicographically.
     - When `seed == Some(s)`: SplitMix64 64-bit pseudo-random hash of physical candidate pair and `s`, falling back to `(u, v)` on hash collision.
2. **Baseline Invariant**: Candidate 0 (`is_baseline: true`) receives `random_seed: None` (or the baseline router's seed), guaranteeing bit-for-bit identical behavior to single-candidate $B=1$ execution. Candidate $i \ge 1$ receives candidate-specific seed $s_i$.
3. **Efficacy on Symmetric Workloads**: On symmetric topologies with genuine equal-score symmetries (e.g. $K_5$ on a 5-qubit ring), seeded tie-breaking successfully produces distinct intermediate layouts and unique routed physical circuits.

### 9.3 Empirical Benchmark Summary

In Phase 77, an extensive comparative benchmark (`tools/independent_oracle/benchmarks/routing_search.py`) was executed across 24 stratified corpus cases evaluating candidate budgets $B \in \{1, 4, 8, 16\}$:

- **100% Verification Rate**: All 96 physical circuits passed independent Qiskit v2.6.2 reference simulation (0 semantic mismatches, 0 ADR-0001 clean-ancilla violations).
- **Strict Non-Regression**: Multi-candidate mode never selected a circuit strictly worse than the deterministic baseline under identical evaluation criteria.
- **Circuit Quality on Corpus**: In all 24 evaluated cases, every candidate budget produced identical physical SWAPs (100 total), depths, and gate counts (0 improved, 24 unchanged, 0 worsened). Under `PlacementStrategy::Weighted` (which is deterministic), SABRE's front cost, lookahead cost, and decay tiers 1–4 are sufficiently discriminating that either no critical ties occurred on the evaluated corpus circuits, or alternative tie branches resulted in equal SWAP counts and were tie-broken in favor of the baseline by `BestValidSoFar`.
- **Scaling Overhead**: Runtime scaled roughly linearly ($1.0\times$ at $B=1$, $2.6\times$ at $B=4$, $4.8\times$ at $B=8$, and $9.2\times$ at $B=16$).

### 9.4 Default-versus-Opt-In Policy

Based on benchmark evidence, QCC maintains **deterministic single-candidate routing ($B=1$) as the universal default**. Multi-candidate search remains strictly **opt-in** (`--routing-candidates <N>`). Furthermore, multi-candidate search should not be expected to improve quality unless initial placement heuristics are diversified.

For full empirical tables, scaling curves, and case breakdown, see [**`docs/routing-search-results.md`**](./routing-search-results.md).

