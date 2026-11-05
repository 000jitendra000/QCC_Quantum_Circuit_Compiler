# Implementation Plan: Phase 58 — Schedule Serialization and Visualization Data

## 1. Executive Summary
- **Phase Number**: Phase 58
- **Exact Phase Title**: Schedule serialization and visualization data
- **Milestone**: Milestone M6: Scheduling and Integrated Compilation (`M6`)
- **Prerequisites**: Phases 55–57 (ASAP scheduler, ALAP scheduler, Resource-constrained scheduler) — Completed and verified.
- **Estimated Effort**: Medium (M)
- **Primary Objective**: Make physical schedules reproducibly exportable and inspectable, while preserving timing provenance, physical operand identity, and hardware resource information. Generate structured timeline visualization inputs suitable for external rendering or terminal inspection without flattening unknown durations to zero.

---

## 2. Architecture & Design Principles

1. **Deterministic Schedule Output Representation ([`src/scheduler/output.rs`](file:///home/ronanki_jitendra/rust/QCC/src/scheduler/output.rs))**:
   - `ScheduleOutput`: Canonical serialized output format encapsulating circuit metadata, target model identification, total scheduled makespan, sorted operation list, active hardware resource list, and schedule summary statistics.
   - `ScheduledOperationOutput`: Operation-level export capturing stable semantic `OperationId`, execution sequence number, operation mnemonic, physical qubit operands (`PhysicalQubitId`), classical register effects (`classical_reads`, `classical_writes`), start and end timestamps in nanoseconds, timing model confidence, required hardware resource reservations, DAG predecessor IDs, and calculated slack.
   - `ScheduleSummary`: Global schedule statistics: total operations, scheduled operations, unknown timing operations count, physical qubits utilized list, critical path operation IDs, and peak concurrency.
   - Deterministic chronological ordering: Operations are strictly sorted by:
     $$(t_{\text{start}}, t_{\text{end}}, \text{sequence\_number}, \text{op\_id})$$
     with canonical IEEE 754 float normalization (`exact_cmp` normalizing $-0.0$ to $+0.0$).
   - Bidirectional JSON fidelity: Full round-trip `to_json()`, `to_json_pretty()`, and `from_json()` methods.

2. **Explicit Unknown Preservation (Non-Negotiable Invariant)**:
   - `OperationTiming`: Explicitly typed enum distinguishing:
     - `Known { duration_ns, unit }`
     - `Estimated { duration_ns, unit }`
     - `Unknown`
   - Unknown timing is NEVER converted into `0.0` or any other fabricated numeric value. In `OperationTiming::Unknown`, `duration_ns` is `None`.
   - Attempting to reconstruct a physical execution `Schedule` via `to_schedule()` from output containing uncalibrated `OperationTiming::Unknown` fails fast with structured `ScheduleError::UnknownDuration`.

3. **Structured Timeline Visualization Model ([`src/scheduler/timeline.rs`](file:///home/ronanki_jitendra/rust/QCC/src/scheduler/timeline.rs))**:
   - `TimelineData`: Structured, machine-readable timeline view model independent of external rendering engines.
   - `TimelineLaneKind`: Typed hardware execution channels:
     - `Qubit(PhysicalQubitId)`
     - `Coupler(PhysicalQubitId, PhysicalQubitId)`
     - `ReadoutLine(u32)`
     - `DriveLine(u32)`
     - `AcquisitionUnit(u32)`
     - `ClassicalBit(ClassicalBitId)`
     - `Custom(String)`
   - `TimelineBlock`: Discrete scheduled intervals placed in parallel lanes with display labels, start/end bounds, instantaneous flags for virtual gates, slack, and critical path indicators.
   - `TimelineDependencyLink`: Explicit visual dependency edges connecting predecessor endpoints to successor start points, classified as `DagPredecessor`, `ClassicalHazard`, or `ResourcePrecedence`.
   - `to_ascii_timeline`: Built-in human-readable ASCII text summary formatting lane intervals and critical path statistics for terminal inspection.

4. **Integration with `Schedule` & `ScheduleValidator`**:
   - Extended `Schedule` with `to_output(target)` and `generate_timeline()` convenience methods.
   - Full round-trip verification: `Schedule -> ScheduleOutput -> JSON -> ScheduleOutput -> Schedule -> ScheduleValidator::validate`.

---

## 3. Files Created and Modified

### Files Created:
1. `src/scheduler/output.rs`: `TimingKind`, `OperationTiming`, `ScheduledOperationOutput`, `ScheduleSummary`, `ScheduleOutput`.
2. `src/scheduler/timeline.rs`: `TimelineLaneKind`, `TimelineLane`, `TimelineBlock`, `DependencyLinkKind`, `TimelineDependencyLink`, `TimelineData`.
3. `tests/scheduler/output.rs`: Comprehensive integration test suite (13 tests).
4. `tests/scheduler_output.rs`: Integration test harness for Cargo discovery.
5. `docs/project-state/plans/phase-058.md`: Phase 58 implementation plan.
6. `docs/project-state/reports/phase-058.md`: Phase 58 completion report.

### Files Modified:
1. `src/scheduler/mod.rs`: Registered `output` and `timeline` modules, added `ScheduleError::IncompleteScheduleData`, and re-exported public API types.
2. `src/scheduler/schedule.rs`: Added `Schedule::to_output` and `Schedule::generate_timeline` methods with doc links.
3. `src/lib.rs`: Exposed Phase 58 public API from `qcc::scheduler`.
4. `docs/project-state/progress.md`: Recorded Phase 58 completion and Milestone M6 progress.
5. `README.md`: Updated project status to Phase 58 Complete.

---

## 4. Verification and Exit Gate
- `cargo fmt --check`: Zero discrepancies.
- `cargo clippy --all-targets -- -D warnings`: Zero warnings.
- `cargo test --test scheduler_output`: All 13 Phase 58 integration tests pass.
- `cargo test --all`: Full workspace regression passes (1,361 tests across 40 test targets).
- `cargo doc --no-deps`: Documentation builds cleanly with zero warnings.
- Git commit created locally as Phase 58 checkpoint; no remote push.
