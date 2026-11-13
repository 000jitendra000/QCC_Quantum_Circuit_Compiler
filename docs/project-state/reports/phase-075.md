# Phase 75 Completion Report: Seeded Multi-Run SABRE Candidate Generation

```text
Phase: Phase 75
Milestone: Milestone M9 — Multi-Candidate Routing Improvements
Roadmap: QCC Engineering Roadmap v3.1 (Phases 60–85)
Date: 2026-11-13
Status: COMPLETED & ACCEPTED
```

---

## 1. Executive Summary

**Phase 75** implements the multi-run SABRE candidate generation and execution engine for **Milestone M9 (Multi-Candidate Routing Improvements)**.

Building on the Phase 74 configuration, budget, and objective contracts, Phase 75 introduces the bounded execution engine (`MultiCandidateSearchRunner`) that orchestrates multiple SABRE routing attempts over varied pseudo-random seeds and placement strategies while strictly enforcing candidate isolation, cumulative resource limits, independent verification of all mandatory hard constraints, and deterministic error preservation.

Key architectural deliverables:
1. **Candidate Isolation**: Every candidate executes against an isolated clone of the input `Circuit` and immutable `Target`. A routing failure, budget exhaustion, or state change in one candidate cannot mutate other candidates or caller state.
2. **Cumulative Search Resource Accounting**: Exact iteration and SWAP counts are tracked across all forward, reverse, and refinement passes of SABRE via extended `SabreRouterReport` instrumentation. Remaining global search budgets are dynamically clamped before each candidate run. When a candidate fails, its partial work is captured and credited to global accounting.
3. **Cooperative Timeout Semantics**: Search deadlines (`budget.timeout_ms`) are evaluated cooperatively between candidate executions. Expired budgets gracefully skip remaining candidates without aborting or losing already-completed valid candidates.
4. **Independent, Truthful Validation Evidence**: Each routed candidate is independently checked against all three mandatory hard constraints:
   - Target physical gate set and coupling directionality legality (`TargetLegalityValidator`).
   - Logical unitary equivalence up to a single common global phase (`LayoutEquivalenceChecker`).
   - ADR-0001 ancilla zero-state preservation (`LayoutEquivalenceChecker`).
   For physical qubit counts $n_{\text{phys}} > 16$, exponential simulation is safely avoided and truthfully reported as `CheckStatus::Unexecuted`.
5. **Preserved Execution Reports on Error**: `RoutingSearchExecutionError` embeds `Box<MultiCandidateExecutionReport>` across all error variants (`Aborted`, `InvalidBaselineFallback`, `AllCandidatesFailed`, `NoValidCandidates`), guaranteeing that diagnostic data for all executed and skipped candidates is preserved.
6. **Provisional Selection API**: Prior to Phase 76 multi-objective ranking, `select_provisional_candidate()` provides deterministic candidate selection, preferring the validated baseline candidate, then the first fully validated candidate, with stable collision-resistant candidate IDs.
7. **100% Backward Compatibility**: For single-candidate configurations (`candidate_count: 1`), compiler pipeline execution takes a zero-overhead fast path directly into `SabreRouter`, preserving identical routing decisions and stage metrics.

---

## 2. Architecture and Implementation Details

### 2.1 File and Module Layout

- **`src/routing/search/runner.rs`**: Core execution engine:
  - `MultiCandidateSearchRunner`: Search orchestration, cumulative budgeting, timeout evaluation, candidate isolation, and fallback handling.
  - `CandidateExecutionRecord`: Per-candidate provenance, execution status, validation results, duration, and running resource totals.
  - `CandidateExecutionStatus`: `Pending`, `Skipped(BudgetExhaustionReason)`, `Success { routed_circuit, report }`, `RoutingFailed(RoutingError)`, `ValidationFailed`. Large variants are boxed to satisfy clippy memory efficiency guidelines.
  - `CandidateValidationRecord`: Individual statuses (`CheckStatus::Passed`, `CheckStatus::Failed`, `CheckStatus::Unexecuted`) for target legality, semantic equivalence, and ancilla zero-state preservation.
  - `MultiCandidateExecutionReport`: Search-wide summary with candidate records, cumulative iterations, cumulative swaps, search duration, and budget exhaustion status.
  - `select_provisional_candidate()`: Deterministic provisional candidate selection returning `ProvisionalCandidateSelection` with stable candidate ID.
- **`src/routing/search/error.rs`**: Extended with `RoutingSearchExecutionError`:
  - `Aborted(SearchBudget, Box<MultiCandidateExecutionReport>)`
  - `InvalidBaselineFallback(Box<CandidateExecutionRecord>, Box<MultiCandidateExecutionReport>)`
  - `AllCandidatesFailed(Box<MultiCandidateExecutionReport>)`
  - `NoValidCandidates(Box<MultiCandidateExecutionReport>)`
  - `Config(RoutingSearchConfigError)`
  - Direct accessor `execution_report(&self) -> Option<&MultiCandidateExecutionReport>`.
- **`src/routing/sabre/bidirectional.rs`**:
  - Instrumented `route_single_pass` to return local loop iteration count.
  - Extended `SabreRouterReport` with `total_iterations: usize` and `total_swaps_executed: usize`.
  - Accumulated exact iterations and SWAPs across forward, reverse, and bidirectional refinement passes in `route_circuit`.
- **`src/verification/layout_equivalence.rs`**:
  - Extended with `LayoutEquivalenceReport` separating `verdict`, `logical_equivalence_passed`, `ancilla_zero_state_passed`, and `max_ancilla_leakage`.
  - Added `check_layout_equivalence_detailed` and `check_layout_equivalence_detailed_with_layouts` without modifying or weakening existing boolean APIs.
- **`src/compiler/pipeline.rs`**:
  - Integrated `MultiCandidateSearchRunner` into `RouterChoice::MultiCandidate`. Single-candidate configurations take the zero-overhead fast path; multi-candidate configurations run the search engine, log progress, and record provisional selection with stable candidate ID in `RoutingStageReport`.
- **`tests/routing/multi_seed.rs`**: Comprehensive test suite with 17 integration and regression tests.

### 2.2 Candidate Execution Lifecycle

```text
[Input Circuit + Target]
         │
         ├──> Validate Search Config against Target
         │
         ├──> Derive Bounded Candidate Descriptors (Phase 74)
         │
         └──> MultiCandidateSearchRunner::run()
                  │
                  ▼
         [For each candidate descriptor]:
           1. Check Cumulative Budgets (swaps, iterations) & Cooperative Timeout
              └─ If exhausted: mark Skipped(reason), break loop
           2. Clamp candidate SabreRouterConfig to remaining search budget
           3. Clone input Circuit & execute SabreRouter::route_circuit()
              ├─ On Failure: record RoutingFailed, accumulate partial work
              └─ On Success: accumulate exact iterations & swaps
           4. Run Independent Hard Constraint Validation:
              ├─ TargetLegalityValidator::validate_circuit()
              ├─ LayoutEquivalenceChecker (logical equivalence)
              └─ ADR-0001 clean ancilla zero-state check
           5. Record CandidateExecutionRecord with elapsed time & running totals
                  │
                  ▼
         [Select Candidate / Handle Fallback]:
           ├─ If all candidates failed or invalid: apply FallbackPolicy
           │    ├─ Abort: return RoutingSearchExecutionError::Aborted
           │    ├─ DeterministicBaseline: select Candidate 0 (fail if invalid)
           │    └─ BestValidSoFar: select best valid (fail if none valid)
           └─ Return MultiCandidateExecutionReport & ProvisionalCandidateSelection
```

### 2.3 Cumulative Search Resource Accounting

Single-candidate SABRE router configs specify per-pass limits (`max_swaps`, `max_iterations`). Multi-candidate search introduces cumulative limits across the entire search batch:
- In `src/routing/sabre/bidirectional.rs`, `route_circuit` tracks total iterations and swaps across forward and reverse passes in both single-pass and multi-round refinement modes.
- Before launching candidate $i$, the runner computes remaining swaps and iterations:
  ```rust
  let remaining_swaps = max_total_swaps.saturating_sub(total_swaps_consumed);
  let remaining_iters = max_total_iterations.saturating_sub(total_iterations_consumed);
  ```
- The candidate's `SabreRouterConfig` is clamped so that an individual candidate's routing pass cannot exceed the global budget.
- When a candidate terminates prematurely (e.g. `RoutingError::MaxSwapsExceeded { swaps_attempted, .. }`), the attempted swaps and partial iterations are extracted and credited to cumulative totals.

### 2.4 Independent Hard Constraint Evidence

In accordance with ADR-0001 and project invariants:
- `TargetLegality`: Verified by `TargetLegalityValidator`.
- `SemanticEquivalence`: Verified by `LayoutEquivalenceChecker::check_layout_equivalence_detailed`.
- `AncillaZeroState`: Verified by `LayoutEquivalenceChecker::check_layout_equivalence_detailed` (checking that all non-mapped physical qubits start in $|0\rangle$ and remain strictly in $|0\rangle$ at circuit end).
- Simulation Limit ($n_{\text{phys}} > 16$): For targets with $> 16$ physical qubits, statevector simulation is safely skipped and reported as `CheckStatus::Unexecuted`. A candidate is marked `is_fully_validated() == true` only when all three checks are `Passed` (never fabricated for unexecuted checks).

---

## 3. Verification and Acceptance Evidence

### 3.1 Verification Commands and Results

| Command | Status | Details |
|---|:---:|---|
| `cargo fmt --check` | **PASS** | Strict rustfmt compliance with zero diffs |
| `cargo clippy --all-targets -- -D warnings` | **PASS** | Zero warnings across all targets; boxed large enum variants |
| `cargo doc --no-deps` | **PASS** | Complete rustdoc coverage for all new public items |
| `cargo test --test routing` | **PASS** | 165/165 routing tests passed (148 historical + 17 Phase 75 tests) |
| `PATH=".venv/bin:$PATH" cargo test --no-fail-fast` | **PASS** | 1,475 passed; 0 failed; 0 ignored; 0 filtered across 40 binaries + 1 doc test |
| `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v` | **PASS** | All 61 Python oracle tests passed (36.5s) |

### 3.2 Automated Test Coverage (`tests/routing/multi_seed.rs`)

The new test suite [`tests/routing/multi_seed.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/routing/multi_seed.rs) contains 17 comprehensive tests:

1. `test_single_candidate_runner_backward_compatibility`: Validates that running 1 candidate through `MultiCandidateSearchRunner` produces identical routed circuit, layout, gate count, and SWAP count as direct `SabreRouter`.
2. `test_multi_seed_candidate_generation_isolation`: Proves that running 4 distinct seeds produces 4 distinct candidate records without cross-candidate mutation or input circuit corruption.
3. `test_multi_layout_strategy_generation`: Verifies candidate generation and execution across multiple placement heuristics (`Weighted`, `Degree`, `Random`).
4. `test_budget_max_candidates_clamping`: Confirms that `budget.max_candidates` bounds candidate execution and skips remaining candidates when reached.
5. `test_cumulative_swap_budget_enforcement`: Asserts that `budget.max_total_swaps` halts search when cumulative SWAPs exceed threshold and marks subsequent candidates `Skipped(BudgetExhaustionReason::MaxTotalSwapsExceeded)`.
6. `test_cumulative_iteration_budget_enforcement`: Asserts that `budget.max_total_iterations` halts search when cumulative iterations exceed threshold.
7. `test_partial_work_recorded_on_routing_failure`: Verifies that when a candidate fails routing due to per-candidate limits, its partial SWAPs and iterations are captured in the record and credited to cumulative totals.
8. `test_cooperative_timeout_enforcement`: Tests that search deadlines (`timeout_ms = 0`) cooperatively skip remaining candidates with `BudgetExhaustionReason::Timeout`.
9. `test_fallback_policy_abort`: Validates that `FallbackPolicy::Abort` fails closed with `RoutingSearchExecutionError::Aborted` preserving the full execution report.
10. `test_fallback_policy_deterministic_baseline_valid`: Verifies that `FallbackPolicy::DeterministicBaseline` selects Candidate 0 when valid.
11. `test_fallback_policy_deterministic_baseline_invalid_rejected`: Verifies that if Candidate 0 failed routing, `DeterministicBaseline` rejects fallback and fails closed with `InvalidBaselineFallback`.
12. `test_fallback_policy_best_valid_so_far`: Proves that `FallbackPolicy::BestValidSoFar` recovers a valid candidate even when earlier candidates exhausted budgets.
13. `test_independent_three_constraint_validation`: Directly tests candidate validation, verifying that `target_legality`, `semantic_equivalence`, and `ancilla_zero_state` are each independently reported as `CheckStatus::Passed`.
14. `test_large_target_unexecuted_validation_honesty`: Proves that for targets with $> 16$ qubits, equivalence and ancilla checks are reported as `CheckStatus::Unexecuted` and `is_fully_validated() == false`.
15. `test_provisional_candidate_selection_stable_id`: Confirms that `select_provisional_candidate()` returns a selection embedding the full 64-hex SHA-256 router and target hashes in its `candidate_id`.
16. `test_deterministic_reproducibility`: Proves that executing identical search configurations on identical circuits and targets produces bit-for-bit identical candidate execution reports.
17. `test_compiler_pipeline_multi_candidate_integration`: Executes full compiler pipeline with multi-candidate routing on a 4-qubit linear target, verifying successful compilation and stage report instrumentation.

---

## 4. Documentation

- **Architecture Guide**: Updated [`docs/routing-search.md`](file:///home/ronanki_jitendra/rust/QCC/docs/routing-search.md) with Section 7 describing `MultiCandidateSearchRunner`, candidate isolation, cumulative resource accounting, cooperative timeouts, independent validation evidence, error preservation, and provisional selection.
- **Progress Ledger**: Updated [`docs/project-state/progress.md`](file:///home/ronanki_jitendra/rust/QCC/docs/project-state/progress.md) to record Phase 75 completion and establish Milestone M9 Phase 76 as the next active task.

---

## 5. Scope Boundary Compliance and Deferred Work

In strict accordance with Roadmap v3.1 and project invariants:
- **Phase 75 Only**: Multi-run SABRE candidate generation, execution, budgeting, and provisional selection.
- **Phase 76 (Candidate Ranking and Selection)**: Objective-based Pareto/lexicographic ranking, tie-breaking, and selection explainability reporting remain strictly deferred to Phase 76.
- **Phase 77 (Diagnostics and Benchmarks)**: Diagnostic reporting, CLI exposure, and comparative benchmarking remain deferred to Phase 77.
- **SABRE Core Semantics**: Underlying SABRE pathfinding, decay heuristics, and bidirectional refinement algorithms were preserved without alteration.

---

## 6. Exit Gate Conclusion

> **ROADMAP v3.1 EXIT GATE**: "Multi-run routing is reproducible, bounded and returns only legal, semantically validated candidates."

**Exit Gate Status: MET & ACCEPTED.**
All requirements have been fully implemented, documented, and verified across unit, integration, and oracle test suites. Phase 75 is complete and ready for commit.

