# Phase 73 Completion Report: Diagnostic CLI Flags and Inspect/Explain Tooling

```text
Phase: Phase 73
Milestone: Milestone M8 — Compiler Introspection and Diagnostics
Roadmap: QCC Engineering Roadmap v3.1 (Phases 60–85)
Date: 2026-11-13
Status: COMPLETED & ACCEPTED
```

---

## 1. Executive Summary

**Phase 73** completes **Milestone M8 (Compiler Introspection and Diagnostics)** by exposing the versioned `qcc.pass_diagnostic` v1.0.0 per-pass diagnostics and reporting framework directly through the QCC command-line interface.

All diagnostic features have been integrated into `qcc compile` with zero overhead when unrequested, safe lightweight defaults (`basic`) when diagnostics are requested, and strict Unix stream separation—preserving pure compiled circuit text on `stdout` and directing all human-readable diagnostic tables, inspection outputs, and telemetry to `stderr`.

### Key Deliverables:
1. **Report Export (`--pass-report <FILE>`)**: Serializes and writes a versioned JSON array of `PassDiagnosticReport` records conforming strictly to `qcc.pass_diagnostic` v1.0.0 (Draft 2020-12). Unwritable output paths gracefully report operational errors with exit code 1 without corrupting `stdout`.
2. **Pass Inspection (`--inspect-pass <ID>`)**: Renders comprehensive human-readable inspection records detailing pass identity, execution order, configuration, before/after snapshot metrics (with explicit availability classification), transformation accounting, circuit fingerprints, and verification evidence. Missing pass IDs gracefully report available IDs with exit code 1; repeated passes in multi-iteration runs display every execution sequentially (`Execution X of Y`).
3. **Execution Summary (`--pass-summary`)**: Emits a concise, formatted ASCII table to `stderr` detailing pass execution sequence, pass ID, status (`ran`, `skipped`, `failed`), wall-clock duration, gate counts before and after, transformation deltas (`+add / -rem / ~rewr`), and verification verdict.
4. **Independent Instrumentation Controls (`--pass-diagnostics <MODE>` and toggles)**: Exposes modes `basic`, `metrics`, `fingerprints`, `standard`, and `full`, defaulting to lightweight `basic` records when diagnostics are requested. Additive toggles `--pass-metrics`, `--pass-fingerprints`, and `--pass-verify` provide granular control.
5. **Optimization Control (`-O, --opt-level <0|1|2|3>`)**: Controls pass manager aggressiveness. Under `-O0`, all optimization passes are truthfully recorded as `status: skipped` with `after: not_applicable` measurements and `verification.status: not_run`.
6. **Cross-Process Deterministic Fingerprints**: Replaced randomized `HashMap` metadata with sorted `BTreeMap` in `SerializedCircuit`, establishing bit-for-bit repeatable SHA-256 circuit fingerprints across separate process invocations.

---

## 2. Architecture and Implementation Details

### 2.1 CLI Configuration Model (`CompileOptions`)

In [`src/cli/compile.rs`](file:///home/ronanki_jitendra/rust/QCC/src/cli/compile.rs), `CompileOptions` was additively extended with fields:
- `pub pass_report: Option<String>`: Path to write the diagnostic JSON report array.
- `pub inspect_pass: Option<String>`: Pass ID to inspect.
- `pub pass_summary: bool`: Flag to print the ASCII execution summary table.
- `pub pass_diagnostics_mode: Option<String>`: Preset mode name (`basic`, `metrics`, `fingerprints`, `standard`, `full`).
- `pub pass_metrics: bool`: Flag to explicitly enable before/after snapshot metrics.
- `pub pass_fingerprints: bool`: Flag to explicitly enable SHA-256 circuit fingerprints.
- `pub pass_verify: bool`: Flag to explicitly enable in-tree unitary equivalence verification hooks.
- `pub opt_level: OptimizationLevel`: Optimization aggressiveness level (defaults to `OptimizationLevel::O2`).

### 2.2 Diagnostic Mode Resolution (`resolve_diagnostic_config`)

Diagnostic configuration resolution strictly enforces the safe default principle:
```rust
fn resolve_diagnostic_config(opts: &CompileOptions) -> PassDiagnosticConfig {
    let diagnostics_requested = opts.pass_report.is_some()
        || opts.inspect_pass.is_some()
        || opts.pass_summary
        || opts.pass_diagnostics_mode.is_some()
        || opts.pass_metrics
        || opts.pass_fingerprints
        || opts.pass_verify;

    if !diagnostics_requested {
        return PassDiagnosticConfig::default(); // enabled: false (zero overhead)
    }

    let mut cfg = match opts.pass_diagnostics_mode.as_deref() {
        Some("basic") => PassDiagnosticConfig::basic(),
        Some("metrics") => PassDiagnosticConfig::basic().with_metrics(true),
        Some("fingerprints") => PassDiagnosticConfig::basic().with_fingerprints(true),
        Some("standard") => PassDiagnosticConfig::standard(),
        Some("full") => PassDiagnosticConfig::full(),
        None => PassDiagnosticConfig::basic(), // Safe default when diagnostics requested
        _ => unreachable!(),
    };

    if opts.pass_metrics {
        cfg = cfg.with_metrics(true);
    }
    if opts.pass_fingerprints {
        cfg = cfg.with_fingerprints(true);
    }
    if opts.pass_verify {
        cfg = cfg.with_verification(true);
    }

    cfg
}
```

### 2.3 Compilation Execution Paths and Diagnostic Preservation

`execute_compile` handles both target-compiled and target-free frontend modes:
1. **Target Mode (`--target <TARGET>`)**:
   - Configures `CompilerPipeline::new(CompilerConfig::for_level(opts.opt_level).with_diagnostics(diag_cfg))`.
   - On success: collects `result.pass_diagnostics`.
   - On optimization failure: `CompilerPipeline::compile` propagates accumulated diagnostic records (all preceding successful passes + the failed pass) via `CompilerError::Optimization { source, diagnostics }`. The CLI extracts `e.pass_diagnostics()`, writes `--pass-report` if requested, prints terminal diagnostics (`--pass-summary`, `--inspect-pass`), and terminates with exit code 2 (`CompileError`).
2. **Frontend Mode (No `--target`)**:
   - If no diagnostics or custom optimization are requested: preserves existing frontend-only behavior (zero pass execution, empty diagnostics).
   - If diagnostics or custom optimization are requested: invokes `OptimizerPipeline::new(OptimizerConfig::new(opts.opt_level).with_diagnostics(diag_cfg))` on the lowered circuit with `PassContext`, collecting `report.pass_diagnostics`.
   - On pass failure: collects accumulated diagnostic records from `PassContext` up to and including the failed pass, writes `--pass-report` if requested, and terminates cleanly with exit code 2 (`CompileError`).

### 2.4 Presentation Formatters

- **`render_pass_summary`**: Emits formatted tabular output:
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
- **`render_pass_inspection`**: Emits detailed per-pass inspection records. Handles missing passes with `error: pass ID '<ID>' was not found in pass diagnostics.\nAvailable pass IDs: ...` and exit code 1. Handles multi-iteration passes by displaying `=== Execution X of Y (Order: N) ===` sequentially.

### 2.5 Cross-Process Deterministic Serialization

During integration testing, a subtle non-determinism was identified: `circuit.metadata` was serialized as `HashMap<String, String>`, which uses randomized SipHash across separate process invocations. In [`src/ir/serialize.rs`](file:///home/ronanki_jitendra/rust/QCC/src/ir/serialize.rs), `SerializedCircuit.metadata` was converted to `BTreeMap<String, String>`. This guarantees sorted key emission order, establishing bit-for-bit identical JSON text and SHA-256 circuit fingerprints across independent process runs.

---

## 3. Automated Verification Evidence

### 3.1 CLI Integration Test Suite (`tests/cli.rs`)

All 17 test scenarios from the specification and acceptance audit were implemented and verified in [`tests/cli.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/cli.rs):

| Test Scenario | Test Function | Verified Behavior | Status |
|---|---|---|:---:|
| 1. Basic report export | `test_cli_compile_pass_report_basic` | Basic records populated; metrics unmeasured; fingerprints uncomputed | **PASS** |
| 2. Metrics mode | `test_cli_compile_pass_report_metrics` | `before` and `after` metrics populated; fingerprints uncomputed | **PASS** |
| 3. Fingerprints mode | `test_cli_compile_pass_report_fingerprints` | SHA-256 hashes populated; metrics unmeasured; `semantic_equivalence_guaranteed == false` | **PASS** |
| 4. Verification mode | `test_cli_compile_pass_report_verify` | Verification status `passed` with `internal` provenance | **PASS** |
| 5. Schema version | `test_cli_compile_pass_report_schema_version` | `schema_id: "qcc.pass_diagnostic"`, `schema_version: "1.0.0"` on all records | **PASS** |
| 6. Execution order | `test_cli_compile_pass_report_execution_order` | Strict sequential indexing `0, 1, 2, ...` | **PASS** |
| 7. Skipped passes under O0 | `test_cli_compile_pass_report_skipped_o0` | All passes `status: skipped`, `after: not_applicable`, `verification: not_run` | **PASS** |
| 8. Failed pass preservation (in-memory) | `test_cli_compile_pass_report_failed_pass_preservation_in_memory` | Prior successful records + failed pass record preserved in PassContext | **PASS** |
| 9. Target mode failed pass preservation (CLI) | `test_cli_compile_target_pass_report_preservation_on_pass_failure` | Real CLI process with `--target linear` exits with code 2, preserves earlier ran records + failed record on disk, passes invariants | **PASS** |
| 10. Target-free failed pass preservation (CLI) | `test_cli_compile_target_free_pass_report_preservation_on_pass_failure` | Real CLI process target-free exits with code 2, preserves earlier ran records + failed record on disk, passes invariants | **PASS** |
| 11. Inspect existing pass | `test_cli_compile_inspect_pass_existing` | Full inspection text on `stderr`; valid OpenQASM 3 on `stdout` | **PASS** |
| 12. Inspect missing pass | `test_cli_compile_inspect_pass_missing` | Clear error and available pass IDs on `stderr`; exit code 1 | **PASS** |
| 13. Inspect repeated pass | `test_cli_compile_inspect_pass_repeated` | Multi-iteration pass displays `Execution 1 of ...` without dropping iterations | **PASS** |
| 14. Unwritable report path | `test_cli_compile_pass_report_write_failure` | Operational error on `stderr`; exit code 1; `stdout` clean | **PASS** |
| 15. Unchanged behavior | `test_cli_compile_behavior_unchanged_without_diagnostics` | Normal compile produces exact same QASM and zero stderr noise | **PASS** |
| 16. Deterministic output | `test_cli_compile_pass_report_deterministic_output` | Identical compilations produce bit-for-bit identical report JSON (normalized duration) | **PASS** |
| 17. Pass summary table | `test_cli_compile_pass_summary` | Formatted ASCII summary table printed to `stderr`; clean `stdout` | **PASS** |

### 3.2 Schema Conformance Validation

Generated CLI pass reports across all execution modes were validated against [`tools/independent_oracle/schema/pass_diagnostic.schema.json`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/schema/pass_diagnostic.schema.json) using Python `jsonschema.Draft202012Validator`:
- `qcc compile examples/routing.qasm --target linear --pass-diagnostics full --pass-report ...`: **100% Valid (12/12 reports)**
- `qcc compile examples/routing.qasm --target linear --pass-report ...` (default basic): **100% Valid (12/12 reports)**
- `qcc compile examples/routing.qasm -O0 --pass-report ...`: **100% Valid (6/6 reports)**

### 3.3 Test Suite Reconciliation

| Test Category | Prior Count (Phase 72) | Phase 73 Delta | Current Count | Status |
|---|:---:|:---:|:---:|:---:|
| **Rust Unit & Integration Tests** | 1,427 | +17 | **1,444** | **41 / 41 targets passed (0 failed, 0 ignored)** |
| **Python Independent Oracle Tests** | 61 | +0 | **61** | **61 / 61 passed (0 failed)** |
| **Compiler Linter (`cargo clippy`)** | 0 warnings | +0 | **0 warnings** | **Pass (`-D warnings`)** |
| **Code Formatting (`cargo fmt`)** | clean | clean | **clean** | **Pass** |
| **API Documentation (`cargo doc`)** | 0 warnings | +0 | **0 warnings** | **Pass** |

---

## 4. Milestone M8 Completion Confirmation

With Phase 73 completed, **Milestone M8 (Compiler Introspection and Diagnostics)** is complete across all three planned phases:
- **Phase 71**: Stable, versioned `qcc.pass_diagnostic` v1.0.0 schema, invariant enforcement, and data models.
- **Phase 72**: Pass-manager execution boundary instrumentation, transaction accounting, and opt-in verification hooks.
- **Phase 73**: Diagnostic CLI flags (`--pass-report`, `--inspect-pass`, `--pass-summary`, `--pass-diagnostics`, toggles, `-O`), inspect/explain presentation, and deterministic report export.

Milestone M9 (Phase 74: Multi-Candidate Routing Configuration and Objectives) is fully unblocked.
