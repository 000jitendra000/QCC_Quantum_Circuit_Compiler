# Phase Plan: Phase 50 — SABRE Front-Layer Analysis

## 1. Metadata
- **Phase Number**: Phase 50
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: L
- **Prerequisites Satisfied**: Phase 44 (Logical/physical layout model), Phase 45 (Layout-aware equivalence), Phase 46 (Initial placement), Phase 47 (Baseline shortest-path router), Phase 48 (SWAP insertion and dependency maintenance), Phase 49 (Routing objectives and metrics).
- **Source Document Page**: Page 26 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Build the dependency frontier used by SABRE, with correct front-layer and extended-lookahead analysis over QccIR dependency DAGs.
- **Explicit Inclusions**:
  1. Implement `SabreFrontier` in `src/routing/sabre/frontier.rs` tracking completed operations, remaining predecessor in-degrees, and currently executable operations based strictly on QccIR DAG dependencies.
  2. Implement `FrontLayer` and `FrontLayerOp` in `src/routing/sabre/front_layer.rs` extracting currently executable two-qubit operations and resolving logical qubits to physical qubits through active layouts.
  3. Implement `LookaheadSet`, `LookaheadOp`, and `LookaheadConfig` in `src/routing/sabre/lookahead.rs` constructing upcoming two-qubit operations within a bounded lookahead window $W$ from DAG successors of the front layer.
  4. Implement `SabreFrontierState` managing the frontier, active layout, front layer, and extended lookahead, with layout versioning and immediate re-mapping upon candidate layout updates to guarantee no stale physical mappings survive.
  5. Add hand-calculated fixtures and integration tests in `tests/routing/sabre_frontier.rs` covering linear dependencies, independent gates, multi-qubit DAG patterns, topologically later non-ready gates, non-identity layout mappings, and candidate mapping updates.
- **Explicit Exclusions**:
  1. No SABRE candidate scoring, decay penalties, or heuristic cost evaluation (Phase 51).
  2. No SWAP selection, routing iterations, or bidirectional layout refinement (Phases 51–52).
  3. No router comparative benchmarking (Phase 53).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero `#![feature(...)]` flags).
- [x] Readiness is strictly determined from QccIR predecessor dependencies, never merely topological sequence.
- [x] The front layer contains strictly executable two-qubit operations whose DAG dependencies are 100% satisfied.
- [x] Deterministic ordering and tie-breaking by stable `OperationId`.
- [x] Distinct typed `QubitId` and `PhysicalQubitId` preserved.
- [x] Physical operands resolved dynamically through current `LayoutMap`.
- [x] Layout mutations and candidate mappings immediately update or invalidate physical mappings, preventing stale states.
- [x] ADR-0001 strict ancilla zero-state policy preserved.

## 4. Step-by-Step Implementation Sequence
1. **Frontier Tracking Module (`src/routing/sabre/frontier.rs`)**:
   - `SabreFrontier` tracking completed operations, remaining predecessor dependencies (`in_degrees`), and ready sets.
   - Methods: `new(circuit)`, `is_completed`, `is_executable`, `executable_operations`, `advance(op_id)`, `advance_many`.
2. **Front-Layer Module (`src/routing/sabre/front_layer.rs`)**:
   - `FrontLayerOp` capturing `id`, logical operands `(q1, q2)`, physical operands `(p1, p2)`, and operation name.
   - `FrontLayer` struct maintaining sorted front-layer operations with layout re-mapping (`update_layout`).
3. **Lookahead Module (`src/routing/sabre/lookahead.rs`)**:
   - `LookaheadOp` with `id`, operands, and lookahead layer depth.
   - `LookaheadConfig` controlling budget/window $W$ (`max_size`) and optional `max_depth`.
   - `LookaheadSet` traversing future DAG layers from simulated execution of the front layer.
4. **Frontier State Manager (`src/routing/sabre/mod.rs`)**:
   - `SabreFrontierState` combining frontier, front layer, lookahead set, and layout with layout versioning and atomic update methods (`update_layout`, `apply_swap`, `retire_operation`).
5. **Public Re-exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Re-export SABRE frontier types under `routing::sabre`.
6. **Hand-Calculated & Integration Tests (`tests/routing/sabre_frontier.rs`, `tests/routing.rs`)**:
   - Test linear dependency chains, independent parallel gates, mixed 1Q/2Q DAGs, non-ready topologically later gates, non-identity layouts, and layout mutation updates.
7. **Verification & Ledger Updates**:
   - Execute formatting, Clippy, full test suite, documentation, and update ledger and completion report.

## 5. Expected Files and Modules
- New files:
  - `src/routing/sabre/mod.rs`
  - `src/routing/sabre/frontier.rs`
  - `src/routing/sabre/front_layer.rs`
  - `src/routing/sabre/lookahead.rs`
  - `tests/routing/sabre_frontier.rs`
  - `docs/project-state/plans/phase-050.md`
  - `docs/project-state/reports/phase-050.md`
- Modified files:
  - `src/routing/mod.rs`
  - `src/lib.rs`
  - `tests/routing.rs`
  - `docs/project-state/progress.md`

## 6. Required Tests & Test Matrix
- Hand-calculated fixtures:
  - `test_linear_dependencies_front_layer_and_lookahead`: Chain of 3 dependent CXs on identical qubits.
  - `test_independent_gates_all_enter_front_layer`: Multiple independent 2Q gates on disjoint qubits.
  - `test_topologically_later_operations_not_in_front_layer`: Later operations blocked by dependencies remain out of front layer.
  - `test_multi_qubit_dag_with_interleaved_single_qubit_gates`: Single-qubit gates advance dependencies to uncover upcoming 2Q gates.
  - `test_non_identity_layout_mapping`: Logical operands correctly map to physical locations under nontrivial permutation.
  - `test_layout_mutation_updates_front_layer_and_lookahead_no_stale_mapping`: Applying physical SWAP updates operand coordinates immediately.
  - `test_lookahead_budget_truncation`: Window $W$ limits lookahead set size deterministically.
  - `test_retire_operation_advances_frontier_and_lookahead`: Retiring front-layer operations dynamically promotes lookahead operations to front layer.

## 7. Definition of Done
- Front-layer and extended-lookahead sets strictly match hand-calculated DAG fixtures.
- Stale physical operand mappings cannot survive layout mutations.
- All tests pass with zero warnings and zero regressions.

## 8. Failure Modes & Debugging Considerations
- Treating all topological nodes as ready without checking dependency in-degrees.
- Allowing single-qubit gates to masquerade as front-layer operations.
- Keeping stale physical operands after a candidate SWAP.

## 9. Explicit Exit Gate
> **EXIT GATE**: Front-layer and extended-set analysis is independently tested.
