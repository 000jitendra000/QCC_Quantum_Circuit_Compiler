# Phase 36 Completion Report — Canonical Logical Gate Catalog

## 1. Executive Summary

- **Phase Number**: Phase 36
- **Phase Title**: Canonical Logical Gate Catalog
- **Milestone**: Milestone M3: Verified Logical Optimizer (**Final Phase of M3**)
- **Exit Gate**: Catalog is stable and covered by signature tests (**PASSED**)
- **Baseline Git Checkpoint**: `5283144` (`fix(phase-35): strengthen optimizer pipeline correctness`)
- **Status**: Completed and fully audited with 100% verification across all test targets.

Phase 36 establishes a single, stable, target-independent logical gate catalog for the Quantum Circuit Compiler (QCC). It separates abstract mathematical quantum operator meaning, arity, continuous parameter domains, operational category, and inverse metadata from hardware-native instruction sets (e.g. cross-resonance, native square-root Pauli, and vendor-specific coupling constraints).

---

## 2. Key Deliverables & Architecture

### 2.1 Canonical `GateId` Enum (`src/gate_signature.rs`)
Defines the authoritative set of 16 target-independent logical gates:
- **Fixed Single-Qubit Gates (9)**: `Id`, `X`, `Y`, `Z`, `H`, `S`, `Sdg`, `T`, `Tdg`.
- **Parameterized Single-Qubit Gates (4)**: `Rx`, `Ry`, `Rz`, `P`.
- **Fixed Two-Qubit Entanglers (3)**: `CX`, `CZ`, `SWAP`.

All 16 variants implement `Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Display`.

### 2.2 Gate Signature, Classification & Parameter Domains (`src/gate_signature.rs`)
- `GateClassification`:
  - `Logical(GateId)`: Universal canonical gate in the catalog.
  - `TargetNative`: Recognized vendor/hardware-native primitive.
  - `Unknown`: Unrecognized or custom gate (crucially, unknown gates are **never** assumed to be target-native).
- `ParameterDomain`:
  - `Angle4Pi`: $4\pi$ spinor periodicity ($R_x, R_y, R_z$). $R_k(2\pi) = -I$, $R_k(4\pi) = I$.
  - `Angle2Pi`: $2\pi$ periodicity with zero global-phase delta ($P$). $P(2\pi) = I$.
  - `UnconstrainedReal`: General real scalar $(-\infty, +\infty)$.
- `InverseKind`:
  - `SelfInverse`: $U^\dagger = U$, $U^2 = I$ (`Id`, `X`, `Y`, `Z`, `H`, `CX`, `CZ`, `SWAP`).
  - `Paired(GateId)`: Discrete symmetric inverse pairs ($S \leftrightarrow S^\dagger$, $T \leftrightarrow T^\dagger$).
  - `ParameterNegation`: $U(\vec{\theta})^\dagger = U(-\vec{\theta})$ ($R_x, R_y, R_z, P$).
  - `None`: Non-unitary operations or uncatalogued gates.
- `GateSignature`:
  Contains `id`, `name`, `aliases`, `quantum_arity`, `parameter_count`, `parameter_domains`, `category`, `inverse_kind`, `is_symmetric`, `operand_roles`, and `description`.

### 2.3 `GateCatalog` Query & Inspection API (`src/gate_catalog.rs`)
- `lookup(name: &str) -> Option<&'static GateSignature>`: Case-insensitive lookup matching canonical names and aliases (`cnot` $\to$ `cx`, `i` $\to$ `id`, `phase`/`u1` $\to$ `p`, `sdag` $\to$ `sdg`).
- `lookup_exact(name: &str) -> Option<&'static GateSignature>`: Exact canonical name match.
- `get(id: GateId) -> &'static GateSignature`: Infallible lookup by canonical `GateId`.
- `classify(name: &str) -> GateClassification`: Categorizes mnemonics into `Logical(GateId)`, `TargetNative`, or `Unknown`.
- `is_logical(name: &str) -> bool`: Checks if name matches a canonical logical gate or alias.
- `all_gates() -> &'static [GateSignature]`: Complete static slice of canonical gates.
- `is_supported(name: &str) -> bool`: Checks if name matches a canonical gate or alias.
- `is_self_inverse(name: &str) -> bool`: Checks if gate satisfies $U^2 = I$.
- `invert_gate_name(name: &str) -> Option<&'static str>`: Canonical inverse gate lookup.
- `matrix_for_gate_1q(id: GateId, params: &[f64]) -> Option<Matrix2x2>`: Evaluates single-qubit unitary matrices.
- `matrix_for_gate_2q(id: GateId) -> Option<Matrix4x4>`: Evaluates two-qubit unitary matrices under little-endian convention.
- Subdivided iterators: `single_qubit_gates()`, `two_qubit_gates()`, `parameterized_gates()`, `fixed_gates()`.
- Hardware boundary check: `is_target_native_candidate(name: &str) -> bool`.

### 2.4 Hardware Separation Policy
Vendor-specific or hardware-native instructions (such as `sx`, `sy`, `sxdg`, `sydg`, `rzx`, `rzz`, `rxx`, `ryy`, `ecr`, `iswap`, `fsim`, `cz_pulse`) are **strictly excluded** from the logical gate catalog. `GateCatalog::lookup` returns `None` for all target-native mnemonics, preventing premature vendor coupling in target-independent optimization passes.

### 2.5 Subsystem Integration & String Logic Elimination
All compiler and optimizer passes have been audited and updated to rely directly on `GateCatalog`:
- `src/ir/validation.rs`: `canonical_gate_signature` delegates to `GateCatalog::lookup`.
- `src/optimizer/algebra/inverses.rs`:
  - `is_self_inverse` and `invert_gate_name` delegate to `GateCatalog`.
  - `invert_operation` uses `GateCatalog::lookup` and `sig.inverse_kind` instead of manual string matching, properly handling `id` and `p`.
  - `are_inverses_with_config` checks `sig1.is_symmetric`, `sig1.inverse_kind`, and parameter domain matching (`Angle4Pi` vs `Angle2Pi`).
- `src/optimizer/passes/identity.rs`: Replaced hardcoded string matching with `GateCatalog::lookup`, `GateId::Id` for explicit identity, and `ParameterDomain` checks for rotation ($4\pi$) and phase ($2\pi$) cancellation.
- `src/optimizer/passes/cancellation.rs`: `categorize_gate` queries `GateCatalog::lookup` for `is_parameterized()`, `is_single_qubit()`, and `is_two_qubit()`.
- `src/optimizer/passes/one_qubit_fusion.rs`: Replaced 100 lines of manual string and matrix matching in `extract_single_qubit_matrix` with `GateCatalog::lookup` and `GateCatalog::matrix_for_gate_1q`.
- `src/optimizer/passes/commutation_reorder.rs`: Line 536 uses `GateCatalog::lookup` to check `sig.is_two_qubit() && sig.is_self_inverse()`.
- `src/commutation/pauli.rs`: `RotationAxis::from_gate_name` and `Pauli::from_gate_name` delegate to `GateCatalog::lookup`.
- `src/commutation/cx.rs`: `single_qubit_cx_commutation` and `single_qubit_cz_commutation` delegate to `GateCatalog::lookup`.
- Remaining string logic across the codebase is legitimately confined to parser token buffers, AST nodes, OpenQASM serialization emission, diagnostic messages, and QccIR operation storage.

### 2.6 Unitary vs Non-Unitary Operation Classification & Boundary
- **Catalog Boundary (`GateCatalog` / `GateId`)**:
  - The logical gate catalog strictly models reversible, norm-preserving unitary quantum gates ($U^\dagger U = I$).
  - Every entry in `CANONICAL_GATES` records `category: OperationCategory::Unitary`, and `GateSignature::is_unitary()` returns `true`.
  - `GateCatalog::is_unitary(name)` returns `true` for all 16 canonical gate names and recognized aliases (`cnot`, `u1`, etc.).
  - Non-unitary circuit operations (`measure`, `reset`, `barrier`, `classical`) are NOT gates, cannot be evaluated as unitary matrices, and are **strictly excluded** from `GateId` and `GateCatalog`. `GateCatalog::lookup` returns `None` and `GateCatalog::is_unitary` returns `false` for them.
- **Circuit / IR Boundary (`Operation` / `SemanticClass` / `OperationCategory`)**:
  - General circuit operations in QccIR are modeled by the `Operation` enum (`Gate`, `Measure`, `Reset`, `Barrier`, `Classical`).
  - `Operation` exposes explicit classification methods:
    - `category(&self) -> OperationCategory`: maps `Gate` to `OperationCategory::Unitary`, and `Measure`, `Reset`, `Barrier`, `Classical` to `OperationCategory::NonUnitary`.
    - `semantic_class(&self) -> SemanticClass`: maps to `SemanticClass::UnitaryGate`, `SemanticClass::Measurement`, `SemanticClass::Reset`, `SemanticClass::Barrier`, or `SemanticClass::ClassicalControl`.
    - `is_unitary(&self) -> bool`: returns `true` for `Gate` and `false` for `Measure`, `Reset`, `Barrier`, `Classical`.
  - This architecture cleanly satisfies Phase 36 Task 4 without creating fake unitary gates or polluting `GateId`.

---

## 3. Focused Correctness Audit Findings

### 3.1 Canonical Identity Adoption
- Duplicated gate-name string matching across `inverses.rs`, `identity.rs`, `cancellation.rs`, `one_qubit_fusion.rs`, `commutation_reorder.rs`, `pauli.rs`, and `cx.rs` has been refactored to use `GateCatalog` and `GateId`.
- No pass invents ad-hoc gate definitions or string tables; all passes reference the authoritative catalog.

### 3.2 Logical versus Target-Native Identity
- `GateClassification` provides a typed boundary: `Logical(GateId)`, `TargetNative`, and `Unknown`.
- Target-native operations cannot be constructed as `GateId` values.
- Unknown names return `GateClassification::Unknown` and `is_target_native_candidate(name) == false`, proving they are not falsely classified as target-native.
- Phase 36 uses `KNOWN_TARGET_NATIVE_CANDIDATES` as the compile-time candidate list; Phase 37 will introduce the full dynamic hardware target capability model.

### 3.3 Alias and Frontend Compatibility
- Internal catalog aliases (`cnot`, `i`, `phase`, `u1`, `sdag`) were traced through `parser`, `semantic`, and `lowering`:
  - `cnot`, `i`, `phase`, `sdag` are rejected by `OpenQasmFrontend::parse` with diagnostic `E0020` (unknown gate).
  - `u1` and `p` are rejected with diagnostic `E0004` (unsupported gate requiring decomposition).
- Internal catalog aliases do NOT silently expand the accepted OpenQASM subset. The documented unsupported status of `u1` and `p` in OpenQASM is strictly preserved.

### 3.4 Signature and Matrix Consistency
- All 16 catalog entries have unique IDs, unique canonical names, disjoint aliases, verified arities, parameter counts, parameter domains, and unitary categories.
- Evaluated matrices are unitary within $10^{-12}$.
- Parameterized gate boundaries and periodicities are strictly distinct:
  - $R_x, R_y, R_z$ have `ParameterDomain::Angle4Pi` ($R(2\pi) = -I \neq I$).
  - $P$ has `ParameterDomain::Angle2Pi` ($P(2\pi) = I$).

### 3.5 P Gate Scope
- `P` was included as the canonical arbitrary single-qubit phase gate $P(\lambda) = \text{diag}(1, e^{i\lambda})$.
- Relation to rotation: $P(\lambda) = e^{i\lambda/2} R_z(\lambda)$.
- Inverses: $P(\lambda)^\dagger = P(-\lambda)$.
- OpenQASM 3 frontend statements `p(...)` remain strictly rejected with `E0004` until Milestone M4 basis translation.

### 3.6 Unitary / Non-Unitary Classification Audit
- **Roadmap Task 4 Conformance**: All 16 canonical logical gates in `CANONICAL_GATES` are explicitly recorded with `OperationCategory::Unitary`.
- **Architectural Boundary**: Non-unitary operations (`Measure`, `Reset`, `Barrier`, `Classical`) are NOT gates, do not have a unitary matrix representation, and are strictly excluded from `GateId` and `GateCatalog`.
- **Unified Classification API**:
  - `Operation::category(&self) -> OperationCategory`
  - `Operation::semantic_class(&self) -> SemanticClass`
  - `Operation::is_unitary(&self) -> bool`
  - `GateCatalog::is_unitary(name: &str) -> bool`
- Verified that all non-unitary operations return `false` from `GateCatalog::is_unitary` and `None` from `GateCatalog::lookup`, while `Operation::Gate` returns `OperationCategory::Unitary` and all other `Operation` variants return `OperationCategory::NonUnitary`.

---

## 4. Verification & Test Evidence

### 4.1 Comprehensive Test Suite (`tests/gate_catalog.rs`)
32 dedicated integration tests verify all catalog contracts:
1. `test_canonical_gate_count_and_completeness`: Exactly 16 gates in `CANONICAL_GATES`.
2. `test_canonical_gate_id_uniqueness`: All `GateId` values and names are unique and match `Display`.
3. `test_aliases_are_disjoint_and_do_not_collide_with_canonical_names`: All aliases are disjoint.
4. `test_signature_internal_consistency`: Arities match operand roles, parameter counts match domains, all canonical gates are `OperationCategory::Unitary`.
5. `test_two_qubit_gate_operand_roles_and_symmetry`: CX has `[Control, Target]` and is asymmetric; CZ and SWAP have `[Target, Target]` and are symmetric.
6. `test_parameter_domains`: $4\pi$ spinor periodicity for $R_x, R_y, R_z$; $2\pi$ periodicity for $P$; empty domains for fixed gates.
7. `test_unitary_category_recording_and_non_unitary_rejection`: Every gate in `CANONICAL_GATES` and its aliases are unitary; non-unitary operations (`measure`, `reset`, `barrier`, `classical`) are excluded from `GateCatalog` and `GateId`.
8. `test_ir_operation_semantic_classification_and_category`: `Operation::Gate` maps to `UnitaryGate` / `Unitary`; `Measure`, `Reset`, `Barrier`, `Classical` map to their respective non-unitary semantic classes.
9. `test_self_inverse_gates`: `Id`, `X`, `Y`, `Z`, `H`, `CX`, `CZ`, `SWAP` confirmed self-inverse.
10. `test_paired_inverse_reciprocity`: $S \leftrightarrow S^\dagger$ and $T \leftrightarrow T^\dagger$ are mutually reciprocal pairs.
11. `test_parameter_negation_inverses`: $R_x, R_y, R_z, P$ invert under parameter negation.
12. `test_single_qubit_matrices_are_unitary`: Evaluated $2 \times 2$ matrices satisfy $U^\dagger U = I$ within $10^{-12}$.
13. `test_two_qubit_matrices_are_unitary`: Evaluated $4 \times 4$ matrices satisfy $U^\dagger U = I$ within $10^{-12}$.
14. `test_matrix_inverse_composition_exactness`: $U \cdot U_{\text{inv}} = I$ for all self-inverse, paired, and parameterized gates.
15. `test_cx_control_target_orientation_asymmetry`: $CX(0, 1) \neq CX(1, 0)$ verified under little-endian basis.
16. `test_cz_and_swap_symmetry_under_qubit_swap`: $SWAP \cdot CZ \cdot SWAP = CZ$ and $SWAP \cdot SWAP = I$.
17. `test_all_16_canonical_gate_ids_have_unambiguous_matrices`: Exhaustive verification that every one of the 16 `GateId` values yields a valid unitary matrix from either `matrix_for_gate_1q` (13 gates) or `matrix_for_gate_2q` (3 gates), with mutual exclusivity.
18. `test_target_native_instructions_are_rejected_from_logical_catalog`: `sx`, `sy`, `ecr`, `iswap`, `rzz`, `fsim` rejected by `lookup()`.
19. `test_logical_gates_are_not_flagged_as_target_native`: Canonical logical gates return `false` from `is_target_native_candidate`.
20. `test_case_insensitive_lookup`: Case-insensitivity and aliases (`cnot`, `i`, `phase`, `u1`, `s_dag`, `tdag`) resolve correctly.
21. `test_unknown_and_malformed_lookups`: Empty strings and invalid tokens return `None`.
22. `test_catalog_subdivision_iterators`: 13 single-qubit, 3 two-qubit, 4 parameterized, 12 fixed gates partition the 16 gates.
23. `test_ir_validation_canonical_gate_signature_integration`: `canonical_gate_signature` matches catalog signatures.
24. `test_optimizer_algebra_inverses_integration`: `is_self_inverse` and `invert_gate_name` match catalog metadata.
25. `test_gate_enum_bridge`: `Gate::gate_id()` correctly bridges all `Gate` variants.
26. `test_gate_classification_api`: Verifies `GateCatalog::classify` across logical, target-native, and unknown names.
27. `test_frontend_rejection_of_catalog_aliases_and_unsupported_gates`: Verifies `cnot`, `i`, `phase`, `sdag` return `E0020`, while `u1` and `p` return `E0004`.
28. `test_invert_operation_integration`: Verifies `invert_operation` on `id`, `p`, `u1`, `s`, and `cnot`.
29. `test_rotation_4pi_vs_phase_2pi_domain_distinction`: Proves $R_z(2\pi) = -I \neq I$, while $P(2\pi) = +I$, and $P(\lambda) = e^{i\lambda/2} R_z(\lambda)$.
30. `prop_rotations_unitarity_for_arbitrary_angles`: Property test over arbitrary angles in $[-100, 100]$.
31. `prop_rotations_inverse_composition`: Property test verifying $U(\theta) \cdot U(-\theta) = I$.
32. `prop_phase_gate_periodicity_2pi`: Property test verifying $P(\lambda + 2\pi) = P(\lambda)$.

### 4.2 Independent OpenQASM 3 Reference Parser Verification
Confirmed execution of external reference parser tests via `cargo test --test qasm3 test_independent -- --nocapture`:
- `lowering::test_independent_openqasm3_parser_on_qasm_example_artifact`: Invokes Python 3 with the pinned reference parser (`openqasm3` v1.0.1) on `examples/qasm/bell.qasm` (7 statements parsed successfully, version 3.0 confirmed).
- `roundtrip::test_independent_parser_presence_and_golden_fixtures_conformance`: Invokes Python 3 with `openqasm3` v1.0.1 across golden test fixtures (`bell.qasm`, `rotations.qasm`, etc.).
- Execution output:
  ```text
  running 2 tests
  test lowering::test_independent_openqasm3_parser_on_qasm_example_artifact ... ok
  test roundtrip::test_independent_parser_presence_and_golden_fixtures_conformance ... ok

  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 208 filtered out; finished in 3.55s
  ```

### 4.3 Full Verification Command Results
- `cargo fmt --check`: Passed with zero diffs.
- `cargo clippy --all-targets -- -D warnings`: Passed with zero warnings.
- `cargo test --all`: **933 tests passed across 28 test binaries** (0 failed, 0 ignored).
- `cargo build --all-targets`: Built cleanly in debug and test profiles.
- `cargo doc --no-deps`: Documentation generated with zero warnings.
- `cargo run --example bell && cargo run --example qasm`: Executed successfully.
- `cargo test --test qasm3`: All 210 OpenQASM 3 integration tests passed.

---

## 5. Milestone M3 Completion Status

With Phase 36 completed, audited, and verified, **Milestone M3: Verified Logical Optimizer** is **100% complete**:
- **Phase 28**: Identity Gate Elimination (Complete)
- **Phase 29**: Inverse Gate Pair Cancellation (Complete)
- **Phase 30**: Single-Qubit Rotation Merging (Complete)
- **Phase 31**: Single-Qubit Euler ZXZ/ZYZ Decomposition (Complete)
- **Phase 32**: One-Qubit Gate Fusion (Complete)
- **Phase 33**: Commutation Analysis Rules (Complete)
- **Phase 34**: DAG-aware Commutation Rewriting (Complete)
- **Phase 35**: Optimizer Integration and Reports (Complete)
- **Phase 36**: Canonical Logical Gate Catalog (Complete)

---

## 6. Next Steps

Do not begin Milestone M4 or write speculative target code until explicitly instructed.
The next scheduled phase is **Phase 37: Generic Target data model** in Milestone M4 (`M4`).
