# Phase Plan: Phase 49 — Routing Objectives and Metrics

## 1. Metadata
- **Phase Number**: Phase 49
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: M
- **Prerequisites Satisfied**: Phase 44 (Logical/physical layout model), Phase 45 (Layout-aware equivalence), Phase 46 (Initial placement), Phase 47 (Baseline shortest-path router), Phase 48 (SWAP insertion and dependency maintenance).
- **Source Document Page**: Page 26 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Measure router quality using explicit, comparable objectives. The metrics must support meaningful comparisons between the existing baseline router and future routing algorithms.
- **Explicit Inclusions**:
  1. Implement `RoutingMetrics` in `src/routing/metrics.rs` to compute DAG critical-path depth, two-qubit critical-path depth, total gate counts, single-qubit gate counts, two-qubit gate counts, multi-qubit gate counts, inserted routing SWAP counts, native entangler counts, direction correction counts, critical-path duration using `DurationCost` under the Explicit Unknown Principle, seed, layout information, and wall-clock execution time.
  2. Implement `RoutingCostWeights` and `RoutingCostModel` in `src/routing/cost.rs` to provide configurable, multi-objective scoring functions with well-defined units and clear semantics.
  3. Integrate `RoutingMetrics` into `BaselineRouterReport` in `src/routing/greedy.rs`, recording execution duration and placement seed.
  4. Create `benches/routing_baseline.rs` with a reproducible baseline benchmark corpus covering representative topologies (line, ring, grid, star, parallel layers, idle ancillae) with deterministic configurations.
  5. Add unit and integration tests in `tests/routing/metrics.rs` covering hand-calculated fixtures for SWAP/entangler counts, DAG depth (sequential, independent, mixed), exact duration vs incomplete/unknown duration, configurable costs, seed/layout preservation, and determinism.
- **Explicit Exclusions**:
  1. No SABRE routing algorithm, front-layer readiness, or lookahead decay heuristics (Phases 50–52).
  2. No automated benchmark regression comparison tooling (Phase 53).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero `#![feature(...)]` flags).
- [x] DAG depth is strictly computed using dependency relationships in the QccIR graph, not sequential gate counts.
- [x] The Explicit Unknown Principle is strictly maintained: missing timing data remains `DurationCost::Absent` / `Incomplete` / `Unknown`, never silently substituted with zero.
- [x] Deterministic metrics: identical circuits, targets, and configurations yield strictly identical metric structs, with wall-clock time isolated.
- [x] Distinct typed `QubitId` and `PhysicalQubitId` identities preserved.
- [x] Provenance is preserved so every metric can be linked to the underlying physical operations and original logical operations.
- [x] ADR-0001 strict ancilla zero-state policy preserved.

## 4. Step-by-Step Implementation Sequence
1. **Routing Metrics Module (`src/routing/metrics.rs`)**:
   - Define `RoutingMetrics` capturing DAG depth, 2Q depth, gate counts, routing SWAP counts, original entanglers, duration cost, layouts, seed, and wall-clock run time.
   - Implement `evaluate_routing_metrics` using Kahn's topological order on the QccIR dependency DAG to compute critical path depths and durations.
   - Implement `is_deterministic_match(&self, other: &Self) -> bool`.
2. **Routing Cost Module (`src/routing/cost.rs`)**:
   - Define `RoutingCostWeights` with configurable weight coefficients.
   - Define `RoutingCostModel` supporting standard and checked evaluation against `RoutingMetrics`.
   - Implement presets: `default()`, `gate_count_only()`, `depth_focused()`, and `duration_focused()`.
3. **Baseline Router Integration (`src/routing/greedy.rs`)**:
   - Add `cost_weights: RoutingCostWeights` to `BaselineRouterConfig`.
   - Measure wall-clock runtime of `route_circuit`.
   - Evaluate `RoutingMetrics` and store it in `BaselineRouterReport.metrics`.
4. **Public Exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Re-export `RoutingMetrics`, `RoutingCostWeights`, `RoutingCostModel`, `RoutingCostError`.
5. **Benchmark Corpus (`benches/routing_baseline.rs`, `Cargo.toml`)**:
   - Add `[[bench]]` configuration to `Cargo.toml` with `harness = false`.
   - Implement benchmark corpus covering line, ring, grid, star, and heavy-ancilla circuits with explicit seeds and known configurations.
   - Format and output structured benchmark metrics.
6. **Hand-Calculated & Integration Tests (`tests/routing/metrics.rs`, `tests/routing.rs`)**:
   - Test DAG depth on sequential, independent, and mixed dependency DAGs.
   - Test SWAP and native entangler counts.
   - Test exact duration on calibrated targets vs incomplete/unknown duration on uncalibrated targets.
   - Test configurable cost calculations against hand-derived values.
   - Test determinism and layout/seed preservation.
7. **Verification & Ledger Updates**:
   - Execute formatting, Clippy, full test suite, documentation, and benchmark verification.
   - Write completion report `docs/project-state/reports/phase-049.md` and update `docs/project-state/progress.md`.

## 5. Expected Files and Modules
- New files:
  - `src/routing/metrics.rs`
  - `src/routing/cost.rs`
  - `benches/routing_baseline.rs`
  - `tests/routing/metrics.rs`
  - `docs/project-state/plans/phase-049.md`
  - `docs/project-state/reports/phase-049.md`
- Modified files:
  - `Cargo.toml`
  - `src/routing/greedy.rs`
  - `src/routing/mod.rs`
  - `src/lib.rs`
  - `tests/routing.rs`
  - `docs/project-state/progress.md`

## 6. Required Tests & Test Matrix
- Unit & Hand-Calculated Fixtures:
  - `test_hand_calculated_sequential_dag_depth`: 3 sequential gates on 1 qubit = depth 3, 2Q depth 0.
  - `test_hand_calculated_independent_parallel_dag_depth`: 3 parallel gates on 3 qubits = depth 1, 2Q depth 0.
  - `test_hand_calculated_mixed_dependencies_dag_depth`: 4 qubits with interleaved 1Q and 2Q gates = depth 3, 2Q depth 2.
  - `test_hand_calculated_swap_and_entangler_counts`: Routed CX across distance-2 line = 1 SWAP, 2 native entanglers, 1 original entangler.
  - `test_hand_calculated_exact_duration_calibrated`: Fully calibrated target = exact critical-path duration in ns.
  - `test_missing_and_incomplete_duration_explicit_unknown`: Partial calibration yields `DurationCost::Incomplete`; zero calibration yields `DurationCost::Unknown`.
  - `test_hand_calculated_weighted_cost`: Verify composite cost against exact manual formula.
  - `test_metrics_determinism_across_runs`: Assert `is_deterministic_match` across repeated executions.
  - `test_seed_and_layout_preservation`: Verify seed and layouts in `report.metrics`.
- Benchmark Corpus:
  - `benches/routing_baseline.rs`: Line, ring, grid, star, ancilla-padded, and parallel fixtures.

## 7. Definition of Done
- `RoutingMetrics` accurately evaluates DAG depth, 2Q depth, gate counts, SWAPs, entanglers, durations, layouts, and costs.
- Unknown timing remains strictly unknown.
- Baseline benchmark corpus runs cleanly and reproducibly.
- All tests pass with zero warnings.

## 8. Failure Modes & Debugging Considerations
- Using gate count instead of DAG longest path for depth.
- Fabricating 0.0 ns for uncalibrated operations.
- Flaky tests comparing non-deterministic wall-clock run time.

## 9. Explicit Exit Gate
> **EXIT GATE**: Baseline benchmark corpus is reproducible and all router results expose comparable metrics and provenance.
