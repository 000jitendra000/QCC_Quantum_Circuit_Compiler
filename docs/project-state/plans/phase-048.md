# Phase Plan: Phase 48 — SWAP Insertion and Dependency Maintenance

## 1. Metadata
- **Phase Number**: Phase 48
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: L
- **Prerequisites Satisfied**: Phase 13 (Transactional DAG rewrites), Phase 44 (Logical/physical layout model), Phase 45 (Layout-aware equivalence), Phase 46 (Initial placement), Phase 47 (Baseline shortest-path router).
- **Source Document Page**: Page 26 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Make physical SWAP insertion a safe, consistent QccIR DAG rewrite. Every inserted SWAP must update the graph and physical layout together, preserving dependency correctness on both affected physical wires and the identity/provenance of original logical operations.
- **Explicit Inclusions**:
  1. Add `add_operation_with_provenance` to `CircuitGraph` and `Circuit` to preserve provenance at the QccIR node level.
  2. Implement `RouteProvenanceTracker` and `RouteOperationKind` in `src/routing/provenance.rs` for tracking physical node provenance and causal links to original logical operations.
  3. Implement `RouteState` in `src/routing/route_state.rs` encapsulating the physical `Circuit`, `CircuitLayout`, `Target`, and `RouteProvenanceTracker`, providing atomic SWAP insertion with synchronous layout mutation, wire dependency updates, and post-rewrite graph validation.
  4. Extend `src/routing/swap_insertion.rs` to support provenance tracking, multi-wire dependency maintenance, and structured `SwapInsertionResult`.
  5. Update `BaselineRouter` in `src/routing/greedy.rs` to populate provenance and integrate with `RouteState`.
  6. Integration tests in `tests/routing/swap_rewrite.rs` testing single SWAPs, repeated SWAPs, dependency ordering on both wires, identity/provenance preservation, routed Bell circuit, QccIR graph validation, and layout-aware equivalence.
- **Explicit Exclusions**:
  1. No routing objective metrics or multi-metric evaluation engine (Phase 49).
  2. No SABRE algorithms, front-layer lookahead, or decay heuristics (Phases 50–52).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero `#![feature(...)]`).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Explicit circuit global phase tracked and preserved.
- [x] Wire dependencies and data hazards (RAW, WAR, WAW, QuantumOrder) strictly modeled.
- [x] Distinct typed `QubitId` and `PhysicalQubitId` preserved.
- [x] ADR-0001 ancilla zero-state policy preserved across all inserted physical SWAPs.
- [x] QccIR validation passes after every rewrite step (`validate_circuit`).

## 4. Step-by-Step Implementation Sequence
1. **QccIR Provenance Node Support**:
   - Add `add_operation_with_provenance` to `CircuitGraph` (`src/ir/graph.rs`) and `Circuit` (`src/ir/circuit.rs`).
2. **Provenance Tracking Module (`src/routing/provenance.rs`)**:
   - `RouteOperationKind` classifying logical operations, routing SWAPs, and direction corrections.
   - `RouteProvenanceTracker` mapping physical `OperationId`s to their causal logical `OperationId`s.
3. **Route State (`src/routing/route_state.rs`)**:
   - `RouteState` struct managing `Circuit`, `CircuitLayout`, `Target`, and `RouteProvenanceTracker`.
   - `insert_swap`, `apply_logical_operation`, `validate`, and state query methods.
4. **SWAP Insertion Enhancements (`src/routing/swap_insertion.rs`)**:
   - `SwapInsertionResult` containing inserted node IDs, swapped physical qubits, and layout snapshot.
   - `emit_physical_swap_with_provenance` helper.
5. **Baseline Router Integration (`src/routing/greedy.rs`)**:
   - Populate provenance during routing; expose `provenance` in `BaselineRouterReport`.
6. **Public API & Re-exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Expose `RouteState`, `RouteProvenanceTracker`, `RouteOperationKind`, `SwapInsertionResult`.
7. **Comprehensive Integration Tests (`tests/routing/swap_rewrite.rs`, `tests/routing.rs`)**:
   - Single SWAP, repeated SWAPs, dependency order, provenance preservation, routed Bell circuit, ADR-0001 ancillae.

## 5. Expected Files and Modules
- New files:
  - `src/routing/provenance.rs`
  - `src/routing/route_state.rs`
  - `tests/routing/swap_rewrite.rs`
  - `docs/project-state/plans/phase-048.md`
  - `docs/project-state/reports/phase-048.md`
- Modified files:
  - `src/ir/graph.rs`
  - `src/ir/circuit.rs`
  - `src/routing/swap_insertion.rs`
  - `src/routing/greedy.rs`
  - `src/routing/mod.rs`
  - `src/lib.rs`
  - `tests/routing.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit and integration tests in `tests/routing/swap_rewrite.rs`:
  - `test_single_inserted_swap_updates_graph_and_layout`
  - `test_repeated_swaps_on_same_wires_restore_layout`
  - `test_swap_dependency_order_on_both_affected_wires`
  - `test_disjoint_wire_operations_do_not_depend_on_swap`
  - `test_original_logical_operation_identity_and_provenance_preserved`
  - `test_routed_bell_circuit_with_swap_passes_validation_and_equivalence`
  - `test_qccir_validation_after_each_swap_rewrite`
  - `test_adr0001_ancilla_zero_state_across_swap_rewrites`
  - `test_inconsistent_layout_update_fails_equivalence`
  - `test_disconnected_qubit_swap_rejected`
- Full regression test across all 35 test binaries.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Each route rewrite passes QccIR validation and layout-aware equivalence.
- Verification command: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
