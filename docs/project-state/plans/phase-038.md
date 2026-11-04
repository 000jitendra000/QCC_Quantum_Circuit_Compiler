# Phase 38 Implementation Plan — Target Capability Queries and Cost Models

## 1. Context & Objectives

- **Phase Number**: Phase 38
- **Title**: Target Capability Queries and Cost Models
- **Milestone**: Milestone M4: Generic Target and Basis Translation
- **Roadmap Specification**: `docs/roadmap/phases-037-043.md#phase-038`
- **Objective**: Provide uniform target lookup and cost interfaces so compiler stages can query target data through a common interface.
- **Prerequisites**: Phase 37 (Generic Target Data Model) verified and complete at checkpoint `cb9e86b`.

---

## 2. Invariants & Semantic Contracts

1. **Common Query Interface**: All compiler stages (basis translation, routing, scheduling) query target capabilities, parameter constraints, and operand directionalities through uniform interfaces.
2. **Explicit Unknown Principle**: Missing timing, calibration, or error metrics must remain strictly unknown (`MetricValue::Absent` or `DurationCost::Incomplete`/`Unknown`), **never fabricated or substituted with zero**.
3. **Tri-State Metric Certainty**: Keep **measured**, **estimated**, and **absent** data distinct:
   - `Measured`: Empirically characterized on hardware via experimental protocols (with provenance).
   - `Estimated`: Nominal or data-sheet design specification.
   - `Absent`: Missing or uncharacterized.
4. **Operation-Specific Directionality**: Legality checks must enforce asymmetric directionality for directed operations (e.g. $(0, 1)$ valid does not imply $(1, 0)$ valid for $CX$).
5. **Deterministic Cost Computation**: Cost evaluations (gate counts, circuit depth, 2Q depth, duration, and multi-objective weighted costs) must be strictly deterministic across platforms.
6. **No Speculative Basis Translation**: Keep this phase focused strictly on query and cost models. Do not implement decomposition libraries or routing passes.
7. **Stable Rust Only**: Zero nightly flags, zero unverified unsafe code, zero warnings under `-D warnings`.

---

## 3. Architecture & Module Design

### 3.1 Module Hierarchy
Under `src/target/`:
- `src/target/metrics.rs`: `MetricValue<T>`, `MetricKind`, `DurationCost`, `ErrorCost`.
- `src/target/query.rs`: Legality validation, parameter domain verification, capability queries, `LegalityError`.
- `src/target/cost.rs`: `CostModel`, `CircuitCost`, `OperationCost`, `CostWeights`, critical-path depth and duration evaluation.
- `src/target/mod.rs`: Re-export all query, metrics, and cost types.
- `src/lib.rs`: Expose primary target query and cost interfaces.

### 3.2 Types & Signatures

#### `src/target/metrics.rs`
- `MetricKind`: `Measured` vs `Estimated`.
- `MetricValue<T>`:
  - `Measured { value: T, provenance: Provenance }`
  - `Estimated { value: T, provenance: Option<Provenance> }`
  - `Absent`
  - Methods: `is_absent()`, `is_measured()`, `is_estimated()`, `value()`, `into_value()`, `provenance()`, `map()`.
- `DurationCost`:
  - `Exact { total_ns: f64, is_measured_only: bool }`
  - `Incomplete { accumulated_known_ns: f64, uncalibrated_operations: usize }`
  - `Unknown`
- `ErrorCost`:
  - `Exact { infidelity_bound: f64, is_measured_only: bool }`
  - `Incomplete { known_infidelity: f64, uncalibrated_operations: usize }`
  - `Unknown`

#### `src/target/query.rs`
- `LegalityError`:
  - `UnsupportedOperation(String)`
  - `ArityMismatch { operation: String, expected: usize, actual: usize }`
  - `QubitOutOfBounds { qubit: PhysicalQubitId, capacity: usize }`
  - `DuplicateOperandQubit { qubit: PhysicalQubitId, operands: Vec<PhysicalQubitId> }`
  - `IllegalCoupling { operation: String, operands: Vec<PhysicalQubitId> }`
  - `ParameterCountMismatch { operation: String, expected: usize, actual: usize }`
  - `NonFiniteParameter { index: usize, value: f64 }`
  - `ParameterDomainViolation { index: usize, domain: ParameterDomain, value: f64 }`
- Query methods on `Target`:
  - `is_supported(&self, op_name: &str) -> bool`
  - `supports_logical_gate(&self, gate_id: GateId) -> bool`
  - `check_operand_legality(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Result<(), LegalityError>`
  - `check_parameter_legality(&self, op_name: &str, params: &[f64]) -> Result<(), LegalityError>`
  - `check_operation_legality(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> Result<(), LegalityError>`
  - `is_legal(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> bool`
  - `query_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>`
  - `query_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>`
  - `query_qubit_t1(&self, qubit: PhysicalQubitId) -> MetricValue<f64>`
  - `query_qubit_t2(&self, qubit: PhysicalQubitId) -> MetricValue<f64>`
  - `query_readout_error(&self, qubit: PhysicalQubitId) -> MetricValue<f64>`

#### `src/target/cost.rs`
- `OperationCost`:
  - `name: TargetOpId`
  - `operands: Vec<PhysicalQubitId>`
  - `is_two_qubit: bool`
  - `duration: MetricValue<f64>`
  - `error_estimate: MetricValue<f64>`
- `CircuitCost`:
  - `total_gates: usize`
  - `single_qubit_gates: usize`
  - `two_qubit_gates: usize`
  - `gate_counts: BTreeMap<TargetOpId, usize>`
  - `depth: usize`
  - `two_qubit_depth: usize`
  - `duration: DurationCost`
  - `error: ErrorCost`
- `CostWeights`:
  - Weights for gate count, 2Q gate count, depth, 2Q depth, duration, and error.
- `CostModel`:
  - `evaluate_operation(&target, op_name, operands) -> OperationCost`
  - `evaluate_physical_sequence(&target, &[(op_name, operands)]) -> CircuitCost`
  - `evaluate_circuit(&target, &circuit) -> CircuitCost`
  - `calculate_score(&self, cost: &CircuitCost) -> Option<f64>`

---

## 4. Test Strategy (`tests/target_capabilities.rs` & `tests/target_cost.rs`)

1. **Operation Support Queries**:
   - Verify native operations report as supported (`cx`, `cz`, `ecr`, `sx`, `rz`).
   - Verify uncatalogued operations return `false`.
   - Verify logical gate queries bridge correctly (`GateId::CX`, `GateId::CZ`, etc.).
2. **Legality & Constraint Validation**:
   - Directionality: Directed CX $(0, 1)$ passes; $(1, 0)$ fails with `IllegalCoupling`.
   - Symmetry: Undirected CZ permits both $(0, 1)$ and $(1, 0)$.
   - Duplicate operands: $(0, 0)$ fails with `DuplicateOperandQubit`.
   - Arity mismatch: Passing 1 qubit to 2Q op fails with `ArityMismatch`.
   - Parameter count mismatch: Passing 0 params to `rx` fails with `ParameterCountMismatch`.
   - Non-finite parameters: Passing `NaN` or `inf` fails with `NonFiniteParameter`.
3. **Tri-State Metrics (Measured vs Estimated vs Absent)**:
   - Qubit / tuple with calibration and provenance returns `MetricValue::Measured`.
   - Operation descriptor nominal duration without tuple calibration returns `MetricValue::Estimated`.
   - Uncalibrated operation / tuple returns `MetricValue::Absent` (never 0.0).
4. **Cost Interface Verification**:
   - Gate count breakdown (total, 1Q, 2Q, multi-Q).
   - Critical path depth and 2Q depth.
   - Exact duration when all operations have calibration.
   - Incomplete / Unknown duration when any operation lacks duration.
   - Deterministic tie-breaking across repeated evaluations.
5. **Partial Target Metadata**:
   - Fixture with partial calibration (only some tuples calibrated) correctly yields `Incomplete` duration cost with exact missing count.

---

## 5. Verification Commands

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell && cargo run --example qasm`
7. Python `openqasm3` parser tests
