# Phase 39 Implementation Plan — Fixed Decomposition Template Library

## 1. Context & Objectives

- **Phase Number**: Phase 39
- **Title**: Fixed Decomposition Template Library
- **Milestone**: Milestone M4: Generic Target and Basis Translation
- **Roadmap Specification**: `docs/roadmap/phases-037-043.md#phase-039`
- **Objective**: Implement a library of mathematically verified fixed decomposition templates before attempting arbitrary two-qubit synthesis.
- **Prerequisites**: Phases 10, 27, 31, and 37 verified and complete.

---

## 2. Invariants & Semantic Contracts

1. **Little-Endian Qubit Indexing**: Logical qubit $q[0]$ is the least-significant bit (LSB) in statevector indices.
2. **Column Vectors & Execution Order**: Operations act by left multiplication. If operation $A$ executes before operation $B$, their combined operator is $B \cdot A$.
3. **Gate Operand Order**: For $CX(q_{\text{ctrl}}, q_{\text{targ}})$: Operand 0 is control, Operand 1 is target.
4. **Circuit-Level Global Phase**: Explicit circuit global phase must be strictly tracked: $U_{\text{circuit}} = e^{i\phi} U_{\text{operations}}$. Templates must record their exact global phase contribution.
5. **Exact Toffoli Requirement**: The selected Toffoli decomposition must implement the exact Toffoli (CCX) gate on all 8 computational basis states, **not a relative-phase Toffoli** (no state-dependent phase errors).
6. **No Measurement-Distribution Equivalence**: Unitary equivalence requires full statevector amplitude comparison, not merely matching measurement probabilities.
7. **Transactional Rewriting**: Graph mutations must use QCC's transactional DAG rewrite infrastructure (`CircuitTransaction` / `replace_sequence`), maintaining DAG convexity, dependency reconnection, and automatic rollback on failure.
8. **Enabled Rule Registry Gate**: Only mathematically verified decompositions may enter the enabled rule registry.
9. **Stable Rust Only**: Zero nightly flags, zero unverified unsafe code, zero `-D warnings`.

---

## 3. Architecture & Module Design

### 3.1 Module Hierarchy
```text
src/
  synthesis/
    mod.rs
    templates/
      mod.rs
      matrix.rs      // Matrix8x8 and independent operator evaluation utilities
      swap.rs        // SWAP -> 3 CX
      cx_cz.rs       // CX -> H CZ H and CZ -> H CX H
      toffoli.rs     // Exact Toffoli (CCX) -> 6 CX + 2 H + 4 T + 3 Tdg
      registry.rs    // TemplateRegistry managing enabled verified templates
```

### 3.2 Types & Signatures

#### `src/synthesis/templates/mod.rs`
- `TemplateId`: Enumeration identifying verified templates (`SwapToCx`, `CxToCz`, `CzToCx`, `ToffoliToCx`).
- `Precondition`: Declared preconditions (`DistinctOperands`, `QuantumArity(usize)`, `ZeroParameters`, `UnitaryTarget`).
- `TemplateError`: Structured errors (`ArityMismatch`, `DuplicateOperands`, `NonZeroParameters`, `NonUnitaryOperation`, `UnsupportedOperation`, `VerificationFailed`, `Rewrite`).
- `DecompositionTemplate` trait:
  - `id(&self) -> TemplateId`
  - `name(&self) -> &'static str`
  - `target_gate(&self) -> &'static str`
  - `quantum_arity(&self) -> usize`
  - `phase_contribution(&self) -> f64`
  - `preconditions(&self) -> &'static [Precondition]`
  - `check_preconditions(&self, operands: &[QubitId]) -> Result<(), TemplateError>`
  - `expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError>`
  - `apply(&self, circuit: &mut Circuit, target: OperationId) -> Result<Vec<OperationId>, TemplateError>`
  - `verify_matrix(&self) -> Result<(), TemplateError>`

#### `src/synthesis/templates/swap.rs`
- `SwapToCx`:
  - Target: `"swap"` (`GateId::SWAP`) on `[a, b]`
  - Expansion: `[CX(a, b), CX(b, a), CX(a, b)]`
  - Phase contribution: $0.0$
  - Preconditions: `a != b`, arity 2
  - Local matrix verification: against `matrix_swap()`

#### `src/synthesis/templates/cx_cz.rs`
- `CxToCz`:
  - Target: `"cx"` (`GateId::CX`) on `[ctrl, targ]`
  - Expansion: `[H(targ), CZ(ctrl, targ), H(targ)]`
  - Phase contribution: $0.0$
  - Preconditions: `ctrl != targ`, arity 2
  - Local matrix verification: against `matrix_cx_c0_t1()`
- `CzToCx`:
  - Target: `"cz"` (`GateId::CZ`) on `[q0, q1]`
  - Expansion: `[H(q1), CX(q0, q1), H(q1)]` (also supports symmetric orientation on `q0`)
  - Phase contribution: $0.0$
  - Preconditions: `q0 != q1`, arity 2
  - Local matrix verification: against `matrix_cz()`

#### `src/synthesis/templates/toffoli.rs`
- `ToffoliToCx`:
  - Target: `"ccx"` / `"toffoli"` on `[ctrl1, ctrl2, target]`
  - Exact Nielsen & Chuang 15-gate sequence:
    1. $H(t)$
    2. $CX(c_2, t)$
    3. $T^\dagger(t)$
    4. $CX(c_1, t)$
    5. $T(t)$
    6. $CX(c_2, t)$
    7. $T^\dagger(t)$
    8. $CX(c_1, t)$
    9. $T(c_2)$
    10. $T(t)$
    11. $CX(c_1, c_2)$
    12. $H(t)$
    13. $T(c_1)$
    14. $T^\dagger(c_2)$
    15. $CX(c_1, c_2)$
  - Exactly 6 CX gates, 2 H, 4 T, 3 Tdg
  - Phase contribution: $0.0$
  - Preconditions: `c1 != c2`, `c1 != t`, `c2 != t`, arity 3
  - Independent matrix verification: against exact 3-qubit Toffoli matrix

#### `src/synthesis/templates/matrix.rs`
- `Matrix8x8`:
  - 64 complex amplitudes for 3-qubit operators
  - `identity()`, `matmul()`, `approx_eq()`, `max_difference()`, `is_unitary()`
  - Helpers to embed 1Q gates, 2Q CX gates, and construct exact Toffoli reference matrix

#### `src/synthesis/templates/registry.rs`
- `TemplateRegistry`:
  - Enabled verified template lookup and management
  - Safe transactional circuit application: `apply_template(&self, circuit: &mut Circuit, target: OperationId, template_id: TemplateId)`

---

## 4. Test Strategy (`tests/synthesis.rs` & `tests/synthesis/templates/`)

1. **Computational Basis Equivalence**:
   - SWAP: All 4 basis states $|00\rangle, |01\rangle, |10\rangle, |11\rangle$.
   - CX -> CZ: All 4 basis states.
   - CZ -> CX: All 4 basis states.
   - Toffoli: All 8 basis states $|000\rangle$ through $|111\rangle$.
2. **Random Complex Superposition State Testing**:
   - Random normalized complex vectors tested through full circuit execution.
3. **Independent Local/Full Matrix Verification**:
   - Element-wise difference $\le 10^{-12}$.
   - Frobenius norm difference $\le 10^{-12}$.
4. **Operand Order & Permutation Tests**:
   - Verify control/target orientation and reversed operand tuples.
5. **Phase-Sensitive Verification**:
   - Proves zero global phase delta and zero relative phase delta on non-111 states.
6. **Negative Precondition Tests**:
   - Duplicate qubits (e.g. `swap(p0, p0)` or `ccx(p0, p0, p1)`).
   - Arity mismatches.
   - Non-unitary operations (`measure`, `reset`).
7. **Transactional Circuit Rewriting**:
   - Verify dependency reconnection, DAG convexity, and global phase preservation across rewrites.

---

## 5. Verification Commands

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell && cargo run --example qasm`
7. Python `openqasm3` parser tests
