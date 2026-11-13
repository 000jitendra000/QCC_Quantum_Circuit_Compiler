# QCC Per-Pass Diagnostic and Report Schema Specification

```text
Document: docs/pass-report-schema.md
Roadmap: QCC Engineering Roadmap v3.1 (Milestone M8 — Phase 71)
Schema Identifier: qcc.pass_diagnostic
Schema Version: 1.0.0
JSON Schema Standard: Draft 2020-12
```

## 1. Overview and Purpose

In high-assurance quantum circuit compilation, aggregate end-to-end metrics alone are insufficient to diagnose compiler behavior, detect subtle bugs, or verify pass isolation. A compiler pipeline may pass end-to-end verification even when individual passes perform unintended transformations, degrade circuit depth, invalidate analyses without notice, or mask upstream errors.

**Phase 71** introduces a stable, machine-readable, versioned per-pass diagnostic and report schema:
- Schema ID: `qcc.pass_diagnostic`
- Semantic Version: `1.0.0`
- JSON Schema: [`tools/independent_oracle/schema/pass_diagnostic.schema.json`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/schema/pass_diagnostic.schema.json)
- Rust Implementation: [`qcc::compiler::pass_diagnostic`](file:///home/ronanki_jitendra/rust/QCC/src/compiler/pass_diagnostic.rs)

This schema records:
1. Exactly what each compiler pass executed, skipped, or failed.
2. Complete before-and-after quantitative snapshots of the circuit DAG.
3. Explicit DAG mutation accounting (operations added, removed, rewritten, global phase adjustments, analysis invalidations).
4. Structural cryptographic fingerprints (SHA-256) of input and output circuits with explicit canonical serialization rules.
5. Explicit, truthful verification status and provenance distinguishing in-tree internal verifiers from external independent oracles.
6. Explicit measurement availability classification distinguishing unmeasured, unavailable, and not applicable metrics without metric fabrication.

---

## 2. Core Non-Negotiable Invariants

### Invariant 1: Fingerprint Equivalence != Semantic Equivalence
Input and output fingerprints (`input_fingerprint`, `output_fingerprint`) are cryptographic SHA-256 hashes of canonical circuit representations.
- Two identical fingerprints indicate that the pass produced bit-for-bit identical serialization (e.g. an idempotent or no-op pass).
- **Fingerprint equality does NOT prove mathematical unitary equivalence.**
- **Fingerprint inequality does NOT imply semantic inequivalence.** (E.g. gate commutation or rotation merging alters the fingerprint while preserving the unitary).
- Every record enforces `fingerprints.semantic_equivalence_guaranteed == false`.

### Invariant 2: Truthful Verification Status and Scope Semantics
Verification status must explicitly be one of four distinct states:
1. `"passed"`: The specified verifier satisfied its equivalence criterion within its declared operational scope and numerical tolerances.
   > [!NOTE]
   > Numerical equivalence checking (such as statevector inner-product or bounded Frobenius norm difference $\le \text{atol}$) establishes empirical agreement up to finite precision. It is **not** an axiomatic formal mathematical proof.
2. `"failed"`: Equivalence was checked and a mathematical or invariant discrepancy exceeding declared tolerances was detected.
3. `"not_run"`: No verification was executed for this pass.
4. `"unsupported"`: Verification could not execute because the circuit contains features outside the verifier's operational domain (e.g. non-unitary operations).

> [!IMPORTANT]
> **Anti-Masking Rule**: `"not_run"` and `"unsupported"` must **never** be collapsed into or reported as `"passed"`.
> Any report claiming `"passed"` must provide non-null `verifier_name` and explicit `provenance`.

### Invariant 3: Provenance Separation
Verification evidence is strictly partitioned by origin:
- `"internal"`: In-tree QCC Rust statevector or algebraic verification checks.
- `"independent_oracle"`: External black-box Qiskit verification oracle tooling ([`tools/independent_oracle/`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/)).
- Normal Rust compilation pipelines must **never** claim independent oracle verification.

### Invariant 4: Explicit Measurement Availability Principle
Missing or uncomputed measurements are modeled as `null` in JSON and `None` in Rust.
To prevent consumer ambiguity between missing metrics, `measurements.availability` provides an explicit status mapping:
- `"available"`: The metric was explicitly measured, computed, and populated.
- `"not_measured"`: Metric measurement was omitted or bypassed (e.g. telemetry disabled for performance).
- `"unavailable"`: Metric computation was attempted but failed or required data was missing (e.g. target calibration data missing, or pass failed).
- `"not_applicable"`: Metric is conceptually meaningless in this compiler pass or stage (e.g. post-pass metrics for a skipped pass, or routing swap metrics in a pre-routing pass).
They are **never** fabricated as zero (`0`) or empty strings.

---

## 3. Canonical Circuit Fingerprint Contract

Deterministic circuit fingerprinting is implemented via [`CircuitFingerprints::from_circuits`](file:///home/ronanki_jitendra/rust/QCC/src/compiler/pass_diagnostic.rs) using QccIR's canonical JSON serialization ([`crate::ir::circuit_to_json`](file:///home/ronanki_jitendra/rust/QCC/src/ir/serialize.rs)).

### Serialization Rules

1. **Deterministic Topological Operation Order**:
   The dependency DAG is traversed using Kahn's algorithm with a `BTreeSet<OperationId>` ready queue. Ready nodes (in-degree 0) are processed in strictly ascending `OperationId` order. This guarantees bit-for-bit identical operation sequences regardless of parallel thread scheduling or graph insertion history.
2. **Stable Semantic Operation IDs**:
   QccIR assigns stable `OperationId` (`u64`) identifiers upon node creation. Transient petgraph internal `NodeIndex` and `EdgeIndex` values are **never** exposed or serialized.
3. **Deterministic Dependency Edge Sorting**:
   All DAG dependency edges are collected and sorted lexicographically by `(from: OperationId, to: OperationId, dependency: WireDependency)` before emission.
4. **Parameter Expressions**:
   Parameter expressions (`ParameterExpr`) are serialized as canonical AST trees (preserving floating-point constants, symbolic identifiers, and algebraic operators with parentheses reflecting operator precedence).
5. **Circuit Global Phase**:
   The circuit-level scalar global phase is serialized as a canonical `ParameterExpr`. Phase shifts introduced by gates or synthesis are preserved explicitly.
6. **Register Declarations**:
   Declared quantum and classical registers are serialized in exact declaration order with their typed bit capacities.
7. **Circuit Metadata**:
   Arbitrary string metadata maps are sorted by key in deterministic lexicographical order.
8. **SHA-256 Digest**:
   The resulting pretty-printed JSON string is hashed using FIPS 180-4 SHA-256 to produce a 64-character lowercase hexadecimal digest.

---

## 4. Schema Structure and Field Semantics

| JSON Field | Rust Type | Description |
|---|---|---|
| `schema_id` | `String` (const `"qcc.pass_diagnostic"`) | Schema identifier. |
| `schema_version` | `String` (const `"1.0.0"`) | Semantic version string. |
| `pass_id` | `String` | Stable machine-readable identifier (e.g. `"rotation_merging"`, `"sabre_routing"`). |
| `pass_name` | `String` | Human-readable pass display name. |
| `execution_order` | `usize` (integer $\ge 0$) | 0-based sequence index within the compiler/pipeline execution. |
| `status` | `PassStatus` (`"ran"`, `"skipped"`, `"failed"`) | Pass execution status. |
| `compiler` | `CompilerIdentityManifest` | Compiler name (`"qcc"`), semantic version, build profile, target OS/arch, and Git commit. |
| `pass_config` | `BTreeMap<String, Value>` | Pass-specific configuration parameters and thresholds. |
| `fingerprints` | `CircuitFingerprints` | Input/output SHA-256 digests and explicit `semantic_equivalence_guaranteed: false`. |
| `measurements` | `PassMeasurements` | Before/after snapshots, execution duration, and explicit `availability` map. |
| `transformation` | `TransformationAccounting` | Operations added, removed, rewritten, global phase adjustment, analysis invalidations. |
| `verification` | `PassVerification` | Status (`passed`, `failed`, `not_run`, `unsupported`), verifier name/version/mode, provenance, tolerances, detail. |

### Circuit Snapshot Metrics (`CircuitSnapshotMetrics`)

Both `measurements.before` and `measurements.after` contain:
- `total_gate_count`: Total unitary quantum gate count (`Option<usize>`).
- `per_gate_counts`: Lower-case mnemonic to frequency map (`Option<BTreeMap<String, usize>>`).
- `cx_count`: Two-qubit controlled-NOT count (`Option<usize>`).
- `logical_swaps`: SWAP gates present in the input logical circuit (`Option<usize>`).
- `router_swaps_inserted`: SWAP gates inserted during physical routing (`Option<usize>`).
- `final_emitted_swaps`: SWAP operations in the final emitted physical circuit (`Option<usize>`).
- `circuit_depth`: Critical path dependency depth in the circuit DAG (`Option<usize>`).
- `cost_metrics`: Optional map of named target-specific costs (e.g. latency in ns, estimated error) (`Option<BTreeMap<String, f64>>`).

---

## 5. Concrete Examples

### Example 1: Successful Pass with Internal Verification

```json
{
  "schema_id": "qcc.pass_diagnostic",
  "schema_version": "1.0.0",
  "pass_id": "rotation_merging",
  "pass_name": "Same-Axis Rotation Merging",
  "execution_order": 1,
  "status": "ran",
  "compiler": {
    "name": "qcc",
    "version": "0.1.0",
    "version_string": "qcc 0.1.0",
    "profile": "release",
    "target_os": "linux",
    "target_arch": "x86_64",
    "git_commit": null
  },
  "pass_config": {
    "max_iterations": 10,
    "merge_rotations": true
  },
  "fingerprints": {
    "algorithm": "sha256",
    "input_fingerprint": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "output_fingerprint": "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
    "semantic_equivalence_guaranteed": false
  },
  "measurements": {
    "before": {
      "total_gate_count": 4,
      "per_gate_counts": { "rz": 3, "cx": 1 },
      "cx_count": 1,
      "logical_swaps": 0,
      "router_swaps_inserted": 0,
      "final_emitted_swaps": 0,
      "circuit_depth": 3,
      "cost_metrics": null
    },
    "after": {
      "total_gate_count": 2,
      "per_gate_counts": { "rz": 1, "cx": 1 },
      "cx_count": 1,
      "logical_swaps": 0,
      "router_swaps_inserted": 0,
      "final_emitted_swaps": 0,
      "circuit_depth": 2,
      "cost_metrics": null
    },
    "duration_ms": 0.42,
    "availability": {
      "before": "available",
      "after": "available",
      "duration_ms": "available",
      "router_swaps_inserted": "not_applicable",
      "final_emitted_swaps": "not_applicable",
      "cost_metrics": "not_measured"
    }
  },
  "transformation": {
    "operations_added": 0,
    "operations_removed": 2,
    "operations_rewritten": 1,
    "global_phase_adjustment": {
      "radians": 0.0,
      "expression": "0"
    },
    "analysis_invalidations": ["depth_cache"]
  },
  "verification": {
    "status": "passed",
    "verifier_name": "qcc_statevector_unitary",
    "verifier_version": "0.1.0",
    "verifier_mode": "exhaustive_unitary",
    "provenance": "internal",
    "tolerances": {
      "atol": 1e-10,
      "rtol": 1e-8
    },
    "detail": null
  }
}
```

### Example 2: Skipped Pass with `not_run` Verification

```json
{
  "schema_id": "qcc.pass_diagnostic",
  "schema_version": "1.0.0",
  "pass_id": "sabre_routing",
  "pass_name": "SABRE Qubit Routing",
  "execution_order": 3,
  "status": "skipped",
  "compiler": {
    "name": "qcc",
    "version": "0.1.0",
    "version_string": "qcc 0.1.0",
    "profile": "release",
    "target_os": "linux",
    "target_arch": "x86_64",
    "git_commit": null
  },
  "pass_config": {},
  "fingerprints": {
    "algorithm": "sha256",
    "input_fingerprint": null,
    "output_fingerprint": null,
    "semantic_equivalence_guaranteed": false
  },
  "measurements": {
    "before": null,
    "after": null,
    "duration_ms": null,
    "availability": {
      "before": "not_applicable",
      "after": "not_applicable",
      "duration_ms": "not_applicable"
    }
  },
  "transformation": {
    "operations_added": 0,
    "operations_removed": 0,
    "operations_rewritten": 0,
    "global_phase_adjustment": null,
    "analysis_invalidations": []
  },
  "verification": {
    "status": "not_run",
    "verifier_name": null,
    "verifier_version": null,
    "verifier_mode": null,
    "provenance": null,
    "tolerances": null,
    "detail": null
  }
}
```

### Example 3: Non-Unitary Pass with `unsupported` Verification

```json
{
  "schema_id": "qcc.pass_diagnostic",
  "schema_version": "1.0.0",
  "pass_id": "terminal_measurement_lowering",
  "pass_name": "Terminal Measurement Lowering",
  "execution_order": 5,
  "status": "ran",
  "compiler": {
    "name": "qcc",
    "version": "0.1.0",
    "version_string": "qcc 0.1.0",
    "profile": "release",
    "target_os": "linux",
    "target_arch": "x86_64",
    "git_commit": null
  },
  "pass_config": {},
  "fingerprints": {
    "algorithm": "sha256",
    "input_fingerprint": "1111111111111111111111111111111111111111111111111111111111111111",
    "output_fingerprint": "2222222222222222222222222222222222222222222222222222222222222222",
    "semantic_equivalence_guaranteed": false
  },
  "measurements": {
    "before": null,
    "after": null,
    "duration_ms": 0.15,
    "availability": {
      "before": "not_measured",
      "after": "not_measured",
      "duration_ms": "available"
    }
  },
  "transformation": {
    "operations_added": 2,
    "operations_removed": 0,
    "operations_rewritten": 0,
    "global_phase_adjustment": null,
    "analysis_invalidations": []
  },
  "verification": {
    "status": "unsupported",
    "verifier_name": null,
    "verifier_version": null,
    "verifier_mode": null,
    "provenance": null,
    "tolerances": null,
    "detail": "Circuit contains non-unitary measurement operations; unitary equivalence checking is undefined."
  }
}
```

---

## 6. Rust API Usage

```rust
use qcc::compiler::{
    CircuitFingerprints, MeasurementAvailability, PassDiagnosticReport, PassMeasurements,
    PassStatus, PassVerification, TransformationAccounting,
};
use std::collections::BTreeMap;

// Construct a report for an executed pass with unverified status
let mut measurements = PassMeasurements::default();
measurements.set_availability("router_swaps_inserted", MeasurementAvailability::NotApplicable);
measurements.set_availability("cost_metrics", MeasurementAvailability::Unavailable);

let report = PassDiagnosticReport::new(
    "identity_elimination",
    "Identity Gate Elimination",
    0,
    PassStatus::Ran,
    BTreeMap::new(),
    CircuitFingerprints::sha256(
        Some("in_sha256...".to_string()),
        Some("out_sha256...".to_string()),
    ),
    measurements,
    TransformationAccounting::no_op(),
    PassVerification::not_run(),
);

// Verify architectural invariants
report.validate_invariants().expect("Invariants must hold");

// Serialize to JSON
let json_string = report.to_json().expect("Serialization succeeds");
```
