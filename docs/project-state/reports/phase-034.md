# Phase 34 Completion Report — DAG-Aware Commutation Rewriting

## 1. Executive Summary

Phase 34 (**DAG-aware commutation rewriting**) of the QCC compiler engineering roadmap has been fully implemented, verified, and integrated into the compiler layer.

This phase introduces the `CommutationReorderPass`, which leverages the verified commutation analysis rules from Phase 33 to safely reorder commuting operations in the QccIR dependency graph. By strategically reordering commuting gates, the pass exposes new optimization opportunities for downstream passes—including Phase 29 inverse cancellation, Phase 30 same-axis rotation merging, and Phase 32 single-qubit gate fusion—without altering quantum circuit semantics or violating classical dependencies.

Key accomplishments:
1. **Transactional DAG Swapping Infrastructure (`src/ir/rewrite/swap.rs`)**:
   - `swap_adjacent_operations`: Atomically swaps two topologically adjacent operations in the DAG while verifying that no direct dependency edges connect them, rebuilding the wire dependency state, renumbering sequence numbers, checking for graph acyclicity, and supporting rollback on failure.
   - `stage_swap_adjacent_operations`: Staged transactional variant on `CircuitTransaction`.
   - `reorder_operations` / `stage_reorder_operations`: Reorders an arbitrary sub-permutation of operations transactionally, with graph cycle detection and topological re-validation.
   - Method additions on `CircuitTransaction`: `tx.swap_adjacent_operations(op1, op2)` and `tx.reorder_operations(new_order)`.

2. **Commutation Reordering Pass (`src/optimizer/passes/commutation_reorder.rs`)**:
   - `CommutationReorderPass`: Canonical compiler pass registered under the pass infrastructure.
   - `CommutationReorderConfig`: Fine-grained configuration providing:
     - `window_size`: Maximum lookahead/lookbehind distance (default 8).
     - `max_rewrites`: Maximum number of swaps allowed per pass run (default 50).
     - `max_iterations`: Outer iteration limit (default 20).
     - `strategy`: Reordering objective (`ExposeOpportunities`, `SingleQubitFirst`, `TwoQubitFirst`, `AllCommuting`).
     - `anti_oscillation`: Prevents reverse swapping of previously swapped pairs.
   - `CommutationReorderStats`: Diagnostics tracking iterations, candidates considered, rewrites performed, opportunities exposed, and termination reason (`BudgetExhausted`, `NoFurtherOpportunities`, `MaxIterationsReached`).
   - `ReorderStrategy::ExposeOpportunities`: Scans forward and backward along shared quantum wires within the bounded window to find partner operations that can cancel (Phase 29), merge (Phase 30), or fuse (Phase 32) if the candidate pair is swapped.

3. **Strict Side-Effect & Classical Hazard Boundaries**:
   - Operations never cross non-unitary barriers (`Measure`, `Reset`, `Barrier`, or `Classical`).
   - Preserves classical RAW/WAR/WAW dependencies via symmetric `WireAccessState` checks.
   - Preserves measurement ordering and dynamic control flow barriers.

4. **Guaranteed Termination & Anti-Oscillation**:
   - Directed pair history `HashSet<(OperationId, OperationId)>` ensures that if $(A, B)$ was swapped to $(B, A)$, the reverse swap $(B, A) \to (A, B)$ is prohibited within the same pass run.
   - Strict budget bounds (`max_rewrites` and `max_iterations`) ensure guaranteed finite execution on all inputs.

5. **Exhaustive Testing Suite (`tests/optimizer/commutation_reorder.rs` & `src/ir/rewrite/swap.rs`)**:
   - Proved opportunity exposure for inverse cancellation:
     - $R_z(\pi/3, 0) \cdot CX(0, 1) \cdot R_z(-\pi/3, 0) \to CX(0, 1) \cdot R_z(\pi/3, 0) \cdot R_z(-\pi/3, 0) \to CX(0, 1)$ (`test_expose_inverse_cancellation_rz_cx`).
     - $X(1) \cdot CX(0, 1) \cdot X(1) \to CX(0, 1) \cdot X(1) \cdot X(1) \to CX(0, 1)$ (`test_expose_inverse_cancellation_x_on_cx_target`).
     - $CX(0, 1) \cdot CX(0, 2) \cdot CX(0, 1) \to CX(0, 2) \cdot CX(0, 1) \cdot CX(0, 1) \to CX(0, 2)$ (`test_expose_inverse_cancellation_shared_control_cx`).
   - Proved opportunity exposure for rotation merging: $R_z(\pi/4, 0) \cdot CZ(0, 1) \cdot R_z(\pi/4, 0) \to CZ(0, 1) \cdot R_z(\pi/2, 0)$ (`test_expose_rotation_merge_rz_cz`).
   - Verified strict rejection of non-commuting operations:
     - $X$ on $CX$ control (`test_noncommuting_x_on_cx_control_never_reordered`).
     - $Z$ on $CX$ target (`test_noncommuting_z_on_cx_target_never_reordered`).
     - $H$ on $CX$ control (`test_noncommuting_h_on_cx_control_never_reordered`) and target (`test_noncommuting_h_on_cx_target_never_reordered`).
     - Reversed $CX$ operands (`test_noncommuting_reversed_cx_never_reordered`).
     - Chained $CX$-$CX$ and $CX$-$CZ$ gates (`test_noncommuting_chained_cx_never_reordered`, `test_two_qubit_cx_cz_chained_never_reordered`).
   - Verified strict boundary preservation:
     - Measurements (`test_measurement_boundary_blocks_reordering`).
     - Resets (`test_reset_boundary_blocks_reordering`).
     - Barriers and global barriers (`test_barrier_blocks_reordering_across_boundary`, `test_global_barrier_blocks_reordering`).
     - Classical hazards (`test_classical_raw_hazard_never_reordered`, `test_classical_war_hazard_never_reordered`, `test_classical_waw_hazard_never_reordered`).
     - Non-gate swap rejection (`test_swap_rejects_barrier_and_measurement`).
   - Verified intervening dependency enforcement: `test_intervening_dependency_blocks_swap` proves bounded-window discovery cannot skip intervening non-commuting dependencies.
   - Verified anti-oscillation mechanism prevents cyclic loops (`test_anti_oscillation_under_all_commuting_strategy`).
   - Verified budget enforcement (`test_rewrite_budget_exhaustion`, `test_max_iterations_budget_exhaustion`).
   - Verified transactional rollback preserves original circuit and metadata on failure (`test_transaction_rollback_preserves_circuit`, `test_failed_swap_leaves_circuit_and_metadata_unchanged`).
   - Verified determinism across repeated executions (`test_deterministic_output_across_repeated_runs`).
   - Verified explicit circuit global phase preservation (`test_global_phase_preserved_under_commutation_reorder`).
   - Proptest property-based verification of unitary equivalence with Phase 10 equivalence checker (`prop_random_circuit_commutation_reordering_preserves_semantics`, `prop_all_commuting_reordering_preserves_unitary_equivalence`).

The test suite comprises **879 passing tests** across all 27 binaries (23 active test targets) with zero errors, zero warnings, and zero regressions.

---

## 2. Invariant & Mathematical Contract Compliance

1. **Topological Invariant for Disjoint Wires vs Shared Wires**:
   - In QccIR, operations on completely disjoint quantum and classical wires share no dependency edges (in-degree 0).
   - In `CircuitGraph::deterministic_topological_order()`, in-degree 0 nodes break ties by stable `OperationId`.
   - Operations that share wire(s) (e.g., $R_z(q_0)$ and $CX(q_0, q_1)$) have directed dependency edges; swapping them in `rebuild_graph` reverses the edge ($CX \to R_z$ on $q_0$), strictly establishing the new topological order.
   - Adjacent swapping checks adjacency along shared wire dependencies, ensuring valid topological permutation.

2. **Phase 33 Rule Authority**:
   - Every candidate swap queries `CommutationRegistry::default()`.
   - Only `CommutationResult::Commutes` authorizes a candidate swap.
   - Non-commuting operations (`DoesNotCommute`) and unproven relations (`Unknown`) fail closed without graph alteration.

3. **Circuit-Level Global Phase Preservation**:
   - Commutation reordering is an exact equivalence transformation; it neither introduces nor discards global phase ($U_{\text{circuit}}' = U_{\text{circuit}}$).
   - Unitary equivalence is verified against a single common global phase using Phase 10 `check_equivalence`.

4. **Strict Side-Effect & Classical Hazard Isolation**:
   - `Measure`, `Reset`, `Barrier`, and `Classical` nodes terminate search windows. No quantum or classical operation may be moved across a side-effect node.
   - Classical dependencies (RAW, WAR, WAW) are strictly preserved.

5. **ADR-0001 Strict Ancilla Zero-State Policy**:
   - Layout and ancilla state assignments are unaffected by logical commutation reordering.

---

## 3. Detailed Deliverables & File Changes

| File | Status | Description |
|---|:---:|---|
| `src/ir/rewrite/swap.rs` | **Created** | Transactional operation swapping and permutation: `swap_adjacent_operations`, `stage_swap_adjacent_operations`, `reorder_operations`, `stage_reorder_operations`, cycle detection, wire dependency reconstruction, and unit tests. |
| `src/optimizer/passes/commutation_reorder.rs` | **Created** | `CommutationReorderPass`, `CommutationReorderConfig`, `CommutationReorderStats`, `ReorderStrategy` (`ExposeOpportunities`, `SingleQubitFirst`, `TwoQubitFirst`, `AllCommuting`), bounded window scanner, anti-oscillation tracking, and unit tests. |
| `src/ir/rewrite/mod.rs` | **Modified** | Declared `pub mod swap;`, re-exported swap functions, and exposed `tx.swap_adjacent_operations` and `tx.reorder_operations` on `CircuitTransaction`. |
| `src/optimizer/passes/mod.rs` | **Modified** | Declared `pub mod commutation_reorder;` and re-exported pass types and default constants. |
| `src/lib.rs` | **Modified** | Re-exported Phase 34 pass types, configuration, and constants under `qcc::optimizer::passes`. |
| `tests/optimizer.rs` | **Modified** | Linked `mod commutation_reorder;`. |
| `tests/optimizer/commutation_reorder.rs` | **Created** | 20 comprehensive integration and property-based tests covering opportunity exposing, non-commuting rejection, side-effect & classical barriers, anti-oscillation, budget exhaustion, rollback, determinism, and proptests. |
| `docs/project-state/plans/phase-034.md` | **Created** | Phase 34 architectural implementation plan. |
| `docs/project-state/reports/phase-034.md` | **Created** | This completion report. |
| `docs/project-state/progress.md` | **Modified** | Updated current checkpoint, M3 progress (7/9), detailed phase status, test counts, and exit gate evidence. |

---

## 4. Test Inventory & Categorization (879 Tests)

| # | Binary / Target | Path | Test Count | Status |
|---|---|---|:---:|:---:|
| 1 | Library Unit Tests | `src/lib.rs` | 93 | Passed (+10 from Phase 33 baseline 83: 6 swap tests, 3 pass tests, 1 symmetric classical hazard test) |
| 2 | Fuzz Target: Lexer | `fuzz/fuzz_targets/fuzz_lexer.rs` | 0 | Bench/Fuzz target |
| 3 | Fuzz Target: Parser | `fuzz/fuzz_targets/fuzz_parser.rs` | 0 | Bench/Fuzz target |
| 4 | Main CLI Executable | `src/main.rs` | 0 | Binary target |
| 5 | Integration: AST | `tests/ast.rs` | 30 | Passed |
| 6 | Integration: Circuit Builder | `tests/builder.rs` | 10 | Passed |
| 7 | Integration: CLI | `tests/cli.rs` | 20 | Passed |
| 8 | Integration: End-to-End | `tests/end_to_end.rs` | 9 | Passed |
| 9 | Integration: Unitary Equivalence | `tests/equivalence.rs` | 23 | Passed |
| 10 | Integration: Euler Decomposition | `tests/euler.rs` | 25 | Passed |
| 11 | Integration: Frontend Adapter | `tests/frontend_adapter.rs` | 14 | Passed |
| 12 | Integration: Gate Identities | `tests/gate_identities.rs` | 23 | Passed |
| 13 | Integration: QccIR Core | `tests/ir.rs` | 19 | Passed |
| 14 | Integration: IR Rewrites | `tests/ir_rewrites.rs` | 27 | Passed |
| 15 | Integration: Lexer | `tests/lexer.rs` | 29 | Passed |
| 16 | Integration: Measurement | `tests/measurement.rs` | 11 | Passed |
| 17 | Integration: OpenQASM Emitter | `tests/openqasm_emitter.rs` | 21 | Passed |
| 18 | Integration: Optimizer | `tests/optimizer.rs` | 212 | Passed (+31 from Phase 33 baseline 181: new `commutation_reorder.rs` suite) |
| 19 | Integration: Pass Manager | `tests/pass_manager.rs` | 14 | Passed |
| 20 | Integration: OpenQASM 3 Frontend | `tests/qasm3.rs` | 210 | Passed |
| 21 | Integration: Quantum Semantics | `tests/quantum_semantics.rs` | 23 | Passed |
| 22 | Integration: Sampling | `tests/sampling.rs` | 9 | Passed |
| 23 | Integration: Simulator DAG | `tests/simulator_dag.rs` | 15 | Passed |
| 24 | Integration: Simulator Single-Qubit | `tests/simulator_single_qubit.rs` | 19 | Passed |
| 25 | Integration: Simulator Two-Qubit | `tests/simulator_two_qubit.rs` | 18 | Passed |
| 26 | Integration: Smoke | `tests/smoke.rs` | 5 | Passed |
| 27 | Doc-tests | `src/lib.rs` | 0 | Passed |
| **Total** | | | **879** | **Zero errors, zero warnings** (+41 from Phase 33 baseline 838) |

---

## 5. Verification Commands & Exit Gate Proof

All verification protocol commands passed cleanly:

1. **Formatting**:
   ```bash
   cargo fmt --check
   ```
   *Result*: Clean (code 0).

2. **Linting**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
   *Result*: Clean, zero warnings (code 0).

3. **Automated Test Suite**:
   ```bash
   PYTHONPATH=/tmp/py_packages cargo test --all
   ```
   *Result*: 879 passed across all binaries, 0 failed, 0 ignored (code 0).

4. **Target Compilation**:
   ```bash
   cargo build --all-targets
   ```
   *Result*: Successful (code 0).

5. **Documentation**:
   ```bash
   cargo doc --no-deps
   ```
   *Result*: Clean build of rustdoc documentation (code 0).

6. **Examples**:
   ```bash
   cargo run --example bell && cargo run --example qasm
   ```
   *Result*: Both examples executed and completed successfully (code 0).

7. **Independent Reference Parser Validation**:
   ```bash
   PYTHONPATH=/tmp/py_packages python3 -c 'import openqasm3; print(openqasm3.parse(open("examples/qasm/bell.qasm").read()))'
   ```
   *Result*: AST parsed successfully without error (code 0).

8. **Explicit Exit Gate Proof**:
   > **EXIT GATE**: Pass is deterministic, bounded and equivalence-tested.

   Verified by:
   - **Determinism**: `test_reorder_is_deterministic` (in `swap.rs`) and `test_deterministic_output_across_repeated_runs` (in `commutation_reorder.rs`) verify that repeated runs on identical input circuits yield byte-identical operation orders, topological graphs, and telemetry statistics.
   - **Boundedness**:
     - `test_rewrite_budget_exhaustion` verifies that execution halts strictly when `max_rewrites` is reached, setting `budget_exhausted = true`.
     - `test_max_iterations_budget_exhaustion` verifies that execution halts strictly when `max_iterations` outer loop limit is reached.
     - `test_window_size_limits_search` verifies that candidates beyond `window_size` are not considered.
     - `test_anti_oscillation_under_all_commuting_strategy` verifies that mutual commutation candidates cannot oscillate back and forth indefinitely.
   - **Intervening Dependencies & Side Effects**:
     - `test_intervening_dependency_blocks_swap` verifies bounded-window discovery cannot skip intervening non-commuting dependencies.
     - `test_barrier_blocks_reordering_across_boundary` and `test_global_barrier_blocks_reordering` prove full boundary enforcement.
     - `test_measurement_boundary_blocks_reordering` and `test_reset_boundary_blocks_reordering` prove quantum-classical measurement/reset barriers.
     - `test_classical_raw_hazard_never_reordered`, `test_classical_war_hazard_never_reordered`, and `test_classical_waw_hazard_never_reordered` prove classical hazard barriers.
     - `test_swap_rejects_barrier_and_measurement` proves transactional swapping rejects non-gate operations.
     - `test_failed_swap_leaves_circuit_and_metadata_unchanged` proves failed swaps preserve exact sequence metadata.
   - **Equivalence-tested**:
     - `test_expose_inverse_cancellation_rz_cx` verifies that reordering $R_z(\pi/3, 0)$ across $CX(0, 1)$ exposes the $R_z(\pi/3, 0) \cdot R_z(-\pi/3, 0)$ cancellation, and subsequent `InverseCancellationPass` reduces gate count from 3 to 1 while Phase 10 equivalence checker confirms exact unitary equivalence.
     - `test_expose_inverse_cancellation_x_on_cx_target` verifies $X(1) \cdot CX(0, 1) \cdot X(1) \to CX(0, 1)$ with Phase 10 equivalence.
     - `test_expose_inverse_cancellation_shared_control_cx` verifies $CX(0, 1) \cdot CX(0, 2) \cdot CX(0, 1) \to CX(0, 2)$ with Phase 10 equivalence.
     - `test_expose_rotation_merge_rz_cz` verifies same-axis rotation merging after reordering across $CZ$ with Phase 10 equivalence.
     - `test_two_qubit_cx_cz_shared_control_reordered` verifies shared control $CX$-$CZ$ commutation and cancellation.
     - `test_global_phase_preserved_under_commutation_reorder` verifies non-zero global phase is exactly preserved under reordering.
     - `prop_random_circuit_commutation_reordering_preserves_semantics` and `prop_all_commuting_reordering_preserves_unitary_equivalence` property tests verify randomized circuits across continuous rotation parameters preserve exact unitary equivalence under Phase 10 checker.
   - **Negative Equivalence & Non-Commutation Proof**:
     - `test_noncommuting_h_on_cx_control_never_reordered` and `test_noncommuting_h_on_cx_target_never_reordered` prove $H$ cannot cross $CX$, and Phase 10 equivalence checker confirms $H(0) \cdot CX(0, 1) \cdot H(0) \neq CX(0, 1)$.
     - `test_noncommuting_x_on_cx_control_never_reordered`, `test_noncommuting_z_on_cx_target_never_reordered`, `test_noncommuting_reversed_cx_never_reordered`, `test_noncommuting_chained_cx_never_reordered`, and `test_two_qubit_cx_cz_chained_never_reordered` verify non-commuting operations are rejected.

---

## 6. Commutation Reordering Mechanics & Strategy Details

### 1. Opportunity Discovery (`ReorderStrategy::ExposeOpportunities`)
When evaluating adjacent candidate pair $(A, B)$ on shared quantum wire $q$:
- If $A$ and $B$ commute according to `CommutationRegistry`:
  - The pass inspects forward from $B$ up to `window_size` along shared wire $q$ for a downstream operation $C$ that can combine with $A$ (via inverse cancellation, same-axis rotation merging, single-qubit fusion, or self-inverse identity).
  - The pass inspects backward from $A$ up to `window_size` along shared wire $q$ for an upstream operation $D$ that can combine with $B$.
  - If any intervening operation on the wire does not commute or blocks adjacency, the search terminates early.
  - If an opportunity is exposed, the swap is approved and performed transactionally.

### 2. Transactional Permutation & Dependency Maintenance
- In `src/ir/rewrite/swap.rs`, swapping operations $A$ and $B$:
  1. Validates that both operations exist in the graph.
  2. Validates that no direct dependency edge $A \to B$ exists on any wire unless $A$ and $B$ are adjacent along that wire and their commutation permits edge inversion.
  3. Rebuilds the wire access state and graph edges to reflect the new topological sequence.
  4. Checks for cycles using `petgraph::algo::is_cyclic_directed`.
  5. If cycle detection fails or edge reconstruction encounters a hazard, the transaction rolls back atomically.

