# Phase 38 Completion Report — Target Capability Queries and Cost Models

## 1. Executive Summary

- **Phase Number**: Phase 38
- **Phase Title**: Target capability queries and cost models
- **Milestone**: Milestone M4: Generic Target and Basis Translation
- **Exit Gate**: Capability and cost queries pass fixtures with partial metadata (**PASSED**)
- **Baseline Git Checkpoint**: `cb9e86b` (`feat(phase-37): implement generic target data model`)
- **Status**: Completed and fully verified with 100% test pass across all targets.

Phase 38 provides uniform target capability lookup, legality validation, and hardware cost modeling interfaces for the Quantum Circuit Compiler (QCC). Compiler stages (including basis translation, routing, synthesis, and scheduling) can now query target instruction sets, validate physical operand tuples and continuous parameter domains, look up execution durations and error rates, and evaluate critical-path depth and multi-objective circuit costs through a standardized interface.

In accordance with QCC core invariants and the **Explicit Unknown Principle**, missing calibration and timing metrics are never defaulted to zero or collapsed into arbitrary numeric constants; they are preserved as distinct tri-state metrics (`Measured`, `Estimated`, `Absent`) and yield structured [`DurationCost::Incomplete`] and [`ErrorCost::Incomplete`] diagnostics.

---

## 2. Key Deliverables & Architecture

### 2.1 Tri-State Hardware Metrics & Availability Classification (`src/target/metrics.rs`)
- **Qualitative Classification (`MetricKind`)**:
  - `MetricKind::Measured`: Empirically benchmarked on hardware via experimental characterization protocols (e.g. randomized benchmarking, spectroscopy).
  - `MetricKind::Estimated`: Nominal or data-sheet design specification.
- **Certainty Preserving Value (`MetricValue<T>`)**:
  - `MetricValue::Measured { value: T, provenance: Provenance }`: Characterized empirical data with explicit provenance (source, timestamp, method).
  - `MetricValue::Estimated { value: T, provenance: Option<Provenance> }`: Nominal design values or model-based estimations.
  - `MetricValue::Absent`: Missing, unmeasured, or uncharacterized data.
  - Methods: `is_absent()`, `is_measured()`, `is_estimated()`, `value()`, `into_value()`, `provenance()`, `kind()`, `map()`.
- **Circuit Duration Cost Representation (`DurationCost`)**:
  - `DurationCost::Exact { total_ns: f64, is_measured_only: bool }`: Calculated when all constituent operations have known timing.
  - `DurationCost::Incomplete { accumulated_known_ns: f64, uncalibrated_operations: usize }`: Returned when some operations lack timing metrics. Provides a rigorous lower bound while reporting the exact count of uncalibrated operations.
  - `DurationCost::Unknown`: Returned when zero timing metrics are available.
- **Circuit Error Cost Representation (`ErrorCost`)**:
  - `ErrorCost::Exact { infidelity_bound: f64, is_measured_only: bool }`: Exact upper bound on circuit infidelity.
  - `ErrorCost::Incomplete { known_infidelity: f64, uncalibrated_operations: usize }`: Incomplete infidelity from calibrated operations with exact uncalibrated count.
  - `ErrorCost::Unknown`: Returned when zero error estimates are available.

### 2.2 Target Capability Queries & Legality Validation (`src/target/query.rs`)
- **Structured Legality Errors (`LegalityError`)**:
  - `UnsupportedOperation(String)`: Operation mnemonic not present in target.
  - `ArityMismatch { operation, expected, actual }`: Operand count differs from declared quantum arity.
  - `QubitOutOfBounds { qubit, capacity }`: Physical qubit index exceeds target capacity.
  - `DuplicateOperandQubit { qubit, operands }`: Operand tuple contains repeated physical qubits (e.g. `cx(p[0], p[0])`).
  - `IllegalCoupling { operation, operands }`: Operand tuple violates coupling graph or asymmetric directionality.
  - `ParameterCountMismatch { operation, expected, actual }`: Parameter argument count differs from declared domain count.
  - `NonFiniteParameter { index, value }`: Parameter is `NaN`, `+inf`, or `-inf`.
  - `ParameterDomainViolation { index, domain, value }`: Parameter violates domain bounds.
- **Common Query Interface (`TargetQuery` trait & `Target` methods)**:
  - `supports_operation(&self, op_name: &str) -> bool`: Operation support lookup.
  - `supports_logical_gate(&self, gate_id: GateId) -> bool`: Canonical logical gate support query (with canonical name, alias resolution, and catalog reverse-lookup).
  - `check_operand_legality(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Result<(), LegalityError>`: Validates arity, capacity bounds, duplicate qubits, and coupling directionality.
  - `check_parameter_legality(&self, op_name: &str, params: &[f64]) -> Result<(), LegalityError>`: Validates parameter counts, finiteness, and parameter domain constraints (`Angle4Pi`, `Angle2Pi`, `UnconstrainedReal`).
  - `check_operation_legality(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> Result<(), LegalityError>`: Combined legality verification.
  - `is_legal(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> bool`: Convenience boolean predicate.
  - `query_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>`: Tri-state duration lookup (tuple calibration with provenance -> `Measured`, descriptor nominal duration -> `Estimated`, missing -> `Absent`).
  - `query_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>`: Tri-state error rate lookup.
  - `query_qubit_t1`, `query_qubit_t2`, `query_readout_error`, `query_readout_duration`, `query_qubit_frequency`: Physical qubit characterization metrics with tri-state certainty.

### 2.3 Hardware Cost Models & Critical Path Analysis (`src/target/cost.rs`)
- **Single Operation Cost (`OperationCost`)**:
  - `operation: TargetOpId`, `operands: Vec<PhysicalQubitId>`, `quantum_arity: usize`, `is_two_qubit: bool`, `duration: MetricValue<f64>`, `error_estimate: MetricValue<f64>`.
- **Aggregated Circuit Cost (`CircuitCost`)**:
  - `total_gates`: Total quantum gate count.
  - `single_qubit_gates`, `two_qubit_gates`, `multi_qubit_gates`: Gate count breakdown by arity.
  - `gate_counts: BTreeMap<TargetOpId, usize>`: Per-operation invocation histogram.
  - `depth: usize`: Critical path circuit depth in the dependency DAG.
  - `two_qubit_depth: usize`: Critical path two-qubit gate depth.
  - `duration: DurationCost`: Critical path execution duration (`Exact`, `Incomplete`, or `Unknown`).
  - `error: ErrorCost`: Infidelity bound (`Exact`, `Incomplete`, or `Unknown`).
- **Multi-Objective Cost Weights (`CostWeights`)**:
  - Configurable coefficients for `total_gates`, `two_qubit_gates`, `depth`, `two_qubit_depth`, `duration_ns`, `error_rate`.
  - Sensible defaults prioritizing two-qubit reductions and critical path depth.
- **Hardware Cost Model (`CostModel`)**:
  - `evaluate_operation`: Single operation cost evaluation.
  - `evaluate_physical_sequence`: Evaluates arbitrary candidate sequences of physical operations with automatic concurrency tracking across disjoint wires.
  - `evaluate_circuit`: Evaluates QccIR `Circuit` dependency DAG assuming 1-to-1 logical-to-physical mapping.
  - `evaluate_circuit_with_layout`: Evaluates QccIR `Circuit` dependency DAG under an explicit `layout: &BTreeMap<QubitId, PhysicalQubitId>`.
  - `calculate_score`: Computes scalar multi-objective score. Refuses evaluation (`None`) if duration or error is assigned a positive weight but is `Unknown`.

---

## 3. Verification & Test Evidence

### 3.1 Comprehensive Test Suite (18 New Dedicated Tests)

#### `tests/target_capabilities.rs` (9 Tests)
1. `test_supports_operation_native_and_unknown`: Validates native operations (`cx`, `cz`, `sx`, `rz`, `measure`) vs unrecognized ops (`ecr`, `iswap`, `fsim`).
2. `test_supports_logical_gate_bridging`: Validates bridging `GateId::CX`, `GateId::CZ`, `GateId::Rz` to target, while verifying unsupported logical gates (`GateId::H`, `GateId::SWAP`) return false.
3. `test_check_operand_legality_directed_vs_undirected`: Validates directed CX permits $(0, 1)$ and strictly rejects $(1, 0)$ with `LegalityError::IllegalCoupling`; verifies undirected CZ permits both orientations.
4. `test_check_operand_legality_arity_and_duplicates`: Validates structured rejection of arity mismatch (1Q passed 2 qubits, 2Q passed 1 qubit), duplicate physical qubits $(p_0, p_0)$, and out-of-bounds physical qubits ($p[99]$).
5. `test_check_parameter_legality`: Validates parameter count matching, parameter finiteness rejection (`NaN` and `+/-inf` produce `LegalityError::NonFiniteParameter`), and valid periodic domains.
6. `test_is_legal_convenience_predicate`: Validates end-to-end boolean legality checking.
7. `test_query_duration_measured_estimated_absent`: Validates tri-state duration lookup: empirical calibration with provenance yields `Measured`, descriptor duration yields `Estimated`, uncalibrated yields `Absent` (never 0.0).
8. `test_query_error_measured_estimated_absent`: Validates tri-state error rate lookup across measured, estimated, and absent states.
9. `test_query_qubit_level_metrics`: Validates $T_1$, $T_2$, readout error, readout duration, and frequency queries with provenance.

#### `tests/target_cost.rs` (9 Tests)
1. `test_evaluate_operation_cost`: Validates single operation cost breakdown for 1Q, 2Q, and uncalibrated gates.
2. `test_evaluate_empty_sequence`: Validates empty sequence evaluates to zero gates, zero depth, and exact 0.0 ns duration.
3. `test_parallel_concurrency_across_disjoint_qubits`: Proves that independent operations on disjoint qubits execute concurrently (depth = 1, duration = $\max(d_0, d_1) = 30.0$ ns, not 60.0 ns), while sequential operations on the same qubit serialize (depth = 2, duration = 60.0 ns).
4. `test_critical_path_depth_and_two_qubit_depth`: Proves DAG critical-path serialization when two 2Q gates share a qubit wire ($p_1$).
5. `test_interleaved_single_and_two_qubit_gates`: Proves critical-path depth (3) vs two-qubit depth (1) on alternating 1Q/2Q sequences.
6. `test_evaluate_circuit_dag`: Proves DAG-aware critical-path depth, 2Q depth, and duration on QccIR `Circuit` structures both with 1-to-1 mapping and explicit layout.
7. **`test_exit_gate_partial_metadata_fixtures` (Hard Exit Gate Proof)**:
   - Complete calibration fixture yields `DurationCost::Exact`.
   - Partial calibration fixture yields `DurationCost::Incomplete` with `accumulated_known_ns = 250.0` and `uncalibrated_operations = 1`.
   - Absent calibration fixture yields `DurationCost::Unknown` and `ErrorCost::Unknown`.
   - Strictly enforces that uncalibrated durations are never treated as 0.0 ns.
8. `test_multi_objective_score_calculation`: Validates weighted multi-objective scoring and verifies `calculate_score` returns `None` when duration is required but unknown.
9. `test_deterministic_cost_evaluation`: Validates that 100 repeated evaluations produce strictly bitwise-identical scores and metrics.

### 3.2 Full Verification Suite Results
- `cargo fmt --check`: **PASS** (zero formatting diffs)
- `cargo clippy --all-targets -- -D warnings`: **PASS** (zero warnings, zero errors)
- `cargo test --all`: **PASS** (977 passed tests across 31 test suites, 0 failed, 0 ignored)
- `cargo build --all-targets`: **PASS**
- `cargo doc --no-deps`: **PASS**
- `cargo run --example bell && cargo run --example qasm`: **PASS** (both examples produce expected statevector and OpenQASM 3 output)
- `PYTHONPATH=/tmp/py_packages cargo test --test qasm3 test_independent -- --nocapture`: **PASS** (2 passed in 3.53s)

---

## 4. Invariant Compliance Checklist

- [x] **Stable Rust Only**: Implemented strictly in stable Rust 2021 (no nightly features, zero unsafe code).
- [x] **Hardware-Independent Target Model**: Dynamic query interface with zero vendor-specific hardcoding.
- [x] **Decoupled Identity**: Target-native operation mnemonics (`TargetOpId`) kept distinct from `GateId`.
- [x] **Typed Physical Qubits**: Uses `PhysicalQubitId`, distinct from logical `QubitId`.
- [x] **Operation-Specific Directionality**: Enforces asymmetric coupling for directed gates (e.g. $CX$) and bidirectional coupling for symmetric gates (e.g. $CZ$).
- [x] **The Explicit Unknown Principle**: Missing calibration and timing metrics are preserved as `Absent`, `DurationCost::Incomplete`, or `DurationCost::Unknown`, never defaulted to zero.
- [x] **Tri-State Metric Certainty**: `Measured`, `Estimated`, and `Absent` data kept distinct with provenance.
- [x] **Deterministic Cost Evaluation**: Consistent tie-breaking and bitwise-reproducible evaluations across platforms.
- [x] **No Unrequested Scope Expansion**: Bounded strictly to capability queries and cost models; no speculative decomposition or routing passes introduced.
