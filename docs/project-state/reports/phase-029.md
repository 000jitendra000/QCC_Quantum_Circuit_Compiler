# Phase Completion Report: Phase 29 — Inverse-Gate Cancellation

## 1. Summary
- **Phase Number**: Phase 29
- **Exact Phase Title**: Inverse-gate cancellation
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`) — **Second Phase of Milestone M3**
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity (Advanced Agentic Coding)
- **Status**: **Completed**
- **Exit Gate Satisfied**: **Yes** — Pass passes unitary equivalence and DAG invariants (`74/74` tests in `tests/optimizer.rs`, `689/689` workspace tests across all test targets, zero warnings, zero errors).

---

## 2. Deliverables & Files Changed

### New Files Created
- `src/optimizer/passes/cancellation.rs`:
  - `InverseCancellationPass`: Transactional optimization pass implementing [`CompilerPass`](../../src/opt/pass.rs).
  - `InverseCancellationConfig`: Pass configuration specifying angular comparison tolerance (default `1e-12`), maximum fixed-point iterations (default `1000`), and runtime symbol environment.
  - `InverseCancellationStats`: Detailed execution telemetry (examined pairs, cancelled pairs, rejected pairs, single-qubit, two-qubit, rotation counts, iteration count).
  - `GateCategory`: Strongly typed classification for cancelled pairs (`SingleQubit`, `TwoQubit`, `Rotation`).
  - `find_adjacent_partner`: Precise dependency DAG wire-adjacency detector enforcing exact ordered operand matching across all shared wires without intervening operations.
  - `categorize_gate`: Helper classifying gate operations into disjoint telemetry categories.
- `tests/optimizer/cancellation.rs`: Comprehensive integration test suite (39 automated unit, property, and randomized integration tests).
- `docs/project-state/plans/phase-029.md`: Implementation plan for Phase 29.
- `docs/project-state/reports/phase-029.md`: Phase 29 completion report.

### Existing Files Modified
- `src/optimizer/algebra/inverses.rs`:
  - Enhanced `are_inverses_with_config` supporting configurable angular comparison tolerance and symbol evaluation environments.
  - Added `are_rotation_angles_inverse_with_config` supporting numeric evaluation and robust Euclidean modulo ($4\pi$) comparison.
  - Added `are_phase_angles_inverse_with_config` for phase gate evaluation modulo $2\pi$.
  - Preserved Phase 14 algebraic inverse semantics without modification to algebraic symmetry.
- `src/optimizer/algebra/mod.rs`:
  - Re-exported `are_inverses_with_config`.
- `src/optimizer/passes/mod.rs`:
  - Declared `cancellation` module and re-exported `InverseCancellationPass`, `InverseCancellationConfig`, `InverseCancellationStats`, `GateCategory`, `DEFAULT_CANCELLATION_TOLERANCE`, `DEFAULT_MAX_ITERATIONS`.
- `src/lib.rs`:
  - Re-exported `are_inverses_with_config` and all cancellation pass types and constants from top-level crate.
- `tests/optimizer.rs`:
  - Registered `cancellation` module runner.
- `docs/project-state/progress.md`:
  - Updated Phase 29 to Completed (`29 / 105` phases complete, Milestone M3 progress `2 / 9`).

---

## 3. Mathematical & Architectural Invariant Verification

### 3.1 Exact Ordered Operand Contract
- **Enforcement at Pass Boundary**:
  - The authoritative Phase 29 roadmap specifies: "Require exact ordered operand matching."
  - In `find_adjacent_partner` and at the cancellation-pass boundary, candidate pairs are recognized only when `partner.qubits() == node.qubits()`.
  - Permuted or reversed operands are rejected from candidate pair formation:
    - `CZ(0, 1)` and `CZ(1, 0)` do not match exact ordered operands and are not considered a candidate pair (`test_two_qubit_cz_reversed_operands_not_candidate_pair`).
    - `SWAP(0, 1)` and `SWAP(1, 0)` do not match exact ordered operands and are not considered a candidate pair (`test_two_qubit_swap_reversed_operands_not_candidate_pair`).
    - Reversed `CX(0, 1)` and `CX(1, 0)` remain non-canceling (`test_directional_cx_reversed_never_cancels`).
  - Exact ordered pairs cancel cleanly:
    - `CZ(0, 1)` and `CZ(0, 1)` cancel (`test_two_qubit_self_inverse_cz_cz`).
    - `SWAP(0, 1)` and `SWAP(0, 1)` cancel (`test_two_qubit_self_inverse_swap_swap`).
    - `CX(0, 1)` and `CX(0, 1)` cancel (`test_two_qubit_self_inverse_cx_cx`).
  - **Shared Inverse Registry Preserved**:
    - `src/optimizer/algebra/inverses.rs` remains untouched with respect to algebraic symmetry. In algebra, $CZ$ and $SWAP$ matrices are mathematically self-inverse across operand permutations, and Phase 14 tests in `tests/gate_identities.rs` remain completely intact. The stricter ordered operand requirement is enforced at the pass boundary.

### 3.2 Unitary Inverses, Periodicity & Global Phase
- **Self-Inverse Operations ($U^2 = I$)**:
  - Single-qubit: $H \cdot H = I$, $X \cdot X = I$, $Y \cdot Y = I$, $Z \cdot Z = I$.
  - Two-qubit: $CX \cdot CX = I$, $CZ \cdot CZ = I$, $SWAP \cdot SWAP = I$.
  - Verified by tests: `test_single_qubit_self_inverse_h_h`, `test_single_qubit_self_inverse_x_x`, `test_single_qubit_self_inverse_y_y`, `test_single_qubit_self_inverse_z_z`, `test_two_qubit_self_inverse_cx_cx`, `test_two_qubit_self_inverse_cz_cz`, `test_two_qubit_self_inverse_swap_swap`.
- **Discrete Adjoint Pairs ($U \cdot U^\dagger = I$)**:
  - $S \cdot S^\dagger = S^\dagger \cdot S = I$, $T \cdot T^\dagger = T^\dagger \cdot T = I$.
  - Verified by tests: `test_discrete_inverse_s_sdg`, `test_discrete_inverse_sdg_s`, `test_discrete_inverse_t_tdg`, `test_discrete_inverse_tdg_t`.
- **Continuous Rotations Modulo $4\pi$**:
  - $R_k(\theta) \cdot R_k(-\theta) = I$ along axis $k \in \{x, y, z\}$.
  - Angles cancel when $(\theta_1 + \theta_2) \equiv 0 \pmod{4\pi}$.
  - Verified by tests: `test_continuous_rotation_inverse_rx`, `test_continuous_rotation_inverse_ry`, `test_continuous_rotation_inverse_rz`, `test_continuous_rotation_four_pi_modulo`.
  - **Rejection of $2\pi$ Rotations**: $R_x(\pi) \cdot R_x(\pi) = R_x(2\pi) = -I \neq +I$. This is strictly rejected from cancellation to avoid introducing a global phase discrepancy (`test_continuous_rotation_two_pi_not_cancelled`).
- **Phase Gates Modulo $2\pi$**:
  - $P(\lambda_1) \cdot P(\lambda_2) = I$ when $(\lambda_1 + \lambda_2) \equiv 0 \pmod{2\pi}$ (`test_phase_gate_inverses`, `test_phase_gate_two_pi_periodicity`).
- **Symbolic Parameters**:
  - Exact symbolic negation $R_k(\theta) \cdot R_k(-\theta)$ cancels unconditionally (`test_symbolic_negation_inverses`).
  - Distinct unbound symbolic parameters ($R_k(\theta) \cdot R_k(\phi)$) are never assumed to cancel (`test_symbolic_distinct_never_cancelled`).
  - Bound symbols evaluated via `symbol_env` cancel when evaluated sum is $0 \pmod{4\pi}$ (`test_symbolic_bound_environment_inverses`).

### 3.3 Multi-Wire Dependency Safety & Commutation Boundaries
- **Candidate Detection via DAG Edges**:
  - Uses petgraph direct edges (`graph.successors(node.id)`) on typed `WireId::Quantum(q)` keys, not topological list adjacency.
- **Intervening Gates Block Cancellation**:
  - If an operation intervenes on wire 0 ($CX(0, 1) \to X(0) \to CX(0, 1)$), cancellation is strictly blocked (`test_intervening_gate_on_shared_wire_0_blocks_multi_qubit_cancellation`).
  - If an operation intervenes on wire 1 ($CX(0, 1) \to X(1) \to CX(0, 1)$), cancellation is strictly blocked (`test_intervening_gate_on_shared_wire_1_blocks_multi_qubit_cancellation`).
- **Commuting Disjoint Wires in DAG**:
  - Operations on disjoint wires commute in the DAG; an interleaved gate on an independent wire does not block wire-adjacent cancellation ($H(0) \to X(1) \to H(0) \implies X(1)$, verified by `test_unrelated_wire_interleaving`, `test_multi_qubit_and_independent_gate_cancellation`).
- **Preservation of Non-Removable Operations**:
  - `Barrier` strictly prevents cancellation on touched wires (`test_barrier_blocks_cancellation`), but does not block independent disjoint wires (`test_barrier_on_disjoint_wire_does_not_block`).
  - `Measure` and `Reset` operations strictly prevent cancellation (`test_measure_blocks_cancellation`, `test_reset_blocks_cancellation`).

### 3.4 Transactional Rewriting & Global Phase Preservation
- **Atomic Rewriting**: Removals are performed through [`CircuitTransaction::remove_operation`](../../src/ir/rewrite/remove.rs), verifying DAG acyclicity and wire reconnection.
- **Cascaded Reductions**: Fixed-point iterative cancellation loop cleanly resolves nested inverse layers ($X \to H \to H \to X \implies \emptyset$, verified by `test_cascaded_cancellation_outer_pairs`, `test_multi_wire_cascaded_cancellation`).
- **Odd Chains**: $H \to H \to H$ reduces deterministically to a single $H$ (`test_odd_chain_cancellation`).
- **Circuit Global Phase**: Since $U^\dagger \cdot U = I$ identically, cancellation has zero net phase change; `circuit.global_phase` is strictly preserved (`test_circuit_level_global_phase_preserved`).
- **Unitary Equivalence**: Validated by Phase 10 [`check_equivalence`](../../src/verification/unitary.rs) across all unitary cancellation test cases.
- **Randomized Property Verification**: 30 randomized Haar-like circuits with randomly injected inverse pairs confirmed that all injected pairs cancel and unitary equivalence holds (`test_randomized_circuit_inverse_cancellation_equivalence`).

---

## 4. Test Inventory Reconciliation & Evidence

### 4.1 Explanation of Discrepancy
- **Phase 28 Final State**:
  - Total workspace tests: **650**.
  - `tests/optimizer.rs`: **35** tests (32 standard `#[test]` functions + 3 property tests in `proptest!`).
  - All other 24 test targets: **615** tests.
- **Phase 29 Implementation**:
  - Added [`tests/optimizer/cancellation.rs`](../../tests/optimizer/cancellation.rs) containing **39** test functions.
  - `tests/optimizer.rs` total: $35 + 39 = \mathbf{74}$ tests.
  - Optimizer target increase: $74 - 35 = \mathbf{39}$ tests.
  - Full workspace total: $615 + 74 = \mathbf{689}$ tests.
  - Workspace increase: $689 - 650 = \mathbf{39}$ tests.
- **Root Cause of Initial Report Discrepancy**:
  - The preliminary Phase 29 plan outlined 36 tests. During implementation, 3 additional specialized edge-case tests were added (bringing the file to 39 tests).
  - The previous summary text cited the preliminary 36 planned figure ($650 + 36 = 686$) instead of the actual 39 executed tests ($650 + 39 = 689$).
  - No tests were added, removed, renamed, or relocated in any of the other 24 targets.

### 4.2 Comprehensive Target-by-Target Inventory (689 Tests)

| # | Test Target Binary / Harness | Test Count | Status | Notes |
|:---:|---|:---:|:---:|---|
| 1 | `src/lib.rs` (unit tests) | 72 | Passed | Unchanged from Phase 28 |
| 2 | `tests/ast.rs` | 30 | Passed | Unchanged from Phase 28 |
| 3 | `tests/builder.rs` | 10 | Passed | Unchanged from Phase 28 |
| 4 | `tests/cli.rs` | 20 | Passed | Unchanged from Phase 28 |
| 5 | `tests/end_to_end.rs` | 9 | Passed | Unchanged from Phase 28 |
| 6 | `tests/equivalence.rs` | 23 | Passed | Unchanged from Phase 28 |
| 7 | `tests/frontend_adapter.rs` | 14 | Passed | Unchanged from Phase 28 |
| 8 | `tests/gate_identities.rs` | 23 | Passed | Unchanged from Phase 28 |
| 9 | `tests/ir.rs` | 19 | Passed | Unchanged from Phase 28 |
| 10 | `tests/ir_rewrites.rs` | 21 | Passed | Unchanged from Phase 28 |
| 11 | `tests/lexer.rs` | 29 | Passed | Unchanged from Phase 28 |
| 12 | `tests/measurement.rs` | 11 | Passed | Unchanged from Phase 28 |
| 13 | `tests/openqasm_emitter.rs` | 21 | Passed | Unchanged from Phase 28 |
| 14 | **`tests/optimizer.rs`** | **74** | **Passed** | **+39 tests** (35 identity + 39 cancellation) |
| 15 | `tests/pass_manager.rs` | 14 | Passed | Unchanged from Phase 28 |
| 16 | `tests/qasm3.rs` | 210 | Passed | Unchanged from Phase 28 |
| 17 | `tests/quantum_semantics.rs` | 23 | Passed | Unchanged from Phase 28 |
| 18 | `tests/sampling.rs` | 9 | Passed | Unchanged from Phase 28 |
| 19 | `tests/simulator_dag.rs` | 15 | Passed | Unchanged from Phase 28 |
| 20 | `tests/simulator_single_qubit.rs` | 19 | Passed | Unchanged from Phase 28 |
| 21 | `tests/simulator_two_qubit.rs` | 18 | Passed | Unchanged from Phase 28 |
| 22 | `tests/smoke.rs` | 5 | Passed | Unchanged from Phase 28 |
| — | **Total Passed Tests** | **689** | **All Passed** | **0 failed, 0 ignored** |

---

## 5. Verification Commands Run
- `cargo fmt --check` -> Passed (zero diffs).
- `cargo clippy --all-targets --all-features -- -D warnings` -> Passed (zero warnings).
- `cargo doc --no-deps` -> Passed (zero warnings).
- `cargo build --all-targets` -> Passed.
- `cargo test --all` -> Passed (689 passed; 0 failed).
- `cargo run --example bell` -> Passed.
- `cargo run --example qasm` -> Passed.

---

## 6. Exit Gate Determination
> **EXIT GATE CRITERIA**: "Pass passes unitary equivalence and DAG invariants."
>
> **STATUS**: **SATISFIED**.
> Every candidate pair is strictly checked for exact mathematical inverse relationship, exact ordered operands (`partner.qubits() == node.qubits()`), and multi-wire dependency adjacency. DAG invariants, wire reconnections, stable IDs, provenance, and circuit global phase are strictly preserved. Unitary equivalence is mathematically validated across all cancellation tests.
