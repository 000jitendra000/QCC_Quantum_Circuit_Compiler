# Phase 37 Completion Report — Generic Target Data Model

## 1. Executive Summary

- **Phase Number**: Phase 37
- **Phase Title**: Generic Target Data Model
- **Milestone**: Milestone M4: Generic Target and Basis Translation (**Initial Phase of M4**)
- **Exit Gate**: Synthetic target fixtures validate correctly (**PASSED**)
- **Baseline Git Checkpoint**: `3ec91ae` (`fix(phase-36): complete operation category classification`)
- **Status**: Completed and fully verified with 100% test pass across all targets.

Phase 37 establishes a provider-neutral, generic hardware target data model for the Quantum Circuit Compiler (QCC). It enables describing arbitrary quantum computing hardware (e.g. IBM-style transmon devices, Rigetti-style architectures, neutral-atom grids, and ion traps) without fixed vendor-specific bases. It explicitly decouples target-native operation identity from the core logical `GateId` catalog, supports operation-specific directed and undirected connectivity, enforces the Explicit Unknown Principle for calibration metrics, and provides deterministic JSON serialization with provenance preservation.

---

## 2. Key Deliverables & Architecture

### 2.1 Strongly Typed Physical Qubits (`src/target/operation.rs`)
- Introduced `PhysicalQubitId(pub u32)`, strictly separate from logical `QubitId` in `src/ir/wire.rs`.
- Formatted as `p[{}]` (e.g. `p[0]`, `p[1]`).
- Supports conversion from `u32` and `usize`, `PartialOrd`, `Ord`, `Hash`, `Serialize`, `Deserialize`.

### 2.2 Decoupled Target-Native Operation Identity (`src/target/operation.rs`)
- Introduced `TargetOpId(pub String)` representing target-native operation mnemonics (such as `"cx"`, `"cz"`, `"ecr"`, `"sx"`, `"sy"`, `"fsim"`, `"rzz"`, `"measure"`).
- Target-native operations are strictly decoupled from the target-independent logical `GateId` catalog (which remains fixed at 16 universal gates).
- `TargetOperationDescriptor`:
  - `name: TargetOpId`: Operation mnemonic.
  - `quantum_arity: usize`: Number of physical qubits acted upon.
  - `parameter_domains: Vec<ParameterDomain>`: Parameter domain constraints (`Angle4Pi`, `Angle2Pi`, `UnconstrainedReal`).
  - `connectivity: OperationConnectivity`: Allowed physical operand tuples and directionality constraints.
  - `duration: Option<f64>`: Optional nominal duration (ns).
  - `error_estimate: Option<f64>`: Optional nominal infidelity ($1 - \mathcal{F} \in [0.0, 1.0]$).
  - `description: Option<String>`: Optional human-readable documentation.
- Fluent builder: `TargetOperationBuilder`.

### 2.3 Operation-Specific Connectivity & Directionality (`src/target/connectivity.rs`)
- `Directionality`: `Directed` vs `Undirected`.
- `CouplingEdge`: Physical coupling edge between `(source, target)`.
- `OperationConnectivity`:
  - `SingleQubit { qubits: BTreeSet<PhysicalQubitId> }`: Single-qubit operations.
  - `DirectedTwoQubit { edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)> }`: Asymmetric directed operations where $(u, v)$ does not permit $(v, u)$ (e.g. $CX$).
  - `UndirectedTwoQubit { edges: BTreeSet<(PhysicalQubitId, PhysicalQubitId)> }`: Symmetric undirected operations where $\{u, v\}$ permits both $(u, v)$ and $(v, u)$ (e.g. $CZ$).
  - `AllToAll`: Fully connected connectivity.
  - `General { tuples: BTreeSet<Vec<PhysicalQubitId>> }`: Explicit multi-qubit tuples.
- `CouplingGraph`: Physical device topology representation with neighbor traversal, degree calculation, connected-component analysis, and standard constructors (`linear_chain`, `grid_2d`, `all_to_all`).

### 2.4 Calibration, Timing, and Provenance Tracking (`src/target/calibration.rs`)
- `Provenance`:
  - `source: String`: Source description (e.g. `"cryogenic_fridge_snapshot_cal_v3"`, `"datasheet"`).
  - `timestamp: Option<String>`: Optional ISO 8601 wall-clock timestamp.
  - `method: Option<String>`: Characterization technique (e.g. `"randomized_benchmarking"`, `"ramsey"`).
- `QubitCalibration`:
  - Qubit metrics: $T_1$, $T_2$, readout error, readout duration, operational frequency, with optional `Provenance`.
- `TupleCalibration`:
  - Tuple-specific metrics: gate duration, infidelity estimate, with optional `Provenance`.
- `TargetCalibration`:
  - `qubits: Vec<QubitCalibrationEntry>`: Qubit calibration entries (sorted by physical qubit for deterministic JSON serialization).
  - `tuples: Vec<TupleCalibrationEntry>`: Per-operation, per-operand-tuple calibration entries (sorted deterministically).
- **The Explicit Unknown Principle**:
  - Missing calibration and timing metrics are modeled strictly as `None` (`Unknown`), **never assumed or substituted with zero**.

### 2.5 Top-Level `Target` Model & Validation (`src/target/model.rs`)
- `Target`:
  - `name: String`: Target name.
  - `num_qubits: usize`: Physical qubit capacity.
  - `operations: BTreeMap<TargetOpId, TargetOperationDescriptor>`: Supported operations map.
  - `calibration: Option<TargetCalibration>`: Optional calibration database.
  - `provenance: Option<Provenance>`: Target definition provenance.
- Validation:
  - Enforces `num_qubits > 0` and non-empty operations.
  - Validates all physical qubit indices against capacity.
  - Validates quantum arity ($1 \le \text{arity} \le \text{num\_qubits}$).
  - Validates that operand tuples contain distinct physical qubits (no duplicate qubits like $(p_0, p_0)$).
  - Validates that coupling edges are not self-loops.
  - Validates that error estimates are finite and in $[0.0, 1.0]$.
  - Validates that durations are finite and non-negative.
  - Validates that calibration data references only declared operations.
- Serialization:
  - Deterministic JSON emission (`to_json`, `to_json_pretty`).
  - Validating deserialization (`from_json`).

---

## 3. Verification & Test Evidence

### 3.1 Comprehensive Test Suite (`tests/target.rs`)
26 dedicated integration tests verify all target contracts:
1. `test_synthetic_cx_directed_target_connectivity`: Validates directed linear chain; $(0, 1)$ valid, $(1, 0)$ rejected.
2. `test_synthetic_cz_undirected_target_connectivity`: Validates undirected ring; both $(0, 1)$ and $(1, 0)$ valid.
3. `test_synthetic_ecr_native_target`: Validates ECR basis; distinct from CX and CZ.
4. `test_synthetic_mixed_multi_family_target`: Validates coexisting CX, CZ, ECR, RZZ operations with distinct connectivities and parameter domains.
5. `test_operand_tuple_arity_mismatch`: Validates rejection of wrong operand counts.
6. `test_operand_tuple_duplicate_physical_qubits_rejected`: Validates rejection of $(p_0, p_0)$.
7. `test_operand_tuple_out_of_bounds_physical_qubit`: Validates rejection of physical qubits $\ge n$.
8. `test_validation_rejects_zero_physical_qubits`: Rejects `num_qubits == 0`.
9. `test_validation_rejects_empty_operations`: Rejects targets without operations.
10. `test_validation_rejects_arity_zero`: Rejects `arity == 0`.
11. `test_validation_rejects_arity_exceeding_capacity`: Rejects `arity > num_qubits`.
12. `test_validation_rejects_out_of_bounds_coupling_reference`: Rejects out-of-bounds coupling edges.
13. `test_validation_rejects_self_loop_coupling`: Rejects coupling edges $(p, p)$.
14. `test_validation_rejects_invalid_error_estimate`: Rejects errors $< 0.0$ or $> 1.0$.
15. `test_validation_rejects_negative_or_nan_duration`: Rejects negative or NaN durations.
16. `test_validation_rejects_calibration_referencing_unknown_operation`: Rejects undeclared operation calibrations.
17. `test_missing_timing_and_calibration_remains_explicitly_unknown`: Missing metrics return `None`, never zero.
18. `test_partial_calibration_returns_none_for_unmeasured_tuples`: Unmeasured tuples return `None`, never zero.
19. `test_provenance_metadata_survives_round_trip`: Full provenance survives JSON round-trip serialization.
20. `test_deterministic_serialization_reproducibility`: JSON output is byte-for-byte identical across runs.
21. `test_round_trip_serialization_fidelity`: Deserialization exactly reproduces original targets.
22. `test_deserialization_rejects_invalid_target_json`: Deserialization of invalid JSON fails closed.
23. `test_coupling_graph_linear_chain`: Validates linear chain neighbor, degree, and connectivity algorithms.
24. `test_coupling_graph_grid_2d`: Validates 2D grid neighbor and degree properties.
25. `test_target_combined_coupling_graph`: Validates multi-operation combined coupling graph extraction.
26. `test_target_native_primitives_require_zero_logical_catalog_changes`: Proves vendor-native operations (`fsim`, `rzz`, `ecr`, `sx`) are modeled without touching `GateId` or `GateCatalog`.

### 3.2 Verification Commands
- `cargo fmt --check`: Passed with zero diffs.
- `cargo clippy --all-targets -- -D warnings`: Passed with zero warnings.
- `cargo test --all`: **959 tests passed across 29 test binaries** (0 failed, 0 ignored).
  - Reconciled against 933-test baseline:
    - +26 integration tests in `tests/target.rs`
    - 933 + 26 = **959 tests total**.
- `cargo build --all-targets`: Built cleanly in debug and test profiles.
- `cargo doc --no-deps`: Documentation generated with zero warnings.
- `cargo run --example bell && cargo run --example qasm`: Executed successfully.
- `cargo test --test qasm3`: All 210 OpenQASM 3 integration tests passed (including external reference parser).

---

## 4. Next Steps

Do not begin Phase 38 until explicitly instructed.
The next scheduled phase is **Phase 38: Target capability queries and cost models** in Milestone M4 (`M4`).
