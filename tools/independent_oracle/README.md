# QCC Independent Verification Oracle (`tools/independent_oracle/`)

**Current Status**: **M7 Session 2 Complete** (Pinned Python/Qiskit Environment & Real-QCC OpenQASM 3 Compatibility).  
**Authoritative Plan**: Roadmap v3.1 (`docs/roadmap/phases-060-085.md`), ADR-0003, ADR-0004.

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

## 2. Smoke & Automated Compatibility Checks

Run the fixture + manifest smoke check:

```bash
.venv/bin/python3 tools/independent_oracle/qasm3_compat.py
```

Run the full Session 2 compatibility test suite (20 tests):

```bash
.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v
```

Validate a single compilation manifest against schema `1.0.0`:

```bash
.venv/bin/python3 -c '
import json, jsonschema
schema = json.load(open("tools/independent_oracle/schema/compile_manifest.schema.json"))
manifest = json.load(open("tools/independent_oracle/fixtures/session2/showcase_linear.manifest.json"))
jsonschema.validate(instance=manifest, schema=schema)
print("Manifest valid:", manifest["schema_id"], manifest["schema_version"])
'
```

---

## 3. Real QCC OpenQASM 3 Compatibility Matrix (`qiskit==2.6.2`, `qiskit-qasm3-import==0.6.0`)

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

## 4. Layout

- [`requirements.txt`](./requirements.txt) — Exact pinned Python dependencies (`==`).
- [`schema/compile_manifest.schema.json`](./schema/compile_manifest.schema.json) — Version `1.0.0` Draft 2020-12 manifest JSON Schema.
- [`qasm3_compat.py`](./qasm3_compat.py) — Pinned environment verifier, AST register-collision alpha-renaming adapter, construct classifier, and manifest consistency checker.
- [`fixtures/session2/`](./fixtures/session2/) — Unmodified real QCC CLI outputs (`.qasm` + `.manifest.json`) for `showcase_linear`, `showcase_ring`, `routing_linear`, `rotations_linear`, and `global_phase_linear`.
- [`tests/test_qasm3_compatibility.py`](./tests/test_qasm3_compatibility.py) — 20 automated compatibility and fail-closed classification tests.
