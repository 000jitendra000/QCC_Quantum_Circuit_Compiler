# Phase 33 Completion Report — Commutation Analysis Rules

## 1. Executive Summary

Phase 33 (**Commutation analysis rules**) of the QCC compiler engineering roadmap has been fully implemented, verified, and integrated into the compiler layer.

This phase establishes a mathematically verified commutation analysis layer for quantum operations. Per phase scope boundaries, this phase implements a pure read-only semantic analysis layer; it does not perform DAG rewrites or circuit mutations (deferred to Phase 34).

Key accomplishments:
1. **Rule Registry & Semantic Preconditions (`src/commutation/mod.rs`)**:
   - `CommutationRule`: 11 explicit rules identifying why operations commute: `DisjointWires`, `IdentityOperand`, `IdenticalOperations`, `SameAxisRotations`, `ZAxisOnCxControl`, `XAxisOnCxTarget`, `ZAxisOnCz`, `TwoQubitDiagonal`, `CxSharedControl`, `CxSharedTarget`, and `PauliString`.
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
   - $CX$-$CX$ commutation: shared control with disjoint targets, shared target with disjoint controls.
   - Strict rejection of cross-axis gates, reversed $CX$ operands ($CX(0, 1)$ with $CX(1, 0)$), and chained control-target pairs.

4. **Non-Unitary Exclusions & Classical Hazards**:
   - Default exclusion of `Measure`, `Reset`, `Barrier`, and `Classical`.
   - Classical read/write conflict detection (RAW, WAR, WAW).

5. **Exhaustive Testing Suite (`tests/optimizer/commutation.rs`)**:
   - Pairwise matrix commutator verification ($[A, B] = A B - B A = 0$ with $\|[A, B]\|_F < 10^{-12}$) for all enabled rules.
   - Negative matrix verification ($\|[A, B]\|_F > 0.1$ or $> 1.0$) for non-commuting pairs.
   - Anticommutation verification ($\{X, Y\} = 0$, $[X, Y] \neq 0$).
   - Parameterized rotation sweeps across boundary angles ($0, \pi/2, \pi, 2\pi, 4\pi$, negative angles).
   - Phase 10 unitary equivalence checker cross-verification on constructed circuits.

The test suite comprises **828 passing tests** across all 27 binaries (23 active test targets) with zero errors, zero warnings, and zero regressions.

---

## 2. Invariant & Mathematical Contract Compliance

1. **Exact Pairwise Matrix Verification**:
   - Every enabled commutation rule was tested against direct matrix multiplication ($M_2 \cdot M_1 = M_1 \cdot M_2$ with residual $< 10^{-12}$).
   - Controlled-NOT matrix ordering strictly follows QCC little-endian indexing and left-multiplication conventions.
   - Verified that $CX(0, 1)$ uses operand 0 as control and operand 1 as target.

2. **Z-Axis on CX Control**:
   - Controlled-NOT: $CX = |0\rangle\langle 0| \otimes I + |1\rangle\langle 1| \otimes X$.
   - $(U \otimes I) \cdot CX = CX \cdot (U \otimes I)$ if and only if $U$ commutes with $|0\rangle\langle 0|$ and $|1\rangle\langle 1|$, which requires $U$ to be diagonal in the computational basis.
   - Supported exact gates: $Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$.
   - Negative cases: $X, Y, H, R_x, R_y$ on control fail commutation both semantically and at matrix level.

3. **X-Axis on CX Target**:
   - $(I \otimes V) \cdot CX = CX \cdot (I \otimes V)$ if and only if $V X = X V$.
   - Supported exact gates: $X, R_x(\theta)$.
   - Negative cases: $Z, Y, H, S, T, R_y, R_z$ on target fail commutation both semantically and at matrix level.

4. **Pauli Anticommutation vs Commutation**:
   - Anticommuting pairs ($\{X, Y\} = 0, \{X, Z\} = 0, \{Y, Z\} = 0$) are never classified as commuting.
   - In multi-qubit Pauli tensors, commutation holds if and only if the number of single-qubit anticommuting positions is even.

5. **Non-Unitary Exclusions**:
   - `Measure`, `Reset`, `Barrier`, and `Classical` return `DoesNotCommute` with reason `NonUnitaryOperation`.
   - Classical hazards return `DoesNotCommute` with reason `ClassicalHazard`.

---

## 3. Detailed Deliverables & File Changes

| File | Status | Description |
|---|:---:|---|
| `src/commutation/mod.rs` | Created | Commutation registry, rule preconditions, result types, and public `operations_commute` API. |
| `src/commutation/pauli.rs` | Created | Pauli algebra, single- and multi-qubit Pauli commutation and anticommutation, and rotation axis classification. |
| `src/commutation/cx.rs` | Created | Dedicated rules for $CX$, $CZ$, and controlled entangler commutation. |
| `src/lib.rs` | Modified | Exposed `pub mod commutation;` and re-exported key commutation types. |
| `tests/optimizer.rs` | Modified | Linked `mod commutation;`. |
| `tests/optimizer/commutation.rs` | Created | Comprehensive integration test suite (24 tests) for commutation analysis rules. |
| `docs/project-state/plans/phase-033.md` | Created | Implementation plan for Phase 33. |
| `docs/project-state/reports/phase-033.md` | Created | Phase completion report with test evidence. |
| `docs/project-state/progress.md` | Modified | Updated progress ledger (Phase 33 completed, M3 progress 6/9, 828 tests). |

---

## 4. Test Inventory & Arithmetic Reconciliation

- **Phase 32 Accepted Baseline**: **796 tests** across 23 active test targets (27 binaries).
- **Phase 33 Additions** (+32 tests):
  - `src/lib.rs`: 75 $\to$ 83 (**+8 unit tests**: 4 in `pauli.rs`, 4 in `cx.rs`)
  - `tests/optimizer.rs`: 147 $\to$ 171 (**+24 integration tests** in `tests/optimizer/commutation.rs`)
- **Final Total**: **828 passed**, 0 failed, 0 ignored.

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
| 18 | Integration: Optimizer | `tests/optimizer.rs` | 171 | Passed (+24) |
| 19 | Integration: Pass Manager | `tests/pass_manager.rs` | 14 | Passed |
| 20 | Integration: QASM 3 | `tests/qasm3.rs` | 210 | Passed |
| 21 | Integration: Quantum Semantics | `tests/quantum_semantics.rs` | 23 | Passed |
| 22 | Integration: Sampling | `tests/sampling.rs` | 9 | Passed |
| 23 | Integration: Simulator DAG | `tests/simulator_dag.rs` | 15 | Passed |
| 24 | Integration: Simulator Single-Qubit | `tests/simulator_single_qubit.rs` | 19 | Passed |
| 25 | Integration: Simulator Two-Qubit | `tests/simulator_two_qubit.rs` | 18 | Passed |
| 26 | Integration: Smoke | `tests/smoke.rs` | 5 | Passed |
| 27 | Doc-tests | `src/lib.rs` | 0 | Passed |
| **Total** | | | **828** | **Zero errors, zero warnings** |

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
   *Result*: 828 passed across all binaries, 0 failed, 0 ignored (code 0).

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
   - `test_matrix_cx_control_z_axis_commutation` ($Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$ on CX control commute with commutator norm $< 10^{-12}$).
   - `test_matrix_cx_control_non_z_axis_strictly_fails` ($X, Y, H, R_x, R_y$ on CX control strictly fail commutation with norm $> 0.1$).
   - `test_matrix_cx_target_x_axis_commutation` ($X, R_x(\theta)$ on CX target commute with commutator norm $< 10^{-12}$).
   - `test_matrix_cx_target_non_x_axis_strictly_fails` ($Z, Y, H, S, T, R_y, R_z$ on CX target strictly fail commutation with norm $> 0.1$).
   - `test_matrix_disjoint_wire_commutation` (Pairwise matrix commutation for all canonical gate combinations on disjoint wires).
   - `test_matrix_cz_commutation_rules` ($Z$-axis gates commute with CZ on both operands; $X, Y$ fail).
   - `test_matrix_cx_cx_reversed_operands_do_not_commute` (Reversed $CX(0, 1)$ and $CX(1, 0)$ commutator norm $> 1.0$).
   - `test_pauli_algebra_and_anticommutation` ($\{X, Y\} = 0$ verified; anticommuting Paulis strictly rejected from commutation).
   - `test_pauli_string_tensor_commutation_matrix` (Two-qubit Pauli tensor product commutation and anticommutation).
   - `test_phase10_equivalence_cross_verification_commuting_pairs` & `test_phase10_equivalence_cross_verification_non_commuting_pairs` (Cross-verified using Phase 10 unitary equivalence checker).
