# Phase 34 Implementation Plan — DAG-Aware Commutation Rewriting

## 1. Executive Overview

Phase 34 implements **DAG-aware commutation rewriting** (`CommutationReorderPass`) for QCC.
Building upon the verified commutation rules established in Phase 33 and the transactional DAG rewriting infrastructure established in Phase 13, this phase enables safe operation reordering within the QccIR dependency graph to expose optimization opportunities (such as adjacent inverse cancellation in Phase 29, same-axis rotation merging in Phase 30, and single-qubit gate fusion in Phase 32).

---

## 2. Architecture & Invariants

1. **Transactional DAG Swapping (`src/ir/rewrite/swap.rs`)**:
   - Provide `swap_adjacent_operations` and sequence permutation `reorder_operations` within `CircuitTransaction`.
   - Maintain QccIR dependency invariants, update wire states, detect cycles, and support atomic rollback on any failure.
   - Stable `OperationId`s and operation provenance are strictly preserved.

2. **Commutation Approval Authority (`CommutationRegistry`)**:
   - Every candidate reordering must query the Phase 33 `CommutationRegistry`.
   - Operations that return `DoesNotCommute` or `Unknown` are never reordered.

3. **Strict Side-Effect & Classical Hazard Boundaries**:
   - Operations never move across `Measure`, `Reset`, `Barrier`, or classical operations.
   - Symmetric classical dependency checks (RAW, WAR, WAW) ensure no classical ordering hazards are introduced.

4. **Bounded Windows & Anti-Oscillation**:
   - Bounded search window (`window_size`, default 8).
   - Iteration budget (`max_iterations`, default 20) and rewrite budget (`max_rewrites`, default 50) guarantee deterministic termination.
   - Directed pair tracking (`swapped_pairs: HashSet<(OperationId, OperationId)>`) prevents swapping $(B, A)$ if $(A, B)$ was already swapped in the same pass invocation.

5. **Opportunity-Driven Strategies**:
   - `ReorderStrategy::ExposeOpportunities`: Swaps commuting operations only when doing so exposes downstream/upstream optimization opportunities (inverse cancellation, same-axis rotation merging, single-qubit gate fusion, or identical self-inverses).
   - `ReorderStrategy::SingleQubitFirst`: Bubble-sorts commuting single-qubit gates before multi-qubit gates.
   - `ReorderStrategy::TwoQubitFirst`: Moves multi-qubit gates earlier when commuting.
   - `ReorderStrategy::AllCommuting`: Reorders any verified commuting adjacent pair.

6. **Equivalence & Semantics**:
   - Preserves explicit circuit global phase.
   - Cross-verified with Phase 10 unitary equivalence checker.

---

## 3. Ordered Implementation Steps

1. **Transactional Swap Utilities (`src/ir/rewrite/swap.rs`)**:
   - Implement `swap_adjacent_operations` and `stage_swap_adjacent_operations`.
   - Implement `reorder_operations` and `stage_reorder_operations`.
   - Expose methods on `CircuitTransaction`.

2. **Commutation Reorder Pass (`src/optimizer/passes/commutation_reorder.rs`)**:
   - Define `CommutationReorderConfig`, `CommutationReorderStats`, `ReorderStrategy`, and `CommutationReorderPass`.
   - Implement candidate pair discovery and opportunity matching.
   - Implement transaction execution with anti-oscillation tracking.

3. **Module Registration & Re-exports**:
   - Register `swap` in `src/ir/rewrite/mod.rs`.
   - Register `commutation_reorder` in `src/optimizer/passes/mod.rs`.
   - Re-export public types in `src/lib.rs`.

4. **Integration & Property Testing (`tests/optimizer/commutation_reorder.rs`)**:
   - Test opportunity exposing (Phase 29 inverse pairs, Phase 30 same-axis rotations).
   - Test non-commuting rejection, side-effect boundaries, anti-oscillation, budgets, and determinism.
   - Test proptest equivalence.

5. **Verification & Ledger Updates**:
   - Run `cargo fmt`, `cargo clippy`, `cargo test --all`, `cargo build`, `cargo doc`, examples, and OpenQASM 3 validation.
   - Write completion report `docs/project-state/reports/phase-034.md`.
   - Update `docs/project-state/progress.md`.
