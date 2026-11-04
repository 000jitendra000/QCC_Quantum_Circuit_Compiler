# Phase Completion Report: Phase 48 — SWAP Insertion and Dependency Maintenance

## 1. Summary
- **Phase Number**: Phase 48
- **Exact Phase Title**: SWAP insertion and dependency maintenance
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Completion Date**: 2026-11-04
- **Engineer / Agent Session**: Antigravity Assistant

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/routing/provenance.rs` — Provenance tracking for routed circuits. Defines `RouteOperationKind` (`Logical`, `RoutingSwap`, `DirectionCorrection`), `RouteProvenanceRecord`, and `RouteProvenanceTracker` supporting bidirectional forward/reverse provenance mapping, step indexing, and complete coverage validation.
  - `src/routing/route_state.rs` — Incremental routing rewrite state manager (`RouteState`). Encapsulates physical `Circuit`, `CircuitLayout`, `Target`, and `RouteProvenanceTracker`. Provides safe, atomic atomic methods for inserting physical SWAPs, multi-hop routing paths, direction correction, and logical operations with post-rewrite QccIR validation and layout-aware equivalence verification.
  - `tests/routing/swap_rewrite.rs` — Comprehensive integration tests covering atomic single SWAP rewrites, repeated SWAPs on identical wires restoring initial layout, multi-wire dependency chains, dependency ordering and isolation across both affected physical wires, provenance preservation, routed Bell circuits with ADR-0001 ancillae, incremental QccIR validation, inconsistent layout regression rejection, and disconnected/duplicate operand rejection.
  - `docs/project-state/plans/phase-048.md` — Phase 48 implementation plan.
  - `docs/project-state/reports/phase-048.md` — This completion report.
- **Modified Files**:
  - `src/ir/graph.rs` — Added `add_operation_with_provenance(operation, provenance)` to `CircuitGraph`.
  - `src/ir/circuit.rs` — Added `add_operation_with_provenance(operation, provenance)` to `Circuit`.
  - `src/routing/swap_insertion.rs` — Added `SwapInsertionResult`, `emit_physical_swap_with_provenance`, `route_operands_along_path_with_provenance`, `ensure_legal_orientation_with_provenance`, with backward-compatible wrappers and clippy annotations.
  - `src/routing/pathfinding.rs` — Added `From<IrValidationError> for RoutingError` conversion.
  - `src/routing/greedy.rs` — Integrated `RouteProvenanceTracker` into `BaselineRouter` and `BaselineRouterReport`.
  - `src/routing/mod.rs` — Re-exported `RouteState`, `RouteProvenanceTracker`, `RouteOperationKind`, `RouteProvenanceRecord`, and `SwapInsertionResult`.
  - `src/lib.rs` — Re-exported top-level routing types.
  - `tests/routing.rs` — Registered `swap_rewrite` integration test module.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**:
  > **EXIT GATE**: Inserted SWAP nodes update layout and dependencies correctly on both wires.
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
  - `cargo test --test routing`: 54 tests passed (10 new integration tests in `swap_rewrite.rs`, 44 existing routing tests, 0 failed).
  - `cargo test`: All tests passed across all test binaries (0 failed, 0 ignored).
  - `cargo doc --no-deps`: Documentation generated cleanly with zero warnings.
  - `cargo run --example bell` and `cargo run --example qasm`: Executed cleanly.
- **Exit Gate Pass Confirmation**: **Pass** (100% verified with automated tests).

## 4. Test Suite Summary
- **Integration Tests Added** (`tests/routing/swap_rewrite.rs`):
  - `test_single_inserted_swap_updates_graph_and_layout` — Verifies that inserting a single physical SWAP updates both physical wires in QccIR, updates `CircuitLayout`, and records routing provenance.
  - `test_repeated_swaps_on_same_wires_restore_layout` — Verifies two successive SWAPs on identical wires restore the layout permutation to identity and satisfy layout-aware semantic equivalence.
  - `test_multi_wire_repeated_swaps_dependency_chain` — Tests a chain of SWAPs across 4 physical wires, verifying cumulative layout permutation and sequential dependency ordering.
  - `test_swap_dependency_order_on_both_affected_wires` — Verifies that an inserted SWAP node depends on the prior operations on *both* affected physical wires, while unaffected physical wires remain completely independent.
  - `test_original_logical_operation_identity_and_provenance_preserved` — Verifies that routed physical operations retain original logical `OperationId` provenance, and inserted SWAPs reference their causal logical operation.
  - `test_routed_bell_circuit_with_swap_passes_validation_and_equivalence` — Verifies a full Bell circuit routed on a 3-qubit line with an idle ancilla satisfies QccIR validation, target legality, and ADR-0001 zero-state physical layout equivalence.
  - `test_qccir_validation_after_each_swap_rewrite` — Validates QccIR graph invariants (acyclicity, wire hazard ordering, single active writer) after each incremental rewrite.
  - `test_regression_inconsistent_layout_update_fails_equivalence` — Regression test confirming that updating the graph without updating the layout (or vice versa) fails equivalence checking.
  - `test_regression_disconnected_qubit_swap_rejected` — Confirms attempt to emit a SWAP between non-adjacent physical qubits is rejected fail-closed with `RoutingError::PhysicalQubitsNotAdjacent`.
  - `test_regression_duplicate_operand_swap_rejected` — Confirms attempt to emit a SWAP where both operands are identical is rejected fail-closed with `RoutingError::DuplicateOperand`.
- **Regression Check**: All unit, property, and integration tests across the repository pass with zero regressions.

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (zero `#![feature(...)]` flags).
- [x] Distinct typed `QubitId` and `PhysicalQubitId` identities preserved.
- [x] Little-endian qubit indexing ($q_0$ is LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$).
- [x] Circuit global phase strictly preserved ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] ADR-0001 strict ancilla zero-state policy preserved across all inserted physical SWAPs.
- [x] Two-wire dependency synchronization: Inserted SWAPs become the latest writer on both affected physical wires.
- [x] Stable `OperationId` provenance preserved for all physical operations.
- [x] Target legality verified using `TargetLegalityValidator`.
- [x] Layout-aware semantic equivalence verified using `LayoutEquivalenceChecker`.
- [x] Scope boundary maintained: SWAP insertion and dependency maintenance only; no routing metrics or SABRE passes.

## 6. Known Limitations & Technical Debt
- Phase 48 establishes transactional and incremental SWAP insertion and dependency maintenance. Routing metrics and depth/crosstalk evaluation are scheduled for Phase 49, followed by the SABRE router in Phases 50–52.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
