# Milestone M7 Session 3 Completion Report

**Session**: M7 Session 3 — Canonical Cases, Independent Runners & Core Semantic Comparator  
**Roadmaps / Phases Covered**: Roadmap v3.1 **Phases 62–66** (`Phase 62` Canonical cases, `Phase 63` Black-box QCC runner integration, `Phase 64` Independent Qiskit logical reference, `Phase 65` Endianness/layout normalization, `Phase 66` Common-phase & full physical-state comparator)  
**Baseline Commit**: `caea8aa` (M7 Session 2)  
**Date Completed**: 2026-11-12  

---

## 1. Architecture & Deliverables

Implemented the complete independent verification pipeline strictly under `tools/independent_oracle/` without modifying `qcc-core` semantics or adding Python dependencies to Rust crates:

1. **Canonical Case Schema & Loader**:
   - [`tools/independent_oracle/schema/canonical_case.schema.json`](../../../tools/independent_oracle/schema/canonical_case.schema.json) (`qcc.oracle_case` v`1.0.0`, Draft 2020-12, `additionalProperties: false`).
   - [`tools/independent_oracle/cases_loader.py`](../../../tools/independent_oracle/cases_loader.py): validates canonical JSON cases, resolves logical QASM text, computes deterministic SHA-256 identity hashes (`case_sha256`, `logical_qasm_sha256`) with zero timestamps.
   - [`tools/independent_oracle/cases/`](../../../tools/independent_oracle/cases/): 10 neutral canonical cases covering single-qubit rotations, asymmetric CX (`CX(0,1)` and `CX(1,0)`), Bell preparation, logical SWAP, non-adjacent routing (`examples/routing.qasm`), nonzero global phase & spare ancilla (`tests/fixtures/qasm3/global_phase.qasm`), 4-qubit QFT (`examples/showcase.qasm` on `linear` and `ring`), concrete parameterized bindings (`--bind`), and explicit non-unitary `UNSUPPORTED` (`examples/qasm/bell.qasm`).
2. **Separated Independent Runners**:
   - [`tools/independent_oracle/runners/qiskit_reference.py`](../../../tools/independent_oracle/runners/qiskit_reference.py): parses canonical logical QASM independently with `openqasm3`/`qiskit`, injects `input float[64]` AST declarations strictly for declared `parameter_bindings` keys (`assign_parameters`), rejects non-unitary or unbound inputs, and constructs `Operator`/`Statevector` without invoking QCC.
   - [`tools/independent_oracle/runners/qcc_runner.py`](../../../tools/independent_oracle/runners/qcc_runner.py): runs `qcc compile --emit-manifest` black-box inside an isolated working directory using relative filenames (ensuring identical manifest hashes across temporary directories), validates `compile_manifest.schema.json`, verifies emitted `.qasm` SHA-256 **before** import, applies the ADR-0004 register adapter via `qasm3_compat.py`, and simulates independently in Qiskit.
3. **Layout, Endianness, Ancilla & Comparator Engine**:
   - [`tools/independent_oracle/equivalence/layout.py`](../../../tools/independent_oracle/equivalence/layout.py): enforces `little_endian_q0_lsb` bit indexing (`q[0]` LSB) and injective `logical_to_physical` embeddings $E_{\text{init}}, E_{\text{final}}: \mathbb{C}^{2^n} \to \mathbb{C}^{2^m}$ with all spare qubits initialized and expected in $|0\rangle$ (never tracing out spare qubits).
   - [`tools/independent_oracle/equivalence/comparator.py`](../../../tools/independent_oracle/equivalence/comparator.py): compares the full $2^m \times K$ physical state matrix against embedded logical reference columns under **one joint common scalar global phase** ($S = \operatorname{Tr}(B^\dagger A)$, $e^{i\phi} = S/|S|$), enforces `ancilla_atol = 1e-12` over `(r & ancilla_mask) != 0` basis probabilities, supports explicit `exhaustive_unitary` ($m \le 8$) and `sampled_statevector` (`exhaustive: false`) modes, and computes the separate informational `strict_phase_accounting` diagnostic (`PASS` / `MISMATCH` / `UNVERIFIED`) without overriding semantic equivalence.
   - [`tools/independent_oracle/equivalence/artifacts.py`](../../../tools/independent_oracle/equivalence/artifacts.py) & [`comparison_result.schema.json`](../../../tools/independent_oracle/schema/comparison_result.schema.json): deterministic result serialization (`PASS`, `MISMATCH`, `UNSUPPORTED`, `INVALID_INPUT`, `ERROR`) and failure artifact preservation (`case.json`, `logical_input.qasm`, `physical_output.qasm`, `manifest.json`, `comparison_result.json`, `diagnostics.txt`).
   - [`tools/independent_oracle/verify_case.py`](../../../tools/independent_oracle/verify_case.py): end-to-end oracle CLI entrypoint.

---

## 2. Numerical Tolerances & Limitations

- `amplitude_atol = 1e-10`, `ancilla_atol = 1e-12`, `phase_atol = 1e-10` — justified by IEEE-754 `f64` OpenQASM text serialization (~15–17 decimal digits) composed over multi-gate rotations.
- Observed errors across all 9 positive canonical QCC runs were $\le 5.98 \times 10^{-16}$, with $0.0$ residual ancilla population (`ancilla_ok=True`) and informational `strict_phase_accounting = PASS`.
- **Empirical Gate-Subset & Width Boundaries**:
  - Physical circuits with `physical_qubits > 8` refuse `exhaustive_unitary` and require explicit `sampled_statevector` mode (`"exhaustive": false`).
  - QCC frontend CLI rejects `sx` directly as a logical input gate (`E0004`, while emitting `sx` natively on physical targets) and rejects `input float` declarations in favor of `--bind name=val`.
  - Non-unitary (`measure`, `reset`), unbound parameters, and missing `_STDGATES` entries are classified `UNSUPPORTED`, never `PASS`.

---

## 3. Canonical Case Verification Outcomes (`verify_case.py`)

| Case ID | Logical $\to$ Physical | Max Amplitude Error | Ancilla Zero | Strict Phase Accounting | Result |
| --- | :---: | :---: | :---: | :---: | --- |
| `01_single_qubit_rotations` | $1 \to 2$ | `5.98e-16` | `True` | `PASS` | `PASS` |
| `02_cx_both_directions` | $2 \to 2$ | `2.61e-16` | `True` | `PASS` | `PASS` |
| `03_bell_pair` | $2 \to 2$ | `1.11e-16` | `True` | `PASS` | `PASS` |
| `04_logical_swap` | $2 \to 2$ | `2.22e-16` | `True` | `PASS` | `PASS` |
| `05_nontrivial_routing` | $3 \to 3$ | `1.11e-16` | `True` | `PASS` | `PASS` |
| `06_nonzero_global_phase_spare_ancilla` | $1 \to 2$ | `1.39e-16` | `True` | `PASS` | `PASS` |
| `07_qft4_showcase_linear` | $4 \to 4$ | `3.40e-16` | `True` | `PASS` | `PASS` |
| `08_qft4_showcase_ring` | $4 \to 4$ | `3.40e-16` | `True` | `PASS` | `PASS` |
| `09_parameterized_bound` | $3 \to 3$ | `3.51e-16` | `True` | `PASS` | `PASS` |
| `10_non_unitary_measure_unsupported` | $2 \to \text{n/a}$ | `n/a` | `n/a` | `UNVERIFIED` | `UNSUPPORTED` |

---

## 4. Verification Commands & Counts

- `.venv/bin/python3 tools/independent_oracle/verify_case.py`: exit code `0` (all 10 canonical cases match expected statuses).
- `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: **34 tests passed**, 0 failed (`20` Session 2 compatibility + `14` Session 3 layout/end-to-end/fault-detection tests, in `3.687s`, exit code `0`).
- Draft 2020-12 JSON Schema checks on `canonical_case.schema.json`, `comparison_result.schema.json`, and all 10 canonical cases: exit code `0`.
- `cargo fmt --check`: exit code `0`.
- `cargo clippy --all-targets -- -D warnings`: exit code `0`.
- `cargo doc --no-deps`: exit code `0`.
- `cargo test --lib --test cli --test compiler_end_to_end -- --quiet`: `172` Python-free tests passed (`117` lib + `30` cli + `25` end-to-end, 0 failed, exit code `0`).
- `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: `1,401` passed across `41` test output entries, 0 failed, 0 ignored, exit code `0`.
