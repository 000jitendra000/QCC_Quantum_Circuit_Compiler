# Phase Completion Report: Phase 47 — Baseline Shortest-Path Router

## 1. Summary
- **Phase Number**: Phase 47
- **Exact Phase Title**: Baseline shortest-path router
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Completion Date**: 2026-11-04
- **Engineer / Agent Session**: Antigravity Assistant

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/routing/pathfinding.rs` — Deterministic shortest-path finding on physical hardware coupling graphs via BFS with sorted neighbor expansion; `RoutingError` definition with typed error variants.
  - `src/routing/swap_insertion.rs` — Path-based SWAP movement strategies (`SourceToTarget`, `TargetToSource`, `Bidirectional`), layout mutation (`apply_physical_swap`), native SWAP emission or target-decomposed CX sequence with direction correction, and operand orientation resolution.
  - `src/routing/greedy.rs` — `BaselineRouter`, `BaselineRouterConfig`, and `BaselineRouterReport` implementing deterministic Kahn's DAG traversal scheduling executable nodes, initial layout resolution via placement algorithms, bounded iteration/SWAP limits, target legality validation, and layout-aware equivalence verification.
  - `tests/routing/greedy.rs` — Comprehensive integration tests across Line, Ring, Grid, Star, Disconnected, and Directed topologies, verifying target legality and layout-aware semantic equivalence under ADR-0001.
  - `docs/project-state/plans/phase-047.md` — Phase 47 implementation plan.
  - `docs/project-state/reports/phase-047.md` — This completion report.
- **Modified Files**:
  - `src/routing/mod.rs` — Re-exported baseline router, pathfinding, and SWAP insertion modules and public types.
  - `src/lib.rs` — Re-exported top-level routing items.
  - `tests/routing.rs` — Registered `greedy` integration test module.
  - `docs/project-state/progress.md` — Updated progress ledger and checkpoint.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**:
  > **EXIT GATE**: Baseline router passes topology and layout-aware semantic tests.
- **Verification Commands Executed**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo doc --no-deps
  cargo run --example bell && cargo run --example qasm
  ```
- **Execution Log / Output**:
  - `cargo fmt --check`: Passed with zero diffs.
  - `cargo clippy --all-targets -- -D warnings`: Passed with zero warnings.
  - `cargo test --test routing`: 44 tests passed (16 new integration tests in `greedy.rs`, 0 failed).
  - `cargo test`: 1,139 tests passed across all 35 test binaries (0 failed, 0 ignored).
  - `cargo doc --no-deps`: Documentation generated cleanly.
  - `cargo run --example bell` and `cargo run --example qasm`: Executed cleanly.
- **Exit Gate Pass Confirmation**: **Pass** (100% verified with automated tests).

## 4. Test Suite Summary
- **Unit Tests Added**:
  - 6 unit tests in `src/routing/pathfinding.rs` (`test_shortest_path_same_qubit`, `test_shortest_path_adjacent`, `test_shortest_path_linear_chain_multi_hop`, `test_shortest_path_ring_wraparound`, `test_shortest_path_disconnected_topology`, `test_shortest_path_out_of_bounds`).
- **Integration Tests Added**:
  - 16 integration tests in `tests/routing/greedy.rs`:
    - `test_baseline_router_line_topology_already_legal` — Directly legal operations require 0 SWAPs.
    - `test_baseline_router_line_topology_single_hop_swap` — Single-hop distance-2 interaction on 3-qubit line.
    - `test_baseline_router_line_topology_multi_hop_swaps` — Multi-hop distance-4 interaction on 5-qubit line.
    - `test_baseline_router_ring_topology_wraparound` — Direct connection across wraparound edge on ring.
    - `test_baseline_router_ring_topology_shortest_wraparound_path` — Choosing shorter wraparound path over linear path.
    - `test_baseline_router_grid_topology_2d` — 2D multi-hop routing across a $2 \times 3$ grid.
    - `test_baseline_router_star_topology_hub` — Leaf-to-leaf routing through central hub on star topology.
    - `test_baseline_router_disconnected_topology_fails_explicitly` — Disconnected components fail fail-closed with `RoutingError::DisconnectedTopology`.
    - `test_baseline_router_directed_coupling_orientation` — Directed coupling orientation detection and resolution.
    - `test_baseline_router_adr0001_ancilla_zero_state_preserved` — Multi-qubit layout-aware equivalence on 4-qubit device with idle ancillae.
    - `test_baseline_router_dependency_preservation` — Interleaved dependent 2-qubit operations on shared wires.
    - `test_baseline_router_swap_strategies` — Verification across `SourceToTarget`, `TargetToSource`, and `Bidirectional` strategies.
    - `test_baseline_router_preserves_parameters_and_global_phase` — Strict preservation of symbolic parameters and circuit global phase.
    - `test_baseline_router_swap_limit_exceeded` — Bounded SWAP limits aborting cleanly with `RoutingError::IterationLimitExceeded`.
    - `test_baseline_router_insufficient_capacity` — Target capacity shortage rejected with `RoutingError::InsufficientCapacity`.
    - `test_baseline_router_decomposed_swap_target` — Routing on target supporting only native CX (no native SWAP), emitting 3 CX decomposition.
- **Regression Check**: All 1,139 tests across the entire repository pass with zero regressions.

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (zero `#![feature(...)]` flags).
- [x] Distinct typed `QubitId` and `PhysicalQubitId` identities preserved.
- [x] Little-endian qubit indexing ($q_0$ is LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$).
- [x] Circuit global phase strictly preserved ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] ADR-0001 strict ancilla zero-state policy preserved across all inserted physical SWAPs.
- [x] Kahn's DAG traversal ordering strictly respects dependencies and deterministic tie-breaking.
- [x] Target legality verified using `TargetLegalityValidator`.
- [x] Layout-aware semantic equivalence verified using `LayoutEquivalenceChecker`.
- [x] Scope boundary maintained: Baseline shortest-path router only; no SABRE front-layer or lookahead heuristics implemented.

## 6. Known Limitations & Technical Debt
- Phase 47 implements the deterministic baseline shortest-path router. The advanced SABRE algorithm incorporating front-layer analysis, lookahead decay heuristics, and bidirectional layout refinement is scheduled for Phases 50–52.
- Arbitrary multi-wire DAG rewrite infrastructure is scheduled for Phase 48.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
