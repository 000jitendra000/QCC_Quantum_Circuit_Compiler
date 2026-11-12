# QCC Independent Verification Oracle (`tools/independent_oracle/`)

**Current Status**: **M7 / v0.2 ACCEPTED** (Sessions 1–5 Complete — Phases 60–70 Complete).  
**Authoritative Plan**: Roadmap v3.1 (`docs/roadmap/phases-060-085.md`), ADR-0001, ADR-0003, ADR-0004.

All Python/Qiskit tooling lives strictly inside `tools/independent_oracle/` as optional dev/CI verification tooling and **is never a dependency of `qcc-core` or normal Rust execution**.

---

## 1. Pinned Environment Setup

- **Verified Python**: `3.12.3` (`>=3.12,<3.13`)
- **Pinned Dependency File**: [`requirements.txt`](./requirements.txt)
- **Key Pinned Versions**:
  - `qiskit==2.6.2`
  - `qiskit-qasm3-import==0.6.0`
  - `openqasm3==1.0.1`
  - `jsonschema==4.26.0`
  - `numpy==2.5.4`
  - `scipy==1.18.1`
  - `antlr4-python3-runtime==4.13.2`
  - `rustworkx==0.18.1`

Setup in an isolated virtual environment (`.venv/`, gitignored):

```bash
python3 -m venv .venv
.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r tools/independent_oracle/requirements.txt
.venv/bin/pip check
```

---

## 2. Running Oracle Verification & Tests

Run the Session 3 end-to-end oracle across all canonical cases in [`cases/`](./cases/):

```bash
cargo build
.venv/bin/python3 tools/independent_oracle/verify_case.py
```

Run a single canonical case and persist any failure bundle under a directory:

```bash
.venv/bin/python3 tools/independent_oracle/verify_case.py \
  --case tools/independent_oracle/cases/07_qft4_showcase_linear.json \
  --failure-dir /tmp/qcc_oracle_failures
```

Run the Session 2 QASM3 compatibility smoke check:

```bash
.venv/bin/python3 tools/independent_oracle/qasm3_compat.py
```

Run the full automated Python oracle suite (Session 2 compatibility + Session 3 canonical cases/comparator + Session 4 corpus, fault-injection & benchmark + Session 5 CI integration tests — **45 tests**):

```bash
.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v
```

### Session 5 CI Entrypoint (Matches `.github/workflows/independent-oracle.yml`)

```bash
# Run Stage 1 (pip check, 7 schemas, Session 2 QASM3 compat, 10 canonical cases, corpus regen, unittests)
.venv/bin/python3 tools/independent_oracle/ci_check.py --stage core

# Run Stage 2 (1,080-case corpus execution, 17-family fault injection, benchmark schema/semantic checks)
.venv/bin/python3 tools/independent_oracle/ci_check.py --stage campaign --benchmark-trials 2
```

### Session 4 Campaigns (Corpus, Fault Injection, Fair Baseline)

```bash
# Regenerate the 1,080-case deterministic difficult corpus
.venv/bin/python3 tools/independent_oracle/corpus/generate_corpus.py

# Execute all 1,080 corpus cases (836 exhaustive + 244 explicitly labeled sampled)
.venv/bin/python3 tools/independent_oracle/corpus/run_corpus.py

# Execute the systematic 17-family (40-trial) fault-injection campaign
.venv/bin/python3 tools/independent_oracle/fault_injection/run_campaign.py

# Execute the fair QCC-vs-Qiskit baseline (18 comparable workloads x N=5 trials)
.venv/bin/python3 tools/independent_oracle/benchmark/run_baseline.py --trials 5
```

---

## 3. Core Comparator Contract (Phases 62–66)

### Architecture Separation
- **Neutral Canonical Cases** ([`cases/`](./cases/), validated against [`canonical_case.schema.json`](./schema/canonical_case.schema.json)) define logical QASM input, `logical_qubits`, `parameter_bindings`, target config, deterministic seed, verification mode, and explicit expected status/limitations.
- **Independent Logical Reference Runner** ([`runners/qiskit_reference.py`](./runners/qiskit_reference.py)) parses canonical logical QASM independently with Qiskit, declares `input float[64]` AST nodes exclusively for `parameter_bindings` keys (`qc.assign_parameters`), rejects non-unitary/unbound inputs, and never invokes QCC.
- **Black-Box QCC & Physical Runner** ([`runners/qcc_runner.py`](./runners/qcc_runner.py)) executes `qcc compile --emit-manifest` inside an isolated working directory with relative filenames, validates `compile_manifest.schema.json` (`1.0.0`), verifies emitted physical `.qasm` SHA-256 **before** import, applies the ADR-0004 AST register adapter via `qasm3_compat.py`, and simulates in Qiskit independently of QCC's internal `LayoutEquivalenceChecker`.

### Mathematical Conventions & Clean-Ancilla Enforcement
1. **Little-Endian Indexing**: Logical qubit $q[j]$ is bit $j$ (`q[0]` LSB); physical qubit $p[a]$ is bit $a$.
2. **Initial & Final Embedding**:
   - $E_{\text{init}}: \mathbb{C}^{2^n} \to \mathbb{C}^{2^m}$ embeds logical basis state $|k\rangle_{\text{log}}$ onto physical index $\pi_{\text{init}}(k) = \sum_{j=0}^{n-1} b_j(k) \cdot 2^{L_{\text{initial}}[j]}$, initializing all physical qubits outside $\operatorname{im}(L_{\text{initial}})$ strictly to $|0\rangle$.
   - $E_{\text{final}}: \mathbb{C}^{2^n} \to \mathbb{C}^{2^m}$ embeds logical reference output amplitudes through $L_{\text{final}}$ and sets all physical amplitudes with any bit in `unused_physical_qubits_final` equal to `1` strictly to $0$ (ADR-0001; spare qubits are **never** traced out).
3. **Single Common Global Phase**:
   - Joint overlap $S = \sum \langle \Phi_{\text{expected}} | \Psi_{\text{physical}} \rangle$ across all $2^m \times K$ compared entries estimates **one scalar factor** $e^{i\phi} = S / |S|$ for the entire subspace/test batch. Relative phase errors cannot be disguised by per-state phases.
4. **Verification Modes**:
   - `exhaustive_unitary` ($m \le 8$): compares all $2^n$ logical basis columns in $2^m$-dimensional physical space using independent Qiskit `Operator` matrices.
   - `sampled_statevector`: compares deterministic computational basis states ($|0\dots0\rangle$, one-hot $|2^j\rangle$, $|1\dots1\rangle$, $|+\dots+\rangle$) plus seeded normalized complex Gaussian random states via $O(2^m)$ `Statevector.evolve`; explicitly flagged `"exhaustive": false`.
5. **Justified Numerical Tolerances**:
   - `amplitude_atol = 1e-10` (IEEE-754 `f64` OpenQASM serialization ~15–17 digits + multi-gate float composition)
   - `ancilla_atol = 1e-12` (ADR-0001 residual ancilla population sum)
   - `phase_atol = 1e-10` (informational global-phase comparison)
6. **Informational Strict Phase-Accounting Diagnostic**:
   - Reported in `strict_phase_accounting` as `PASS`, `MISMATCH`, or `UNVERIFIED` (`informational_only: true`); never overrides authoritative common-global-phase semantic equivalence.

### Limitations & Unsupported Constructs
- Static unitary domain only (`measure`, `reset`, unbound parameters, and Qiskit-missing `_STDGATES` entries `sxdg`/`i`/`u` are classified `UNSUPPORTED` or `INVALID_INPUT`, never `PASS`).
- QCC CLI accepts `x, y, z, h, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap` as frontend gates (ADR-0003); `sx` is emitted on physical targets (`linear`, `ring`), not accepted as a logical frontend gate.
- `exhaustive_unitary` enforces `physical_qubits <= 8`; wider circuits must use explicit `sampled_statevector`.

---

## 4. Real QCC OpenQASM 3 Compatibility Matrix (`qiskit==2.6.2`, `qiskit-qasm3-import==0.6.0`)

| Construct / Circuit | AST Parse (`openqasm3`) | Raw Qiskit Import | Adapted Qiskit Import | Simulate (`Statevector` / `Operator`) | Classification & Semantics Notes |
| --- | --- | --- | --- | --- | --- |
| Physical QCC QASM (`qubit[N] p;` + `include "stdgates.inc";`) | PASS | **FAIL** (`Symbol 'p' already inserted in Scope.GLOBAL`) | **PASS** (`p -> qcc_reg_p`) | **PASS** (SV + OP) | `SUPPORTED_WITH_REGISTER_RENAME` via AST alpha-renaming ([ADR-0004](../../docs/decisions/ADR-0004-qasm3-importer-register-collision-fallback.md)) |
| Frontend QASM (`qubit[N] q;` + `stdgates.inc`) | PASS | PASS | PASS | PASS | `SUPPORTED_DIRECT` |
| `rz(theta)`, `sx`, `cx q[c], q[t]` | PASS | PASS (non-`p` reg) | PASS | PASS | Exact little-endian (`q[0]` LSB) & operand order (`ctrl=0, targ=1`) match QCC invariants |
| `gphase(0)` & nonzero / negative / >$2\pi$ `gphase(...)` | PASS | PASS (non-`p` reg) | PASS | PASS | Qiskit normalizes `qc.global_phase` mod $2\pi$; `Statevector` and `Operator` apply exact $e^{i\phi}$ |
| Parameter expressions (`pi / 4`, `3 * pi / 4 - 0.5`, `-(pi / 4)`, `π / 2`, `1e-12`) | PASS | PASS (non-`p` reg) | PASS | PASS | Exact floating evaluation confirmed against analytical matrices |
| Multi-register (`qubit[2] a; qubit[1] b;`) & scalar (`qubit q;`) | PASS | PASS | PASS | PASS | Declaration order preserved |
| `c[0] = measure q[0];`, arrow `measure`, `reset` | PASS | PASS (non-`p` reg) | PASS | SV (`reset`), FAIL (`measure`/OP) | Explicitly classified `UNSUPPORTED_NON_UNITARY`; never dropped |
| `input float theta;` (unbound parameter) | PASS | PASS (non-`p` reg) | PASS | FAIL (`TypeError`) | Explicitly classified `UNSUPPORTED_UNBOUND_PARAM` |
| `sxdg`, `i`, lowercase `u(...)`, custom `include` | PASS | FAIL (`not defined`) | FAIL | FAIL | Explicitly classified `UNSUPPORTED_CONSTRUCT`; QCC `OpenQasmEmitter` accepts `sxdg`/`i`/`u`, Qiskit `_STDGATES` lacks them (`sx`, `id`, `U`, `u1`–`u3` work) |
| Malformed OpenQASM 3 syntax | FAIL | FAIL | FAIL | FAIL | Explicitly classified `PARSE_ERROR` |

---

## 5. Layout

- [`requirements.txt`](./requirements.txt) — Exact pinned Python dependencies (`==`).
- [`schema/compile_manifest.schema.json`](./schema/compile_manifest.schema.json) — Version `1.0.0` Draft 2020-12 compilation manifest JSON Schema.
- [`schema/canonical_case.schema.json`](./schema/canonical_case.schema.json) — Version `1.0.0` Draft 2020-12 canonical case JSON Schema.
- [`schema/comparison_result.schema.json`](./schema/comparison_result.schema.json) — Version `1.0.0` Draft 2020-12 comparison result JSON Schema.
- [`schema/corpus_summary.schema.json`](./schema/corpus_summary.schema.json) — Version `1.0.0` Draft 2020-12 corpus generation & execution summary schema.
- [`schema/fault_campaign_summary.schema.json`](./schema/fault_campaign_summary.schema.json) — Version `1.0.0` Draft 2020-12 fault-injection campaign summary schema.
- [`schema/benchmark_trial.schema.json`](./schema/benchmark_trial.schema.json) & [`schema/benchmark_aggregate.schema.json`](./schema/benchmark_aggregate.schema.json) — Version `1.0.0` Draft 2020-12 fair baseline per-trial & aggregate schemas.
- [`cases/`](./cases/) — 10 versioned canonical circuit cases (9 positive unitary + 1 explicit non-unitary `UNSUPPORTED` case).
- [`corpus/`](./corpus/) — Deterministic generator (`generate_corpus.py`), executor (`run_corpus.py`), committed `corpus_v1.jsonl` (1,080 cases), and `corpus_generation_summary.json`.
- [`fault_injection/run_campaign.py`](./fault_injection/run_campaign.py) — Systematic 17-family (40-trial) fault-injection harness.
- [`benchmark/run_baseline.py`](./benchmark/run_baseline.py) — Fair, semantically gated QCC-vs-Qiskit compilation-cost baseline.
- [`reports/`](./reports/) — Schema-validated Session 4 execution, fault-campaign, and benchmark summaries (`corpus_execution_summary.json`, `fault_campaign_summary.json`, `benchmark_trials.jsonl`, `benchmark_aggregate.json`).
- [`cases_loader.py`](./cases_loader.py) — Canonical case loader, validator, and SHA-256 identity helper.
- [`qasm3_compat.py`](./qasm3_compat.py) — Session 2 pinned environment verifier, AST register-collision adapter, and OpenQASM 3 compatibility classifier.
- [`runners/qiskit_reference.py`](./runners/qiskit_reference.py) — Independent Qiskit logical reference runner.
- [`runners/qcc_runner.py`](./runners/qcc_runner.py) — Black-box QCC CLI compiler & independent physical circuit runner.
- [`equivalence/layout.py`](./equivalence/layout.py) — Little-endian layout validation, $2^n \to 2^m$ embedding, and ADR-0001 ancilla population checks.
- [`equivalence/comparator.py`](./equivalence/comparator.py) — Core layout-aware, single-common-phase, clean-ancilla semantic comparator.
- [`equivalence/artifacts.py`](./equivalence/artifacts.py) — Deterministic result serializer and reproducible failure-artifact writer.
- [`verify_case.py`](./verify_case.py) — End-to-end oracle CLI runner.
- [`fixtures/session2/`](./fixtures/session2/) — Session 2 unmodified QCC CLI physical outputs.
- [`tests/test_qasm3_compatibility.py`](./tests/test_qasm3_compatibility.py) — 20 automated compatibility tests.
- [`tests/test_oracle_session3.py`](./tests/test_oracle_session3.py) — 14 automated Session 3 end-to-end & fault-detection oracle tests.
- [`tests/test_oracle_session4.py`](./tests/test_oracle_session4.py) — 7 automated Session 4 corpus, fault-campaign & benchmark baseline tests.
- [`tests/test_oracle_session5.py`](./tests/test_oracle_session5.py) — 4 automated Session 5 CI workflow & schema-integrity tests.
- [`ci_check.py`](./ci_check.py) — Two-stage CI runner invoked by [`.github/workflows/independent-oracle.yml`](../../.github/workflows/independent-oracle.yml).
