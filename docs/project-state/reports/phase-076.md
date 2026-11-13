# Phase 76 Completion Report: Deterministic Candidate Ranking and Selection

```text
Phase: Phase 76
Milestone: Milestone M9 — Multi-Candidate Routing Improvements
Roadmap: QCC Engineering Roadmap v3.1 (Phases 60–85)
Date: 2026-11-13
Status: COMPLETED
```

---

## 1. Executive Summary

**Phase 76** implements deterministic candidate ranking, objective evaluation, Pareto dominance analysis, multi-tier tie-breaking, and explainability reporting for **Milestone M9 (Multi-Candidate Routing Improvements)**.

Building upon the bounded multi-candidate generation engine from Phase 75, Phase 76 replaces provisional selection with a mathematically rigorous, fully explainable ranking engine. Candidates are evaluated across declared objective functions over six fundamental circuit metrics, mandatory hard constraints act as non-negotiable filters, multi-objective trade-offs are partitioned into Pareto fronts, and an immutable multi-tier tie-breaking hierarchy ending in canonical candidate IDs guarantees bit-for-bit repeatability across executions and compilers.

Key architectural deliverables:
1. **Strongly Typed Metric Extraction (`CandidateMetricsSnapshot`)**:
   Captures physical `swap_count`, `cx_count`, `two_qubit_gate_count`, `total_gate_count`, `circuit_depth`, and `estimated_duration_ns` directly from the routed circuit DAG and router report.
2. **Strict Hardware Duration Semantics (Explicit Unknown Principle)**:
   `estimated_duration_ns` is populated if and only if the target provides a calibrated `TimingModel` and all constituent operations have known durations (`DurationCost::Exact`). Missing or uncalibrated timing is preserved as `None` (`Unknown`), **never assumed or fabricated as zero**. Any candidate evaluated under an objective requiring `EstimatedDuration` when timing is missing is disqualified fail-closed with an explicit diagnostic reason.
3. **Objective Function Evaluation (`ObjectiveEvaluator`)**:
   Evaluates `Single` metric minimization, `Lexicographic` multi-objective priority vectors, and `Weighted` linear composite scores ($\sum w_i m_i$). All objectives operate strictly in the minimization direction.
4. **Pareto Dominance and Non-Dominated Sorting**:
   Implements the Deb fast non-dominated sorting algorithm over active objective metrics, partitioning eligible candidates into Pareto fronts ($F_0, F_1, \dots$). Candidates in $F_0$ are non-dominated.
5. **Deterministic Multi-Tier Tie-Breaking**:
   Sorts eligible candidates by:
   - Primary: Pareto front index ($F_0 < F_1 < \dots$; non-dominated candidates strictly take precedence).
   - Secondary: Objective score (lower is better).
   - Tertiary: Designated baseline preference (`is_baseline == true` wins on score/front tie).
   - Quaternary: Canonical `candidate_id` alphabetical string comparison.
6. **Mandatory Hard Constraint Filtering**:
   `TargetLegality`, `SemanticEquivalence`, and `AncillaZeroState` (ADR-0001) act as non-negotiable filters. Any candidate violating a hard constraint or with unexecuted validation is immediately disqualified from ranking (`CandidateRankEligibility::Ineligible`).
7. **Explainability and Compile Reports (`RankingReport`)**:
   Preserves the complete candidate table with raw metrics, objective scores, eligibility statuses, Pareto fronts, overall ranks, and human-readable justifications for winning selection or disqualification.
8. **Selection Policy Updates**:
   `FallbackPolicy::BestValidSoFar` selects the #1 ranked candidate from `RankingReport`. `FallbackPolicy::DeterministicBaseline` selects the designated baseline candidate only when valid.
9. **Backward Compatibility**:
   Single-candidate fast path (`candidate_count: 1`) remains untouched with zero regressions. All Phase 74 and Phase 75 contracts are fully preserved.

---

## 2. Architecture and Implementation Details

### 2.1 File and Module Layout

- **`src/routing/search/evaluator.rs` (New Module)**:
  - `CandidateMetricsSnapshot`: Quantitative metrics extraction directly from routed circuit DAG and SABRE report.
  - `MetricEvaluationError`: Structured errors for unavailable duration or unsupported metrics.
  - `CandidateObjectiveScore`: Enum supporting `Single`, `Lexicographic`, and `Weighted` composite scores with deterministic `cmp_score()` ordering.
  - `ObjectiveEvaluator`: Scores candidate metric snapshots against objective functions.
- **`src/routing/search/ranking.rs` (New Module)**:
  - `CandidateRankEligibility`: Strongly typed eligibility (`Eligible` vs `Ineligible { reason }`).
  - `CandidateEvaluationRecord`: Complete candidate evaluation record including identity, metrics, score, Pareto front, overall rank, and explanation.
  - `RankingReport`: Full candidate evaluation report, winner ID, selection reason, and formatted ASCII inspection table.
  - `RankingEngine`: Orchestrates hard constraint filtering, objective evaluation, Pareto front assignment, and deterministic tie-breaking.
- **`src/routing/search/runner.rs` (Updated)**:
  - Extended `MultiCandidateExecutionReport` with `pub ranking: Option<RankingReport>`.
  - Added `select_candidate()` returning `CandidateSelection` with evaluation metadata.
  - Retained `select_provisional_candidate()` as a backwards-compatible alias.
  - Integrated `RankingEngine::evaluate_and_rank()` into `MultiCandidateSearchRunner::run_with_descriptors()`.
- **`src/routing/search/mod.rs` (Updated)**:
  - Re-exported new Phase 76 types: `CandidateMetricsSnapshot`, `CandidateObjectiveScore`, `MetricEvaluationError`, `ObjectiveEvaluator`, `CandidateRankEligibility`, `CandidateEvaluationRecord`, `RankingEngine`, `RankingReport`, `CandidateSelection`.
- **`tests/routing/candidate_selection.rs` (New Test Suite)**:
  - 20 comprehensive integration tests covering all Phase 76 features.
- **`docs/routing-search.md` (Updated)**:
  - Added Section 8 detailing Phase 76 metrics extraction, duration semantics, objective scoring, Pareto dominance, tie-breaking, and explainability.

---

## 3. Verification Evidence and Quality Gates

### 3.1 Verification Commands Executed

```bash
# 1. Formatting check
cargo fmt --check
# Result: Passed (code 0)

# 2. Clippy linter with strict warning denial
cargo clippy --all-targets -- -D warnings
# Result: Passed (code 0)

# 3. Documentation generation
cargo doc --no-deps
# Result: Passed (code 0, zero broken links, zero warnings)

# 4. Routing module integration tests
cargo test --test routing
# Result: Passed (196/196 passed, including 26 Phase 76 tests; code 0)

# 5. Full workspace Rust test suite
PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast
# Result: Passed (1,506 passed; 0 failed; 0 ignored across 41 test targets; code 0)

# 6. Independent Python oracle test suite
.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v
# Result: Passed (61/61 passed; code 0)
```

### 3.2 Test Count Reconciliation

- **Phase 75 Checkpoint**: 1,480 passing Rust tests across 41 test targets; 61 Python oracle tests.
- **Phase 76 Added Tests (+26 in `tests/routing/candidate_selection.rs`)**:
  1. `test_objective_swap_count_minimization`: Verifies minimization ranking on physical SWAP count.
  2. `test_objective_cx_count_minimization`: Verifies minimization ranking on physical CX count.
  3. `test_objective_two_qubit_gate_count_minimization`: Verifies minimization ranking on 2Q gate count.
  4. `test_objective_total_gate_count_minimization`: Verifies minimization ranking on total gate count.
  5. `test_objective_circuit_depth_minimization`: Verifies minimization ranking on topological circuit depth.
  6. `test_objective_estimated_duration_with_calibrated_timing_model`: Evaluates nanosecond duration under calibrated `TimingModel`.
  7. `test_objective_estimated_duration_fails_closed_when_timing_is_unavailable`: Fails closed and marks candidate ineligible when timing is uncalibrated.
  8. `test_lexicographic_objective_ranking_primary_and_secondary`: Verifies lexicographic multi-metric priority order.
  9. `test_weighted_objective_linear_combination`: Verifies linear composite scoring $\sum w_i m_i$.
  10. `test_pareto_dominance_and_front_assignment`: Verifies non-dominated sorting and multi-front assignment ($F_0, F_1$).
  11. `test_tie_breaking_designated_baseline_preference`: Verifies baseline candidate preference on score and front tie.
  12. `test_tie_breaking_canonical_candidate_id_alphabetical`: Verifies immutable alphabetical tie-breaking by candidate ID.
  13. `test_hard_constraint_semantic_equivalence_disqualification`: Disqualifies candidate with invalid equivalence despite superior swap score.
  14. `test_hard_constraint_unexecuted_validation_disqualification`: Disqualifies candidate with unexecuted validation checks.
  15. `test_hard_constraint_target_legality_disqualification`: Disqualifies candidate violating physical connectivity.
  16. `test_hard_constraint_ancilla_zero_state_disqualification`: Disqualifies candidate violating ADR-0001 ancilla zero-state.
  17. `test_multi_candidate_runner_ranking_integration_and_explainability`: Verifies full runner ranking report and formatted ASCII table.
  18. `test_repeat_run_determinism`: Verifies bit-for-bit repeatability of candidate scores, fronts, ranks, and explanations.
  19. `test_fallback_policies_with_ranking`: Verifies `BestValidSoFar` and `DeterministicBaseline` selection policies under ranking.
  20. `test_compiler_pipeline_multi_candidate_ranking_integration`: Verifies end-to-end `CompilerPipeline` routing stage with multi-candidate search.
  21. `test_invalid_weights_and_normalization_edge_cases`: Validates rejection of non-finite weights (NaN, Inf) and confirms raw linear combination without dynamic batch scaling.
  22. `test_pareto_dominance_comparator_strictly_prefers_non_dominated_over_dominated`: Proves non-dominated candidates in $F_0$ with higher single-metric scores defeat dominated candidates in $F_1$.
  23. `test_timing_concurrency_makespan_not_sum`: Proves parallel disjoint gates execute concurrently (makespan is max duration, not sum).
  24. `test_timing_mixed_known_and_unknown_durations_fail_closed`: Validates fail-closed disqualification when some gates lack calibrated timing.
  25. `test_no_valid_candidates_preserves_full_report`: Validates preservation of complete candidate evaluations and ranking report on `NoValidCandidates`.
  26. `test_abort_budget_exhausted_preserves_accumulated_report`: Validates preservation of partial work and evaluated candidates when budget aborts search.
- **Phase 76 Final Count**: **1,506 passing Rust tests** (+26 tests) across 41 test targets + **61 passing Python tests**. Zero failures, zero warnings, zero ignored tests.

---

## 4. Acceptance Audit Reconciliation

An in-depth source-level acceptance audit was conducted across 6 core requirements:

1. **Weighted Objective Normalization**:
   - Confirmed that the authoritative contract specifies unnormalized linear combination $\sum w_i m_i$ with weights $w_i \ge 0.0$ and $\sum w_i > 0.0$.
   - Dynamic batch normalization (min-max scaling) is avoided because it breaks candidate evaluation isolation and is singular when candidates have identical metrics. Weights provide user-calibrated unit scaling.
   - Verified that non-finite weights (NaN, Inf) are rejected during configuration validation.
2. **Pareto Dominance and Objective Ordering**:
   - Resolved a comparator inversion: the ranking comparator was updated to enforce **`Tier 1: Pareto Front (F0 < F1 < ...)`**, followed by **`Tier 2: Objective Score`**.
   - This guarantees that non-dominated candidates are strictly preferred over dominated candidates, preventing dominated candidates with favorable biased weights from leapfrogging the Pareto frontier.
3. **Timing Metric Correctness**:
   - Verified that makespan calculation reflects DAG critical-path concurrency (maximum path length, not serial summation).
   - Enhanced `TargetQuery::query_duration` to resolve durations through `target.timing` (`TimingModel`) in addition to tuple calibrations.
   - Enforced strict fail-closed behavior under the Explicit Unknown Principle: uncalibrated gates yield `None`, and duration-dependent objectives disqualify the candidate rather than defaulting to zero.
4. **Mandatory Hard Constraints & Error Handling**:
   - Confirmed that `TargetLegality`, `SemanticEquivalence`, and `AncillaZeroState` are non-negotiable hard filters; invalid or unexecuted candidates are immediately disqualified.
   - Verified complete report preservation across `NoValidCandidates` and `Aborted` execution errors.
   - Aligned `FallbackPolicy::DeterministicBaseline` ranking report fields (`selected_candidate_id` and `selection_reason`) to reflect baseline selection.
5. **Determinism and Explainability**:
   - Verified bit-for-bit repeatability across repeated runs.
   - Verified multi-tier tie-breaking: Pareto Front $\to$ Objective Score $\to$ Designated Baseline $\to$ Canonical Candidate ID.
   - Formatted ASCII inspection tables and structured diagnostics provide full transparency into selection and disqualification decisions.
6. **Regression Test Reconciliation**:
   - Reconciled all audit findings with 6 dedicated regression tests in `tests/routing/candidate_selection.rs` (lines 800–1000).

---

## 4. Exit Gate Verification

> **EXIT GATE**: The selected candidate is deterministic and the report shows the objective, all candidate scores and selection reason.

- **Deterministic Selection**: Validated via `test_repeat_run_determinism`, `test_tie_breaking_designated_baseline_preference`, and `test_tie_breaking_canonical_candidate_id_alphabetical`. Identical inputs produce bit-for-bit identical ranking tables, Pareto fronts, candidate scores, and selected candidate IDs.
- **Report Completeness**: `RankingReport` retains every candidate (both eligible and disqualified), showing candidate identity, metrics snapshot, evaluated objective score, Pareto front index, 1-based overall rank, and human-readable explanation of selection or disqualification.
- **Selection Reason**: Every selected candidate provides an explicit explanation accessible via `selection.selection_reason()`, `report.selection_reason`, and `ranking.candidates[0].explanation`.

Phase 76 is complete and ready for acceptance. Milestone M9 Phase 77 (Comparative Benchmarks) is unblocked.

