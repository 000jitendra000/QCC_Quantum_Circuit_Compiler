# Phase Completion Report: Phase 49 — Routing Objectives and Metrics

## 1. Summary
- **Phase Number**: Phase 49
- **Exact Phase Title**: Routing objectives and metrics
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Completion Date**: 2026-11-04
- **Engineer / Agent Session**: Antigravity Assistant

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/routing/metrics.rs` — Comprehensive physical circuit evaluation. Implements `RoutingMetrics` capturing critical-path DAG depth, critical-path two-qubit depth, total gate count, single-qubit gate count, two-qubit gate count, multi-qubit gate count, inserted routing SWAPs count, routing SWAP gate count, original entanglers count, direction correction gates count, total routing overhead operations, gate count histogram, critical-path execution duration under the Explicit Unknown Principle, initial and final layouts, placement seed, wall-clock run time, and composite weighted cost. Implements `is_deterministic_match` for exact reproducible comparison across runs.
  - `src/routing/cost.rs` — Configurable multi-objective scoring models. Implements `RoutingCostWeights`, `RoutingCostModel`, and `RoutingCostError` supporting standard evaluation, checked evaluation (enforcing non-negative weights and optional exact timing requirement), and preset configurations (`default`, `gate_count_only`, `depth_focused`, `duration_focused`).
  - `benches/routing_baseline.rs` — Reproducible baseline routing benchmark corpus. Evaluates 8 representative circuit fixtures across linear, ring, grid, star, ancilla-padded, parallel, and seeded-random topologies, recording SWAP overhead, depth, cost, and execution duration.
  - `tests/routing/metrics.rs` — Hand-calculated fixtures and integration tests validating sequential DAG depth, independent parallel DAG depth, mixed-layer DAG depth, SWAP and entangler counts under native SWAP and decomposed CX, calibrated exact duration, incomplete/unknown duration under the Explicit Unknown Principle, weighted cost formulas, determinism, and seed/layout preservation.
  - `docs/project-state/plans/phase-049.md` — Phase 49 implementation plan.
  - `docs/project-state/reports/phase-049.md` — This completion report.
- **Modified Files**:
  - `Cargo.toml` — Registered `[[bench]]` binary `routing_baseline` with `harness = false`.
  - `src/routing/greedy.rs` — Integrated `RoutingMetrics` and `cost_weights` into `BaselineRouterConfig` and `BaselineRouterReport`, capturing wall-clock duration and placement seed.
  - `src/routing/mod.rs` — Re-exported `cost` and `metrics` modules, along with `RoutingMetrics`, `RoutingCostWeights`, `RoutingCostModel`, and `RoutingCostError`.
  - `src/lib.rs` — Re-exported top-level routing cost and metric types.
  - `tests/routing.rs` — Registered `metrics` integration test module.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**:
  > **EXIT GATE**: Baseline benchmark corpus is reproducible and all router results expose comparable metrics and provenance.
- **Verification Commands Executed**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo doc --no-deps
  cargo run --example bell && cargo run --example qasm
  cargo bench --bench routing_baseline
  ```
- **Execution Log / Output**:
  - `cargo fmt --check`: Passed with zero diffs.
  - `cargo clippy --all-targets -- -D warnings`: Passed with zero warnings.
  - `cargo test --test routing`: 65 tests passed (11 new integration tests in `metrics.rs`, 0 failed).
  - `cargo test`: All tests passed across all test binaries (0 failed, 0 ignored).
  - `cargo doc --no-deps`: Documentation generated cleanly with zero warnings.
  - `cargo run --example bell` and `cargo run --example qasm`: Executed cleanly.
  - `cargo bench --bench routing_baseline`: Successfully evaluated 8 benchmark fixtures and confirmed 100% deterministic reproducibility across repeated runs.
- **Exit Gate Pass Confirmation**: **Pass** (100% verified with automated tests and benchmark corpus).

## 4. Test Suite Summary
- **Hand-Calculated & Integration Tests Added** (`tests/routing/metrics.rs`):
  - `test_hand_calculated_sequential_dag_depth` — 3 sequential operations on single qubit yield DAG depth 3, 2Q depth 0.
  - `test_hand_calculated_independent_parallel_dag_depth` — 4 parallel operations on 4 disjoint qubits yield DAG depth 1, 2Q depth 0.
  - `test_hand_calculated_mixed_dependencies_dag_depth` — 4 qubits with interleaved 1Q and 2Q gates yield DAG depth 4, 2Q depth 3.
  - `test_hand_calculated_swap_and_entangler_counts_native_swap` — Distance-2 CX on target with native SWAP yields 1 SWAP, 2 physical 2Q gates, 1 original entangler, 1 overhead op.
  - `test_hand_calculated_swap_and_entangler_counts_decomposed_cx_swap` — Target without native SWAP decomposes SWAP to 3 CX gates; yields 1 logical SWAP, 3 SWAP gates, 4 physical 2Q gates, 3 overhead ops.
  - `test_hand_calculated_exact_duration_calibrated` — Fully calibrated target yields exact critical path duration (180.0 ns).
  - `test_missing_and_incomplete_duration_explicit_unknown` — Uncalibrated operations yield `DurationCost::Incomplete` (never fabricating 0.0 ns) or `DurationCost::Unknown`.
  - `test_hand_calculated_weighted_cost` — Evaluates composite score against exact manual formula (165.0), verifying checked evaluation and error handling.
  - `test_negative_weight_validation` — Confirms negative cost weights fail validation with `RoutingCostError::NegativeWeight`.
  - `test_metrics_determinism_across_repeated_runs` — Repeated router runs yield `is_deterministic_match == true`.
  - `test_seed_and_layout_preservation` — Asserts random seed, initial layout, final layout, and elapsed run time are recorded in `report.metrics`.

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (zero `#![feature(...)]` flags).
- [x] DAG depth is strictly computed along dependency edges in the QccIR graph, not sequential gate counts.
- [x] The Explicit Unknown Principle is preserved: missing timing data remains `DurationCost::Absent` / `Incomplete` / `Unknown`, never assumed to be zero.
- [x] Deterministic metrics: identical circuits, targets, and configurations yield strictly identical metric structs, with wall-clock time isolated.
- [x] Distinct typed `QubitId` and `PhysicalQubitId` identities preserved.
- [x] Provenance is preserved so every metric can be linked to the underlying physical operations and original logical operations.
- [x] ADR-0001 strict ancilla zero-state policy preserved.

## 6. Known Limitations & Technical Debt
- Phase 49 establishes quantitative routing metrics and the baseline benchmark corpus. The SABRE routing algorithm incorporating front-layer analysis, lookahead decay heuristics, and bidirectional layout refinement is scheduled for Phases 50–52.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
