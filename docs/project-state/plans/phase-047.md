# Phase Plan: Phase 47 — Baseline Shortest-Path Router

## 1. Metadata
- **Phase Number**: Phase 47
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: XL
- **Prerequisites Satisfied**: Phase 39 (Decomposition templates), Phase 43 (Final target legality validator), Phase 44 (Logical/physical layout model), Phase 45 (Layout-aware equivalence), Phase 46 (Initial placement algorithms).
- **Source Document Page**: Page 25 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Build a simple, correct, deterministic pathfinding router (Dijkstra/BFS on target coupling) before implementing SABRE. The router transforms routable logical two-qubit interactions into topology-valid physical interactions while maintaining a correct logical-to-physical layout across all SWAP mutations.
- **Explicit Inclusions**:
  1. DAG dependency scheduling identifying executable two-qubit and single-qubit nodes in QccIR.
  2. Physical coupling pathfinding (`find_shortest_path` on target `CouplingGraph` via deterministic BFS/Dijkstra).
  3. Bounded path-based SWAP strategy (`SourceToTarget`, `TargetToSource`, `Bidirectional`) moving operands until adjacent.
  4. Operand orientation check and resolution (respecting directed coupling and target capabilities).
  5. Layout mutation tracking (`cl.apply_physical_swap`) preserving ADR-0001 ancilla zero-state invariants.
  6. Native SWAP emission or target-decomposed CX sequence with direction correction.
  7. Strict iteration limits and explicit errors for disconnected/impossible topologies or capacity shortages.
  8. Layout-aware equivalence validation with `LayoutEquivalenceChecker` and target legality validation with `TargetLegalityValidator`.
- **Explicit Exclusions**:
  1. No SABRE algorithms or heuristics (Phases 50–52): no front-layer lookahead, no decay penalties, no bidirectional refinement. The baseline router must never be labeled as SABRE.
  2. No general transactional arbitrary DAG rewrite infrastructure for arbitrary SWAP insertion (Phase 48).
  3. No routing objective metrics or multi-metric evaluation engine (Phase 49).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero `#![feature(...)]`).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Explicit circuit global phase tracked and preserved ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] Distinct typed `QubitId` and `PhysicalQubitId` identities preserved.
- [x] Target legality fail-closed (`TargetLegalityValidator`).
- [x] ADR-0001 ancilla zero-state policy preserved across all inserted physical SWAPs.
- [x] Deterministic execution: deterministic BFS tie-breaking and node scheduling.

## 4. Step-by-Step Implementation Sequence
1. **Pathfinding (`src/routing/pathfinding.rs`)**:
   - Deterministic BFS on target `CouplingGraph`.
   - Returns shortest path `Vec<PhysicalQubitId>`.
   - Returns `Err(RoutingError::DisconnectedTopology)` if physical qubits are in disconnected components.
2. **SWAP Insertion & Strategy (`src/routing/swap_insertion.rs`)**:
   - `PathSwapStrategy` enum (`SourceToTarget`, `TargetToSource`, `Bidirectional`).
   - `emit_physical_swap` helper emitting native SWAP or decomposed CX sequence and updating `CircuitLayout`.
   - `route_operands_along_path` helper moving operands along shortest path.
3. **Baseline Router (`src/routing/greedy.rs`)**:
   - `BaselineRouterConfig`, `BaselineRouterReport`, `BaselineRouter`, and `RoutingError`.
   - Dependency-respecting DAG traversal scheduling executable nodes.
   - Initial layout initialization via placement algorithms or caller-supplied map.
   - Enforce iteration limits.
4. **Integration & Re-exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Re-export `BaselineRouter`, `BaselineRouterConfig`, `BaselineRouterReport`, `RoutingError`, `PathSwapStrategy`, `find_shortest_path`.
5. **Integration Tests (`tests/routing/greedy.rs`, `tests/routing.rs`)**:
   - Line, ring, grid, star, disconnected, directed topologies.
   - ADR-0001 idle physical ancillae.
   - Semantic verification using `LayoutEquivalenceChecker`.

## 5. Expected Files and Modules
- New files:
  - `src/routing/pathfinding.rs`
  - `src/routing/swap_insertion.rs`
  - `src/routing/greedy.rs`
  - `tests/routing/greedy.rs`
  - `docs/project-state/plans/phase-047.md`
  - `docs/project-state/reports/phase-047.md`
- Modified files:
  - `src/routing/mod.rs`
  - `src/lib.rs`
  - `tests/routing.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit & integration tests in `tests/routing/greedy.rs`:
  - `test_baseline_router_line_topology_already_legal`
  - `test_baseline_router_line_topology_single_hop_swap`
  - `test_baseline_router_line_topology_multi_hop_swaps`
  - `test_baseline_router_ring_topology_wraparound_shortest_path`
  - `test_baseline_router_grid_topology_2d_hops`
  - `test_baseline_router_star_topology_hub_routing`
  - `test_baseline_router_disconnected_topology_fails_explicitly`
  - `test_baseline_router_directed_coupling_orientation`
  - `test_baseline_router_adr0001_ancilla_zero_state_preserved`
  - `test_baseline_router_preserves_circuit_global_phase_and_parameters`
  - `test_baseline_router_iteration_limit_exceeded`
- Full regression test across all 35 test binaries.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Baseline router passes topology and layout-aware semantic tests.
- Commands: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Expected outcome: All tests pass with 0 errors and 0 warnings.
