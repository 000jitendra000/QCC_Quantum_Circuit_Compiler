# QCC Implementation Report — Phase 71: Per-Pass Diagnostic and Report Schema

```text
Milestone: M8 (Compiler Introspection and Diagnostics)
Phase: Phase 71
Title: Per-pass diagnostic and report schema
Roadmap Reference: Roadmap v3.1 (docs/roadmap/phases-060-085.md)
Prerequisite: Phase 59 pass manager/pipeline contracts; Phase 70 oracle result schema & M7 acceptance
Status: Completed & Verified
Date: 2026-11-13
```

## 1. Executive Summary & Prerequisite Verification

Phase 71 defines a versioned, machine-readable per-pass diagnostic and report schema capable of recording the effects, measurements, transformations, cryptographic fingerprints, and verification provenance of individual compiler passes in QCC.

### Prerequisite Audit:
1. **Milestone M7 (Phases 60–70) Acceptance**: Formally verified as completed and accepted at commit `27de63a` across 5 implementation sessions.
2. **Phase 70 Oracle Result Schema**: Verified intact at `tools/independent_oracle/schema/comparison_result.schema.json` (schema `qcc.comparison_result` v1.0.0).
3. **Phase 59 Pass Manager & Pipeline Contracts**: Verified intact in `src/compiler/pipeline.rs`, `src/compiler/result.rs`, `src/optimizer/report.rs`, and `src/opt/pass.rs`.

---

## 2. Schema Design and Field Semantics

The per-pass diagnostic report schema is registered under identifier `qcc.pass_diagnostic` with semantic version `1.0.0` (Draft 2020-12, `additionalProperties: false`):
- Location: `tools/independent_oracle/schema/pass_diagnostic.schema.json`
- Rust implementation: `src/compiler/pass_diagnostic.rs`

### Core Schema Sections:
1. **Pass Identity and Execution**:
   - `pass_id`: Stable machine-readable identifier (e.g. `"identity_elimination"`, `"sabre_routing"`).
   - `pass_name`: Human-readable display name.
   - `execution_order`: 0-based sequence index within the compiler/pipeline run.
   - `status`: Execution status enum (`"ran"`, `"skipped"`, `"failed"`).
   - `compiler`: Compiler package identity, version, build profile, target OS, architecture, and Git commit (`CompilerIdentityManifest`).
   - `pass_config`: Arbitrary pass configuration parameters/thresholds object.

2. **Circuit Fingerprints (`CircuitFingerprints`)**:
   - `algorithm`: Cryptographic hash algorithm (`"sha256"`).
   - `input_fingerprint` & `output_fingerprint`: Hexadecimal digests of canonical DAG serialization, or `null` if uncomputed/skipped.
   - `semantic_equivalence_guaranteed`: Explicit boolean invariant (always `false`), preventing any downstream tool or user from conflating structural fingerprint equivalence with mathematical unitary equivalence.

3. **Detailed Measurements and Availability Classification (`PassMeasurements`)**:
   - Before and after snapshots (`CircuitSnapshotMetrics`):
     - `total_gate_count`: Unitary gate count (or `null`).
     - `per_gate_counts`: Lowercase gate mnemonic histogram (or `null`).
     - `cx_count`: Two-qubit controlled-NOT gate count (or `null`).
     - `logical_swaps`: SWAPs declared in input logical circuit.
     - `router_swaps_inserted`: SWAPs inserted during hardware placement/routing.
     - `final_emitted_swaps`: SWAPs present in final emitted physical circuit.
     - `circuit_depth`: Critical path dependency depth in DAG.
     - `cost_metrics`: Target-dependent costs (e.g. latency, error, duration).
   - `duration_ms`: Nondeterministic execution timing telemetry in milliseconds (or `null` if skipped/unmeasured).
   - `availability`: Explicit availability classification mapping metric/snapshot names to:
     - `"available"`: Metric was evaluated and populated.
     - `"not_measured"`: Metric measurement was omitted or bypassed (e.g. telemetry disabled).
     - `"unavailable"`: Metric computation was attempted but data was missing or pass failed.
     - `"not_applicable"`: Metric is conceptually meaningless for this stage (e.g. routing metrics before placement, or post-metrics for skipped passes).
   - Prevents downstream consumers from conflating unmeasured, unavailable, and not applicable values, with zero metric fabrication.

4. **Transformation Accounting (`TransformationAccounting`)**:
   - `operations_added`: Count of operations inserted.
   - `operations_removed`: Count of operations deleted.
   - `operations_rewritten`: Count of operations modified/rewritten in-place.
   - `global_phase_adjustment`: Scalar global phase adjustment (`radians`, `expression`), or `null` if unmodified.
   - `analysis_invalidations`: List of compiler cache/analysis IDs invalidated by the pass mutations.

5. **Verification Provenance and Truthful Status (`PassVerification`)**:
   - `status`: Explicit verification status enum:
     - `"passed"`: Verifier satisfied its equivalence criterion within declared scope and numerical tolerances (records empirical agreement, not an axiomatic formal mathematical proof).
     - `"failed"`: Semantic mismatch or invariant violation detected.
     - `"not_run"`: Verification was not executed.
     - `"unsupported"`: Circuit contains features (e.g. non-unitary operations) outside the verifier's operational domain.
   - **Non-Negotiable Invariant**: `"not_run"` and `"unsupported"` must **never** be collapsed into or reported as `"passed"`.
   - `verifier_name`: Identifier of verifier engine (e.g. `"qcc_statevector_unitary"`, `"qiskit_independent_oracle"`).
   - `verifier_version`: Verifier engine version string.
   - `verifier_mode`: Mode (`"exhaustive_unitary"`, `"sampled_statevector"`).
   - `provenance`: Origin enum: `"internal"` (QCC in-tree Rust verifier) vs `"independent_oracle"` (external Qiskit black-box oracle).
   - `tolerances`: Map of numerical tolerances applied (`atol`, `rtol`, etc.).
   - `detail`: Diagnostic error message, discrepancy explanation, or unsupported reason.

---

## 3. Files Created and Modified

1. **JSON Schema**:
   - `tools/independent_oracle/schema/pass_diagnostic.schema.json`: Complete Draft 2020-12 schema (`qcc.pass_diagnostic` v1.0.0).
2. **Rust Data Models & Helpers**:
   - `src/compiler/pass_diagnostic.rs`: Strongly typed models (`PassDiagnosticReport`, `PassStatus`, `PassVerificationStatus`, `VerifierProvenance`, `CircuitFingerprints`, `CircuitSnapshotMetrics`, `PassMeasurements`, `TransformationAccounting`, `PassVerification`, `GlobalPhaseAdjustment`, `PassDiagnosticValidationError`).
   - `src/compiler/mod.rs`: Module declaration and public re-exports.
   - `src/lib.rs`: Crate-level re-exports.
3. **Architecture Documentation**:
   - `docs/pass-report-schema.md`: Complete specification of field semantics, mathematical/structural invariants, concrete valid/invalid JSON examples, and Rust API usage.
4. **Integration Tests**:
   - `tests/compiler/pass_diagnostic.rs`: 11 Rust integration tests covering all execution statuses, verification outcomes, invariant validations, and JSON round-trips.
   - `tests/compiler_end_to_end.rs`: Wiring of pass diagnostic test module.
   - `tools/independent_oracle/tests/test_pass_diagnostic_schema.py`: 14 Python test cases validating Draft 2020-12 schema compliance, acceptance of all valid states, and strict rejection of invariant violations.
   - `tools/independent_oracle/tests/test_oracle_session5.py`: Reconciled schema count check (`>= 7`).

---

## 4. Tests and Verification Evidence

- `cargo fmt --check`: Passed (clean formatting).
- `cargo clippy --all-targets -- -D warnings`: Passed (zero warnings, zero errors).
- `cargo doc --no-deps`: Passed (documentation built cleanly).
- `cargo test --test compiler_end_to_end pass_diagnostic`: 11 / 11 tests passed in 0.00s.
- `python3 -m unittest tools/independent_oracle/tests/test_pass_diagnostic_schema.py`: 14 / 14 tests passed in 0.021s.
- `python3 -m unittest discover -s tools/independent_oracle/tests`: 59 / 59 tests passed.

---

## 5. Compatibility Decisions

1. **Isolation of Python/Qiskit**: Zero Python or Qiskit runtime dependencies were introduced into `qcc-core` or Rust crates. Rust data models use standard `serde` and `serde_json`.
2. **Explicit Nulls over Omitted Fields**: Nullable fields in `CircuitSnapshotMetrics` and `PassVerification` are serialized as `null` when missing rather than omitted, ensuring strict schema conformance and predictable consumption across languages.
3. **Draft 2020-12 Standard**: Aligned with all existing QCC schemas in `tools/independent_oracle/schema/`.

---

## 6. Limitations and Scope Boundary

- **Phase 71 Scope Only**: Phase 71 defines the schema and its data models. Pass-manager instrumentation and runtime execution hooks are explicitly reserved for **Phase 72**. CLI presentation and formatting tables are explicitly reserved for **Phase 73**.
- Compiler optimization algorithms and compilation pipeline execution semantics were strictly left untouched.

---

## 7. Exit Gate Verification

> **EXIT GATE (Phase 71)**: A versioned, tested schema records pass effects, metrics, provenance and truthful verification status.

**Status**: **SATISFIED**.

