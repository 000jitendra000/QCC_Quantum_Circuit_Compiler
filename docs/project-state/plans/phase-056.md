# Implementation Plan: Phase 56 — ALAP Scheduler

## 1. Executive Summary
- **Phase Number**: Phase 56
- **Exact Phase Title**: ALAP scheduler
- **Milestone**: Milestone M6: Scheduling and Integrated Compilation (`M6`)
- **Prerequisites**: Phase 55 (ASAP Scheduler) — Completed and audited.
- **Estimated Effort**: Large (L)
- **Primary Objective**: Schedule operations as late as possible within a defined or derived makespan window, calculating operation slack, identifying critical path operations, and detecting infeasible schedule windows or negative slack.

---

## 2. Architecture & Design Principles

1. **Explicit Makespan Policy**:
   - `AlapMakespanPolicy::DerivedFromAsap`: Automatically calculates minimum makespan $M_{\text{ASAP}}$ and earliest start times $\text{start}_{\text{ASAP}}$ via an initial ASAP pass.
   - `AlapMakespanPolicy::Explicit(f64)`: Enforces an explicit makespan window $M$. If $M + \epsilon < M_{\text{ASAP}}$, scheduling fails fast with `ScheduleError::InfeasibleMakespan`.
2. **RC-CPM (Resource-Constrained Critical Path Method) Resource Precedence Preservation**:
   - For every pair of operations $(u, v)$ with target resource conflict (`operations_conflict`), preserves the execution order decided by ASAP: $u \prec_{\text{res}} v$ if $u$ completed before $v$ in ASAP.
   - Preserves resource-ordering equivalence classes between forward and backward scheduling, mathematically guaranteeing $\text{Slack}(u) \ge 0$ for all $M \ge M_{\text{ASAP}}$ and preventing heuristic resource reordering artifacts.
3. **Descending Reverse Traversal Order**:
   - Sorts operations in descending order of `(asap_end, asap_start, seq_num, op_id)`.
   - Guarantees that for every DAG or resource precedence $u \prec^* v$, $v$ is visited and scheduled prior to $u$.
4. **Latest Completion from Successors**:
   - For an operation $u$:
     $$T_{\text{succ}}^*(u) = \min (\{ M \} \cup \{ \text{start}_{\text{ALAP}}(v) \mid v \in \text{DAG-succ}(u) \cup \text{Resource-succ}(u) \})$$
5. **Latest Feasible Placement Search**:
   - Candidate end times $\mathcal{C}_{\text{end}}(u) \subseteq [0, T_{\text{succ}}^*(u)]$ are derived from transition boundaries ($T_{\text{succ}}^*$, scheduled operation starts and ends, duration offsets, and explicitly $\text{end}_{\text{ASAP}}(u)$).
   - Candidates are evaluated in descending chronological order using exact half-open interval semantics (`asap::is_interval_feasible`), selecting the latest conflict-free placement.
   - If placement fails, returns `ScheduleError::AlgorithmicPlacementFailure` (distinguished from mathematical makespan infeasibility).
6. **Operation Slack & Infeasibility Detection**:
   - Operation slack is computed as $\text{Slack}(u) = \text{start}_{\text{ALAP}}(u) - \text{start}_{\text{ASAP}}(u)$.
   - Critical path operations have $\text{Slack}(u) = 0.0$.
   - Any negative slack ($< -\epsilon$) is strictly reported via `ScheduleError::NegativeSlack`.
7. **Schedule Representation & Query Extensions**:
   - `ScheduledOperation`: stores optional `slack`.
   - `Schedule`: stores optional `requested_makespan`, and provides helper methods `slack(op_id)`, `slacks()`, `critical_path()`, and `critical_path_with_tolerance(eps)`.
8. **Decoupled Physical & Independent Slack Validation in `ScheduleValidator`**:
   - Standard `validate` performs physical schedule validation (target legality, durations, qubit/coupler collisions, exclusive resources, shared capacities, mutual exclusions, makespan window `WindowExceeded`, and self-reported `NegativeSlack`).
   - `validate_alap` and `.with_alap_validation(true)` independently derive an ASAP baseline, requiring calculated slack (`MissingSlack`), detecting value discrepancies (`SlackMismatch`), and rejecting negative or forged positive slack.

---

## 3. Files Created and Modified

### Files Created:
1. `src/scheduler/alap.rs`: `AlapScheduler`, `AlapMakespanPolicy`.
2. `tests/scheduler/alap.rs`: Comprehensive integration test suite (36 tests).
3. `tests/scheduler_alap.rs`: Test target runner for Cargo discovery.
4. `docs/project-state/plans/phase-056.md`: Phase 56 implementation plan.
5. `docs/project-state/reports/phase-056.md`: Phase 56 completion report.

### Files Modified:
1. `src/scheduler/schedule.rs`: Added `slack` to `ScheduledOperation` and `requested_makespan`, `with_requested_makespan`, `slack`, `slacks`, `critical_path` to `Schedule`.
2. `src/scheduler/validator.rs`: Added `WindowExceeded`, `NegativeSlack`, `MissingSlack`, `SlackMismatch` to `ScheduleValidationError`; added `verify_alap_slack`, `with_alap_validation`, `validate_alap` and independent ASAP baseline verification to `ScheduleValidator`.
3. `src/scheduler/mod.rs`: Added `InfeasibleMakespan`, `NegativeSlack`, `AlgorithmicPlacementFailure` to `ScheduleError`; exported `AlapScheduler` and `AlapMakespanPolicy`.
4. `src/scheduler/asap.rs`: Shared `is_interval_feasible` as `pub(crate)`.
5. `docs/project-state/progress.md`: Update Phase 56 status.

---

## 4. Verification and Exit Gate
- `cargo fmt --check`: Clean formatting across the entire codebase.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `cargo test --test scheduler_alap`: 36 Phase 56 integration tests pass.
- `cargo test --all`: Full workspace regression passes (1,328 tests across 38 test targets).
- `cargo doc --no-deps`: Documentation builds cleanly with zero warnings.
- Git commit created locally as Phase 56 checkpoint; no remote push.
