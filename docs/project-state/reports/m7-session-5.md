# Milestone M7 Session 5 & Independent Verification Oracle v0.2 Final Acceptance Report

**Session**: M7 Session 5 — CI Integration & Final M7 (v0.2) Acceptance  
**Roadmaps / Phases Covered**: Roadmap v3.1 **Phase 69** (`CI oracle integration and reproducible regression gates`) & **Phase 70** (`Fair Qiskit comparison and v0.2 acceptance report`) — consolidating Sessions 1–5 (`Phases 60–70`)  
**Baseline Commit**: `f993e7d` (M7 Session 4)  
**Date Completed**: 2026-11-12  
**Milestone Decision**: **ACCEPTED (M7 / v0.2 Complete — Authorized to Advance to M8)**

---

## 1. Phase-to-Session Mapping Reconciliation (`phases-060-085.md`)

Preserving Sessions 1–4 historical reports intact without renumbering:

| Session | Roadmap `phases-060-085.md` Assignment | Commit | Scope Delivered |
| --- | --- | :---: | --- |
| **Session 1** | Phases **60, 63** *(manifest)* | `a9cabad` | ADR-0003 oracle contract, `compile_manifest.schema.json` (`1.0.0`), `CompilationManifest`, `qcc compile --emit-manifest` |
| **Session 2** | Phases **61, 63** *(QASM import)* | `caea8aa` | Pinned Python `3.12.3` / Qiskit `2.6.2` env (`requirements.txt`), ADR-0004 AST register-collision adapter, real QCC QASM compatibility checks |
| **Session 3** | Phases **62–66** | `236e2f8` | Canonical case schema + 10 cases, separated logical/physical runners, layout/endian embedding, single-common-phase & clean-ancilla comparator + failure artifacts |
| **Session 4** | Phases **67, 68, 70** *(baseline)* | `f993e7d` | 1,080-case difficult corpus + execution, 17-family (`40`-trial) fault-injection campaign, matched-$N$ (`18` workloads $\times$ `5` trials) fair Qiskit baseline |
| **Session 5** | Phases **69, 70** *(acceptance)* | *(this session)* | `.github/workflows/independent-oracle.yml`, `tools/independent_oracle/ci_check.py`, `test_oracle_session5.py`, README updates, consolidated v0.2 acceptance report |

---

## 2. Consolidated Verification Evidence Across M7 (v0.2)

### A. Compilation Manifest Contract (Session 1 — Phase 60, Phase 63)
- **Schema**: [`tools/independent_oracle/schema/compile_manifest.schema.json`](../../../tools/independent_oracle/schema/compile_manifest.schema.json) (`qcc.compile_manifest` v`1.0.0`, Draft 2020-12, `additionalProperties: false`).
- **Metadata Coverage**: Exposes compiler identity (`version`, `profile`, `target_os`, `target_arch`, optional `git_commit`), `compilation_mode` (`target_compiled` vs `frontend_only`), explicit `logical_to_physical` initial & final layouts (`little_endian_q0_lsb`), `unused_physical_qubits_initial`/`final` (ADR-0001), `input_global_phase` & `global_phase`, emitted QASM FIPS 180-4 SHA-256 + path, unambiguous `logical_swaps`/`router_swaps_inserted`/`final_emitted_swaps`, `per_gate_counts`, and explicit `null` when metadata is unavailable.
- **Determinism & CLI Integration**: Built directly in Rust from `CompilationResult` (and `Circuit` in frontend-only mode) via additive `qcc compile --emit-manifest <FILE>` (`src/compiler/manifest.rs`, `src/cli/compile.rs`) with `BTreeMap` ordering and zero stdout scraping.

### B. Pinned OpenQASM 3 Compatibility (Session 2 — Phase 61, Phase 63)
- **Pinned Environment** ([`tools/independent_oracle/requirements.txt`](../../../tools/independent_oracle/requirements.txt)): Python `3.12.3`, `qiskit==2.6.2`, `qiskit-qasm3-import==0.6.0`, `openqasm3==1.0.1`, `jsonschema==4.26.0`, `numpy==2.5.4`, plus all transitive dependencies exact-pinned (`==`), completely isolated from `Cargo.toml` and `qcc-core`.
- **Empirical Collision & Semantics-Preserving Adapter ([ADR-0004](../../decisions/ADR-0004-qasm3-importer-register-collision-fallback.md))**: Unmodified QCC physical QASM (`qubit[N] p;` + `include "stdgates.inc";`) parses cleanly with `openqasm3`, but raw `qiskit.qasm3.loads()` rejects `"p"` due to `stdgates.inc` pre-populating `PhaseGate` `"p"` in `Scope.GLOBAL`. [`qasm3_compat.py`](../../../tools/independent_oracle/qasm3_compat.py) applies `adapt_colliding_registers()` to rename colliding top-level register identifiers (`p -> qcc_reg_p`) in operand slots only (leaving `QuantumGate.name`, parameters, and `gphase` untouched, and failing closed on custom gates/control flow).
- **Global Phase & Construct Classification**: `qiskit==2.6.2` normalizes `QuantumCircuit.global_phase` modulo $2\pi$ and preserves exact $e^{i\phi}$ in `Operator` and `Statevector`. Non-unitary ops (`measure`, `reset`), unbound parameters, unrecognized gates (`sxdg`, `i`, lowercase `u`), and syntax errors are explicitly classified (`UNSUPPORTED` / `INVALID_INPUT`), never silently dropped.

### C. Independent Semantic Verification & Core Comparator (Session 3 — Phases 62–66)
- **Canonical Cases & Separated Runners**: 10 neutral cases ([`canonical_case.schema.json`](../../../tools/independent_oracle/schema/canonical_case.schema.json)) executed by strictly separated adapters — [`qiskit_reference.py`](../../../tools/independent_oracle/runners/qiskit_reference.py) (logical reference) and [`qcc_runner.py`](../../../tools/independent_oracle/runners/qcc_runner.py) (black-box CLI + SHA-256 verification before QASM import). Never calls QCC's internal `LayoutEquivalenceChecker`.
- **Layout, Endianness, Common Phase & Clean-Ancilla Enforcement**:
  - `q[0]` LSB (`little_endian_q0_lsb`), injective $L_{\text{initial}}, L_{\text{final}}$ embeddings into the complete $2^m$ physical space without tracing out spare qubits (ADR-0001).
  - Joint Frobenius overlap $S = \operatorname{Tr}(B^\dagger A)$ estimates **one common scalar global phase factor** $e^{i\phi} = S/|S|$ across the full subspace (`amplitude_atol = 1e-10`, `ancilla_atol = 1e-12`, `phase_atol = 1e-10`).
  - Informational `strict_phase_accounting` (`PASS` / `MISMATCH` / `UNVERIFIED`) is tracked independently without overriding common-phase semantic equivalence.
- **Canonical Outcomes**: `9 / 9` positive cases `PASS` (`max_amp_err <= 5.98e-16`, `ancilla_zero_ok = True`), `1 / 1` non-unitary `measure` case `UNSUPPORTED`.

### D. Difficult Deterministic Corpus (Session 4 — Phase 68)
- **Total & Uniqueness**: `1,080` distinct cases in [`corpus_v1.jsonl`](../../../tools/independent_oracle/corpus/corpus_v1.jsonl) (`master_seed = 20261112`), `distinct_case_ids = 1,080`, `distinct_logical_hashes = 1,080`, `duplicate_logical_hashes = 0`; repeated regeneration yields byte-for-byte identical SHA-256 `f865990ea401f7174bbe777371bf74c32c6f24da6aed1388a604858314226b6f`.
- **Coverage**: `8` circuit families (`135` each), `4` QCC targets (`linear: 272`, `linear-generic: 272`, `ring: 272`, `ring-generic: 264`), logical widths `1..7` (`95, 220, 187, 187, 182, 121, 88`), depth targets `4..26` (median `12.0`), interaction densities (`dense: 455`, `medium: 253`, `sparse: 210`, `single_qubit_only: 162`).
- **Execution Counts**:
  - Exhaustive unitary ($m \le 5$): **`836 / 836 PASS`** (`0` failed).
  - Sampled statevector ($m \ge 6$ + $m \in \{4,5\}$ slice, 16 states/case, `exhaustive: false`): **`244 / 244 PASS`** (`0` failed).
  - Worst observed amplitude error: `2.29e-15`; max residual ancilla population: `0.0`.

### E. Systematic Fault Injection (Session 4 — Phase 67)
- **Coverage**: `40` mutations across all **17 required families** ([`fault_campaign_summary.json`](../../../tools/independent_oracle/reports/fault_campaign_summary.json)): **`40 / 40` (`100.0%`) detected correctly**, `misclassified = 0`, `undetected_semantic_defects = 0` (`MISMATCH: 27`, `INVALID_INPUT: 9`, `UNSUPPORTED: 2`, `PASS` + informational `strict_phase_accounting = MISMATCH: 2`), preserving complete failure artifact bundles including `mutation_description.json`.

### F. Fair Matched-$N$ QCC-vs-Qiskit Baseline (Session 4 — Phase 70 Baseline)
- **Matched Workloads & Versions**: `18` comparable `linear`/`ring` workloads $\times$ `5` trials per compiler (`qcc 0.1.0` `CompilerPipeline::default` `O2` vs `qiskit 2.6.2` `optimization_level=2`, `basis_gates=["rz", "sx", "cx"]`, `seed_transpiler=1000..1004`). Non-comparable `linear-generic`/`ring-generic`/`frontend_only` targets classify explicitly as `UNAVAILABLE`.
- **Semantic Gate**: **`90 / 90 PASS`** (QCC) and **`90 / 90 PASS`** (Qiskit).
- **Cost Comparison & Honest Losses**: Median CX Win/Tie/Loss (QCC vs Qiskit): **`0 / 5 / 13`**; Median Depth W/T/L: **`1 / 2 / 15`**.
- **Disclosures**: QCC executes `1` deterministic SABRE configuration (`is_deterministic = true`, `internal_search_trials = 1`); Qiskit `O2` runs **`20` internal randomized SABRE trials** (`equal_internal_compute_budget = false`). Wall-clock compile time is informational only (subprocess CLI startup + file I/O vs in-process `PassManager.run()`), with zero flaky CI speed assertions.

### G. CI Workflow Integration & Artifact Retention (Session 5 — Phase 69)
- **Workflow Files**:
  - [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) — preserved untouched for Python-free Rust fmt/clippy/test/build/doc checks plus `openqasm3` Bell AST check.
  - [`.github/workflows/independent-oracle.yml`](../../../.github/workflows/independent-oracle.yml) — isolated two-stage Python/Qiskit oracle workflow triggered on `push` (`main`), `pull_request` (`main`), `schedule` (`15 3 * * 0` UTC weekly), and `workflow_dispatch`:
    - **`oracle-core-gate`**: Builds `qcc`, sets up pinned Python `3.12` + cached `.venv` from `requirements.txt`, executes [`ci_check.py`](../../../tools/independent_oracle/ci_check.py) `--stage core` (`pip check`, all 7 schemas + committed artifacts, 5 Session 2 fixtures, 10 canonical cases, deterministic corpus regeneration diff, all **45 Python unit tests**). Uploads `${{ runner.temp }}/oracle_core_artifacts/` (`independent-oracle-core-diagnostics`, 30-day retention) on `failure()`.
    - **`oracle-campaign-and-baseline`** (`needs: oracle-core-gate`): Runs `ci_check.py --stage campaign --benchmark-trials 2` (full 1,080-case corpus execution, 40-trial fault-injection campaign, and semantically verified benchmark schema/consistency check without wall-clock thresholds). Uploads `${{ runner.temp }}/oracle_campaign_artifacts/` (`independent-oracle-campaign-artifacts`, 30-day retention) on `always()`, avoiding uploading `.venv/` or `target/`.

---

## 3. Explicit Limitations & Acceptance Boundaries

1. **Sampled verification is not exhaustive proof**: The `244` corpus cases in `sampled_statevector` mode test deterministic basis + 16 seeded random states (`exhaustive: false`). They provide probabilistic regression signal, not formal exhaustive unitary equivalence.
2. **Exhaustive width limit**: `exhaustive_unitary` is capped at `physical_qubits <= 8` (`2^8 x 2^n` full physical state comparison); larger widths require explicit `sampled_statevector`.
3. **Unsupported OpenQASM 3 / QCC boundaries**: Dynamic control flow, mid-circuit measurement/reset, noise models, unbound symbolic parameters (`input float` in Qiskit vs CLI `--bind`), and `sxdg`/`i`/`u` under `qiskit-qasm3-import==0.6.0` remain unsupported/classified explicitly.
4. **Benchmark fairness limitations**: QCC exposes deterministic single-config compilation via CLI, whereas Qiskit `O2` executes 20 internal SABRE trials; CLI subprocess spawn time vs in-process Python execution time are not apples-to-apples speed metrics; `linear-generic`/`ring-generic` cannot be fairly compared without aligned 2Q cost models and `sxdg` import support.
5. **Informational strict phase accounting**: `strict_phase_accounting` (`PASS`/`MISMATCH`/`UNVERIFIED`) monitors manifest-vs-QASM phase consistency without overriding authoritative common-global-phase semantic equivalence.
6. **Not formal verification or production certification**: The independent oracle establishes empirical bug detection and independent semantic verification over the tested unitary domain; it is **not** a mechanized formal proof of compiler correctness, nor a claim that QCC is production-ready.

---

## 4. Final Verification Commands (`Session 5`)

- `.venv/bin/python3 tools/independent_oracle/ci_check.py --stage all --benchmark-trials 2`: Passed (`core` + `campaign`: pinned deps OK, 7 schemas & committed artifacts valid, 5 Session 2 QASM3 compat fixtures OK, 10 canonical cases PASS/UNSUPPORTED matched, deterministic corpus regen SHA-256 matched, 1,080/1,080 corpus PASS [836 exhaustive, 244 sampled], 40/40 fault injection detected, 18 workloads × 2 benchmark trials semantic+schema PASS, exit code `0`)
- `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: **45 tests passed**, 0 failed (Sessions 2–5 unified oracle & CI test suite, exit code `0`)
- `cargo fmt --check`: exit code `0`
- `cargo clippy --all-targets -- -D warnings`: exit code `0`
- `cargo doc --no-deps`: exit code `0`
- `cargo test --lib --test cli --test compiler_end_to_end -- --quiet`: `172` Python-free Rust tests passed (`117` lib + `30` cli + `25` end-to-end, 0 failed, exit code `0`)
- `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: `1,401` Rust tests passed across `41` test output entries, 0 failed, 0 ignored (exit code `0`)

---

## 5. Final M7 (v0.2) Acceptance Checklist

- [x] Versioned manifest schema (`1.0.0`) & Rust generation valid (`a9cabad`).
- [x] Pinned Python `3.12.3` / Qiskit `2.6.2` / `qiskit-qasm3-import==0.6.0` environment reproducible and isolated from `qcc-core` (`caea8aa`).
- [x] Actual QCC-emitted physical QASM handled cleanly via ADR-0004 AST register adapter (`caea8aa`).
- [x] Independent Qiskit logical reference and black-box QCC physical runners strictly separated (`236e2f8`).
- [x] Common-global-phase comparison, initial/final `logical_to_physical` layouts, and ADR-0001 strict clean-ancilla enforcement verified across $2^m$ physical states (`236e2f8`).
- [x] Fault injection (`40/40` across 17 families, `0` undetected) and 1,080-case deterministic corpus (`1,080/1,080 PASS`) verified (`f993e7d`).
- [x] Sampled mode explicitly flagged non-exhaustive; benchmark limitations & internal SABRE trial disclosures documented (`f993e7d`, Session 5).
- [x] CI workflow [`.github/workflows/independent-oracle.yml`](../../../.github/workflows/independent-oracle.yml) enforces required oracle checks and preserves diagnostic failure artifacts.
- [x] All Rust checks remain green with zero semantic changes or Python runtime dependencies added to Rust crates.

**Formal Verdict**: **M7 / v0.2 ACCEPTED**. Next authorized unit under Roadmap v3.1: **Milestone M8 (Phase 71)** — stopped here pending next session.

