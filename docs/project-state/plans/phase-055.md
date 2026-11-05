# Implementation Plan: Phase 55 — ASAP Scheduler

## 1. Executive Summary
- **Phase Number**: Phase 55
- **Exact Phase Title**: ASAP scheduler
- **Milestone**: Milestone M6: Scheduling and Integrated Compilation (`M6`)
- **Prerequisites**: Phase 43 (Final Target Legality Validator), Phase 53 (Router Validation), Phase 54 (Target Timing & Resource Model) — All completed and verified.
- **Estimated Effort**: Large (L)
- **Primary Objective**: Schedule final routed physical operations as early as DAG dependencies and hardware resource constraints permit, with an independent schedule validator.

---

## 2. Architecture & Design Principles

1. **Scheduling After Routing**:
   Operates strictly on the final mapped physical QccIR DAG. Qubit references are physical device qubits ($P$).
2. **Stable Deterministic Topological Ordering**:
   Traverses operations using Kahn's algorithm with deterministic tie-breaking by `OperationId`, ensuring 100% bit-for-bit repeatable schedules across executions.
3. **Predecessor & Hazard Respect**:
   Earliest start time $T_{\text{pred}}(u) = \max(\{ \text{end\_time}(v) \mid v \in \text{predecessors}(u) \} \cup \{ 0.0 \})$, respecting quantum ordering, barriers, and classical data hazards (RAW, WAR, WAW).
4. **Hardware Resource Constraints**:
   Consults Phase 54 `Target` timing and resource model:
   - Physical durations derived via `target.operation_duration(name, operands)`.
   - Dedicated exclusive `Qubit` locks for operands.
   - Dedicated symmetric `Coupler` locks for 2-qubit interactions.
   - Shared drive lines (`DriveLine`) preventing disjoint gates from running concurrently when sharing microwave lines.
   - Multiplexed readout feedlines (`ReadoutLine`) with finite capacity.
   - Mutual exclusion rules (global exclusive operations, spectator cross-talk pairs, readout exclusion groups).
5. **The Explicit Unknown Principle**:
   Unknown duration or missing resource metadata is **never silently interpreted as zero duration or unrestricted concurrency**. Fails fast with structured `ScheduleError`.
6. **Independent Schedule Validator**:
   Decoupled validator verifying completeness, predecessor completion, qubit single-occupancy, coupler exclusivity, shared capacity limits, mutual exclusions, and duration matching against hardware calibration.

---

## 3. Files to Create and Modify

### Files to Create:
1. `src/scheduler/mod.rs`: Module exports and error types (`ScheduleError`).
2. `src/scheduler/schedule.rs`: `ScheduledOperation` and `Schedule` data structures.
3. `src/scheduler/asap.rs`: `AsapScheduler` and `ScheduleConfig`.
4. `src/scheduler/validator.rs`: `ScheduleValidator`, `ScheduleValidationReport`, `ScheduleValidationError`.
5. `tests/scheduler/asap.rs`: Comprehensive integration test suite.
6. `tests/scheduler_asap.rs`: Test target runner for Cargo discovery.

### Files to Modify:
1. `src/lib.rs`: Expose `pub mod scheduler;` and re-export primary scheduling types.
2. `docs/project-state/progress.md`: Update Phase 55 status upon completion.
3. `docs/project-state/reports/phase-055.md`: Phase 55 completion report.

---

## 4. Verification and Exit Gate
- `cargo fmt --check`: Clean formatting.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `cargo test --test scheduler_asap`: All Phase 55 integration tests pass.
- `cargo test --all`: Full workspace regression passes.
- `cargo doc --no-deps`: Documentation builds cleanly.
- Git commit created locally as Phase 55 checkpoint; no push.
