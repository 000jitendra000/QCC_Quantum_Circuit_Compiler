# Implementation Plan: Phase 57 — Resource-Constrained Scheduling

## 1. Executive Summary
- **Phase Number**: Phase 57
- **Exact Phase Title**: Resource-constrained scheduling
- **Milestone**: Milestone M6: Scheduling and Integrated Compilation (`M6`)
- **Prerequisites**: Phases 54–56 (Target timing/resource model, ASAP scheduler, ALAP scheduler) — Completed and audited.
- **Estimated Effort**: Extra Large (XL)
- **Primary Objective**: Apply device concurrency and shared-resource limits by associating physical operations with required hardware execution channels, tracking reserved intervals via a dedicated timeline calendar, enforcing aggregate capacities and mutual exclusion rules, handling readout groups and shared control resources, and reporting structured constraints that prevent parallel execution.

---

## 2. Architecture & Design Principles

1. **Hardware Resource Timeline Tracking ([`src/scheduler/resource_calendar.rs`](file:///home/ronanki_jitendra/rust/QCC/src/scheduler/resource_calendar.rs))**:
   - Implements `ResourceReservation` and `ResourceCalendar`.
   - Tracks exact half-open time intervals $[t_{\text{start}}, t_{\text{end}})$ across dedicated qubit channels, couplers, shared microwave drive lines, multiplexed readout feedlines, and custom hardware resources.
   - Reuses identical exact half-open interval overlap semantics:
     $$\max(s_1, s_2) < \min(e_1, e_2)$$
     with explicit support for zero-duration virtual operations at boundaries and inside intervals.
   - Maintains aggregate concurrent capacity tracking:
     $$U(R, t) = \sum_{r \in \text{reservations}(R), r.\text{is\_active\_at}(t)} r.\text{amount} \le \text{capacity}(R)$$
   - Provides rich timeline queries: `active_usage_at`, `active_reservations_at`, `peak_usage`, `timeline_for`, and `operations_active_at`.
   - Implements `ResourceCalendar::from_schedule` building and validating a full calendar from an existing schedule.

2. **Readout Groups & Shared Control Channels ([`src/scheduler/resource_constraints.rs`](file:///home/ronanki_jitendra/rust/QCC/src/scheduler/resource_constraints.rs))**:
   - `ReadoutGroup`: Models multiplexed readout feedlines sharing physical measurement infrastructure with declared capacity limits and configurable `ReadoutSynchronizationPolicy`:
     - `Independent`: operations can be triggered independently as long as feedline capacity is respected.
     - `SimultaneousOnly`: concurrent measurements in the readout group must start at the exact same timestamp ($|t_1 - t_2| \le \epsilon$).
     - `StrictlySerialized`: only one measurement at a time (capacity 1).
   - `SharedControlGroup`: Models microwave drive lines, flux channels, or local oscillators shared among a cluster of physical qubits in exclusive or shared capacity mode.

3. **Concurrency Analysis & Placement Blocker Explainability**:
   - `ResourceConstraintManager::analyze_concurrency`: Evaluates whether two operations can execute concurrently, reporting ALL inhibiting constraints:
     - `SharedQubit(PhysicalQubitId)`: qubit collision.
     - `SharedCoupler(PhysicalQubitId, PhysicalQubitId)`: coupler collision.
     - `ExclusiveResource(ResourceId)`: exclusive resource contention.
     - `CapacityExceeded { resource, capacity, combined_demand }`: aggregate capacity overcommitment.
     - `MutualExclusion { rule, reason }`: target mutual exclusion rule violation.
     - `ReadoutExclusion { qubit1, qubit2, reason }`: spectator readout exclusion.
     - `MissingMetadata { op_name, operands }`: Explicit Unknown Principle violation.
   - `ResourceConstraintManager::explain_placement_blockers`: Diagnostic query explaining *why* an operation cannot start at candidate timestamp $t$ and identifying the expected release time when the resource frees.

4. **Resource-Constrained Scheduler**:
   - `ResourceConstrainedScheduler`: Integrates `ResourceConstraintManager` and `ResourceCalendar` to schedule a QccIR circuit DAG into a physical `Schedule`, recording explainable `SchedulingTraceEntry` diagnostics for every gate delayed past its predecessor bound.

---

## 3. Files Created and Modified

### Files Created:
1. `src/scheduler/resource_calendar.rs`: `ResourceCalendar`, `ResourceReservation`, `ResourceConflictReport`, `ResourceConflictReason`, `BlockingOpSummary`.
2. `src/scheduler/resource_constraints.rs`: `ResourceConstraintManager`, `ResourceConstrainedScheduler`, `ReadoutGroup`, `SharedControlGroup`, `ReadoutSynchronizationPolicy`, `InhibitingConstraint`, `ConcurrencyAnalysis`, `PlacementBlocker`, `SchedulingTraceEntry`.
3. `tests/scheduler/resources.rs`: Comprehensive integration test suite (20 tests).
4. `tests/scheduler_resources.rs`: Integration test runner for Cargo discovery.
5. `docs/project-state/plans/phase-057.md`: Phase 57 implementation plan.
6. `docs/project-state/reports/phase-057.md`: Phase 57 completion report.

### Files Modified:
1. `src/scheduler/mod.rs`: Exported `resource_calendar` and `resource_constraints` types.
2. `src/lib.rs`: Exposed Phase 57 public API.
3. `docs/project-state/progress.md`: Recorded Phase 57 completion and Milestone M6 progress.
4. `README.md`: Updated project status.

---

## 4. Verification and Exit Gate
- `cargo fmt --check`: Clean formatting across the entire codebase.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `cargo test --test scheduler_resources`: All 20 Phase 57 integration tests pass.
- `cargo test --all`: Full workspace regression passes (1,348 tests across 39 test targets).
- `cargo doc --no-deps`: Documentation builds cleanly with zero warnings.
- Git commit created locally as Phase 57 checkpoint; no remote push.
