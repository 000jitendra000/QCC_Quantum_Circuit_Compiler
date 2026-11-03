# Phase 33 Completion Report — Commutation Analysis Rules

## 1. Executive Summary

Phase 33 (**Commutation analysis rules**) of the QCC compiler engineering roadmap has been fully implemented, verified, and integrated into the compiler layer.

This phase establishes a mathematically verified commutation analysis layer for quantum operations. Per phase scope boundaries, this phase implements a pure read-only semantic analysis layer; it does not perform DAG rewrites or circuit mutations (deferred to Phase 34).

Key accomplishments:
1. **Rule Registry & Semantic Preconditions (`src/commutation/mod.rs`)**:
   - `CommutationRule`: 12 explicit rules identifying why operations commute: `DisjointWires`, `IdentityOperand`, `IdenticalOperations`, `SameAxisRotations`, `ZAxisOnCxControl`, `XAxisOnCxTarget`, `ZAxisOnCz`, `TwoQubitDiagonal`, `CxCzSharedControl`, `CxSharedControl`, `CxSharedTarget`, and `PauliString`.
   - `NonCommutationReason`: 10 explicit causes for non-commutation: `NonUnitaryOperation`, `ClassicalHazard`, `CrossAxisRotations`, `CxControlNonZAxis`, `CxTargetNonXAxis`, `CxChainedDependency`, `CxReversedOperands`, `CzNonZAxis`, `PauliAnticommuting`, and `UnsupportedOperation`.
   - `CommutationResult`: Structured result (`Commutes`, `DoesNotCommute`, `Unknown`) with exact rule/reason and human-readable explanation.
   - `CommutationPrecondition`: Complete preconditions defining supported operation types, operand relations, parameter constraints, and conditionality.
   - `CommutationRegistry`: Central registry with deterministic rule evaluation order.

2. **Pauli Algebra & Axis Classification (`src/commutation/pauli.rs`)**:
   - `RotationAxis` ($X$, $Y$, $Z$) and `Pauli` ($I$, $X$, $Y$, $Z$).
   - Single-qubit Pauli self-commutation and anticommutation ($\{X, Y\} = 0$, $[X, Y] \neq 0$).
   - Multi-qubit Pauli string commutation via parity of single-qubit anticommutations ($(-1)^k$).
   - Same-axis single-qubit rotation commutation.

3. **Controlled Gate Commutation Rules (`src/commutation/cx.rs`)**:
   - $Z$-axis operations ($Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$) on $CX$ control.
   - $X$-axis operations ($X, R_x(\theta)$) on $CX$ target.
   - $Z$-axis operations on either operand of symmetric $CZ$.
   - Two-qubit diagonal commutation: $CZ$ with $CZ$.
   - $CX$ and $CZ$ sharing $CX$ control with disjoint targets (`CxCzSharedControl`).
   - $CX$-$CX$ commutation: shared control with disjoint targets, shared target with disjoint controls.
   - Strict rejection of cross-axis gates, reversed $CX$ operands ($CX(0, 1)$ with $CX(1, 0)$), and chained control-target pairs.

4. **Non-Unitary Exclusions & Classical Hazards**:
   - Default exclusion of `Measure`, `Reset`, `Barrier`, and `Classical`.
   - Classical read/write conflict detection (RAW, WAR, WAW).

5. **Exhaustive Testing Suite (`tests/optimizer/commutation.rs`)**:
   - Pairwise matrix commutator verification ($[A, B] = A B - B A = 0$ with $\|[A, B]\|_F < 10^{-12}$) for all enabled rules.
   - Negative matrix verification ($\|[A, B]\|_F > 0.1$ or $> 1.0$) for non-commuting pairs.
   - Anticommutation verification ($\{X, Y\} = 0$, $[X, Y] \neq 0$).
   - Basis state action verification under little-endian Kronecker embeddings.
   - Independent 3-qubit basis permutation verification for controlled gates.
   - Parameterized rotation sweeps across boundary angles ($0, \pi/2, \pi, 2\pi, 4\pi$, negative angles).
   - Phase 10 unitary equivalence checker cross-verification on constructed circuits.

The test suite comprises **833 passing tests** across all 27 binaries (23 active test targets) with zero errors, zero warnings, and zero regressions.

---

## 2. Invariant & Mathematical Contract Compliance

1. **Exact Pairwise Matrix Verification & Little-Endian Tensor Conventions**:
   - In QCC, basis state indices are little-endian: $|j\rangle = |q_{n-1} \dots q_1 q_0\rangle$, where $q_0$ is the least-significant bit (LSB) and $q_{n-1}$ is the most-significant bit (MSB).
   - For a 2-qubit system $\{q_0, q_1\}$, basis vectors are $|q_1 q_0\rangle = |q_1\rangle \otimes |q_0\rangle$.
   - In standard matrix Kronecker product ordering $(A \otimes B)_{2r_1+r_0, 2c_1+c_0} = A_{r_1, c_1} \cdot B_{r_0, c_0}$, row/column index $2r_1 + r_0$ assigns bit $r_1$ to $q_1$ and $r_0$ to $q_0$.
   - Consequently:
     - An operator $U$ on $q_0$ (LSB) embeds as $(I)_{q_1} \otimes (U)_{q_0} = \text{kron}(I, U)$.
     - An operator $V$ on $q_1$ (MSB) embeds as $(V)_{q_1} \otimes (I)_{q_0} = \text{kron}(V, I)$.
   - Verified empirically by `test_basis_state_action_proves_kronecker_little_endian`:
     - $\text{kron}(I, X)$ flips bit $q_0$ on all 4 basis states: $|00\rangle \leftrightarrow |01\rangle$ and $|10\rangle \leftrightarrow |11\rangle$.
     - $\text{kron}(X, I)$ flips bit $q_1$ on all 4 basis states: $|00\rangle \leftrightarrow |10\rangle$ and $|01\rangle \leftrightarrow |11\rangle$.

2. **Z-Axis on CX Control**:
   - For $CX(0, 1)$: operand 0 is control ($q_0$), operand 1 is target ($q_1$).
     - Embedding for $Z$-axis operation $U_z$ on control $q_0$: $(I)_{q_1} \otimes (U_z)_{q_0} = \text{kron}(I, U_z)$.
     - Matrix commutator: $\|[CX(0, 1), I \otimes U_z]\|_F < 10^{-12}$.
   - For $CX(1, 0)$: operand 0 is control ($q_1$), operand 1 is target ($q_0$).
     - Embedding for $Z$-axis operation $U_z$ on control $q_1$: $(U_z)_{q_1} \otimes (I)_{q_0} = \text{kron}(U_z, I)$.
     - Matrix commutator: $\|[CX(1, 0), U_z \otimes I]\|_F < 10^{-12}$.
   - Supported exact gates: $Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$.
   - Negative cases: $X, Y, H, R_x, R_y$ on control fail commutation both semantically and at matrix level ($\|[A, B]\|_F > 0.1$).

3. **X-Axis on CX Target**:
   - For $CX(0, 1)$: target is $q_1$.
     - Embedding for $X$-axis operation $U_x$ on target $q_1$: $(U_x)_{q_1} \otimes (I)_{q_0} = \text{kron}(U_x, I)$.
     - Matrix commutator: $\|[CX(0, 1), U_x \otimes I]\|_F < 10^{-12}$.
   - For $CX(1, 0)$: target is $q_0$.
     - Embedding for $X$-axis operation $U_x$ on target $q_0$: $(I)_{q_1} \otimes (U_x)_{q_0} = \text{kron}(I, U_x)$.
     - Matrix commutator: $\|[CX(1, 0), I \otimes U_x]\|_F < 10^{-12}$.
   - Supported exact gates: $X, R_x(\theta)$.
   - Negative cases: $Z, Y, H, S, T, R_y, R_z$ on target fail commutation both semantically and at matrix level ($\|[A, B]\|_F > 0.1$).

4. **Pauli Anticommutation vs Commutation**:
   - Anticommuting pairs ($\{X, Y\} = 0, \{X, Z\} = 0, \{Y, Z\} = 0$) are never classified as commuting.
   - In multi-qubit Pauli tensors, commutation holds if and only if the number of single-qubit anticommuting positions is even ($(-1)^k = +1$).

5. **Two-Qubit Controlled Gates ($CZ$ and $CX$)**:
   - $CZ$ is diagonal in the computational basis ($\text{diag}(1, 1, 1, -1)$); all $CZ$ gates mutually commute (`TwoQubitDiagonal`).
   - $CX(c, t)$ and $CZ(c, z)$ share the $CX$ control qubit with disjoint targets ($t \neq z$): they commute (`CxCzSharedControl`) because both branch on the same computational basis projectors $|0\rangle\langle 0|_c, |1\rangle\langle 1|_c$ on the control qubit, while acting on disjoint target wires ($X_t$ and $Z_z$).
   - $CX$ is never described as diagonal.

6. **Non-Unitary Exclusions**:
   - `Measure`, `Reset`, `Barrier`, and `Classical` return `DoesNotCommute` with reason `NonUnitaryOperation`.
   - Classical hazards return `DoesNotCommute` with reason `ClassicalHazard`.

---

## 3. Detailed Deliverables & File Changes

| File | Status | Description |
|---|:---:|---|
| `src/commutation/mod.rs` | Created | Commutation registry, 12 rule preconditions, result types, and public `operations_commute` API. |
| `src/commutation/pauli.rs` | Created | Pauli algebra, single- and multi-qubit Pauli commutation and anticommutation, and rotation axis classification. |
| `src/commutation/cx.rs` | Created | Dedicated rules for $CX$, $CZ$, and controlled entangler commutation. |
| `src/lib.rs` | Modified | Exposed `pub mod commutation;` and re-exported key commutation types. |
| `tests/optimizer.rs` | Modified | Linked `mod commutation;`. |
| `tests/optimizer/commutation.rs` | Created | Comprehensive integration test suite (29 tests) for commutation analysis rules. |
| `docs/project-state/plans/phase-033.md` | Created | Implementation plan for Phase 33. |
| `docs/project-state/reports/phase-033.md` | Created | Phase completion report with test evidence. |
| `docs/project-state/progress.md` | Modified | Updated progress ledger (Phase 33 completed, M3 progress 6/9, 833 tests). |

---

## 4. Test Inventory & Arithmetic Reconciliation

- **Phase 32 Accepted Baseline**: **796 tests** across 23 active test targets (27 binaries).
- **Phase 33 Additions** (+37 tests):
  - `src/lib.rs`: 75 $\to$ 83 (**+8 unit tests**: 4 in `pauli.rs`, 4 in `cx.rs`)
  - `tests/optimizer.rs`: 147 $\to$ 176 (**+29 integration tests** in `tests/optimizer/commutation.rs`)
- **Final Total**: **833 passed**, 0 failed, 0 ignored.

### Detailed Test Inventory by Binary Target
| # | Target Name | Binary Path | Tests | Status |
|---|---|---|---|---|
| 1 | Unit tests | `src/lib.rs` | 83 | Passed (+8) |
| 2 | Bench target | `benches/simulator_bench.rs` | 0 | Passed (entrypoint) |
| 3 | Parser fuzz target | `fuzz/fuzz_targets/fuzz_parser.rs` | 0 | Passed (entrypoint) |
| 4 | Main binary | `src/main.rs` | 0 | Passed (entrypoint) |
| 5 | Integration: AST | `tests/ast.rs` | 30 | Passed |
| 6 | Integration: Builder | `tests/builder.rs` | 10 | Passed |
| 7 | Integration: CLI | `tests/cli.rs` | 20 | Passed |
| 8 | Integration: End-to-end | `tests/end_to_end.rs` | 9 | Passed |
| 9 | Integration: Equivalence | `tests/equivalence.rs` | 23 | Passed |
| 10 | Integration: Euler | `tests/euler.rs` | 25 | Passed |
| 11 | Integration: Frontend Adapter | `tests/frontend_adapter.rs` | 14 | Passed |
| 12 | Integration: Gate Identities | `tests/gate_identities.rs` | 23 | Passed |
| 13 | Integration: IR | `tests/ir.rs` | 19 | Passed |
| 14 | Integration: IR Rewrites | `tests/ir_rewrites.rs` | 27 | Passed |
| 15 | Integration: Lexer | `tests/lexer.rs` | 29 | Passed |
| 16 | Integration: Measurement | `tests/measurement.rs` | 11 | Passed |
| 17 | Integration: OpenQASM Emitter | `tests/openqasm_emitter.rs` | 21 | Passed |
| 18 | Integration: Optimizer | `tests/optimizer.rs` | 176 | Passed (+29) |
| 19 | Integration: Pass Manager | `tests/pass_manager.rs` | 14 | Passed |
| 20 | Integration: QASM 3 | `tests/qasm3.rs` | 210 | Passed |
| 21 | Integration: Quantum Semantics | `tests/quantum_semantics.rs` | 23 | Passed |
| 22 | Integration: Sampling | `tests/sampling.rs` | 9 | Passed |
| 23 | Integration: Simulator DAG | `tests/simulator_dag.rs` | 15 | Passed |
| 24 | Integration: Simulator Single-Qubit | `tests/simulator_single_qubit.rs` | 19 | Passed |
| 25 | Integration: Simulator Two-Qubit | `tests/simulator_two_qubit.rs` | 18 | Passed |
| 26 | Integration: Smoke | `tests/smoke.rs` | 5 | Passed |
| 27 | Doc-tests | `src/lib.rs` | 0 | Passed |
| **Total** | | | **833** | **Zero errors, zero warnings** |

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
   cargo test --all
   ```
   *Result*: 833 passed across all binaries, 0 failed, 0 ignored (code 0).

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
   > **EXIT GATE**: All commutation rules pass pairwise matrix tests.

   Verified by:
   - `test_basis_state_action_proves_kronecker_little_endian` (Explicit verification that $\text{kron}(I, X)$ flips $q_0$ and $\text{kron}(X, I)$ flips $q_1$).
   - `test_matrix_cx_control_z_axis_commutation` ($Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$ on CX control commute with commutator norm $< 10^{-12}$).
   - `test_matrix_cx_control_non_z_axis_strictly_fails` ($X, Y, H, R_x, R_y$ on CX control strictly fail commutation with norm $> 0.1$).
   - `test_matrix_cx_target_x_axis_commutation` ($X, R_x(\theta)$ on CX target commute with commutator norm $< 10^{-12}$).
   - `test_matrix_cx_target_non_x_axis_strictly_fails` ($Z, Y, H, S, T, R_y, R_z$ on CX target strictly fail commutation with norm $> 0.1$).
   - `test_matrix_reversed_cx_orientation_commutation` (Reversed orientation $CX(1, 0)$ verified for both control on $q_1$ and target on $q_0$).
   - `test_matrix_same_axis_rotations_pairwise` (Pairwise matrix commutator check for same-axis rotations on $X, Y, Z$).
   - `test_matrix_identity_operand_pairwise` (Scalar $\pm I$ commutes with arbitrary unitary matrices).
   - `test_3qubit_controlled_commutation_pairwise` (Independent 3-qubit basis permutation verification for shared control, shared target, and CX-CZ control overlap).
   - `test_matrix_disjoint_wire_commutation` (Pairwise matrix commutation for all canonical gate combinations on disjoint wires).
   - `test_matrix_cz_commutation_rules` ($Z$-axis gates commute with CZ on both operands; $X, Y$ fail).
   - `test_matrix_cx_cx_reversed_operands_do_not_commute` (Reversed $CX(0, 1)$ and $CX(1, 0)$ commutator norm $> 1.0$).
   - `test_pauli_algebra_and_anticommutation` ($\{X, Y\} = 0$ verified; anticommuting Paulis strictly rejected from commutation).
   - `test_pauli_string_tensor_commutation_matrix` (Two-qubit Pauli tensor product commutation and anticommutation).
   - `test_phase10_equivalence_cross_verification_commuting_pairs` & `test_phase10_equivalence_cross_verification_non_commuting_pairs` (Cross-verified using Phase 10 unitary equivalence checker).

---

## 6. Matrix Convention and Rule Soundness Audit Details

### 1. Tensor Product Ordering Reconciliation
- **QCC Invariant**: $q_0$ is LSB with bit weight 1; $q_1$ is MSB with bit weight 2.
- **Dirac Basis**: $|q_1 q_0\rangle = |q_1\rangle \otimes |q_0\rangle$.
- **Standard Kronecker Product**: $(A \otimes B)_{2r_1+r_0, 2c_1+c_0} = A_{r_1, c_1} \cdot B_{r_0, c_0}$.
  - The first/outer tensor factor $A$ corresponds to bit $r_1$ ($q_1$).
  - The second/inner tensor factor $B$ corresponds to bit $r_0$ ($q_0$).
- **Embeddings**:
  - Gate $U_z$ on control $q_0$ of $CX(0, 1)$: $(I)_{q_1} \otimes (U_z)_{q_0} \iff \text{kron}(I, U_z)$.
  - Gate $U_x$ on target $q_1$ of $CX(0, 1)$: $(U_x)_{q_1} \otimes (I)_{q_0} \iff \text{kron}(U_x, I)$.
  - Gate $U_z$ on control $q_1$ of $CX(1, 0)$: $(U_z)_{q_1} \otimes (I)_{q_0} \iff \text{kron}(U_z, I)$.
  - Gate $U_x$ on target $q_0$ of $CX(1, 0)$: $(I)_{q_1} \otimes (U_x)_{q_0} \iff \text{kron}(I, U_x)$.
- **Empirical Proof**:
  - `test_basis_state_action_proves_kronecker_little_endian` explicitly tests multiplying these $4 \times 4$ matrices by standard basis vectors $e_0, e_1, e_2, e_3$ and proves the exact bit flipping behavior.

### 2. Soundness of Every Enabled Rule
1. `DisjointWires`: Operands disjoint ($Q_1 \cap Q_2 = \emptyset$) and no classical RAW/WAR/WAW hazard. Exact matrix property: $A \otimes B = (A \otimes I)(I \otimes B) = (I \otimes B)(A \otimes I)$.
2. `IdentityOperand`: Operates when an operand is $\pm I$ across the entire single-qubit space (via `norm.is_identity()` checking $0 \pmod{4\pi}$ or $2\pi \pmod{4\pi}$). Scalar multiples of $I$ commute with every linear operator.
3. `IdenticalOperations`: Identical gate, operands, and evaluated parameters satisfy $[U, U] = 0$.
4. `SameAxisRotations`: Any two rotations sharing the same coordinate axis satisfy $[e^{-i\theta_1 \sigma / 2}, e^{-i\theta_2 \sigma / 2}] = 0$.
5. `ZAxisOnCxControl`: $CX$ control projects onto $|0\rangle\langle 0|$ and $|1\rangle\langle 1|$; any operator diagonal in the computational basis ($Z, S, S^\dagger, T, T^\dagger, R_z$) commutes with these projectors.
6. `XAxisOnCxTarget`: $CX$ applies $X$ to target when control is $|1\rangle$; any operator commuting with $X$ ($X, R_x$) commutes with $CX$.
7. `ZAxisOnCz`: $CZ$ is diagonal in the computational basis; $Z$-axis operations on either operand commute with $CZ$.
8. `TwoQubitDiagonal`: $CZ(a, b)$ and $CZ(c, d)$ are completely diagonal in the computational basis, hence mutually commute.
9. `CxCzSharedControl`: $CX(c, t)$ and $CZ(c, z)$ sharing control $c$ with $t \neq z$ commute because both branch on standard computational basis projectors on $c$ and act on disjoint target wires ($X_t$ and $Z_z$).
10. `CxSharedControl`: $CX(c, t_1)$ and $CX(c, t_2)$ commute because both project on $c$ and apply $X$ to disjoint targets $t_1, t_2$.
11. `CxSharedTarget`: $CX(c_1, t)$ and $CX(c_2, t)$ commute because $[X_t, X_t] = 0$ on target and controls $c_1, c_2$ are disjoint.
12. `PauliString`: Multi-qubit Pauli tensor products commute if and only if the number of single-qubit anticommuting components is even ($(-1)^k = +1$).

### 3. Deliberately Noncommuting & Negative Cases
- Non-$Z$ on $CX$ control: $X, Y, H, R_x, R_y$ fail ($> 0.1$).
- Non-$X$ on $CX$ target: $Z, Y, H, S, T, R_y, R_z$ fail ($> 0.1$).
- Reversed $CX(0, 1)$ vs $CX(1, 0)$ fails ($> 1.0$).
- Chained $CX(0, 1)$ vs $CX(1, 2)$ fails on $|001\rangle$.
- $CX(0, 1)$ vs $CZ(1, 2)$ touches $CX$ target, fails.
- Pauli anticommuting pairs $\{X, Y\} = 0, \{X, Z\} = 0, \{Y, Z\} = 0$ strictly rejected.
- Non-unitary operations (`Measure`, `Reset`, `Barrier`, `Classical`) strictly rejected.
