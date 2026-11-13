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

### Invariant 1: Deterministic Representation Fingerprint (Not Semantic Proof)
Input and output fingerprints (`input_fingerprint`, `output_fingerprint`) are cryptographic SHA-256 hashes of a specific QccIR DAG representation.
- Two identical fingerprints indicate that the pass produced bit-for-bit identical serialization (e.g. an idempotent or no-op pass on the same representation).
- **Fingerprint equality does NOT prove mathematical unitary equivalence.**
- **Fingerprint inequality does NOT imply semantic inequivalence.** (E.g. gate commutation or rotation merging alters the representation while preserving the unitary).
- Serialization explicitly depends on allocated `OperationId`s and tie-breaking order; different concurrent insertion orders produce different fingerprints.
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

## 3. Deterministic Representation Fingerprint Contract

Deterministic circuit fingerprinting is implemented via [`CircuitFingerprints::from_circuits`](file:///home/ronanki_jitendra/rust/QCC/src/compiler/pass_diagnostic.rs) using QccIR's JSON serialization ([`crate::ir::circuit_to_json`](file:///home/ronanki_jitendra/rust/QCC/src/ir/serialize.rs)).

The schema contract establishes **deterministic representation fingerprints** of in-memory QccIR circuit representations, rather than representation-independent or semantic proofs.

### Established Guarantees

1. **Repeatability for the Same Representation**:
   Repeated serialization of the exact same in-memory `Circuit` instance produces bit-for-bit identical JSON text and SHA-256 digests across repeated calls, across threads, and across runs.
2. **Pointer and Memory Independence**:
   Topological traversal uses Kahn's algorithm with a `BTreeSet<OperationId>` ready queue, and dependency edges are sorted lexicographically by `(from: OperationId, to: OperationId, dependency: WireDependency)`. For identical `OperationId` values, this eliminates any iteration-order nondeterminism from internal hash tables (`HashMap`) or memory addresses.
3. **Exact Value and AST Preservation**:
   - Parameter expressions (`ParameterExpr`) are serialized as canonical AST trees (preserving floating-point constants, symbolic identifiers, and algebraic operators with parentheses reflecting operator precedence).
   - Circuit-level scalar global phase is serialized as an exact `ParameterExpr`.
   - Declared quantum and classical registers are serialized in exact declaration order with typed bit capacities.
   - Circuit metadata maps are sorted by key in deterministic lexicographical order.
4. **SHA-256 Digest**:
   The resulting pretty-printed JSON string is hashed using FIPS 180-4 SHA-256 to produce a 64-character lowercase hexadecimal digest.

### Explicit Limitations (Representation-Dependent)

1. **OperationId Sensitivity**:
   QccIR allocates `OperationId` (`u64`) sequentially upon node creation (`self.next_id += 1`). Serialized output (`SerializedOperation`, `SerializedDependency`) explicitly records these operation IDs.
2. **Insertion Order for Concurrent Operations**:
   When operations have no causal or wire dependency between them (e.g. concurrent operations on separate qubits), Kahn's algorithm breaks tie-breaking ordering using ascending `OperationId`. If concurrent operations are inserted into the circuit builder in a different order, they receive swapped `OperationId`s, which alters both the tie-breaking emission sequence and the serialized ID values.
3. **No Isomorphic Canonicalization Across Different Operation IDs**:
   Two circuits with identical graph topology will produce different fingerprints if their operations were assigned different `OperationId` values. The system does not perform graph isomorphism or permutation-invariant relabeling.
4. **No Semantic Equivalence Proof**:
   Matching fingerprints prove representation identity only. Semantically equivalent circuits with different representations (such as $X \cdot X = I$, gate commutation, or decomposed rotations like $R_z(\pi/4) \cdot R_z(\pi/4)$ vs $R_z(\pi/2)$) produce distinct fingerprints. The boolean flag `semantic_equivalence_guaranteed` is unconditionally `false`.

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

---

## 7. Pass-Manager Instrumentation and Execution Lifecycle (Phase 72)

Phase 72 integrates the `qcc.pass_diagnostic` v1.0.0 schema into QCC's execution boundaries (`PassManager` and `OptimizerPipeline`).

### 7.1 Architecture and Instrumentation Hooks

1. **Pass Lifecycle Hook**:
   - Before pass execution: captures pre-pass circuit snapshot, computes input fingerprint (SHA-256), and calculates pre-pass metrics (`CircuitSnapshotMetrics::from_circuit`).
   - Conditional execution: checks `pass.is_enabled(circuit, context)`. If disabled, records a diagnostic report with `status: skipped`, `measurements.after: null` with availability `not_applicable`, and skips pass execution.
   - Post-pass capture: captures post-pass circuit snapshot, computes output fingerprint (SHA-256), and calculates post-pass metrics.
   - Transformation accounting: computes exact additions ($S_{\text{after}} \setminus S_{\text{before}}$), removals ($S_{\text{before}} \setminus S_{\text{after}}$), and in-place rewrites ($S_{\text{before}} \cap S_{\text{after}}$ with mutated operations) by tracking stable `OperationId` sets.
   - Invalidation tracking: records analysis IDs purged during pass execution.
   - Phase accounting: calculates scalar global phase difference $\Delta\phi = \phi_{\text{after}} - \phi_{\text{before}}$.
   - Verification hook: executes optional development/testing equivalence check comparing pre- and post-pass states.
2. **Error and Failure Preservation**:
   - If a pass fails or returns an error, the failure is recorded with `status: failed`, `measurements.after: null` with availability `unavailable`, and preserved in `PassManagerReport.pass_diagnostics` and `PassContext`.
   - Earlier successful pass records are preserved in full chronological order.
3. **Verification Hook Lifecycle**:
   - Configured via `PassVerificationHookConfig` inside `PassDiagnosticConfig`.
   - Disabled by default (`enabled: false`).
   - Supports execution bounds via `max_qubits` (default 16) to prevent exponential simulation overhead during testing.
   - Unitary eligibility validation: checks whether before and after circuits contain non-unitary operations (measurements, resets, barriers). If non-unitary, reports status `unsupported` rather than false failure or false pass.
   - Provenance is strictly recorded as `VerifierProvenance::Internal`.

### 7.3 Independent Instrumentation Controls (`PassDiagnosticConfig`)

To satisfy strict performance and diagnostic requirements, `PassDiagnosticConfig` provides four independently configurable controls without altering the schema or duplicating structures:

| Control | Field | Default | Behavior when Disabled |
|---|---|:---:|---|
| **Diagnostic Enablement** | `enabled` | `false` | Zero diagnostic overhead, zero extra allocations, no pass reports emitted. |
| **Detailed Metrics** | `collect_metrics` | `true` | Bypasses `CircuitSnapshotMetrics::from_circuit`. Sets `measurements.before` and `after` to `null` with availability `"not_measured"`. |
| **Cryptographic Fingerprints** | `collect_fingerprints` | `true` | Completely avoids QccIR JSON serialization and SHA-256 computation. Sets `input_fingerprint` and `output_fingerprint` to `null`. |
| **Verification Hooks** | `verification_hook.enabled` | `false` | Bypasses in-tree `EquivalenceChecker`. Sets `verification.status` to `"not_run"` and verifier fields to `null`. |

#### Named Presets & Builder Methods:
- `PassDiagnosticConfig::disabled()`: Completely disables diagnostics (zero overhead).
- `PassDiagnosticConfig::basic()`: Enables execution records, pass identity, timing, and transformation accounting, completely omitting circuit JSON serialization and snapshot metrics.
- `PassDiagnosticConfig::standard()`: Enables metrics and fingerprints with verification hooks disabled (standard audit).
- `PassDiagnosticConfig::full()`: Enables metrics, fingerprints, and in-tree verification hooks.
- Builders: `with_metrics(bool)`, `with_fingerprints(bool)`, `with_verification(bool)`, `with_tolerances(...)`, `with_fail_on_mismatch(bool)`.

### 7.4 Instrumentation Overhead Breakdown by Configuration

Benchmarked on a representative 100-gate, 4-qubit circuit across 5 compilation trials (`CompilerPipeline::compile`, `-O2`):

| Configuration | Description | Trials (ms) | Median (ms) | Min (ms) | Max (ms) | Spread (ms) | Abs Overhead (ms) | % Overhead |
|---|---|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **1. Disabled** | Zero diagnostics (`CompilerConfig::o2()`) | `[89.1, 87.8, 85.7, 83.5, 90.0]` | **87.78** | 83.54 | 90.02 | 6.48 | Baseline | **0.0%** |
| **2. Basic Records Only** | Identity, timing, transformation accounting | `[89.3, 88.4, 91.3, 97.7, 90.5]` | **90.50** | 88.43 | 97.67 | 9.24 | +2.72 | **+3.1%** |
| **3. Basic + Metrics** | Basic records + gate counts & circuit depth | `[91.8, 91.4, 94.8, 95.0, 92.6]` | **92.60** | 91.43 | 95.02 | 3.59 | +4.83 | **+5.5%** |
| **4. Basic + Fingerprints** | Basic records + SHA-256 JSON hashes | `[161.6, 166.2, 156.5, 162.9, 162.3]` | **162.28** | 156.48 | 166.23 | 9.74 | +74.50 | **+84.9%** |
| **5. Standard** | Metrics + Fingerprints (audit mode) | `[167.2, 166.0, 166.7, 166.4, 169.3]` | **166.66** | 166.02 | 169.32 | 3.30 | +78.89 | **+89.9%** |
| **6. Full** | Standard + In-tree Verification Hook | `[549.6, 550.2, 548.4, 556.3, 562.7]` | **550.16** | 548.35 | 562.71 | 14.36 | +462.38 | **+526.8%** |

#### Key Empirical Observations:
1. **Lightweight Basic Records**: Basic pass accounting and metrics add negligible latency (+3.1% to +5.5%), confirming that recording pass execution, timing, and transformation diffs has minimal runtime cost.
2. **Fingerprinting Cost**: Cryptographic SHA-256 representation fingerprinting accounts for the vast majority of non-verification overhead (+84.9%), caused by serializing the full DAG to pretty-printed JSON text before and after every pass.
3. **Verification Hook Cost**: Full statevector unitary equivalence checking adds +526.8% latency. This confirms the architectural decision that verification hooks must remain opt-in and disabled by default.

---

## 8. CLI Diagnostic Flags and Inspect/Explain Tooling (Phase 73)

Phase 73 exposes per-pass diagnostics and audit tooling directly through the `qcc compile` command-line interface.

### 8.1 CLI Options Reference

| Flag / Option | Argument | Description | Default |
|---|---|---|---|
| `-O, --opt-level` | `<0\|1\|2\|3>` | Optimization aggressiveness level (0 = none, 1 = peephole, 2 = standard, 3 = aggressive) | `2` |
| `--pass-report` | `<FILE>` | Export chronological JSON array of Phase 71 `qcc.pass_diagnostic` reports to `<FILE>` | `None` |
| `--inspect-pass` | `<ID>` | Print comprehensive human-readable diagnostic report for pass `<ID>` to `stderr` | `None` |
| `--pass-summary` | *(none)* | Print concise ASCII table summarizing pass execution order, status, timing, deltas, and verification to `stderr` | `false` |
| `--pass-diagnostics` | `<MODE>` | Diagnostic mode preset: `'basic'`, `'metrics'`, `'fingerprints'`, `'standard'`, `'full'` | `'basic'` when report/inspect/summary requested |
| `--pass-metrics` | *(none)* | Explicitly enable before/after circuit snapshot metrics collection | `false` |
| `--pass-fingerprints` | *(none)* | Explicitly enable SHA-256 circuit representation fingerprint computation | `false` |
| `--pass-verify` | *(none)* | Explicitly enable in-tree unitary equivalence verification hooks | `false` |

### 8.2 Default Configuration and Precedence

1. **Zero-Overhead Default**: In normal compilation (`qcc compile input.qasm`), diagnostic collection is completely disabled (`PassDiagnosticConfig::default()`, `enabled: false`), imposing zero runtime or memory overhead.
2. **Safe Diagnostic Default**: When diagnostic reporting is requested (`--pass-report`, `--inspect-pass`, or `--pass-summary`) without specifying `--pass-diagnostics <MODE>`, QCC defaults to **`PassDiagnosticConfig::basic()`**. Heavy SHA-256 JSON fingerprinting and expensive statevector verification hooks are never silently enabled.
3. **Preset Modes**:
   - `basic`: Status, pass identity, execution order, configuration, transformation accounting, duration.
   - `metrics`: Basic + before/after snapshot metrics (gate count, gate histogram, CX count, depth).
   - `fingerprints`: Basic + input and output SHA-256 representation fingerprints.
   - `standard`: Metrics + fingerprints (full audit without verification).
   - `full`: Metrics + fingerprints + in-tree equivalence verification hooks.
4. **Additive Flag Overrides**: Individual flags (`--pass-metrics`, `--pass-fingerprints`, `--pass-verify`) override preset settings additively.
5. **Clean Stream Separation**: All diagnostic tables, inspection text, and operational warnings are directed to `stderr`. `stdout` is strictly reserved for the compiled output (OpenQASM 3 or QccIR), preserving UNIX pipeline composability (`qcc compile ... | other_tool`).

### 8.3 Example Invocations

#### Export Structured JSON Pass Report
```bash
# Export standard audit report to file while compiling for a linear hardware target
qcc compile circuit.qasm --target linear --pass-diagnostics standard --pass-report target/diagnostics.json -o out.qasm
```

#### Terminal Execution Summary
```bash
qcc compile circuit.qasm --target linear --pass-summary -o out.qasm
```
Output emitted to `stderr`:
```text
Pass Execution Summary:
+------+-----------------------+---------+----------+------------------+----------------+--------------+
| Step | Pass ID               | Status  | Time(ms) | Gates (In -> Out)| Delta (+/-/~ ) | Verification |
+------+-----------------------+---------+----------+------------------+----------------+--------------+
|    0 | identity_elimination  | ran     |     0.05 | 10 -> 8          | +0 / -2 / ~0   | not_run      |
|    1 | inverse_cancellation  | ran     |     0.03 | 8 -> 8           | +0 / -0 / ~0   | not_run      |
|    2 | rotation_merge        | ran     |     0.04 | 8 -> 6           | +0 / -2 / ~0   | not_run      |
|    3 | one_qubit_fusion      | ran     |     0.06 | 6 -> 4           | +0 / -2 / ~0   | not_run      |
+------+-----------------------+---------+----------+------------------+----------------+--------------+
Total passes: 4 (ran: 4, skipped: 0, failed: 0)
```

#### Inspect Specific Compiler Pass
```bash
qcc compile circuit.qasm --target linear --inspect-pass identity_elimination -o out.qasm
```
Output emitted to `stderr`:
```text
=== Pass Inspection: identity_elimination (Order: 0) ===
Pass ID:        identity_elimination
Pass Name:      Eliminates explicit identity gates and proven zero rotations while preserving circuit semantics and QccIR DAG invariants.
Execution Order:0
Status:         ran
Configuration:
  eliminate_two_pi_rotations: true
  tolerance: 1e-9
Measurements:
  Duration:     0.045 ms
  Before Snapshot:
    Total Gates:   10
    CX Gates:      4
    Circuit Depth: 6
    Histogram:     cx: 4, h: 2, id: 2, rz: 2
  After Snapshot:
    Total Gates:   8
    CX Gates:      4
    Circuit Depth: 5
    Histogram:     cx: 4, h: 2, rz: 2
Transformation Accounting:
  Operations Added:     0
  Operations Removed:   2
  Operations Rewritten: 0
  Phase Adjustment:     none
  Invalidations:        none
Circuit Fingerprints:
  Algorithm:            sha256
  Input Fingerprint:    42b503b1b667bd83fb7c6da30621e6109a429fd2290664719c591a0429a77a14
  Output Fingerprint:   764359ebb7b17e69b5ae3763d1ee8b28fdea4b00b1a64dba71f152c3112f36be
  Semantic Equivalence Guaranteed: false (representation fingerprint only)
Verification:
  Status:     passed
  Verifier:   qcc_unitary_equivalence (v0.1.0, mode: exhaustive_unitary_4_states)
  Provenance: internal
```

### 8.4 Diagnostic Preservation on Compilation Failure

When compilation encounters an error (either in `--target` mode or in target-free frontend mode):
1. **Accumulated Records Preserved**: All pass diagnostic reports accumulated up to and including the failing pass are preserved without loss. Earlier successful passes maintain `status: "ran"`, and the failing pass is recorded with `status: "failed"`.
2. **Post-Failure State Classification**: In accordance with Invariant 4, the failing pass truthfully records `measurements.after` as `unavailable` because execution aborted prior to producing a valid output DAG.
3. **Report Export**: If `--pass-report <FILE>` is provided, QCC writes the preserved chronological JSON array to disk before terminating with exit code 2 (`CompileError`).
4. **Introspection Flags**: If `--pass-summary` or `--inspect-pass <ID>` are supplied, terminal summaries and pass inspections are emitted to `stderr` from the preserved diagnostic records.


