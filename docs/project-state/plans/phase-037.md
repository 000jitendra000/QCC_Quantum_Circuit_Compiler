# Phase 37 Implementation Plan — Generic Target Data Model

## 1. Context & Objectives

- **Phase Number**: Phase 37
- **Title**: Generic Target Data Model
- **Milestone**: Milestone M4: Generic Target and Basis Translation (Initial Phase of M4)
- **Roadmap Specification**: `docs/roadmap/phases-037-043.md#phase-037`
- **Objective**: Describe hardware without a fixed vendor-specific basis.
- **Prerequisites**: Phases 24–25 (Frontend & Lowering) and Phase 36 (Canonical Logical Gate Catalog) verified and complete.

---

## 2. Invariants & Semantic Contracts

1. **Hardware-Independent Target Model**: Target operations, coupling topologies, and directionalities must be dynamically resolved from `Target`. Compiler passes must never hardcode vendor-specific bases (e.g. IBM CX, Rigetti CZ, Quantinuum ZZ).
2. **Decoupled Operation Identity**: Target-native operation identity (`TargetOpId`) is strictly decoupled from the target-independent logical `GateId` catalog. Target operations can represent arbitrary native primitives (`cx`, `cz`, `ecr`, `sx`, `sy`, `fsim`, `rzz`, etc.) without modifying core logical gate definitions.
3. **Distinct Physical Qubit Identifier**: Logical qubits (`QubitId`) and physical device qubits (`PhysicalQubitId`) are strictly separated by distinct typed identifiers.
4. **Operation-Specific Directed & Undirected Connectivity**: Connectivity is per-operation. A symmetric undirected coupling (e.g. $CZ$) and an asymmetric directed coupling (e.g. $CX(q_{\text{ctrl}}, q_{\text{targ}})$) are strictly non-interchangeable.
5. **The Explicit Unknown Principle**: Missing timing or calibration metrics are modeled as `None` (`Unknown`), **never assumed or substituted with zero**.
6. **Full Provenance Tracking**: Calibration and timing entries maintain optional `Provenance` records (`source`, `timestamp`, `method`) that survive serialization.
7. **Deterministic Serialization & Validation**: JSON serialization order is deterministic. Deserialization strictly validates all physical qubit indices, arities, parameter domains, and topology bounds.
8. **Stable Rust Only**: Zero nightly flags, zero unverified unsafe code, zero warnings under `-D warnings`.

---

## 3. Architecture & Module Design

### 3.1 Module Hierarchy
Create `src/target/`:
- `src/target/mod.rs`: Top-level module re-exports and facade.
- `src/target/target.rs`: `Target`, `TargetBuilder`, physical qubit count, target-level queries.
- `src/target/operation.rs`: `PhysicalQubitId`, `TargetOpId`, `TargetOperationDescriptor`.
- `src/target/connectivity.rs`: `Directionality`, `OperationConnectivity`, `CouplingEdge`, `CouplingGraph`.
- `src/target/calibration.rs`: `Provenance`, `QubitCalibration`, `TupleCalibration`, `TargetCalibration`.
- `src/target/error.rs`: `TargetError` enum with complete validation diagnostics.

### 3.2 Types & Signatures

#### `src/target/operation.rs`
- `PhysicalQubitId(pub u32)`: Typed physical qubit index, formatting as `p[{}]`.
- `TargetOpId(pub String)`: Target operation mnemonic (e.g. `"cx"`, `"cz"`, `"ecr"`, `"sx"`, `"rz"`, `"measure"`).
- `TargetOperationDescriptor`:
  - `name: TargetOpId`
  - `quantum_arity: usize`
  - `parameter_domains: Vec<ParameterDomain>` (reusing `crate::gate_signature::ParameterDomain`)
  - `connectivity: OperationConnectivity`
  - `duration: Option<f64>` (default duration in seconds/nanoseconds)
  - `error_estimate: Option<f64>` (default infidelity $1 - F \in [0.0, 1.0]$)
  - `description: Option<String>`
- Methods: `is_single_qubit()`, `is_two_qubit()`, `is_parameterized()`, `is_directed()`, `is_valid_tuple(&[PhysicalQubitId])`.

#### `src/target/connectivity.rs`
- `Directionality`: `Directed` vs `Undirected`.
- `CouplingEdge`: `(control: PhysicalQubitId, target: PhysicalQubitId)`.
- `OperationConnectivity`:
  - `SingleQubit { qubits: BTreeSet<PhysicalQubitId> }` (or all)
  - `DirectedTwoQubit { edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)> }`
  - `UndirectedTwoQubit { edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)> }` (normalized $u < v$)
  - `AllToAll`
  - `General { tuples: BTreeSet<Vec<PhysicalQubitId>> }`
- `CouplingGraph`: Graph representation supporting neighbor queries, degree queries, edge validation.

#### `src/target/calibration.rs`
- `Provenance`:
  - `source: String`
  - `timestamp: Option<String>`
  - `method: Option<String>`
- `QubitCalibration`:
  - `t1: Option<f64>`
  - `t2: Option<f64>`
  - `readout_error: Option<f64>`
  - `readout_duration: Option<f64>`
  - `frequency: Option<f64>`
  - `provenance: Option<Provenance>`
- `TupleCalibration`:
  - `duration: Option<f64>`
  - `error_estimate: Option<f64>`
  - `provenance: Option<Provenance>`
- `TargetCalibration`:
  - `qubits: BTreeMap<PhysicalQubitId, QubitCalibration>`
  - `tuples: BTreeMap<(TargetOpId, Vec<PhysicalQubitId>), TupleCalibration>`

#### `src/target/target.rs`
- `Target`:
  - `name: String`
  - `num_qubits: usize`
  - `operations: BTreeMap<TargetOpId, TargetOperationDescriptor>`
  - `calibration: Option<TargetCalibration>`
  - `provenance: Option<Provenance>`
- Methods:
  - `validate(&self) -> Result<(), TargetError>`
  - `operation(&self, name: &str) -> Option<&TargetOperationDescriptor>`
  - `is_operation_supported(&self, name: &str) -> bool`
  - `is_valid_operand_tuple(&self, op_name: &str, operands: &[PhysicalQubitId]) -> bool`
  - `tuple_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Option<f64>`
  - `tuple_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Option<f64>`
  - `to_json(&self) -> Result<String, TargetError>`
  - `to_json_pretty(&self) -> Result<String, TargetError>`
  - `from_json(json_str: &str) -> Result<Self, TargetError>`
- `TargetBuilder`: Fluent construction API.

---

## 4. Test Strategy (`tests/target.rs`)

1. **Synthetic Target Fixtures**:
   - `synthetic_cx_directed`: Directed CX linear/grid target.
   - `synthetic_cz_undirected`: Undirected CZ square grid target.
   - `synthetic_ecr_target`: ECR native basis target.
   - `synthetic_mixed_target`: Target supporting CX, CZ, ECR, RZZ simultaneously with distinct connectivities.
2. **Operand Tuple Legality**:
   - Directed CX: $(0, 1)$ valid, $(1, 0)$ invalid.
   - Undirected CZ: both $(0, 1)$ and $(1, 0)$ valid.
   - ECR: $(1, 2)$ valid, $(2, 1)$ invalid.
   - Multi-family differentiation on identical physical qubits.
3. **Validation Rejections**:
   - Zero physical qubits (`num_qubits == 0`).
   - No operations declared.
   - Out-of-bounds physical qubit references.
   - Self-loop couplings `(p, p)`.
   - Arity mismatch (wrong number of qubits in tuple).
   - Duplicate qubits within an operand tuple.
   - Error estimates outside $[0.0, 1.0]$.
   - Negative or infinite durations.
4. **Explicit Unknown Principle**:
   - Uncalibrated target returns `None`, never zero.
   - Partial calibration returns `None` for unmeasured properties.
5. **Provenance Preservation**:
   - Source, timestamp, method survive round-trip JSON serialization.
6. **Deterministic Serialization & Validation**:
   - Identical JSON byte output across multiple serialization runs.
   - Deserialization of invalid JSON fails with structured error.
7. **Hardware Separation**:
   - `GateId` logical gate definitions remain completely untouched.

---

## 5. Verification Commands

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell && cargo run --example qasm`
7. Python `openqasm3` parser tests
