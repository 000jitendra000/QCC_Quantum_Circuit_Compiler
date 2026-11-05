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
   - `AlapMakespanPolicy::Explicit(f64)`: Enforces an explicit makespan window $M$. If $M < M_{\text{ASAP}}$, scheduling fails explicitly with `ScheduleError::InfeasibleMakespan`.
2. **Reverse Stable Topological Traversal**:
   - Traverses DAG nodes in reverse deterministic topological order (`dag_order.iter().rev()`), ensuring that all successors of an operation are scheduled prior to scheduling the operation itself.
3. **Latest Completion from Successors**:
   - For an operation $u$:
     $$T_{\text{succ}}(u) = \begin{cases} \min_{v \in \text{succ}(u)} \text{start}(v) & \text{if } \text{succ}(u) \neq \emptyset \\ M & \text{otherwise (terminal/sink)} \end{cases}$$
4. **Latest Feasible Placement Search**:
   - Candidate end times $\mathcal{C}_{\text{end}}(u) \subseteq [0, T_{\text{succ}}(u)]$ are derived from transition boundaries ($T_{\text{succ}}$, scheduled operation starts and ends, and duration offsets).
   - Candidates are evaluated in descending chronological order using exact half-open interval semantics (`asap::is_interval_feasible`), selecting the latest conflict-free placement.
5. **Operation Slack & Infeasibility Detection**:
   - Operation slack is computed as $\text{Slack}(u) = \text{start}_{\text{ALAP}}(u) - \text{start}_{\text{ASAP}}(u)$.
   - Critical path operations have $\text{Slack}(u) = 0.0$.
   - Any negative slack ($< -\epsilon$) is strictly reported via `ScheduleError::NegativeSlack`.
6. **Schedule Representation & Query Extensions**:
   - `ScheduledOperation`: stores optional `slack`.
   - `Schedule`: stores optional `requested_makespan`, and provides helper methods `slack(op_id)`, `slacks()`, `critical_path()`, and `critical_path_with_tolerance(eps)`.
7. **Independent Schedule Validator Verification**:
   - Decoupled `ScheduleValidator` checks bijective DAG node coverage, predecessor causality, target durations, qubit/coupler collisions, exclusive resource collisions, shared capacities, mutual exclusions, makespan window enforcement (`WindowExceeded`), and operation slack (`NegativeSlack`).

---

## 3. Files Created and Modified

### Files Created:
1. `src/scheduler/alap.rs`: `AlapScheduler`, `AlapMakespanPolicy`.
2. `tests/scheduler/alap.rs`: Comprehensive integration test suite (20 tests).
3. `tests/scheduler_alap.rs`: Test target runner for Cargo discovery.
4. `docs/project-state/plans/phase-056.md`: Phase 56 implementation plan.
5. `docs/project-state/reports/phase-056.md`: Phase 56 completion report.

### Files Modified:
1. `src/scheduler/schedule.rs`: Added `slack` to `ScheduledOperation` and `requested_makespan`, `with_requested_makespan`, `slack`, `slacks`, `critical_path` to `Schedule`.
2. `src/scheduler/validator.rs`: Added `WindowExceeded` and `NegativeSlack` to `ScheduleValidationError` and validator checks.
3. `src/scheduler/mod.rs`: Added `InfeasibleMakespan` and `NegativeSlack` to `ScheduleError`; exported `AlapScheduler` and `AlapMakespanPolicy`.
4. `src/scheduler/asap.rs`: Shared `is_interval_feasible` as `pub(crate)`.
5. `docs/project-state/progress.md`: Update Phase 56 status.

---

## 4. Verification and Exit Gate
- `cargo fmt --check`: Clean formatting across the entire codebase.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `cargo test --test scheduler_alap`: 20 Phase 56 integration tests pass.
- `cargo test --all`: Full workspace regression passes (1,312 tests across 36 test targets).
- `cargo doc --no-deps`: Documentation builds cleanly.
- Git commit created locally as Phase 56 checkpoint; no remote push.
