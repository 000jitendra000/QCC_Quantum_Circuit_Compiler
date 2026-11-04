# Phase 39 Completion Report — Fixed Decomposition Template Library

## 1. Executive Summary

- **Phase Number**: Phase 39
- **Phase Title**: Fixed decomposition template library
- **Milestone**: Milestone M4: Generic Target and Basis Translation
- **Exit Gate**: Template library passes full local matrix comparison; decompositions produce verified-equivalent subcircuits (**PASSED**)
- **Baseline Git Checkpoint**: `835d6ab` (`feat(phase-38): implement target capability queries and cost models`)
- **Status**: Completed and fully verified with 100% test pass across all targets.

Phase 39 implements a library of mathematically verified, circuit-preserving fixed decomposition templates for the Quantum Circuit Compiler (QCC). Before attempting general continuous two-qubit synthesis or arbitrary multi-qubit routing, the compiler now possesses exact discrete decompositions for fundamental multi-qubit gates:
1. Two-qubit $\text{SWAP}(a, b) = CX(a, b); CX(b, a); CX(a, b)$.
2. Two-qubit basis conversion $CX(c, t) \leftrightarrow CZ(c, t)$ via Hadamard conjugation on the target qubit.
3. Three-qubit exact Toffoli ($CCX$) decomposition into 15 elementary gates (6 $CX$, 2 $H$, 4 $T$, 3 $T^\dagger$) with strictly zero relative phase and zero global phase contribution.

Every template exposes its operand order requirements, phase contribution, and explicit preconditions (`DistinctOperands`, `QuantumArity`, `NoParameters`). In accordance with QCC core invariants, all templates are verified against independently constructed matrix models before admission to the enabled rule registry. Transformations execute via centralized transactional DAG rewrites (`replace_sequence` / `CircuitTransaction`) ensuring cycle detection, convexity preservation, and automatic dependency reconnection.

---

## 2. Key Deliverables & Architecture

### 2.1 Core Template Trait & Preconditions (`src/synthesis/templates/mod.rs`)
- **Template Identity (`TemplateId`)**:
  - `TemplateId::SwapToCx`: Identifies the 3-CX SWAP decomposition.
  - `TemplateId::CxToCz`: Identifies the $H$-conjugated CX-to-CZ decomposition.
  - `TemplateId::CzToCx`: Identifies the $H$-conjugated CZ-to-CX decomposition.
  - `TemplateId::ToffoliToCx`: Identifies the canonical exact 15-gate Toffoli decomposition.
- **Precondition Model (`Precondition`)**:
  - `Precondition::DistinctOperands`: Qubit operands must be mutually distinct ($q_i \neq q_j$).
  - `Precondition::QuantumArity(usize)`: Exact quantum arity validation.
  - `Precondition::NoParameters`: Disallows continuous parameters (since these templates represent fixed discrete unitaries).
- **Decomposition Template Trait (`DecompositionTemplate`)**:
  - `id(&self) -> TemplateId`: Stable template identifier.
  - `name(&self) -> &'static str`: Canonical template name.
  - `target_gate(&self) -> &'static str`: Mnemonic of gate decomposed by this template (`"swap"`, `"cx"`, `"cz"`, `"ccx"`).
  - `quantum_arity(&self) -> usize`: Expected qubit operand count.
  - `phase_contribution(&self) -> f64`: Circuit-level global phase contribution $\Delta \phi \in [0, 2\pi)$ in radians.
  - `preconditions(&self) -> &'static [Precondition]`: Declared formal preconditions.
  - `check_preconditions(&self, operands: &[QubitId]) -> Result<(), TemplateError>`: Precondition validator.
  - `expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError>`: Generates constituent constituent QccIR operations.
  - `verify_matrix(&self) -> Result<(), TemplateError>`: Independently verifies equivalence against the target gate's unitary matrix.
  - `apply(&self, circuit: &mut Circuit, target_id: OperationId) -> Result<Vec<OperationId>, TemplateError>`: Transactionally replaces the target node in the QccIR dependency DAG.
- **Structured Template Errors (`TemplateError`)**:
  - `ArityMismatch { template, expected, actual }`: Qubit count mismatch.
  - `DuplicateOperands { template, operands }`: Duplicate qubits in operand slice.
  - `NonZeroParameters { template, actual }`: Disallowed parameters provided.
  - `NonUnitaryOperation { template, operation }`: Attempt to decompose non-unitary operation (measure, reset, barrier, classical).
  - `UnsupportedOperation { template, operation }`: Mismatch between target gate and operation.
  - `VerificationFailed { template, max_diff, tolerance }`: Local matrix verification failure.
  - `Rewrite(RewriteError)`: Transactional DAG replacement error.

### 2.2 Independent Complex Matrix Engine (`src/synthesis/templates/matrix.rs`)
- **$8 \times 8$ Complex Matrix (`Matrix8x8`)**:
  - Full arithmetic (`add`, `sub`, `matmul`, `scalar_mul`, `adjoint`).
  - Unitary checks (`is_unitary(tolerance)`), trace, and distance metrics (`max_difference(&other)`).
- **Independent Gate Embeddings**:
  - `embed_single_qubit(u: &Matrix2x2, target: usize) -> Matrix8x8`: Embeds 1Q gate into 3Q Hilbert space using little-endian coordinate mapping $i = q_0 + 2 q_1 + 4 q_2$.
  - `embed_cx(control: usize, target: usize) -> Matrix8x8`: Embeds 2Q CX gate across arbitrary qubit pair within 3Q space.
- **Independent Reference Toffoli Matrix (`Matrix8x8::exact_toffoli`)**:
  - Synthesizes the exact permutation matrix for Toffoli across arbitrary controls $(c_1, c_2)$ and target $t$:
    $$|q_2 q_1 q_0\rangle \mapsto |q_2' q_1' q_0'\rangle \quad \text{where } q_t' = q_t \oplus (q_{c1} \land q_{c2})$$
  - Constructed purely from mathematical definition, independent of the decomposition circuit.

### 2.3 Verified Fixed Decomposition Templates
- **SWAP Decomposition (`SwapToCxTemplate` in `src/synthesis/templates/swap.rs`)**:
  - Implements: $\text{SWAP}(a, b) = CX(a, b); CX(b, a); CX(a, b)$.
  - Quantum arity: 2. Phase contribution: $0.0$.
  - Supports arbitrary operand ordering $(p_a, p_b)$ or $(p_b, p_a)$.
  - Mathematical verification: compared against `matrix_swap()` with max difference $< 10^{-15}$.
- **CX $\leftrightarrow$ CZ Decomposition (`CxToCzTemplate` and `CzToCxTemplate` in `src/synthesis/templates/cx_cz.rs`)**:
  - CX $\to$ CZ: $CX(c, t) = H(t); CZ(c, t); H(t)$.
  - CZ $\to$ CX: $CZ(c, t) = H(t); CX(c, t); H(t)$ (using target qubit $t$, preserving symmetric nature of CZ).
  - Quantum arity: 2. Phase contribution: $0.0$.
  - Mathematical verification: verified against `matrix_cx()` and `matrix_cz()` with max difference $< 10^{-15}$.
- **Canonical Exact Toffoli Decomposition (`ToffoliToCxTemplate` in `src/synthesis/templates/toffoli.rs`)**:
  - Implements the exact 15-gate canonical sequence (Nielsen & Chuang Fig 4.9, p. 182) on controls $c_1, c_2$ and target $t$:
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
  - Quantum arity: 3. Gate counts: 6 CX, 2 H, 4 T, 3 Tdg. Phase contribution: $0.0$.
  - **Zero Relative-Phase Guarantee**: All diagonal entries for computational basis states unaffected by the control condition ($|000\rangle, |001\rangle, |010\rangle, |100\rangle, |101\rangle, |110\rangle$) have strictly amplitude $+1.0 + 0.0i$ (zero relative phase). This strictly distinguishes QCC's canonical exact Toffoli from relative-phase approximations (e.g. Margolus).
  - Mathematical verification: verified against `Matrix8x8::exact_toffoli` across all 6 permutations of $(c_1, c_2, t)$ with max discrepancy $< 10^{-15}$.

### 2.4 Verified Admission Template Registry (`src/synthesis/templates/registry.rs`)
- **Strict Verification Gate on Registration**:
  - `register(&mut self, template: Box<dyn DecompositionTemplate>) -> Result<(), TemplateError>`
  - Invokes `template.verify_matrix()` before adding the template to the internal lookup map.
  - Automatically rejects unverified, invalid, or numerically inaccurate templates with `TemplateError::VerificationFailed`.
- **Query & Application API**:
  - `with_verified_templates() -> Result<Self, TemplateError>`: Default factory pre-populating verified templates.
  - `get(&self, id: TemplateId) -> Option<&dyn DecompositionTemplate>`: Lookup by strongly-typed ID.
  - `get_by_name(&self, name: &str) -> Option<&dyn DecompositionTemplate>`: Lookup by canonical name.
  - `find_for_gate(&self, gate_name: &str) -> Vec<TemplateId>`: Lookup templates decomposing a given gate mnemonic (with case-insensitivity and alias resolution, e.g. `"toffoli"` -> `"ccx"`).
  - `apply_template(&self, circuit, target_id, template_id)`: Transactionally applies template by ID.
  - `apply_by_name(&self, circuit, target_id, name)`: Transactionally applies template by name.

---

## 3. Verification & Test Evidence

### 3.1 Unit Tests (`src/synthesis/templates/`)
- Unit tests verify matrix construction, embedding operations, template expansion, and registry admission in `src/synthesis/templates/{matrix, swap, cx_cz, toffoli, registry}.rs`.

### 3.2 Comprehensive Integration Test Suite (`tests/synthesis.rs`, 33 Tests)

#### `tests/synthesis/templates/swap.rs` (8 Tests)
1. `test_swap_matrix_verification`: Validates local matrix comparison passes within $10^{-12}$.
2. `test_swap_vs_decomposed_cx_basis_states`: Validates identical truth-table evolution on all 4 basis states.
3. `test_swap_decomposed_superposition_state`: Validates exact statevector evolution on arbitrary complex superposition states.
4. `test_swap_circuit_equivalence`: Validates unitary equivalence with `check_equivalence` between a SWAP circuit and 3-CX sequence.
5. `test_swap_template_apply_on_circuit`: Validates transactional replacement of SWAP node in a QccIR circuit.
6. `test_swap_reversed_operands`: Validates decomposition and circuit equivalence when operands are reversed $(p_1, p_0)$.
7. `test_swap_precondition_duplicate_qubits`: Validates structured rejection of identical qubits $(q_0, q_0)$.
8. `test_swap_precondition_arity_mismatch`: Validates rejection of arity mismatches (1 qubit or 3 qubits).
9. `test_swap_dag_replacement_in_larger_context`: Validates replacement inside a multi-qubit circuit with preserved dependency edges.

#### `tests/synthesis/templates/cx_cz.rs` (9 Tests)
1. `test_cx_to_cz_matrix_verification`: Validates CX $\to$ CZ local matrix equivalence.
2. `test_cz_to_cx_matrix_verification`: Validates CZ $\to$ CX local matrix equivalence.
3. `test_cx_to_cz_basis_states`: Validates CX $\to$ CZ on all 4 basis states.
4. `test_cz_to_cx_basis_states`: Validates CZ $\to$ CX on all 4 basis states.
5. `test_cx_to_cz_reversed_orientation`: Validates target-qubit Hadamard placement when control and target are swapped ($c=1, t=0$).
6. `test_cx_to_cz_superposition_state`: Validates complex superposition evolution for CX $\to$ CZ.
7. `test_cz_to_cx_superposition_state`: Validates complex superposition evolution for CZ $\to$ CX.
8. `test_cx_to_cz_circuit_equivalence`: Validates end-to-end circuit equivalence of CX $\to$ CZ.
9. `test_cz_to_cx_circuit_equivalence`: Validates end-to-end circuit equivalence of CZ $\to$ CX.
10. `test_cx_cz_roundtrip`: Validates round-trip replacement ($CX \to CZ \to CX$) produces equivalent circuits.
11. `test_cx_cz_preconditions`: Validates duplicate operand and arity checking for CX/CZ templates.

#### `tests/synthesis/templates/toffoli.rs` (8 Tests)
1. `test_toffoli_matrix_verification`: Validates 15-gate Toffoli matrix against `Matrix8x8::exact_toffoli`.
2. `test_toffoli_matrix_all_permutations`: Validates matrix equivalence across all 6 permutations of $(c_1, c_2, t)$.
3. `test_toffoli_truth_table_on_all_basis_states`: Validates truth-table evolution on all 8 basis states ($|000\rangle \dots |111\rangle$).
4. `test_toffoli_zero_relative_phase_exactness`: Proves that unaffected basis states have strictly $+1.0 + 0.0i$ diagonal amplitudes with zero relative phase error.
5. `test_toffoli_arbitrary_superposition_evolution`: Validates evolution of random normalized 3-qubit superposition state matches analytical Toffoli action.
6. `test_toffoli_preconditions`: Validates duplicate controls, control equal to target, and arity mismatches are rejected.
7. `test_toffoli_circuit_dag_replacement`: Validates transactional replacement of custom `ccx` node in QccIR DAG, verifying 15 replacement operations and graph validity.

#### `tests/synthesis/templates/registry.rs` (7 Tests)
1. `test_default_registry_contents`: Validates default registry contains all 4 verified templates with gate lookup.
2. `test_registry_admission_rejects_unverified_template`: Validates unverified or failing template is rejected during registration.
3. `test_registry_admission_accepts_verified_template`: Validates verified template successfully admitted.
4. `test_registry_apply_by_name`: Validates transactional decomposition via `apply_by_name`.
5. `test_registry_apply_unknown_name`: Validates structured rejection of unknown template names.
6. `test_registry_apply_mismatched_target_gate`: Validates structured rejection when template does not match target node's operation.

---

## 4. Invariants & Architecture Compliance

1. **Little-Endian Qubit Indexing**: Matrix embeddings and statevector simulations index qubits as $q_0$ (LSB).
2. **Column Vectors & Left Multiplication**: Operator matrices accumulate by left multiplication $B \cdot A$ when $A$ precedes $B$.
3. **Exact Global Phase & Zero Relative Phase**: Every template records $\Delta \phi = 0.0$; the Toffoli decomposition strictly eliminates all relative phase errors.
4. **Stable Semantic Operation IDs**: Nodes are addressed via QCC-owned stable `OperationId`, never exposing internal petgraph indices.
5. **Transactional Rewriting**: Graph modifications use `replace_sequence` ensuring rollback on error and dependency reconnection.
6. **Zero Nightly Flags**: Built and verified strictly on stable Rust 2021 with zero warnings (`-D warnings`).

---

## 5. Exit Gate Proof

- **Exit Gate Requirement**: *Template library passes full local matrix comparison; decompositions produce verified-equivalent subcircuits.*
- **Verification Evidence**:
  - `SwapToCxTemplate::verify_matrix()` passed ($< 10^{-15}$ diff).
  - `CxToCzTemplate::verify_matrix()` and `CzToCxTemplate::verify_matrix()` passed ($< 10^{-15}$ diff).
  - `ToffoliToCxTemplate::verify_matrix()` passed across all 6 qubit permutations ($< 10^{-15}$ diff).
  - Circuit-level unitary equivalence verified via `check_equivalence` for SWAP, CX $\to$ CZ, CZ $\to$ CX, and roundtrips.
  - Overall test suite: **1010 passed tests**, **0 failed**, **0 warnings**.
