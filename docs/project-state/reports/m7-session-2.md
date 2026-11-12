# Milestone M7 Session 2 Completion Report

**Session**: M7 Session 2 — Pinned Python/Qiskit Environment & Real-QCC OpenQASM 3 Compatibility  
**Roadmaps / Phases Covered**: Roadmap v3.1 **Phase 61** (Pinned Python/Qiskit dev environment) & **Phase 63** (Real-QCC OpenQASM 3 import/simulation compatibility investigation & tests)  
**Baseline Commit**: `a9cabad` (M7 Session 1)  
**Date Completed**: 2026-11-12  

---

## 1. Objective & Governance Compliance

- Established an isolated, explicitly pinned Python/Qiskit verification environment under `.venv/` and [`tools/independent_oracle/requirements.txt`](../../../tools/independent_oracle/requirements.txt) with zero Python/Qiskit dependencies added to `Cargo.toml` or `qcc-core`.
- Tested actual unmodified physical OpenQASM 3 circuits and manifests generated directly by `qcc compile --emit-manifest` across QCC's real examples and fixtures (`examples/showcase.qasm`, `examples/routing.qasm`, `tests/fixtures/qasm3/rotations.qasm`, `tests/fixtures/qasm3/global_phase.qasm`).
- Evaluated construct-by-construct parsing, Qiskit `QuantumCircuit` construction, unitary `Operator` / `Statevector` simulation, global-phase preservation (`gphase`), parameter expressions, register collisions, and fail-closed rejection/classification without calling QCC's internal equivalence checker.

---

## 2. Pinned Python / Qiskit Environment

- **Python Runtime**: `3.12.3` (`.venv/bin/python3`)
- **Pinned Specification**: [`tools/independent_oracle/requirements.txt`](../../../tools/independent_oracle/requirements.txt) (all direct and transitive packages pinned strictly with `==`):
  - `qiskit==2.6.2`
  - `qiskit-qasm3-import==0.6.0`
  - `openqasm3==1.0.1`
  - `jsonschema==4.26.0`
  - `numpy==2.5.4`
  - `scipy==1.18.1`
  - `rustworkx==0.18.1`
  - `antlr4-python3-runtime==4.13.2`
  - `dill==0.4.1`, `stevedore==5.10.0`, `attrs==26.1.0`, `referencing==0.37.0`, `jsonschema-specifications==2025.9.1`, `rpds-py==2026.9.1`, `typing_extensions==4.16.0`
- Verified `.venv/bin/pip check` -> `No broken requirements found.`

---

## 3. Critical Empirical Findings (Real QCC Physical QASM vs Pinned Qiskit Importer)

1. **Physical Register Collision (`qubit[N] p;` vs `include "stdgates.inc";`) — [ADR-0004](../../decisions/ADR-0004-qasm3-importer-register-collision-fallback.md)**:
   - QCC hardware routers (`SabreBidirectionalRouter`, greedy router) name the physical register `p` (`qubit[N] p;`), and `OpenQasmEmitter` emits `include "stdgates.inc";` whenever standard gates are used.
   - `openqasm3.parser.parse()` (`openqasm3==1.0.1`) parses 100% of unmodified QCC physical OpenQASM 3 circuits cleanly.
   - However, raw `qiskit.qasm3.loads()` (`qiskit-qasm3-import==0.6.0`) **fails immediately on 100% of QCC-emitted physical OpenQASM 3 files with gates**:
     `QASM3ImporterError: "<line>,0: Symbol 'p' already inserted in symbol table in this scope: Scope.GLOBAL"` because `visit_Include("stdgates.inc")` pre-populates 30 gate names in `Scope.GLOBAL`, including `"p"` (`PhaseGate`).
   - **Documented Semantics-Preserving Adapter**: Rather than mutating QCC's Rust compiler output or rewriting fixtures by hand, [`tools/independent_oracle/qasm3_compat.py`](../../../tools/independent_oracle/qasm3_compat.py) implements `adapt_colliding_registers()`, an AST-level top-level register alpha-renamer (`p` -> `qcc_reg_p`) that:
     - Only triggers when raw import hits a `Scope.GLOBAL` symbol-table collision on declared top-level registers,
     - Renames only colliding top-level `QubitDeclaration` / `ClassicalDeclaration` identifiers and their operand/target slots (never `QuantumGate.name`, parameters, or `gphase`),
     - Fails closed (`ValueError` / `UNSUPPORTED_CONSTRUCT`) if nested gate definitions, subroutines, calibrations, aliases, or control-flow blocks exist,
     - Verifies exact statement-signature invariance via post-serialization re-parsing.
2. **Global Phase (`gphase`) Preservation**:
   - `qiskit-qasm3-import==0.6.0` translates top-level `gphase(theta);` into `QuantumCircuit.global_phase += theta` (normalized mod $2\pi$ to $[0, 2\pi)$ inside `QuantumCircuit.global_phase`).
   - Both `qiskit.quantum_info.Operator(qc)` and `qiskit.quantum_info.Statevector.from_instruction(qc)` preserve the exact scalar factor $e^{i\phi}$ (verified for $0$, $\pi/4$, $10.210176124166829$, negative scientific $-1.5\times 10^{-10}$, and multiple cumulative `gphase` statements within `< 1e-12`).
3. **Little-Endian, Operand Order, & Native Gate Semantics**:
   - Confirmed exact agreement between `qiskit==2.6.2` and QCC mathematical conventions on `rz(theta)`, `sx`, `cx q[ctrl], q[targ]` ($q[0]$ LSB, operand 0 = control, operand 1 = target), arithmetic parameter expressions (`pi / 4`, `3 * pi / 4 - 0.5`, `-(pi / 4)`, `π / 2`, `1e-12`), scalar `qubit`, and multi-register ordering.
4. **Explicitly Classified Unsupported Constructs (Never Silently Dropped)**:
   - Unrecognized/non-`_STDGATES` gates accepted by QCC `OpenQasmEmitter` (`sxdg`, `i`, lowercase `u(...)`): classified `UNSUPPORTED_CONSTRUCT` (`qiskit-qasm3-import==0.6.0` lacks them in `_STDGATES`; physical QCC targets `{rz, sx, cx}` are fully supported).
   - Non-unitary ops (`c[i] = measure q[i];`, `measure q[i] -> c[i];`, `reset q[i];`): classified `UNSUPPORTED_NON_UNITARY`.
   - Unbound parameters (`input float theta;`): classified `UNSUPPORTED_UNBOUND_PARAM`.
   - Invalid syntax / custom `include`: classified `PARSE_ERROR` / `UNSUPPORTED_CONSTRUCT`.

---

## 4. Real Unmodified QCC Fixtures Validated

All 5 fixtures in [`tools/independent_oracle/fixtures/session2/`](../../../tools/independent_oracle/fixtures/session2/) were compiled directly with `target/debug/qcc compile` (unmodified) and validated against `compile_manifest.schema.json`, SHA-256 checksum, physical width, gate counts, `gphase`, and Qiskit `Statevector`/`Operator` unitary simulation:

| Fixture | Source Circuit & Target | Physical Qubits | Gate Counts | Emitted `gphase` (rad) | Qiskit `global_phase` (rad) | Status |
| --- | --- | :---: | --- | :---: | :---: | --- |
| `showcase_linear` | `examples/showcase.qasm` (`linear`) | 4 | `cx: 27, rz: 26, sx: 7` | `10.210176124166829` | `3.9269908169872423` ($\equiv \text{mod } 2\pi$) | `supported_with_register_rename` |
| `showcase_ring` | `examples/showcase.qasm` (`ring`) | 4 | `rz: 26, cx: 24, sx: 7` | `10.210176124166829` | `3.9269908169872423` ($\equiv \text{mod } 2\pi$) | `supported_with_register_rename` |
| `routing_linear` | `examples/routing.qasm` (`linear`) | 3 | `cx: 6, rz: 2, sx: 1` | `0.7853981633974483` | `0.7853981633974483` | `supported_with_register_rename` |
| `rotations_linear` | `tests/fixtures/qasm3/rotations.qasm` (`linear`) | 2 | `rz: 7, sx: 4, cx: 1` | `3.141592653589793` | `3.141592653589793` | `supported_with_register_rename` |
| `global_phase_linear` | `tests/fixtures/qasm3/global_phase.qasm` (`linear`, 1Q $\to$ 2Q) | 2 | `rz: 3, sx: 2` | `5.497787143782138` | `5.497787143782138` | `supported_with_register_rename` |

---

## 5. Verification Evidence

- `.venv/bin/pip check`: Passed (`No broken requirements found.`, exit code `0`).
- `.venv/bin/python3 tools/independent_oracle/qasm3_compat.py`: Passed (all 5 real QCC fixtures + schema + SHA-256 + `Statevector`/`Operator` valid, exit code `0`).
- `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (**20 tests passed**, 0 failed, in `1.027s`, exit code `0`).
- `cargo fmt --check`: Passed (exit code `0`).
- `cargo clippy --all-targets -- -D warnings`: Passed (exit code `0`).
- `cargo doc --no-deps`: Passed (exit code `0`).
- `cargo test --lib --test cli --test compiler_end_to_end -- --quiet`: Passed (`172` Python-free tests: `117` lib + `30` cli + `25` end-to-end, 0 failed, exit code `0`).
- `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: Passed (`1,401` passed across `41` test output entries, 0 failed, 0 ignored, exit code `0`).

---

## 6. Deferred to Next Session (**M7 Session 3**)

- Canonical logical circuit reference construction (independent of QCC's equivalence checker).
- Black-box QCC compilation runner and independent Qiskit physical simulation runner.
- Endianness & initial/final layout embedding + strict ADR-0001 physical ancilla zero-state check across full $2^{n_{\text{phys}}}$ statevectors.
- Authoritative common-global-phase comparator + separate informational strict phase-accounting diagnostic (`PASS` / `MISMATCH` / `UNVERIFIED`).
