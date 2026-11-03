# Phase 36 Implementation Plan — Canonical Logical Gate Catalog

## 1. Context & Objectives

- **Phase Number**: Phase 36
- **Title**: Canonical Logical Gate Catalog
- **Milestone**: Milestone M3: Verified Logical Optimizer (Final Phase of M3)
- **Roadmap Specification**: `docs/roadmap/phases-028-036.md#phase-036`
- **Objective**: Separate abstract logical gate meaning from hardware-native instructions through one stable, target-independent catalog.

## 2. Invariants & Semantic Contracts

1. **Target-Independent Primitives**: The catalog contains only universal logical gates (`Id`, `X`, `Y`, `Z`, `H`, `S`, `Sdg`, `T`, `Tdg`, `Rx`, `Ry`, `Rz`, `P`, `CX`, `CZ`, `SWAP`). Vendor-specific or hardware-native operations (e.g. `sx`, `sy`, `ecr`, `iswap`, `rzz`, `fsim`) must not be accepted as universal logical gates.
2. **Dirac Notation & Little-Endian Indexing**: Basis vector indexing follows little-endian ordering where $q_0$ is LSB. For two-qubit gates, operand 0 is control and operand 1 is target for `CX`.
3. **Parameter Domains**: Single-qubit rotations ($R_x, R_y, R_z$) operate on $4\pi$ spinor periodicity (`ParameterDomain::Angle4Pi`), phase gates operate on $2\pi$ periodicity (`ParameterDomain::Angle2Pi`), and unconstrained scalars are typed as `ParameterDomain::UnconstrainedReal`.
4. **Inverse Semantics**: Inverses are mathematically verified: self-inverses satisfy $U^2 = I$, discrete paired gates invert each other ($S \leftrightarrow S^\dagger$, $T \leftrightarrow T^\dagger$), and parameterized gates invert via parameter negation $U(\vec{\theta})^\dagger = U(-\vec{\theta})$.
5. **No Nightly Features**: Implement strictly in stable Rust 2021 edition. Zero warnings under `-D warnings`.

## 3. Architecture & Module Design

### 3.1 `src/gate_signature.rs`
- `GateId`: Canonical enum of logical gate variants:
  - Fixed 1-qubit: `Id`, `X`, `Y`, `Z`, `H`, `S`, `Sdg`, `T`, `Tdg`.
  - Parameterized 1-qubit: `Rx`, `Ry`, `Rz`, `P`.
  - Fixed 2-qubit: `CX`, `CZ`, `SWAP`.
- `ParameterDomain`: Enum specifying continuous parameter constraints (`Angle4Pi`, `Angle2Pi`, `UnconstrainedReal`).
- `InverseKind`: Enum specifying exact mathematical inverse behavior (`SelfInverse`, `Paired(GateId)`, `ParameterNegation`, `None`).
- `GateSignature`: Comprehensive struct capturing:
  - `id: GateId`
  - `name: &'static str`
  - `aliases: &'static [&'static str]`
  - `quantum_arity: usize`
  - `parameter_count: usize`
  - `parameter_domains: &'static [ParameterDomain]`
  - `category: OperationCategory` (all unitary)
  - `inverse_kind: InverseKind`
  - `is_symmetric: bool`
  - `operand_roles: &'static [OperandRole]`
  - `description: &'static str`
- Inspection methods: `is_single_qubit()`, `is_two_qubit()`, `is_parameterized()`, `is_unitary()`, `is_self_inverse()`, `matches_name(name: &str)`.

### 3.2 `src/gate_catalog.rs`
- `GateCatalog`: Centralized catalog defining all 16 canonical logical gate signatures.
- Methods:
  - `lookup(name: &str) -> Option<&'static GateSignature>` (case-insensitive, matches canonical name or aliases).
  - `lookup_exact(name: &str) -> Option<&'static GateSignature>` (exact canonical name match).
  - `get(id: GateId) -> &'static GateSignature`.
  - `all_gates() -> &'static [GateSignature]`.
  - `is_canonical_name(name: &str) -> bool`.
  - `is_supported(name: &str) -> bool`.
  - `is_self_inverse(name: &str) -> bool`.
  - `invert_gate_name(name: &str) -> Option<&'static str>`.
  - `matrix_for_gate_1q(id: GateId, params: &[f64]) -> Option<Matrix2x2>`.
  - `matrix_for_gate_2q(id: GateId) -> Option<Matrix4x4>`.
  - Iterators: `single_qubit_gates()`, `two_qubit_gates()`, `parameterized_gates()`, `fixed_gates()`.
  - Hardware boundary: `is_target_native_candidate(name: &str) -> bool`.

### 3.3 System Integration
- `src/quantum/gate_matrix.rs`:
  - Add `matrix_id() -> Matrix2x2` (identity).
  - Add `matrix_p(lambda: Angle) -> Matrix2x2` (phase rotation).
- `src/quantum/gate.rs`:
  - Add `Gate::gate_id(&self) -> GateId` and `Gate::from_id(id: GateId, params: &[Angle]) -> Option<Gate>`.
- `src/ir/validation.rs`:
  - Delegate `canonical_gate_signature` to `GateCatalog::lookup`.
- `src/optimizer/algebra/inverses.rs`:
  - Delegate `is_self_inverse` and `invert_gate_name` to `GateCatalog`.
- `src/lib.rs`:
  - Expose `pub mod gate_catalog;` and `pub mod gate_signature;`.
  - Re-export `GateCatalog`, `GateId`, `GateSignature`, `ParameterDomain`, `InverseKind`.

## 4. Test Strategy (`tests/gate_catalog.rs`)

1. **Unique IDs & Names**: All `GateId` variants have distinct values, unique canonical names, and disjoint aliases.
2. **Signature Internal Consistency**:
   - `quantum_arity` equals `operand_roles.len()`.
   - `parameter_count` equals `parameter_domains.len()`.
   - `is_single_qubit()` matches `quantum_arity == 1`.
   - `is_two_qubit()` matches `quantum_arity == 2`.
   - `is_parameterized()` matches `parameter_count > 0`.
3. **Unitary Category**: All canonical logical gates are `OperationCategory::Unitary`.
4. **Inverse Consistency**:
   - Paired gates are strictly symmetric pairs ($A \to B \implies B \to A$).
   - Self-inverse gates match `is_self_inverse() == true`.
5. **Matrix Cross-Check**:
   - Evaluated matrices are unitary ($U^\dagger U = I$) within $10^{-12}$.
   - Evaluated inverses satisfy $U \cdot U^\dagger = I$ within $10^{-12}$.
   - Controlled gate asymmetry ($CX(0, 1) \neq CX(1, 0)$) and symmetry ($CZ(0, 1) = CZ(1, 0)$, $SWAP(0, 1) = SWAP(1, 0)$).
6. **Hardware Separation**:
   - Target-native operations (`sx`, `sy`, `ecr`, `iswap`, `rzz`, `fsim`) return `None` from `GateCatalog::lookup`.
   - Recognized by `is_target_native_candidate`.
7. **Existing System Compatibility**:
   - Verify `ir::validation::canonical_gate_signature` matches catalog signatures.
   - Verify `optimizer::algebra::inverses` matches catalog inverse properties.

## 5. Verification Commands

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell && cargo run --example qasm`
7. Reference OpenQASM 3 parser regression check.
