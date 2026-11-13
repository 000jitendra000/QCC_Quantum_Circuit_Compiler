# QCC Implementation Progress Ledger

```text
Project: QCC
Repository: QCC_Quantum_Circuit_Compiler
Historical Checkpoint: Phase 59 (Integrated Compilation Pipeline — Phases 1–59 Complete)
Active Roadmap: Roadmap v3.1 (Phases 60–85, Milestones M7–M12; previous 105-phase roadmap superseded)
Active Next Task: Milestone M9 Phase 77 — Routing search validation and comparative benchmark (Phase 76 Completed)
```

## Current Checkpoint

- **Historical Baseline Checkpoint**: **Phase 59** (`59` phases complete across Milestones M1–M6, preserved and verified)
- **Active Engineering Plan**: **Roadmap v3.1** (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap_v3.1.docx`, 12 November 2026 — Phases 60–85 across Milestones M7–M12). The previous 105-phase roadmap (old Phases 60–105) is **superseded**.
- **Completed v3.1 Milestones**:
  - **Milestone M7 — Independent Verification Oracle v0.2 (Phases 60–70, Sessions 1–5 Complete & Accepted 2026-11-12)**
  - **Milestone M8 — Compiler Introspection and Diagnostics (Phases 71–73 Complete & Accepted 2026-11-13)**
- **Active Next Action**: **Milestone M9 (Phase 77) — Routing search validation and comparative benchmark**
- **Latest Accepted Checkpoint**: **Phase 76 Complete** (`Phases 60–76` complete).

---

## Milestone Progress Overview

> [!NOTE]
> Phases 1–59 (M1–M6) represent the completed historical baseline at the Phase 59 checkpoint.
> Active continuation follows **Roadmap v3.1 (Phases 60–85)**. Noise simulation, density-matrix simulation, dynamic CFG execution, KAK, MPS, and provider adapters remain deferred outside Phases 60–85.

| Milestone | Code | Phases | Completed | Status |
|---|:---:|:---:|:---:|:---:|
| **Milestone M1: Bell Vertical Slice** | `M1` | 1–11 | 11 / 11 | **Completed (Baseline)** |
| **Milestone M2: Compiler Foundations & QASM** | `M2` | 12–27 | 16 / 16 | **Completed (Baseline)** |
| **Milestone M3: Verified Logical Optimizer** | `M3` | 28–36 | 9 / 9 | **Completed (Baseline)** |
| **Milestone M4: Generic Target & Basis Translation** | `M4` | 37–43 | 7 / 7 | **Completed (Baseline)** |
| **Milestone M5: Logical-to-Physical Mapping & Routing** | `M5` | 44–53 | 10 / 10 | **Completed (Baseline)** |
| **Milestone M6: Scheduling & Integrated Compiler** | `M6` | 54–59 | 6 / 6 | **Completed (Checkpoint)** |
| **Milestone M7: Independent Verification Oracle (v0.2)** | `M7` | 60–70 *(5 Sessions)* | 11 / 11 *(5 / 5 sessions)* | **Completed (v0.2 Accepted)** |
| **Milestone M8: Compiler Introspection & Diagnostics** | `M8` | 71–73 | 3 / 3 | **Completed (Accepted)** |
| **Milestone M9: Multi-Candidate Routing Improvements** | `M9` | 74–77 | 3 / 4 | **In Progress** |
| **Milestone M10: Evidence-Selected Optimizer Improvements** | `M10` | 78–81 | 0 / 4 | **Planned (Evidence-Gated)** |
| **Optional Milestone M11: Bounded Hardware Profiles** | `M11` | 82–84 | 0 / 3 | **Optional (Go/No-Go after M10)** |
| **Milestone M12: Integrated Upgrade Acceptance** | `M12` | 85 | 0 / 1 | **Planned** |

---

## M7 Five-Session Tracker (Phases 60–70 Execution Plan)

| Session | Phases Covered | Focus | Status | Exit Gate Proof |
|---|---|---|---|---|
| **Session 1** | 60, 63 *(manifest)* | Contract, ADR, manifest schema, manifest-first Rust emission | **Completed** (2026-11-12) | ADR-0003, JSON Schema, `--emit-manifest`, 1,401 tests + `jsonschema` pass ([report](./reports/m7-session-1.md)) |
| **Session 2** | 61, 63 *(QASM import)* | Pinned Python/Qiskit env & real QCC-emitted QASM import validation | **Completed** (2026-11-12) | Pinned `requirements.txt`, ADR-0004, `qasm3_compat.py`, 5 real fixtures, 20 Python compat tests + 1,401 Rust tests pass ([report](./reports/m7-session-2.md)) |
| **Session 3** | 62–66 | Canonical cases, independent runners, layout/endian/phase/ancilla comparator + informational phase accounting | **Completed** (2026-11-12) | 10 canonical cases, independent Qiskit/QCC runners, layout/ancilla/common-phase comparator, 34 Python + 1,401 Rust tests pass ([report](./reports/m7-session-3.md)) |
| **Session 4** | 67, 68, 70 *(baseline)* | Fault injection, 1,000+ difficult deterministic corpus, matched-$N$ Qiskit baseline | **Completed** (2026-11-12) | 1,080/1,080 corpus PASS, 40/40 faults (17 families) caught, 180 benchmark trials (90/90 QCC+Qiskit PASS), 41 Python + 1,401 Rust tests ([report](./reports/m7-session-4.md)) |
| **Session 5** | 69, 70 *(acceptance)* | Required CI oracle job, reproducible artifacts, README & v0.2 acceptance report | **Completed** (2026-11-12) | `.github/workflows/independent-oracle.yml`, `ci_check.py`, `README.md`, 45 Python + 1,401 Rust tests pass, M7 v0.2 ACCEPTED ([report](./reports/m7-session-5.md)) |

---

## Detailed Phase Status Ledger (Baseline 1–59 + v3.1 Phases 60–85)

> [!NOTE]
> Phases 60–105 of the previous 105-phase roadmap have been **superseded** by Roadmap v3.1 Phases 60–85 listed below.

| Phase # | Title | Milestone | Effort | Status | Completion Date | Exit Gate Satisfied |
|:---:|---|:---:|:---:|:---:|:---:|:---:|
| 1 | Repository scaffold, CI and error infrastructure | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 2 | Quantum semantics and gate definitions | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 3 | Single-qubit in-place statevector kernels | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 4 | Two-qubit and controlled-gate kernels | `M1` | **L** | **Completed** | 2026-11-01 | Passed |
| 5 | Terminal measurement and seeded sampling | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 6 | DAG-based QccIR | `M1` | **XL** | **Completed** | 2026-11-01 | Passed |
| 7 | Rust circuit builder API | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 8 | QccIR execution adapter | `M1` | **L** | **Completed** | 2026-11-01 | Passed |
| 9 | OpenQASM 3 emitter | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 10 | Global-phase-aware unitary equivalence checker | `M1` | **XL** | **Completed** | 2026-11-01 | Passed |
| 11 | Bell vertical-slice acceptance | `M1` | **M** | **Completed** | 2026-11-01 | Passed |
| 12 | Pass manager interface | `M2` | **L** | **Completed** | 2026-11-01 | Passed |
| 13 | Transactional DAG rewrite infrastructure | `M2` | **XL** | **Completed** | 2026-11-01 | Passed |
| 14 | Gate algebra and inverse registry | `M2` | **M** | **Completed** | 2026-11-01 | Passed |
| 15 | OpenQASM parser dependency evaluation | `M2` | **M** | **Completed** | 2026-11-01 | Passed |
| 16 | QASM frontend adapter boundary | `M2` | **M** | **Completed** | 2026-11-01 | Passed |
| 17 | QASM source buffer and lexer | `M2` | **L** | **Completed** | 2026-11-01 | Passed |
| 18 | OpenQASM AST model | `M2` | **L** | **Completed** | 2026-11-01 | Passed |
| 19 | QASM register declaration parsing | `M2` | **M** | **Completed** | 2026-11-01 | Passed |
| 20 | Standard gate statement parsing | `M2` | **M** | **Completed** | 2026-11-02 | Passed |
| 21 | Parameter expression parser | `M2` | **L** | **Completed** | 2026-11-02 | Passed |
| 22 | Measurement, reset and barrier parser | `M2` | **M** | **Completed** | 2026-11-02 | Passed |
| 23 | Semantic analysis and diagnostics | `M2` | **L** | **Completed** | 2026-11-02 | Passed |
| 24 | Unsupported syntax detection | `M2` | **M** | **Completed** | 2026-11-02 | Passed |
| 25 | AST-to-QccIR lowering | `M2` | **L** | **Completed** | 2026-11-02 | Passed |
| 26 | QASM import/export round-trip | `M2` | **L** | **Completed** | 2026-11-02 | Passed |
| 27 | QASM fuzzing, conformance and frontend CLI | `M2` | **L** | **Completed** | 2026-11-02 | Passed |
| 28 | Identity-gate elimination | `M3` | **S** | **Completed** | 2026-11-02 | Passed |
| 29 | Inverse-gate cancellation | `M3` | **M** | **Completed** | 2026-11-02 | Passed |
| 30 | Same-axis rotation merging | `M3` | **M** | **Completed** | 2026-11-02 | Passed |
| 31 | Single-qubit Euler decomposition | `M3` | **XL** | **Completed** | 2026-11-03 | Passed |
| 32 | One-qubit gate fusion | `M3` | **L** | **Completed** | 2026-11-03 | Passed |
| 33 | Commutation analysis rules | `M3` | **L** | **Completed** | 2026-11-03 | Passed |
| 34 | DAG-aware commutation rewriting | `M3` | **XL** | **Completed** | 2026-11-03 | Passed |
| 35 | Optimizer integration and reports | `M3` | **L** | **Completed** | 2026-11-03 | Passed |
| 36 | Canonical logical gate catalog | `M3` | **M** | **Completed** | 2026-11-03 | Passed |
| 37 | Generic Target data model | `M4` | **L** | **Completed** | 2026-11-04 | Passed |
| 38 | Target capability queries and cost models | `M4` | **M** | **Completed** | 2026-11-04 | Passed |
| 39 | Fixed decomposition template library | `M4` | **L** | **Completed** | 2026-11-04 | Passed |
| 40 | Native single-qubit synthesis | `M4` | **XL** | **Completed** | 2026-11-04 | Passed |
| 41 | Basis translation engine | `M4` | **XL** | **Completed** | 2026-11-04 | Passed |
| 42 | Two-qubit direction correction | `M4` | **M** | **Completed** | 2026-11-04 | Passed |
| 43 | Final target legality validator | `M4` | **M** | **Completed** | 2026-11-04 | Passed |
| 44 | Logical/physical layout model | `M5` | **M** | **Completed** | 2026-11-04 | Passed |
| 45 | Layout-aware equivalence | `M5` | **L** | **Completed** | 2026-11-04 | Passed |
| 46 | Initial placement algorithms | `M5` | **L** | **Completed** | 2026-11-04 | Passed |
| 47 | Baseline shortest-path router | `M5` | **XL** | **Completed** | 2026-11-04 | Passed |
| 48 | SWAP insertion and dependency maintenance | `M5` | **L** | **Completed** | 2026-11-04 | Passed |
| 49 | Routing objectives and metrics | `M5` | **M** | **Completed** | 2026-11-04 | Passed |
| 50 | SABRE front-layer analysis | `M5` | **L** | **Completed** | 2026-11-05 | Passed |
| 51 | SABRE heuristic, candidate scoring and decay | `M5` | **XL** | **Completed** | 2026-11-05 | Passed |
| 52 | Bidirectional SABRE and layout refinement | `M5` | **XL** | **Completed** | 2026-11-05 | Passed |
| 53 | Router validation and comparative benchmarks | `M5` | **XL** | **Completed** | 2026-11-05 | Passed |
| 54 | Target timing and resource model | `M6` | **L** | **Completed** | 2026-11-05 | Passed |
| 55 | ASAP scheduler | `M6` | **L** | **Completed** | 2026-11-05 | Passed |
| 56 | ALAP scheduler | `M6` | **L** | **Completed** | 2026-11-05 | Passed |
| 57 | Resource-constrained scheduling | `M6` | **XL** | **Completed** | 2026-11-05 | Passed |
| 58 | Schedule serialization and visualization data | `M6` | **M** | **Completed** | 2026-11-05 | Passed |
| 59 | Integrated compilation pipeline | `M6` | **XL** | **Completed** | 2026-11-05 | Passed |
| 60 | Independent oracle architecture and acceptance contract | `M7` *(Session 1)* | **M** | **Completed** | 2026-11-12 | Passed |
| 61 | Pinned Python and Qiskit development environment | `M7` *(Session 2)* | **M** | **Completed** | 2026-11-12 | Passed |
| 62 | Canonical circuit cases and deterministic generators | `M7` *(Session 3)* | **L** | **Completed** | 2026-11-12 | Passed |
| 63 | QCC black-box runner and compilation manifest | `M7` *(Sessions 1–3)* | **M/L** | **Completed** | 2026-11-12 | Passed |
| 64 | Qiskit logical reference and transpilation baseline | `M7` *(Session 3)* | **L** | **Completed** | 2026-11-12 | Passed |
| 65 | Endianness and initial/final layout normalization | `M7` *(Session 3)* | **L** | **Completed** | 2026-11-12 | Passed |
| 66 | Common-global-phase and full physical-state comparator | `M7` *(Session 3)* | **XL** | **Completed** | 2026-11-12 | Passed |
| 67 | Oracle self-tests and fault-injection validation | `M7` *(Session 4)* | **L** | **Completed** | 2026-11-12 | Passed |
| 68 | Deterministic circuit corpus and scale-up campaign | `M7` *(Session 4)* | **XL** | **Completed** | 2026-11-12 | Passed |
| 69 | CI oracle integration and reproducible regression gates | `M7` *(Session 5)* | **L** | **Completed** | 2026-11-12 | Passed |
| 70 | Fair Qiskit comparison and v0.2 acceptance report | `M7` *(Sessions 4–5)* | **L** | **Completed** | 2026-11-12 | Passed |
| 71 | Per-pass diagnostic and report schema | `M8` | **M** | **Completed** | 2026-11-13 | Passed |
| 72 | Pass-manager instrumentation and verification hooks | `M8` | **L** | **Completed** | 2026-11-13 | Passed |
| 73 | CLI and documentation for compiler introspection | `M8` | **M** | **Completed** | 2026-11-13 | Passed |
| 74 | Multi-candidate routing configuration and objectives | `M9` | **M** | **Completed** | 2026-11-13 | Passed |
| 75 | Seeded multi-run SABRE candidate generation | `M9` | **L** | **Active Next** | — | Pending |
| 76 | Deterministic candidate ranking and selection | `M9` | **M** | **Not Started** | — | Pending |
| 77 | Routing search validation and comparative benchmark | `M9` | **L** | **Not Started** | — | Pending |
| 78 | Benchmark-gap analysis and optimizer work selection | `M10` | **M** | **Not Started** | — | Pending |
| 79 | Selected optimizer rule specification and local proof obligations | `M10` | **L** | **Not Started** | — | Pending |
| 80 | Implement one evidence-selected optimizer improvement | `M10` | **L** | **Not Started** | — | Pending |
| 81 | Independent optimizer validation and release decision | `M10` | **L** | **Not Started** | — | Pending |
| 82 | Hardware target capability gap audit | `M11` *(Optional)* | **M** | **Not Started** | — | Pending |
| 83 | Per-qubit hardware properties and target profile loading | `M11` *(Optional)* | **L** | **Not Started** | — | Pending |
| 84 | Hardware-profile-aware legality and scheduling validation | `M11` *(Optional)* | **L** | **Not Started** | — | Pending |
| 85 | Integrated upgrade acceptance and roadmap closeout | `M12` | **L** | **Not Started** | — | Pending |

---

## Completed Phase Evidence Log

### Phase 1 — Repository scaffold, CI and error infrastructure
- **Completion Date**: 2026-11-01
- **Milestone**: Milestone M1 (`M1`)
- **Files Created**: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/errors.rs`, `src/diagnostics/{mod,span,diagnostic}.rs`, `tests/smoke.rs`, `.github/workflows/ci.yml`.
- **Files Modified**: `README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-001.md`](./reports/phase-001.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test`: Passed (13 tests passed, 0 failed, 0 ignored: 8 unit tests in `src/lib.rs`, 5 integration tests in `tests/smoke.rs` including 500 proptest iterations)
  - `cargo build`: Passed (code 0)
  - `cargo run -- --version`: Passed (code 0, outputs `qcc 0.1.0`)
- **Exit Gate Proof**: CI workflow authored and validated with matching commands; CLI version command and smoke tests pass.

### Phase 2 — Quantum semantics and gate definitions
- **Completion Date**: 2026-11-01
- **Milestone**: Milestone M1 (`M1`)
- **Files Created**: `src/quantum/{mod,gate,gate_matrix,semantics,parameter}.rs`, `tests/quantum_semantics.rs`, `docs/semantics.md`, `docs/project-state/plans/phase-002.md`, `docs/project-state/reports/phase-002.md`.
- **Files Modified**: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-002.md`](./reports/phase-002.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test`: Passed (58 tests passed, 0 failed, 0 ignored: 30 unit tests, 23 quantum semantics tests including proptests, 5 smoke tests)
  - `cargo build`: Passed (code 0)
  - `cargo run -- --version`: Passed (code 0, outputs `qcc 0.1.0`)
- **Exit Gate Proof**: Documented semantic conventions in `docs/semantics.md`; exact matrix definitions, unitarity, inverse-composition, rotation identity, and operand order tests pass.

### Phase 18 — OpenQASM AST model
- **Completion Date**: 2026-11-01
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/ast/{mod,program,declaration,statement,expression}.rs`, `tests/ast.rs`, `tests/ast/{program,declaration,statement,expression,golden,spans}.rs`, `docs/project-state/plans/phase-018.md`, `docs/project-state/reports/phase-018.md`.
- **Files Modified**: `docs/project-state/progress.md`.
- **Files Removed**: `src/frontend/ast.rs` (replaced by modular directory).
- **Report**: [`docs/project-state/reports/phase-018.md`](./reports/phase-018.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all-targets`: Passed (361 tests passed across 20 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - Examples and Python reference: Passed (code 0)
- **Exit Gate Proof**: AST completely covers the documented input subset (program headers, version, includes, sized/scalar register declarations, gate applications, measurements, resets, barriers, global phase directives, parameter expressions, and hierarchical source spans). Deterministic structural serialization and span containment tests pass.

### Phase 19 — QASM register declaration parsing
- **Completion Date**: 2026-11-01
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/parser/{mod,registers,declarations}.rs`, `tests/qasm3.rs`, `tests/qasm3/declarations.rs`, `docs/project-state/plans/phase-019.md`, `docs/project-state/reports/phase-019.md`.
- **Files Modified**: `src/frontend/mod.rs`, `src/frontend/adapter.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-019.md`](./reports/phase-019.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all-targets`: Passed (375 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Register declaration syntax parsing (`OPENQASM 3.0;`, `include`, sized/scalar `qubit` and `bit` declarations) and semantic validation (symbol table, duplicate detection across namespaces, zero/invalid sizes, undeclared registers, $[0, n)$ bounds validation, and declaration order preservation) fully tested and passed.

### Phase 20 — Standard gate statement parsing
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/parser/{gates,operands}.rs`, `tests/qasm3/gates.rs`, `docs/project-state/plans/phase-020.md`, `docs/project-state/reports/phase-020.md`.
- **Files Modified**: `src/frontend/parser/mod.rs`, `src/frontend/adapter.rs`, `src/frontend/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-020.md`](./reports/phase-020.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all-targets`: Passed (389 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Parsing of all supported standard MVP gates (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `rx`, `ry`, `rz`, `cx`, `cz`, `swap`), case-sensitive validation, parameter-first syntax, parameter expression AST preservation, operand ordering preservation ($CX$ control/target and $SWAP$/$CZ$ sequences), register bounds checks, and source-located diagnostics pass with zero errors and zero warnings.

### Phase 21 — Parameter expression parser
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/parser/expressions.rs`, `tests/qasm3/expressions.rs`, `docs/project-state/plans/phase-021.md`, `docs/project-state/reports/phase-021.md`.
- **Files Modified**: `src/frontend/parser/mod.rs`, `src/frontend/parser/gates.rs`, `src/frontend/adapter.rs`, `src/frontend/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-021.md`](./reports/phase-021.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all-targets`: Passed (408 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Parameter expressions parse correctly with Pratt precedence and left associativity, preserve exact AST structure and source spans without lossy eager conversion, round-trip cleanly, reject literal division by zero with `E0030`, reject post-binding division by zero with `E0030`, and reject unbound symbols at evaluation time with `E0031` rather than silently defaulting to zero.

### Phase 22 — Measurement, reset and barrier parser
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/parser/{measurement,reset,barrier}.rs`, `tests/qasm3/non_unitary.rs`, `docs/project-state/plans/phase-022.md`, `docs/project-state/reports/phase-022.md`.
- **Files Modified**: `src/frontend/parser/registers.rs`, `src/frontend/parser/operands.rs`, `src/frontend/parser/mod.rs`, `src/frontend/adapter.rs`, `src/frontend/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-022.md`](./reports/phase-022.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all-targets`: Passed (419 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Measurement statement parsing (single-bit and whole-register in both assignment and arrow forms), active reset statement parsing, global and selective barrier synchronization parsing, quantum source and classical target type validation, bounds checks, register size matching (`E0025`), non-unitary classification, and statement ordering pass all automated tests with zero warnings and zero errors.

### Phase 23 — Semantic analysis and diagnostics
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/diagnostics/qasm.rs`, `src/frontend/semantic/{mod,symbols,checks}.rs`, `tests/qasm3/semantic.rs`, `docs/project-state/plans/phase-023.md`, `docs/project-state/reports/phase-023.md`.
- **Files Modified**: `src/diagnostics/mod.rs`, `src/frontend/adapter.rs`, `src/frontend/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-023.md`](./reports/phase-023.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (463 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Semantic negative corpus and positive validation suite (43 semantic tests, 102 total qasm3 tests) pass cleanly with zero warnings and zero errors. The semantic analyzer rejects undeclared registers (`E0012`), out-of-bounds indices (`E0014`), parameter and qubit arity mismatches (`E0021`, `E0022`), invalid measurement destinations/sources (`E0013`), whole-register size mismatches (`E0025`), unresolved parameters (`E0031`), division by zero (`E0030`), non-finite parameters (`E0032`), parameter domain violations (`E0024`), duplicate qubit operands in multi-qubit gates (`E0023`), and invalid reset/barrier operands (`E0013`), attaching stable diagnostic codes and exact source spans.

### Phase 24 — Unsupported syntax detection
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/unsupported.rs`, `tests/qasm3/unsupported/{mod,conditionals,loops,subroutines,gate_defs,calibrations,types,gates,recovery}.rs`, `docs/qasm-supported-subset.md`, `docs/project-state/plans/phase-024.md`, `docs/project-state/reports/phase-024.md`.
- **Files Modified**: `src/frontend/{token,lexer,adapter,mod}.rs`, `src/frontend/parser/gates.rs`, `src/frontend/semantic/checks.rs`, `src/diagnostics/qasm.rs`, `src/diagnostics/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-024.md`](./reports/phase-024.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (514 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Unsupported-feature conformance tests pass cleanly (`130/130` in `tests/qasm3.rs`, `514/514` workspace total). Unsupported constructs (conditionals, loops, subroutines, gate definitions, calibrations, unsupported types, modifiers, timing delays, legacy keywords, three-qubit gates, and un-decomposed standard gates) produce deterministic, source-located diagnostics (`E0002`–`E0005`), recover cleanly via `AstStatement::Unsupported` without silent statement omission, and fail closed before lowering.

### Phase 25 — AST-to-QccIR lowering
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `src/frontend/lowering/{mod,registers,operations}.rs`, `tests/qasm3/lowering.rs`, `docs/project-state/plans/phase-025.md`, `docs/project-state/reports/phase-025.md`.
- **Files Modified**: `src/diagnostics/qasm.rs`, `src/frontend/adapter.rs`, `src/frontend/mod.rs`, `src/lib.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-025.md`](./reports/phase-025.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (537 tests passed across 21 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Semantic equivalence between supported OpenQASM-lowered circuits and equivalent Rust builder-created circuits verified via UnitaryEquivalenceChecker (max error < 1e-10) for Bell states, parameterized rotations, bound symbolic expressions, and global phase accumulation. Register allocation, typed QubitId/ClassicalBitId mapping, CX operand ordering (control=0, target=1), single-bit and whole-register measurement, active reset, synchronization barriers, and graph invariants validated cleanly across 23 lowering integration and exit-gate validation tests (153 total in `tests/qasm3.rs`, 537 total workspace tests). Non-unitary measurement and reset operations are rigorously excluded from unitary equivalence checking, while structural, topological (wire dependencies), and behavioral sampling equivalence are proven against builder circuits.

### Phase 26 — QASM import/export round-trip
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`)
- **Files Created**: `tests/fixtures/qasm3/{bell,rotations,non_unitary,multi_register,global_phase,teleportation}.qasm`, `tests/qasm3/roundtrip.rs`, `docs/qasm3_support.md`, `docs/project-state/plans/phase-026.md`, `docs/project-state/reports/phase-026.md`.
- **Files Modified**: `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-026.md`](./reports/phase-026.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (553 tests passed across all 21 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0 on all fixtures and emitted programs)
- **Exit Gate Proof**: Independent parse and semantic round-trip suite passes cleanly (16 round-trip tests, 169 total in `tests/qasm3.rs`, 553 total workspace tests). Emitted QASM text validates against independent reference parser `openqasm3` v1.0.1. Re-imported circuits match original circuits structurally, topologically, and behaviorally (unitary equivalence for pure-unitary circuits, identical seeded sampling for measurement circuits). Double round-trip re-emission is strictly idempotent ($S_1 == S_2$), and formatting insensitivity (whitespace, comments, multi-line expressions, arrow vs assignment syntax) is verified. Unitary and non-unitary randomized property tests, runtime symbol binding round trips, and automated reference parser checks all pass with zero warnings and zero errors.

### Phase 27 — QASM fuzzing, conformance and frontend CLI
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M2 (`M2`) — **Milestone Complete**
- **Files Created**: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/{fuzz_lexer,fuzz_parser}.rs`, `fuzz/corpus/{lexer,parser}/*`, `src/cli/{mod,compile,inspect}.rs`, `tests/cli.rs`, `tests/qasm3/fuzz_regression.rs`, `tests/qasm3/conformance/{mod,fixtures/*}`, `docs/qasm-conformance.md`, `docs/qasm-fuzzing.md`, `docs/cli-reference.md`, `docs/project-state/plans/phase-027.md`, `docs/project-state/reports/phase-027.md`.
- **Files Modified**: `Cargo.toml`, `src/diagnostics/qasm.rs`, `src/frontend/lowering/registers.rs`, `src/frontend/parser/{declarations,expressions,gates,operands}.rs`, `src/lib.rs`, `src/main.rs`, `tests/qasm3.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-027.md`](./reports/phase-027.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (615 tests passed across all 24 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo run --bin fuzz_lexer -- --smoke`: Passed (code 0, 1000 iterations + 14 synthetic edge cases)
  - `cargo run --bin fuzz_parser -- --smoke`: Passed (code 0, 1000 iterations + 19 synthetic edge cases)
  - `cargo run --bin fuzz_lexer -- fuzz/corpus/lexer/` and `cargo run --bin fuzz_parser -- fuzz/corpus/parser/`: Passed (code 0)
  - `cargo run --bin qcc -- --help`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0 on all 7 supported fixtures)
- **Exit Gate Proof**: Complete frontend conformance suite (34 automated tests covering 7 supported fixtures, 13 unsupported fail-closed constructs including gate `p` requiring decomposition, and 13 syntax/semantic error fixtures), fuzz targets and regression corpus (6 tests + standalone binaries with recursion and operand limits), and CLI integration suite (20 tests covering `qcc compile` and `qcc inspect`, format flags, parameter bindings, stdin, rendered diagnostics, and stable exit codes 0/1/2/3) all pass cleanly (615 total workspace tests across all 24 test targets, +62 net new tests from Phase 26 baseline). Diagnostic code immutability (E0010–E0016, E0020–E0025) validated with `test_diagnostic_code_stability_constants`. Milestone M2 (Phases 12–27) is fully completed.

### Phase 28 — Identity-gate elimination
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/optimizer/passes/identity.rs`, `tests/optimizer/identity.rs`, `docs/project-state/plans/phase-028.md`, `docs/project-state/reports/phase-028.md`.
- **Files Modified**: `src/optimizer/passes/mod.rs`, `src/optimizer/mod.rs`, `src/lib.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-028.md`](./reports/phase-028.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (650 tests passed across all 25 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Identity-elimination pass satisfies all equivalence and DAG invariant tests (35 tests in `tests/optimizer.rs`, 650 total workspace tests). Explicit identities (`id`, `i`), continuous rotations zero modulo $4\pi$ (exact zero), and rotations $2\pi$ modulo $4\pi$ (with $+\pi$ scalar global-phase tracking) are eliminated while preserving circuit unitary equivalence up to one common global phase. Non-unitary boundaries (measure, reset, barrier, classical computation) and unresolved symbolic expressions are strictly preserved.

### Phase 29 — Inverse-gate cancellation
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/optimizer/passes/cancellation.rs`, `tests/optimizer/cancellation.rs`, `docs/project-state/plans/phase-029.md`, `docs/project-state/reports/phase-029.md`.
- **Files Modified**: `src/optimizer/passes/mod.rs`, `src/lib.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-029.md`](./reports/phase-029.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (689 tests passed across all 26 test targets, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser: Passed (code 0)
- **Exit Gate Proof**: Inverse-gate cancellation pass strictly preserves circuit semantics and DAG invariants (74 tests in `tests/optimizer.rs`, 689 total workspace tests). Single-qubit discrete self-inverses (H, X, Y, Z), discrete adjoint pairs (S-Sdg, T-Tdg), two-qubit self-inverses (CX-CX, CZ-CZ, SWAP-SWAP), and continuous rotation pairs ($0 \pmod{4\pi}$) cancel cleanly. Strict exact ordered operand contract enforced at pass boundary (`CZ(0,1)`/`CZ(1,0)` and `SWAP(0,1)`/`SWAP(1,0)` rejected as candidate pairs). Intervening gates on shared wires and non-unitary boundaries (`Measure`, `Reset`, `Barrier`) strictly block cancellation, while commuting disjoint wires cancel safely.

### Phase 30 — Same-axis rotation merging
- **Completion Date**: 2026-11-02
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/quantum/angle.rs`, `src/optimizer/passes/rotation_merge.rs`, `tests/optimizer/rotation_merge.rs`, `docs/project-state/plans/phase-030.md`, `docs/project-state/reports/phase-030.md`.
- **Files Modified**: `src/ir/operation.rs`, `src/lib.rs`, `src/optimizer/passes/mod.rs`, `src/quantum/mod.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-030.md`](./reports/phase-030.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (731 tests passed across all 26 test targets [22 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: Rotation merging passes randomized unitary-equivalence checks across 30 randomized multi-qubit trials and property-based tests (113 tests in `tests/optimizer.rs`, 731 total workspace tests). Adjacent same-axis single-qubit rotations ($R_x, R_y, R_z$) combine angles accurately. Exact $4\pi$ matrix periodicity eliminates exact zero identities ($0$ phase adjustment), and $2\pi$ shifts apply $+\pi$ global phase adjustments. Unresolved symbolic parameters remain unchanged under `PreserveUnresolved`, while algebraic opposites ($p$ and $-p$, including simplified forms) safely cancel to zero. Expressions containing division by zero fail closed. Intervening operations, non-unitary boundaries (`Measure`, `Reset`, `Barrier`), and different rotation axes strictly prevent merging, while operations on unrelated wires commute in the DAG. Stable IDs, provenance, and transactional rollback are fully preserved.

### Phase 31 — Single-qubit Euler decomposition
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/decomposition/mod.rs`, `src/decomposition/euler.rs`, `tests/euler.rs`, `docs/project-state/plans/phase-031.md`, `docs/project-state/reports/phase-031.md`.
- **Files Modified**: `src/quantum/gate_matrix.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-031.md`](./reports/phase-031.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (756 tests passed across all 27 test targets [23 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: Matrix reconstruction and phase tests pass over randomized cases (25 tests in `tests/euler.rs`, 756 total workspace tests). Arbitrary $2 \times 2$ unitary matrices factor cleanly into $SU(2)$ rotation sequences and scalar global phase factors. Canonical $Z\text{-}Y\text{-}Z$ and $Z\text{-}X\text{-}Z$ bases strictly observe column-vector and left-multiplication conventions. Polar angle $\theta$ is extracted stably in $[0, \pi]$, and gimbal-lock configurations ($\theta = 0$ and $\theta = \pi$) resolve deterministically without numerical instability. Double-cover spinor alignment correctly tracks $2\pi$ phase adjustments, ensuring reconstructed matrices $U_{\text{rec}} = e^{i\alpha} V$ match the original within machine precision ($\le 10^{-10}$). Verified against 100 Haar-distributed random unitaries, proptest property suites, canonical quantum gates, and Phase 10 unitary equivalence checking.

### Phase 32 — One-qubit gate fusion
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/optimizer/passes/one_qubit_fusion.rs`, `tests/optimizer/fusion.rs`, `docs/project-state/plans/phase-032.md`, `docs/project-state/reports/phase-032.md`.
- **Files Modified**: `src/ir/rewrite/replace.rs`, `src/ir/rewrite/mod.rs`, `src/optimizer/passes/mod.rs`, `src/lib.rs`, `tests/optimizer.rs`, `tests/ir_rewrites.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-032.md`](./reports/phase-032.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (796 tests passed across all 27 test targets [23 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: Random fused blocks pass common-phase equivalence (34 tests in `tests/optimizer/fusion.rs`, 6 tests in `tests/ir_rewrites.rs`, 796 total workspace tests). Single-qubit sequences on each wire are fused in left-multiplication order ($M = U_k \dots U_1$), decomposed into canonical Euler rotations via Phase 31 (`decompose_euler`), and resynthesized with exact scalar global phase preservation. Non-unitary boundaries (`Measure`, `Reset`, `Barrier`, `Classical`), multi-qubit gates ($CX, CZ, SWAP$), and unresolvable symbolic parameters strictly terminate fusion windows. Disjoint wire operations fuse independently. 50 randomized fused blocks of lengths 2 to 8 verified against Phase 10 unitary equivalence checker with common global phase, passing with machine-precision residuals ($\le 10^{-14}$).

### Phase 33 — Commutation analysis rules
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/commutation/mod.rs`, `src/commutation/pauli.rs`, `src/commutation/cx.rs`, `tests/optimizer/commutation.rs`, `docs/project-state/plans/phase-033.md`, `docs/project-state/reports/phase-033.md`.
- **Files Modified**: `src/lib.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-033.md`](./reports/phase-033.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (838 tests passed across all 27 test targets [23 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: All commutation rules pass pairwise matrix tests (34 tests in `tests/optimizer/commutation.rs`, 8 unit tests in `src/lib.rs`, 838 total workspace tests). Established mathematically verified commutation analysis registry with explicit semantic preconditions across 12 verified rules. Enforced symmetric two-direction classical hazard checks (RAW, WAR, WAW) ensuring no classical reordering can be approved. Proved pairwise commutation via matrix commutator Frobenius norm ($\|[A, B]\|_F < 10^{-12}$) for disjoint wires, Z-axis operations on CX control ($Z, S, S^\dagger, T, T^\dagger, R_z$), X-axis operations on CX target ($X, R_x$), symmetric CZ with Z-axis, CX shared-control, CX shared-target, and CX-CZ control overlap pairs. Pauli algebra verifies commuting and anticommuting pairs ($\{X, Y\} = 0$ never classified as commuting). Non-commuting pairs ($X$ on control, $Z$ on target, reversed $CX(0, 1)$ vs $CX(1, 0)$) and non-unitary operations (`Measure`, `Reset`, `Barrier`, `Classical`) strictly rejected. Verified against Phase 10 unitary equivalence checker and basis state permutations.

### Phase 34 — DAG-aware commutation rewriting
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/ir/rewrite/swap.rs`, `src/optimizer/passes/commutation_reorder.rs`, `tests/optimizer/commutation_reorder.rs`, `docs/project-state/plans/phase-034.md`, `docs/project-state/reports/phase-034.md`.
- **Files Modified**: `src/ir/rewrite/mod.rs`, `src/optimizer/passes/mod.rs`, `src/lib.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-034.md`](./reports/phase-034.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (879 tests passed across all 27 test targets [23 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: Pass is deterministic, bounded and equivalence-tested (31 tests in `tests/optimizer/commutation_reorder.rs`, 9 unit tests in `src/lib.rs`, 879 total workspace tests). Bounded reordering pass (`CommutationReorderPass`) evaluates adjacent candidates within `window_size` and reorders commuting operations via transactional swapping (`swap_adjacent_operations`, `reorder_operations`). Operations never cross side-effect boundaries (`Measure`, `Reset`, `Barrier`, `Classical`) or classical hazard boundaries (RAW, WAR, WAW). Bounded-window discovery strictly enforces intervening dependency checking and cannot skip intervening non-commuting operations. Anti-oscillation history tracking and finite rewrite budgets (`max_rewrites`, `max_iterations`) guarantee termination and strict cycle freedom. Opportunity-driven strategy successfully exposes adjacent cancellation ($R_z/CX$ control in Phase 29, $X/CX$ target, $CX$-$CX$ shared control) and same-axis merging ($R_z/CZ$ in Phase 30). Non-commuting operations ($H$ on $CX$ control or target, $X$ on $CX$ control, $Z$ on $CX$ target, reversed $CX(0, 1)$ vs $CX(1, 0)$) are strictly rejected. All reorderings pass unitary equivalence against Phase 10 checker under a common global phase.

### Phase 35 — Optimizer integration and reports
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`)
- **Files Created**: `src/optimizer/config.rs`, `src/optimizer/pipeline.rs`, `src/optimizer/report.rs`, `tests/optimizer/pipeline.rs`, `docs/project-state/plans/phase-035.md`, `docs/project-state/reports/phase-035.md`.
- **Files Modified**: `src/optimizer/mod.rs`, `src/lib.rs`, `tests/optimizer.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-035.md`](./reports/phase-035.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (900 tests passed across all 27 test targets [23 active targets], 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1): Passed (code 0)
- **Exit Gate Proof**: Optimizer pipeline is deterministic, passes equivalence verification, and produces structured pass reports (21 tests in `tests/optimizer/pipeline.rs`, 232 total optimizer integration tests, 900 total workspace tests). Composed canonical optimization pipeline (`OptimizerPipeline`) across optimization levels `O0` through `O3`, integrating identity elimination (Phase 28), inverse cancellation (Phase 29), same-axis rotation merging (Phase 30), single-qubit Euler fusion (Phases 31-32), and DAG-aware commutation reordering (Phases 33-34). Audited and hardened: (1) transactional candidate circuit execution preserving pre-pass circuit state upon graph validation or equivalence failure, (2) per-pass equivalence verification using immediate pre/post circuits with configured tolerances and symbol environment, (3) proper classification of barriers as semantically unitary identity operations eligible for equivalence checking, (4) strict distinction of fixed-point convergence (`Converged`) from iteration-limit exhaustion (`IterationLimitReached`), and (5) exact tracking of accumulated circuit global phase across multiple passes. Full test-mode unitary equivalence verified against Phase 10 checker under a single common global phase.

### Phase 36 — Canonical logical gate catalog
- **Completion Date**: 2026-11-03
- **Milestone**: Milestone M3 (`M3`) — **Milestone M3 Completed**
- **Files Created**: `src/gate_signature.rs`, `src/gate_catalog.rs`, `tests/gate_catalog.rs`, `docs/project-state/plans/phase-036.md`, `docs/project-state/reports/phase-036.md`.
- **Files Modified**: `src/commutation/cx.rs`, `src/commutation/pauli.rs`, `src/gate_catalog.rs`, `src/ir/operation.rs`, `src/ir/validation.rs`, `src/lib.rs`, `src/optimizer/algebra/inverses.rs`, `src/optimizer/passes/cancellation.rs`, `src/optimizer/passes/commutation_reorder.rs`, `src/optimizer/passes/identity.rs`, `src/optimizer/passes/one_qubit_fusion.rs`, `src/quantum/gate.rs`, `src/quantum/gate_matrix.rs`, `tests/gate_catalog.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-036.md`](./reports/phase-036.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (933 tests passed across all 28 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Catalog is stable and covered by signature tests (32 tests in `tests/gate_catalog.rs`, 933 total workspace tests across 28 test binaries). Established canonical `GateId` (16 target-independent logical gates: 9 fixed 1Q gates, 4 parameterized 1Q gates, 3 fixed 2Q gates), `GateSignature`, and `GateClassification` (`Logical`, `TargetNative`, `Unknown`), defining quantum arity, parameter count, parameter domains (`Angle4Pi` for $R_x, R_y, R_z$, `Angle2Pi` for $P$, `UnconstrainedReal`), operational category (strictly `OperationCategory::Unitary`), inverse metadata (`SelfInverse`, `Paired`, `ParameterNegation`), operand roles, and symmetry. Strictly separates target-independent logical gates from hardware-native instruction sets: vendor operations (`sx`, `sy`, `ecr`, `iswap`, `rzz`, `fsim`) return `None` from `GateCatalog::lookup` and classify as `TargetNative`, while unknown names strictly classify as `Unknown`. Proved internal catalog aliases (`cnot`, `i`, `phase`, `u1`, `sdag`) do not silently expand the accepted OpenQASM subset (`cnot`/`i`/`phase`/`sdag` reject with `E0020`, `u1`/`p` reject with `E0004`). Unitary matrix cross-checks ($U^\dagger U = I$ within $10^{-12}$, $U \cdot U_{\text{inv}} = I$), CX asymmetry ($CX(0, 1) \neq CX(1, 0)$), CZ/SWAP symmetry ($SWAP \cdot CZ \cdot SWAP = CZ$), exhaustive 16-gate matrix coverage, case-insensitive alias lookup, property-based tests for arbitrary rotation angles, complete adoption across `ir::validation`, `optimizer::algebra::inverses`, `optimizer::passes::{identity, cancellation, one_qubit_fusion, commutation_reorder}`, and `commutation::{cx, pauli}`, and explicit operation classification (`Operation::category()`, `Operation::semantic_class()`, `Operation::is_unitary()`, `GateCatalog::is_unitary()`) separating unitary gates from non-unitary operations (`Measure`, `Reset`, `Barrier`, `Classical`) are fully verified. Milestone M3 (Verified Logical Optimizer) is now 100% complete (9 / 9 phases).

### Phase 37 — Generic Target data model
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`) — Initial Phase of M4
- **Files Created**: `src/target/mod.rs`, `src/target/model.rs`, `src/target/operation.rs`, `src/target/connectivity.rs`, `src/target/calibration.rs`, `src/target/error.rs`, `tests/target.rs`, `docs/project-state/plans/phase-037.md`, `docs/project-state/reports/phase-037.md`.
- **Files Modified**: `src/errors.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-037.md`](./reports/phase-037.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (959 tests passed across all 29 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Synthetic target fixtures validate correctly (26 tests in `tests/target.rs`, 959 total workspace tests across 29 test binaries). Established generic hardware `Target`, `PhysicalQubitId`, `TargetOpId`, `TargetOperationDescriptor`, operation-specific `OperationConnectivity` (directed vs undirected coupling, physical operand tuple validation), `CouplingGraph` topology algorithms, and `TargetCalibration` with full `Provenance` tracking (`source`, `timestamp`, `method`) adhering strictly to the Explicit Unknown Principle (missing timing/calibration modeled as `None`, never zero). Verified synthetic targets for directed CX, undirected CZ, native ECR, and mixed multi-family architectures; validated error rejection on zero qubits, out-of-bounds qubits, self-loop edges, arity mismatches, and invalid metric bounds; confirmed deterministic JSON serialization and validating deserialization; and proved complete decoupling from compiler-core logical `GateId` definitions.

### Phase 38 — Target capability queries and cost models
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`)
- **Files Created**: `src/target/metrics.rs`, `src/target/query.rs`, `src/target/cost.rs`, `tests/target_capabilities.rs`, `tests/target_cost.rs`, `docs/project-state/plans/phase-038.md`, `docs/project-state/reports/phase-038.md`.
- **Files Modified**: `src/target/mod.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-038.md`](./reports/phase-038.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (977 tests passed across all 31 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Capability and cost queries pass fixtures with partial metadata (18 new tests: 9 in `tests/target_capabilities.rs`, 9 in `tests/target_cost.rs`, 977 total workspace tests across 31 test binaries). Established `MetricKind` (`Measured`, `Estimated`), `MetricValue<T>` (`Measured`, `Estimated`, `Absent`), `DurationCost` (`Exact`, `Incomplete`, `Unknown`), `ErrorCost` (`Exact`, `Incomplete`, `Unknown`), `LegalityError` enum, `TargetQuery` trait and inherent methods, `OperationCost`, `CircuitCost`, `CostWeights`, and `CostModel`. Strictly preserves the Explicit Unknown Principle: uncalibrated metrics remain `Absent`, never defaulted to zero. Fully calibrated operations yield exact critical path duration and error bounds; fixtures with partial calibration yield `DurationCost::Incomplete` with the exact count of uncalibrated operations and accumulated lower-bound duration; and completely absent calibrations yield `DurationCost::Unknown`. Validates native operation support, canonical `GateId` bridging with alias resolution, parameter count/finiteness/periodic domain checking, asymmetric directed coupling enforcement, duplicate operand detection, concurrent execution across disjoint wires, DAG critical path tracking with wire and barrier dependencies, and bitwise-deterministic multi-objective weighted scoring.

### Phase 39 — Fixed decomposition template library
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`)
- **Files Created**: `src/synthesis/mod.rs`, `src/synthesis/templates/mod.rs`, `src/synthesis/templates/matrix.rs`, `src/synthesis/templates/swap.rs`, `src/synthesis/templates/cx_cz.rs`, `src/synthesis/templates/toffoli.rs`, `src/synthesis/templates/registry.rs`, `tests/synthesis.rs`, `tests/synthesis/templates/swap.rs`, `tests/synthesis/templates/cx_cz.rs`, `tests/synthesis/templates/toffoli.rs`, `tests/synthesis/templates/registry.rs`, `docs/project-state/plans/phase-039.md`, `docs/project-state/reports/phase-039.md`.
- **Files Modified**: `src/quantum/gate_matrix.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-039.md`](./reports/phase-039.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1010 tests passed across all 32 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Template library passes full local matrix comparison; decompositions produce verified-equivalent subcircuits (33 new integration tests in `tests/synthesis.rs`, 8 unit tests in `src/synthesis/templates/`, 1010 total workspace tests across 32 test binaries). Established `DecompositionTemplate` trait, `TemplateId`, `Precondition` constraints (`DistinctOperands`, `QuantumArity`, `NoParameters`), `TemplateError` structured diagnostics, `Matrix8x8` complex matrix representation with independent single-qubit and CX embeddings and exact reference Toffoli matrix construction, and `TemplateRegistry` with strictly verified admission. Implemented exact discrete decompositions: (1) 2Q $\text{SWAP}(a, b) = CX(a, b); CX(b, a); CX(a, b)$, (2) 2Q basis transformations $CX \to CZ$ and $CZ \to CX$ via Hadamard conjugation on the target qubit, and (3) canonical 15-gate Nielsen & Chuang exact Toffoli ($CCX$) decomposition (6 $CX$, 2 $H$, 4 $T$, 3 $T^\dagger$) guaranteeing strictly zero relative phase across all basis states ($\Delta \phi = 0$). All templates verified against independent matrix models with discrepancies $< 10^{-15}$. Centralized transactional graph replacements (`replace_sequence` / `CircuitTransaction`) validated on QccIR circuits preserving dependency DAG convexity, cycle-freedom, and wire hazards. Full circuit-level unitary equivalence verified via Phase 10 checker.

### Phase 40 — Native single-qubit synthesis
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`)
- **Files Created**: `src/target/synthesis_basis.rs`, `src/synthesis/single_qubit.rs`, `tests/synthesis/single_qubit.rs`, `docs/project-state/plans/phase-040.md`, `docs/project-state/reports/phase-040.md`.
- **Files Modified**: `src/quantum/gate_matrix.rs`, `src/simulator/operation_dispatch.rs`, `src/target/model.rs`, `src/target/mod.rs`, `src/synthesis/mod.rs`, `src/lib.rs`, `tests/synthesis.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-040.md`](./reports/phase-040.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1022 tests passed across all 32 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Random unitary synthesis tests and target legality checks pass (12 new integration tests in `tests/synthesis/single_qubit.rs`, 1022 total workspace tests across 32 test binaries). Defined `SingleQubitBasis` (`Zyz`, `Zxz`, `RzSx`, `U3`), target basis capability queries (`supported_single_qubit_bases`, `preferred_single_qubit_basis`), `SingleQubitSynthesis` trait and `SingleQubitSynthesizer`. Implemented generic $Z\text{-}Y\text{-}Z$, $Z\text{-}X\text{-}Z$, $RZ\text{-}SX$, and $U3$ synthesis routines with exact global phase preservation ($U = e^{i\alpha} \prod U_k$), continuous angle normalization into $[0, 2\pi)$ with modulo $4\pi$ spinor sign tracking ($e^{i\pi} = -1$), and deterministic Euler gimbal-lock singularity resolution. All emitted operations verified via `target.check_operation_legality`. Haar-random unitary synthesis verified within $10^{-12}$ across all supported target bases, and end-to-end circuit lowering verified with statevector simulation and Phase 10 equivalence checker.

### Phase 41 — Basis translation engine
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`)
- **Files Created**: `src/translation/mod.rs`, `src/translation/rules.rs`, `src/translation/engine.rs`, `src/translation/pass.rs`, `tests/translation.rs`, `tests/translation/engine.rs`, `docs/project-state/plans/phase-041.md`, `docs/project-state/reports/phase-041.md`.
- **Files Modified**: `src/lib.rs`, `src/simulator/kernels/controlled.rs`, `src/simulator/kernels/mod.rs`, `src/simulator/operation_dispatch.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-041.md`](./reports/phase-041.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,039 tests passed across all 33 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Translated circuits pass local unitary equivalence verification and target operation checks (17 new integration tests in `tests/translation/engine.rs`, 1,039 total workspace tests across 33 test binaries). Implemented `TranslationRule` trait, `TranslationRuleRegistry`, `BasisTranslationEngine`, `BasisTranslationPass`, and built-in rules for SWAP, CX, CZ, Toffoli, single-qubit Euler synthesis, and symbolic parameter preservation. Unsupported operations recursively decomposed with cycle protection and recursion bounds, candidate sequences selected via `CostModel`, and local matrix equivalence verified prior to candidate acceptance. Emitted operations verified against target constraints.

### Phase 42 — Two-qubit direction correction
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`)
- **Files Created**: `src/target/direction.rs`, `src/translation/direction.rs`, `tests/translation/direction.rs`, `docs/project-state/plans/phase-042.md`, `docs/project-state/reports/phase-042.md`.
- **Files Modified**: `src/lib.rs`, `src/target/mod.rs`, `src/translation/mod.rs`, `src/translation/rules.rs`, `src/simulator/kernels/controlled.rs`, `tests/translation.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-042.md`](./reports/phase-042.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,050 tests passed across all 33 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Direction correction tests pass for both orientations. Every direction-sensitive operation is either corrected into a legal target orientation or explicitly rejected before routing/export (11 new integration tests in `tests/translation/direction.rs`, 26 total translation tests, 1,050 total workspace tests across 33 test binaries). Implemented `DirectionStatus` enum (`Legal`, `NeedsReversal`, `NoCoupling`, `UnsupportedOperation`, `ArityMismatch`), `DirectionError` enum, and target direction queries (`Target::check_direction`, `is_operation_directed`, `is_direction_sensitive`). Defined `CxDirectionCorrectionRule` executing canonical verified Hadamard-conjugated reversal ($(H \otimes H) CX_{1,0} (H \otimes H) = CX_{0,1}$), `CzDirectionCommutationRule` commuting symmetric $CZ$ operands on directed targets without inserting Hadamards, `TargetDirectionRule` trait, `DirectionCorrectionRegistry`, `TwoQubitDirectionCorrector`, and `TwoQubitDirectionPass`. Unified direction rules into default `TranslationRuleRegistry` for end-to-end basis translation with recursive single-qubit synthesis into native $RZ\text{-}SX$ bases. Strict little-endian matrix equality verified within $10^{-12}$, impossible orientations on disconnected qubits explicitly rejected with `NoViableDirection`, and final target legality verified across all emitted operations.

### Phase 43 — Final target legality validator
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M4 (`M4`) — **Milestone Complete**
- **Files Created**: `src/diagnostics/target.rs`, `src/target/legality.rs`, `tests/target/legality.rs`, `tests/target_legality.rs`, `docs/project-state/plans/phase-043.md`, `docs/project-state/reports/phase-043.md`.
- **Files Modified**: `src/diagnostics/mod.rs`, `src/target/mod.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-043.md`](./reports/phase-043.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,064 tests passed across all 34 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: All intentionally illegal target fixtures fail with useful diagnostics, while valid fixtures pass (14 new integration tests in `tests/target_legality.rs`, 1,064 total workspace tests across 34 test binaries). Implemented `TargetDiagnosticCode` enum (`T0001`–`T0008`) and structured diagnostic builder `create_target_diagnostic` with sequence numbers, node IDs, and actionable hints. Implemented independent `TargetLegalityValidator` validating operation identity, quantum arity, parameter count/domain, physical qubit capacity, operand distinctness, and directed coupling topology. Implemented `TargetLegalityPass` (`PassMutation::None`) executing seamlessly inside `PassManager`. Strictly enforced "Explicit Unknown" invariant ensuring missing duration or calibration metadata does not make operations illegal. Demonstrated re-validation capabilities across translation, direction correction, and post-rewrite lowering. All tests pass with zero warnings.

### Phase 44 — Logical/physical layout model
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/mod.rs`, `src/routing/layout_map.rs`, `src/routing/layout.rs`, `tests/routing/layout.rs`, `tests/routing.rs`, `docs/project-state/plans/phase-044.md`, `docs/project-state/reports/phase-044.md`.
- **Files Modified**: `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-044.md`](./reports/phase-044.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,080 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Layout invariants are validated and round-trip correctly. Logical and physical identities are distinct and cannot be accidentally interchanged (9 new integration tests in `tests/routing.rs`, 7 unit tests in `src/routing/`, 1,080 total workspace tests across 35 test binaries). Reused distinct `QubitId` and `PhysicalQubitId` types. Implemented `LayoutMap` (aliased as `Layout`) with deterministic `BTreeMap` storage, forward/reverse bidirectional lookups, injectivity validation, partial mapping support, explicit unused physical qubit tracking (`unused_physical_qubits`, `is_physical_idle` per ADR-0001), and physical SWAP mutation (`swap_physical(p1, p2)`). Implemented `CircuitLayout` encapsulating `initial`, `current`, and `final_layout` states across compiler passes. Implemented deterministic serialization and validating deserialization rejecting corrupted/duplicate mappings fail-closed. All tests pass with zero warnings.

### Phase 45 — Layout-aware equivalence
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/verification/layout_equivalence.rs`, `tests/equivalence/layout.rs`, `docs/project-state/plans/phase-045.md`, `docs/project-state/reports/phase-045.md`.
- **Files Modified**: `src/verification/unitary.rs`, `src/verification/mod.rs`, `src/lib.rs`, `tests/equivalence.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-045.md`](./reports/phase-045.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,098 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Correctly routed circuits pass layout-aware semantic comparison, incorrect final permutations fail, unused physical qubits satisfy the mandatory $|0\rangle$ ancilla policy, and all router fixtures use layout-aware semantic comparison (16 integration tests in `tests/equivalence/layout.rs`, 2 unit tests in `src/verification/layout_equivalence.rs`, 1,098 total workspace tests across 35 test binaries). Compiler router pass algorithms (`BaselineRouter`, `SabreRouter`, etc.) are scheduled for Phases 47–53 and do not yet exist in the codebase; all existing routing verification fixtures and tests use `LayoutEquivalenceChecker`. Single common global phase is strictly enforced across all input probes via `GlobalPhaseEstimator::estimate_common_phase`, accepting uniform non-trivial global phases (e.g. $\phi = \pi/3$) and rejecting state-dependent relative phase differences ($Z$ vs $I$, $S$ vs $I$) across both basis-probe and randomized-probe modes. Implemented `embed_logical_state` embedding $2^m$ logical statevectors into $2^n$ physical registers respecting little-endian bit positions and zeroing unused ancillae. Implemented `LayoutEquivalenceChecker` and `check_layout_equivalence` verifying full $2^n$-dimensional statevectors against one common scalar global phase. Enforced strict ADR-0001 zero-state check detecting ancilla leakage, uncomputed operations, or residual entanglement with precise diagnostic attribution. All tests pass with zero warnings.

### Phase 46 — Initial placement algorithms
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/placement/{mod,interaction_graph,distance,trivial,degree,weighted,score,strategy}.rs`, `tests/routing/placement.rs`, `docs/project-state/plans/phase-046.md`, `docs/project-state/reports/phase-046.md`.
- **Files Modified**: `src/routing/mod.rs`, `src/lib.rs`, `src/target/connectivity.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-046.md`](./reports/phase-046.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,117 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo build --all-targets`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - Python `openqasm3` reference parser (pinned v1.0.1 via `test_independent_openqasm3_parser_on_qasm_example_artifact` and `test_independent_parser_presence_and_golden_fixtures_conformance`): Passed (code 0)
- **Exit Gate Proof**: Placement fixtures yield reproducible, valid mappings or explicitly report impossible capacity (19 integration tests in `tests/routing/placement.rs`, 28 total routing tests in `tests/routing.rs`, 1,117 total workspace tests across 35 test binaries). Implemented `InteractionGraph` extracting two-qubit interaction frequencies, incident degrees, and weighted rankings. Implemented `TopologyDistances` computing all-pairs shortest paths and physical degrees via BFS. Implemented `IdentityPlacer` ($q_i \to p_i$) and `RandomPlacer` (strictly reproducible Fisher-Yates shuffle parameterized by an explicit 64-bit seed). Implemented `DegreePlacer` matching logical interaction hubs to physical topology hubs and adjacent physical neighbors. Implemented `WeightedPlacer` evaluating candidate physical anchor edges for the heaviest logical edge and greedily minimizing weighted distance sums. Implemented `PlacementScore` evaluating weighted/unweighted distance sums, max distance, direct interaction fraction, and disconnected penalties with deterministic ordering. Added `CouplingGraph::ring` and `CouplingGraph::star` topology constructors. Verified across Line, Ring, Grid, Star, and Irregular topologies. Verified complete logical coverage for isolated/single-qubit/measured qubits, capacity rejection across all strategies, and disconnected component penalty handling. Injective mapping and ADR-0001 idle physical ancilla tracking verified. All tests pass with zero warnings.

### Phase 47 — Baseline shortest-path router
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/pathfinding.rs`, `src/routing/swap_insertion.rs`, `src/routing/greedy.rs`, `tests/routing/greedy.rs`, `docs/project-state/plans/phase-047.md`, `docs/project-state/reports/phase-047.md`.
- **Files Modified**: `src/routing/mod.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-047.md`](./reports/phase-047.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,139 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
- **Exit Gate Proof**: Baseline router passes topology and layout-aware semantic tests (16 new integration tests in `tests/routing/greedy.rs`, 6 unit tests in `src/routing/pathfinding.rs`, 44 total routing tests in `tests/routing.rs`, 1,139 total workspace tests across 35 test binaries). Implemented `find_shortest_path` executing deterministic BFS on coupling graphs with sorted neighbor expansion and explicit disconnected component detection. Implemented path-based SWAP movement strategies (`SourceToTarget`, `TargetToSource`, `Bidirectional`) with synchronous `CircuitLayout` mutation. Handled native SWAP vs decomposed CX with direction correction. Implemented `BaselineRouter` with deterministic Kahn's DAG traversal scheduling executable nodes, initial layout resolution via placement algorithms, bounded iteration/SWAP limits, target legality validation via `TargetLegalityValidator`, and layout-aware equivalence verification via `LayoutEquivalenceChecker`. Verified across Line, Ring, Grid, Star, Disconnected, and Directed topologies, with ADR-0001 strict ancilla zero-state preservation on partially occupied devices. All tests pass with zero warnings.

### Phase 48 — SWAP insertion and dependency maintenance
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/provenance.rs`, `src/routing/route_state.rs`, `tests/routing/swap_rewrite.rs`, `docs/project-state/plans/phase-048.md`, `docs/project-state/reports/phase-048.md`.
- **Files Modified**: `src/ir/graph.rs`, `src/ir/circuit.rs`, `src/routing/swap_insertion.rs`, `src/routing/pathfinding.rs`, `src/routing/greedy.rs`, `src/routing/mod.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-048.md`](./reports/phase-048.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,149 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
- **Exit Gate Proof**: Inserted SWAP nodes update layout and dependencies correctly on both wires (10 new integration tests in `tests/routing/swap_rewrite.rs`, 54 total routing tests in `tests/routing.rs`, 1,149 total workspace tests across 35 test binaries). Implemented `RouteProvenanceTracker` and `RouteProvenanceRecord` tracking forward/reverse provenance mapping for logical operations, routing SWAPs, and direction corrections. Implemented `RouteState` transactional state manager encapsulating physical `Circuit`, `CircuitLayout`, `Target`, and provenance tracker with post-rewrite QccIR validation and layout-aware semantic equivalence verification. Inserted physical SWAPs correctly update both physical wires in the QccIR dependency DAG (Read-After-Write, Write-After-Read, Write-After-Write hazard synchronization) and active layout permutations. Preserved original logical operation IDs across all physical operations. Confirmed fail-closed rejection of non-adjacent physical SWAPs and duplicate operands, and confirmed inconsistent layout updates fail equivalence checks. All tests pass with zero warnings.

### Phase 49 — Routing objectives and metrics
- **Completion Date**: 2026-11-04
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/metrics.rs`, `src/routing/cost.rs`, `benches/routing_baseline.rs`, `tests/routing/metrics.rs`, `docs/project-state/plans/phase-049.md`, `docs/project-state/reports/phase-049.md`.
- **Files Modified**: `Cargo.toml`, `src/routing/greedy.rs`, `src/routing/mod.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-049.md`](./reports/phase-049.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,160 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - `cargo bench --bench routing_baseline`: Passed (code 0, 8 benchmark fixtures verified for 100% deterministic reproducibility)
- **Exit Gate Proof**: Baseline benchmark corpus is reproducible and all router results expose comparable metrics and provenance (11 new integration tests in `tests/routing/metrics.rs`, 65 total routing tests in `tests/routing.rs`, 1,160 total workspace tests across 35 test binaries, 8 reproducible benchmark fixtures in `benches/routing_baseline.rs`). Implemented `RoutingMetrics` calculating critical-path DAG depth, critical-path two-qubit depth, total/1Q/2Q/multi-qubit gate counts, logical routing SWAP counts, native entangler counts, direction corrections, gate count histograms, hardware duration under the Explicit Unknown Principle (`DurationCost::Exact`, `Incomplete`, `Unknown`), initial and final layout snapshots, random seed, wall-clock duration, and composite cost. Implemented `RoutingCostWeights` and `RoutingCostModel` providing customizable, multi-objective evaluation with validation against negative weights and optional strict timing requirements. Verified hand-calculated fixtures for depth, entanglers, duration, and costs. Benchmarked on representative topologies (linear, ring, grid, star, parallel layers, idle ancillae, and random-seeded placement) and confirmed strict reproducibility. All tests pass with zero warnings.

### Phase 50 — SABRE front-layer analysis
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/sabre/mod.rs`, `src/routing/sabre/frontier.rs`, `src/routing/sabre/front_layer.rs`, `src/routing/sabre/lookahead.rs`, `tests/routing/sabre_frontier.rs`, `docs/project-state/plans/phase-050.md`, `docs/project-state/reports/phase-050.md`.
- **Files Modified**: `src/routing/mod.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-050.md`](./reports/phase-050.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,178 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - `cargo bench --bench routing_baseline`: Passed (code 0, 8 benchmark fixtures verified for 100% deterministic reproducibility)
- **Exit Gate Proof**: Front layer and lookahead layers correctly reflect DAG dependencies and are verified against hand-calculated DAGs (18 integration tests in `tests/routing/sabre_frontier.rs`, 83 total routing tests in `tests/routing.rs`, 1,178 total workspace tests across 35 test binaries). Implemented `SabreFrontier` tracking DAG in-degrees, completed operations, and currently executable 1Q and 2Q operations strictly obeying predecessor dependencies and QccIR wire hazards. Implemented `FrontLayerOp` and `FrontLayer` dynamically resolving physical operands through active layouts, detecting direct target adjacency/executability, and supporting dynamic layout permutations. Implemented `LookaheadConfig`, `LookaheadOp`, and `LookaheadSet` simulating forward DAG progression, transparently advancing intervening single-qubit gates, and bounding upcoming two-qubit operations by budget $W$ (`max_size`) and layer depth (`max_depth`). Implemented `SabreFrontierState` coordinating frontier, front layer, lookahead set, and layout mapping with atomic `layout_version` tracking preventing stale physical coordinates upon candidate SWAP evaluation. Verified across linear chains, independent parallel gates, diamond DAGs, interleaved single-qubit gates, candidate SWAP mutations, budget truncations, depth limits, and direct hardware executability. All tests pass with zero warnings.

### Phase 51 — SABRE heuristic, candidate scoring and decay
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/sabre/candidate.rs`, `src/routing/sabre/decay.rs`, `src/routing/sabre/scoring.rs`, `src/routing/sabre/heuristic.rs`, `tests/routing/sabre_heuristic.rs`, `docs/project-state/plans/phase-051.md`, `docs/project-state/reports/phase-051.md`.
- **Files Modified**: `src/routing/sabre/mod.rs`, `src/routing/mod.rs`, `src/routing/pathfinding.rs`, `src/routing/sabre/frontier.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-051.md`](./reports/phase-051.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,196 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - `cargo bench --bench routing_baseline`: Passed (code 0, 8 benchmark fixtures verified for 100% deterministic reproducibility)
- **Exit Gate Proof**: Heuristic fixture rankings and repeatability tests pass (18 integration tests in `tests/routing/sabre_heuristic.rs`, 101 total routing tests in `tests/routing.rs`, 1,196 total workspace tests across 35 test binaries). Implemented `CandidateSwap` canonical undirected physical pairs ($u < v$) and `CandidateGenerator` extracting unique candidate SWAPs on coupling edges adjacent to unresolved front-layer operands. Implemented `DecayTable` managing dynamic per-qubit decay multipliers ($\text{decay}(p) \ge 1.0$) with configurable increment $\delta$, transactional bounds checking, and reset upon forward front-layer progress. Implemented `ScoreComponents` with explainable breakdowns ($H_F, H_E, W, decay\_factor, total\_score$) and strict transitive 5-tier deterministic total order hierarchy using IEEE 754-2008 `total_cmp`. Implemented `SabreHeuristicConfig` and `SabreHeuristic` evaluating candidates against hypothetical layout clones without mutating active compiler state, computing normalized front-layer and lookahead distance sums, decay multipliers, and deterministic candidate rankings. Handled disconnected physical coupling graphs with mathematically justified penalty ($\max(N, 1) \times 1000 \gg N - 1$) and fast-fail error handling (`RoutingError::DisconnectedTopology`). Verified on hand-calculated 5-qubit line fixtures, varying lookahead weights ($W=0.0, 0.5, 2.0$), decay updates, symmetric star tie-breaking, disconnected topologies, state immutability, and 100% deterministic repeatability across runs. All tests pass with zero warnings.

### Phase 52 — Bidirectional SABRE and layout refinement
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M5 (`M5`)
- **Files Created**: `src/routing/sabre/bidirectional.rs`, `src/routing/sabre/layout_refinement.rs`, `tests/routing/sabre_bidirectional.rs`, `docs/project-state/plans/phase-052.md`, `docs/project-state/reports/phase-052.md`.
- **Files Modified**: `src/routing/pathfinding.rs`, `src/routing/sabre/mod.rs`, `src/routing/sabre/heuristic.rs`, `src/routing/sabre/layout_refinement.rs`, `src/routing/sabre/bidirectional.rs`, `src/routing/mod.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-052.md`](./reports/phase-052.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --all`: Passed (1,214 tests passed across all 35 test binaries, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo run --example bell` and `cargo run --example qasm`: Passed (code 0)
  - `cargo bench --bench routing_baseline`: Passed (code 0, 8 benchmark fixtures verified for 100% deterministic reproducibility)
- **Exit Gate Proof**: Bidirectional routing is validated and its layout transformations are correct (18 integration tests in `tests/routing/sabre_bidirectional.rs`, 119 total routing tests in `tests/routing.rs`, 1,214 total workspace tests across 35 test binaries). Implemented `reverse_circuit` inverting DAG dependency edges, operation order, and explicit circuit global phase ($\phi_{\text{rev}} = -\phi$) while preserving original circuit immutability. Applied exact mathematical unitary inversion ($U^\dagger$) to every quantum gate: self-inverses ($H, X, Y, Z, CX, CZ, SWAP$), discrete paired gates ($S \leftrightarrow S^\dagger, T \leftrightarrow T^\dagger$), continuous rotations ($Rx, Ry, Rz, P$) with parameter negation ($\theta \to -\theta$), and barrier preservation; proved $C_{\text{rev}} \cdot C = I$ via statevector simulation. Replaced silent non-unitary pass-through with explicit error handling (`RoutingError::NonUnitaryReverseTraversal`) for measurements, resets, and classical operations. Rigorously verified full unitary inversion across all 28 computational basis states for 2-qubit (4 states), 3-qubit (8 states), and 4-qubit (16 states) circuits, 25 independent pseudo-random normalized complex states via `SplitMix64`, and operator-level column reconstruction confirming Frobenius norm difference $\|U_{\text{composed}} - I\|_F < 10^{-12}$ under one single common global phase ($0.0$). Verified symbolic parameter inversion and continuous rotations with signed angles. Implemented `emit_non_2q_operation` executing ready 1Q gates, measurements, resets, barriers, and classical operations with full provenance tracking. Implemented `RefinementRoundRecord`, `RefinementSummary`, and `is_refinement_improvement` comparing candidate metrics against best-seen metrics under a hierarchical decision rule strictly dominated by composite weighted cost with deterministic 3-tier tie-breakers (SWAP count, critical depth, 2Q depth), explicit $10^{-12}$ float tolerance boundaries, and non-finite rejection (NaN and $\pm\infty$). Proven that rejected rounds never mutate winning outputs. Implemented `SabreRouterConfig`, `SabreRouterReport`, and `SabreBidirectionalRouter` coordinating multi-round forward and reverse routing passes. Enforced layout permutation invariant ($L_{\text{initial}}^{\text{candidate}} = L_{\text{rev\_final}}$) without invalid inverse transpositions ($L \neq L^{-1}$), verified on hand-calculated 4-cycle asymmetric layouts. Enforced ADR-0001 strict ancilla policy on partially occupied devices, verified using `LayoutEquivalenceChecker` across full $2^n$-dimensional statevectors. Hand-calculated 3-qubit line fixture verified reverse traversal derivation reducing SWAPs from 1 to 0. Fast-fail disconnected topology handling, capacity bounds checks, telemetry separation of forward vs reverse metrics, and 5-run bit-for-bit repeatability confirmed. All tests pass with zero warnings.
### Phase 54 — Target timing and resource model
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/target/timing.rs`, `src/target/resources.rs`, `tests/target/timing.rs`, `tests/target_timing.rs`, `docs/project-state/reports/phase-054.md`.
- **Files Modified**: `.gitignore`, `src/target/calibration.rs`, `src/target/error.rs`, `src/target/model.rs`, `src/target/mod.rs`, `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-054.md`](./reports/phase-054.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test target_timing`: Passed (code 0, 21 integration tests passed)
  - `cargo test --all`: Passed (1,250 tests passed across all 36 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: Timing and resource fixtures validate and preserve unknown data (21 new integration tests in `tests/target_timing.rs`, 1,250 total workspace tests across 36 test targets). Implemented `DurationUnit` (`Picoseconds`, `Nanoseconds`, `Microseconds`, `Milliseconds`, `Seconds`, `Cycles`) with safe exact unit conversions, clock period scaling, and case-insensitive string parsing. Enforced the Explicit Unknown Principle via `OperationDuration` (`Known`, `Estimated`, `Unknown`): `Unknown` is explicitly preserved and never defaulted or coerced to zero. Implemented multi-stage readout timing (`ReadoutTiming`: pulse, acquisition, depletion, total) and reset timing (`ResetTiming`: pulse, settling, total) with internal consistency cross-validation. Implemented hardware resource modeling (`ResourceModel`, `ResourceId`, `ResourceRequirement`, `ResourceUsageMode`, `ResourceCapacityEntry`): prevented assuming disconnected gates can run concurrently without hardware verification by checking shared drive lines and multiplexed feedlines. Implemented mutual exclusion rules (`MutualExclusionRule`): supported operation-pair cross-talk exclusions, global exclusive operations, and simultaneous readout exclusion groups. Extended `Provenance` with expiration timestamps (`valid_until`), zero-dependency ISO 8601 / RFC 3339 parsing to epoch seconds via Howard Hinnant's civil day algorithm, and staleness validation (`validate_freshness`, `is_stale`) rejecting expired or stale calibrations. Verified complete Serde JSON round-trip fidelity. All tests pass with zero warnings. Milestone M6 Phase 55 is unblocked.

### Phase 55 — ASAP scheduler
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/scheduler/mod.rs`, `src/scheduler/asap.rs`, `src/scheduler/schedule.rs`, `src/scheduler/validator.rs`, `tests/scheduler/asap.rs`, `tests/scheduler_asap.rs`, `docs/project-state/plans/phase-055.md`, `docs/project-state/reports/phase-055.md`.
- **Files Modified**: `src/lib.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-055.md`](./reports/phase-055.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test scheduler_asap`: Passed (code 0, 23 integration tests passed)
  - `cargo test --all`: Passed (1,273 tests passed across all 37 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: ASAP schedule obeys all predecessor and resource constraints and passes independent validation (23 new integration tests in `tests/scheduler_asap.rs`, 1,273 total workspace tests across 37 test targets). Implemented `ScheduledOperation` and `Schedule` tracking start/end times in nanoseconds, hardware resource requirements, classical bit effects, and makespan with full Serde JSON round-trip fidelity. Implemented `AsapScheduler` traversing deterministic topological order via Kahn's algorithm with min-priority tie-breaking, calculating earliest start time $T_{\text{pred}}$, and scanning release events for conflict-free placement obeying qubit/coupler single-occupancy, shared drive lines, multiplexed feedline capacity limits, and target mutual exclusion rules. Enforced the Explicit Unknown Principle by rejecting unknown durations and unmodeled resources. Implemented decoupled `ScheduleValidator` independently verifying DAG node coverage, predecessor dependencies, duration matching against calibration, qubit collisions, coupler collisions, exclusive resource collisions, capacity limits, and mutual exclusion rules. All tests pass with zero warnings. Milestone M6 Phase 56 is unblocked.

### Phase 56 — ALAP scheduler
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/scheduler/alap.rs`, `tests/scheduler/alap.rs`, `tests/scheduler_alap.rs`, `docs/project-state/plans/phase-056.md`, `docs/project-state/reports/phase-056.md`.
- **Files Modified**: `src/scheduler/mod.rs`, `src/scheduler/asap.rs`, `src/scheduler/schedule.rs`, `src/scheduler/validator.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-056.md`](./reports/phase-056.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test scheduler_alap`: Passed (code 0, 36 integration tests passed)
  - `cargo test --all`: Passed (1,328 tests passed across all 38 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: ALAP schedule passes timing and dependency validation (36 integration tests in `tests/scheduler_alap.rs`, 1,328 total workspace tests across 38 test targets). Implemented `AlapScheduler` and `AlapMakespanPolicy` (`DerivedFromAsap` and `Explicit`) with RC-CPM (Resource-Constrained Critical Path Method) resource precedence preservation, eliminating artificial negative slack and heuristic resource reordering artifacts. Enforced mathematically exact half-open interval ordering (`Ordering::Less` / `Ordering::Equal`) and canonicalized floating-point timing values (signed-zero normalization and finite slack checks), eliminating cycle formation and proving strict acyclicity for arbitrarily small positive durations. Reverse traversal order sorted descending by `(asap_end, asap_start, seq_num, op_id)` guarantees all DAG and resource successors are placed before an operation. Evaluated candidate end times in descending chronological order under exact half-open interval semantics (`is_interval_feasible`), placing operations at the latest feasible conflict-free start time. Accurately calculated operation slack ($\text{Slack} = \text{start}_{\text{ALAP}} - \text{start}_{\text{ASAP}}$), identified critical path operations ($\text{Slack} = 0$), and distinguished mathematical makespan infeasibility (`InfeasibleMakespan`) from heuristic search starvation (`AlgorithmicPlacementFailure`). Decoupled physical schedule validation from independent ALAP slack verification in `ScheduleValidator`, detecting missing slack (`MissingSlack`), slack discrepancies (`SlackMismatch`), non-finite slack, and forged positive or negative slack. All tests pass with zero warnings. Milestone M6 Phase 57 is unblocked.

### Phase 57 — Resource-constrained scheduling
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/scheduler/resource_calendar.rs`, `src/scheduler/resource_constraints.rs`, `tests/scheduler/resources.rs`, `tests/scheduler_resources.rs`, `docs/project-state/plans/phase-057.md`, `docs/project-state/reports/phase-057.md`.
- **Files Modified**: `src/scheduler/mod.rs`, `src/lib.rs`, `docs/project-state/progress.md`, `README.md`.
- **Report**: [`docs/project-state/reports/phase-057.md`](./reports/phase-057.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test scheduler_resources`: Passed (code 0, 20 integration tests passed)
  - `cargo test --all`: Passed (1,348 tests passed across all 39 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: Synthetic resource-conflict schedules pass validator (20 new integration tests in `tests/scheduler_resources.rs`, 1,348 total workspace tests across 39 test targets). Implemented `ResourceReservation` and `ResourceCalendar` tracking exact half-open execution intervals $[t_{\text{start}}, t_{\text{end}})$ across dedicated qubits, coupler channels, shared control lines, multiplexed feedlines, and custom hardware channels with aggregate concurrent capacity enforcement ($U(R, t) \le \text{capacity}(R)$). Implemented structured `ResourceConflictReport` and `ResourceConflictReason` (`QubitOccupied`, `CouplerOccupied`, `ExclusiveCollision`, `CapacityExceeded`, `MutualExclusion`, `ReadoutExclusion`, `MissingMetadata`). Implemented `find_earliest_feasible_time` forward interval search and `find_latest_feasible_time` backward search. Implemented `ReadoutGroup` with configurable synchronization policies (`Independent`, `SimultaneousOnly`, `StrictlySerialized`) and `SharedControlGroup`. Implemented `ResourceConstraintManager` with comprehensive pairwise explainability (`analyze_concurrency`) and candidate blocker diagnostics (`explain_placement_blockers`). Implemented `ResourceConstrainedScheduler` recording explainable `SchedulingTraceEntry` diagnostics for delayed operations. Verified complex synthetic circuits with shared drive lines, feedline capacities, and mutual exclusions passing `ScheduleValidator::validate` and `ResourceCalendar::from_schedule`. All tests pass with zero warnings. Milestone M6 Phase 58 is unblocked.

### Phase 58 — Schedule serialization and visualization data
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/scheduler/output.rs`, `src/scheduler/timeline.rs`, `tests/scheduler/output.rs`, `tests/scheduler_output.rs`, `docs/project-state/plans/phase-058.md`, `docs/project-state/reports/phase-058.md`.
- **Files Modified**: `src/scheduler/mod.rs`, `src/scheduler/schedule.rs`, `src/lib.rs`, `docs/project-state/progress.md`, `README.md`.
- **Report**: [`docs/project-state/reports/phase-058.md`](./reports/phase-058.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test scheduler_output`: Passed (code 0, 13 integration tests passed)
  - `cargo test --all`: Passed (1,361 tests passed across all 40 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: Schedule output round-trips to JSON with 100% fidelity and reconstructed schedules pass independent validator (13 new integration tests in `tests/scheduler_output.rs`, 1,361 total workspace tests across 40 test targets). Implemented `ScheduleOutput`, `ScheduledOperationOutput`, and `ScheduleSummary` preserving stable semantic `OperationId`, physical operand identities (`PhysicalQubitId`), classical hazard effects (`classical_reads`, `classical_writes`), start and end timestamps in nanoseconds, hardware resource requirements, ALAP slacks, and critical path operation IDs. Enforced deterministic chronological ordering sorted by `(start_time, end_time, sequence_number, id)` with canonical IEEE 754 float normalization. Enforced the Explicit Unknown Principle via `OperationTiming` (`Known`, `Estimated`, `Unknown`) ensuring unknown durations are never converted into zero or fabricated numeric values and rejecting incomplete execution reconstructions with structured errors. Implemented structured, machine-readable `TimelineData`, parallel execution lanes `TimelineLane` (`TimelineLaneKind`: `Qubit`, `Coupler`, `ReadoutLine`, `DriveLine`, `AcquisitionUnit`, `ClassicalBit`, `Custom`), discrete execution intervals `TimelineBlock` with instantaneous virtual gate handling, dependency links `TimelineDependencyLink`, active block queries `blocks_at(t)`, and built-in human-readable ASCII summary visualization `to_ascii_timeline()`. Extended `Schedule` with `to_output` and `generate_timeline` convenience methods. Proved complete round-trip fidelity: `Schedule -> ScheduleOutput -> JSON -> ScheduleOutput -> Schedule -> ScheduleValidator::validate`. All tests pass with zero warnings. Milestone M6 Phase 59 is unblocked.

### Phase 59 — Integrated compilation pipeline
- **Completion Date**: 2026-11-05
- **Milestone**: Milestone M6 (`M6`)
- **Files Created**: `src/compiler/mod.rs`, `src/compiler/config.rs`, `src/compiler/result.rs`, `src/compiler/pipeline.rs`, `tests/compiler/end_to_end.rs`, `tests/compiler_end_to_end.rs`, `docs/project-state/plans/phase-059.md`, `docs/project-state/reports/phase-059.md`.
- **Files Modified**: `src/lib.rs`, `docs/project-state/progress.md`, `README.md`.
- **Report**: [`docs/project-state/reports/phase-059.md`](./reports/phase-059.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo test --test compiler_end_to_end`: Passed (code 0, 14 integration tests passed)
  - `cargo test --all`: Passed (1,375 tests passed across all 41 test targets, 0 failed, 0 ignored)
  - `cargo doc --no-deps`: Passed (code 0)
- **Exit Gate Proof**: End-to-end compilation output is legal, semantically verified and optionally scheduled (14 new integration tests in `tests/compiler_end_to_end.rs`, 1,375 total workspace tests across 41 test targets). Implemented `CompilerConfig` supporting reproducible preset levels (`o0`, `o1`, `o2`, `o3`), router choice (`Sabre`, `Baseline`), scheduler choice (`Disabled`, `Asap`, `Alap`, `ResourceConstrained`), translation configuration, and verification options. Implemented `CompilationResult` and `CompilationReport` capturing comprehensive per-stage provenance with 100% JSON round-trip fidelity. Implemented `CompilerPipeline::compile` executing the 8 ordered stages: (0) Pre-flight input and target capacity validation; (1) Logical optimization via `OptimizerPipeline`; (2) Pre-routing target basis translation via `BasisTranslationEngine` with relaxed all-to-all connectivity; (3) Physical placement and routing along coupling edges with layout tracking via `SabreBidirectionalRouter` or `BaselineRouter`; (4) Post-routing direction correction via `TwoQubitDirectionCorrector` with Hadamard conjugation ($CX(0, 1) = (H_0 \otimes H_1) CX(1, 0) (H_0 \otimes H_1)$) and in-place basis lowering; (5) Fail-closed target legality validation via `TargetLegalityValidator`; (6) Optional post-routing scheduling and independent schedule validation via `ScheduleValidator`; (7) Layout-aware semantic verification via `LayoutEquivalenceChecker` strictly verifying ADR-0001 ancilla zero-state initialization and return-to-zero. Verified Bell circuits, line/ring coupling topologies, directed target topologies, impossible topologies failing fast, timing targets with scheduling, multi-level optimization, and deterministic bit-for-bit reproducibility. All tests pass with zero warnings. Milestone M6 is complete. *(Historical note from 2026-11-05: the old Phases 60–105 were superseded on 2026-11-12 by **Roadmap v3.1** Phases 60–85, continuing from this Phase 59 checkpoint with M7 Session 1.)*

### M7 Session 1 — Contract and manifest-first Rust work (Phases 60 & 63 manifest)
- **Completion Date**: 2026-11-12
- **Milestone**: Milestone M7 (`M7` / v0.2)
- **Files Created**: `docs/decisions/ADR-0003-independent-oracle-and-manifest-contract.md`, `tools/independent_oracle/schema/compile_manifest.schema.json`, `tools/independent_oracle/README.md`, `src/compiler/manifest.rs`, `tests/compiler/manifest.rs`, `docs/project-state/reports/m7-session-1.md`.
- **Files Modified**: `src/compiler/result.rs`, `src/compiler/pipeline.rs`, `src/compiler/mod.rs`, `src/lib.rs`, `src/cli/compile.rs`, `tests/compiler_end_to_end.rs`, `tests/cli.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/m7-session-1.md`](./reports/m7-session-1.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: Passed (1,401 tests passed across 37 test targets, 0 failed, 0 ignored)
  - `jsonschema.validate` on `target_compiled` and `frontend_only` manifests against `tools/independent_oracle/schema/compile_manifest.schema.json`: Passed (code 0)
- **Exit Gate Proof**: Established ADR-0003 oracle contract and versioned `qcc.compile_manifest` (`1.0.0`) JSON schema; implemented `CompilationManifest` built directly from `CompilationResult` (and `Circuit` in frontend-only mode) and additive `qcc compile --emit-manifest <FILE>` CLI flag with explicit `logical_to_physical` layouts, `little_endian_q0_lsb` convention, ADR-0001 unused ancilla lists, numeric/symbolic global phase tracking, unambiguous SWAP counts, SHA-256 artifact identity, `null` representation for genuinely absent fields, and bit-for-bit deterministic JSON output. Next unit: **M7 Session 2**.

### M7 Session 2 — Pinned Python/Qiskit environment & real-QCC-QASM compatibility (Phase 61 & Phase 63 QASM import)
- **Completion Date**: 2026-11-12
- **Milestone**: Milestone M7 (`M7` / v0.2)
- **Files Created**: `tools/independent_oracle/requirements.txt`, `tools/independent_oracle/qasm3_compat.py`, `tools/independent_oracle/tests/test_qasm3_compatibility.py`, `tools/independent_oracle/fixtures/session2/*`, `docs/decisions/ADR-0004-qasm3-importer-register-collision-fallback.md`, `docs/project-state/reports/m7-session-2.md`.
- **Files Modified**: `tools/independent_oracle/README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/m7-session-2.md`](./reports/m7-session-2.md)
- **Verification Commands Executed**:
  - `.venv/bin/pip check`: Passed (`No broken requirements found.`, code 0)
  - `.venv/bin/python3 tools/independent_oracle/qasm3_compat.py`: Passed (5 real unmodified QCC fixtures + manifest consistency, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (20 tests passed, code 0)
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --lib --test cli --test compiler_end_to_end -- --quiet`: Passed (172 tests passed, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: Passed (1,401 tests passed across 41 test output entries, 0 failed, 0 ignored)
- **Exit Gate Proof**: Established strictly pinned Python `3.12.3` / `qiskit==2.6.2` / `qiskit-qasm3-import==0.6.0` / `openqasm3==1.0.1` environment isolated under `tools/independent_oracle/`. Proved raw `qiskit.qasm3.loads()` rejects QCC physical `qubit[N] p;` when `include "stdgates.inc";` is present (`Symbol 'p' already inserted in Scope.GLOBAL`), and implemented ADR-0004 AST-level register alpha-renaming (`p -> qcc_reg_p`) preserving exact gate/gphase/register semantics. Verified construct-level semantics (`rz`, `sx`, `cx`, `gphase`, parameter expressions, scalar/multi-register) and fail-closed classification (`PARSE_ERROR`, `UNSUPPORTED_NON_UNITARY`, `UNSUPPORTED_UNBOUND_PARAM`, `UNSUPPORTED_CONSTRUCT`) across 20 automated Python tests and 5 real unmodified QCC fixtures (`showcase_linear`, `showcase_ring`, `routing_linear`, `rotations_linear`, `global_phase_linear`). Next unit: **M7 Session 3**.

### M7 Session 3 — Canonical cases, independent runners & core semantic comparator (Phases 62–66)
- **Completion Date**: 2026-11-12
- **Milestone**: Milestone M7 (`M7` / v0.2)
- **Files Created**: `tools/independent_oracle/schema/canonical_case.schema.json`, `tools/independent_oracle/schema/comparison_result.schema.json`, `tools/independent_oracle/cases/*.json` (10 canonical cases), `tools/independent_oracle/cases_loader.py`, `tools/independent_oracle/runners/{__init__,qiskit_reference,qcc_runner}.py`, `tools/independent_oracle/equivalence/{__init__,layout,comparator,artifacts}.py`, `tools/independent_oracle/verify_case.py`, `tools/independent_oracle/tests/test_oracle_session3.py`, `docs/project-state/reports/m7-session-3.md`.
- **Files Modified**: `tools/independent_oracle/qasm3_compat.py`, `tools/independent_oracle/README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/m7-session-3.md`](./reports/m7-session-3.md)
- **Verification Commands Executed**:
  - `.venv/bin/python3 tools/independent_oracle/verify_case.py`: Passed (10/10 canonical cases match expected oracle status, max amplitude error $\le 5.98 \times 10^{-16}$, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (34 tests passed across Session 2 + Session 3 suites, code 0)
  - `Draft202012Validator` schema checks across `canonical_case.schema.json`, `comparison_result.schema.json`, and 10 canonical cases: Passed (code 0)
  - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --lib --test cli --test compiler_end_to_end -- --quiet`: Passed (172 tests passed, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: Passed (1,401 passed across 41 test output entries, 0 failed, 0 ignored)
- **Exit Gate Proof**: Built neutral canonical case schema + 10 cases, strictly separated Qiskit logical-reference and black-box QCC physical runners, little-endian $2^n \to 2^m$ initial/final layout embedding, joint single-common-phase semantic comparison across full physical space (exhaustive unitary & seeded sampled statevector modes), strict ADR-0001 clean-ancilla population & entanglement detection, informational `strict_phase_accounting` (`PASS`/`MISMATCH`/`UNVERIFIED`), deterministic comparison result serialization, and complete failure-artifact bundle preservation. Next unit: **M7 Session 4**.

### M7 Session 4 — Difficult corpus, systematic fault injection & fair Qiskit baseline (Phases 67, 68, 70 baseline)
- **Completion Date**: 2026-11-12
- **Milestone**: Milestone M7 (`M7` / v0.2)
- **Files Created**: `tools/independent_oracle/schema/{corpus_summary,fault_campaign_summary,benchmark_trial,benchmark_aggregate}.schema.json`, `tools/independent_oracle/corpus/{__init__,generate_corpus,run_corpus}.py`, `tools/independent_oracle/corpus/{corpus_v1.jsonl,corpus_generation_summary.json}`, `tools/independent_oracle/fault_injection/{__init__,run_campaign}.py`, `tools/independent_oracle/benchmark/{__init__,run_baseline}.py`, `tools/independent_oracle/reports/{corpus_execution_summary,fault_campaign_summary,benchmark_aggregate}.json`, `tools/independent_oracle/reports/benchmark_trials.jsonl`, `tools/independent_oracle/tests/test_oracle_session4.py`, `docs/project-state/reports/m7-session-4.md`.
- **Files Modified**: `tools/independent_oracle/schema/canonical_case.schema.json`, `tools/independent_oracle/cases_loader.py`, `tools/independent_oracle/README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/m7-session-4.md`](./reports/m7-session-4.md)
- **Verification Commands Executed**:
  - `.venv/bin/python3 tools/independent_oracle/corpus/run_corpus.py`: Passed (`1,080/1,080 PASS`: `836` exhaustive + `244` explicitly labeled sampled, `max_amp_err=2.29e-15`, `0` duplicates, exit code `0`)
  - `.venv/bin/python3 tools/independent_oracle/fault_injection/run_campaign.py`: Passed (`40/40` detected across 17 families, `0` misclassified, `0` undetected semantic defects, exit code `0`)
  - `.venv/bin/python3 tools/independent_oracle/benchmark/run_baseline.py --trials 5`: Passed (`18` workloads $\times$ `5` trials, `90/90` QCC + `90/90` Qiskit semantic PASS; Median CX W/T/L `0/5/13`, Depth `1/2/15`, internal 20-trial Qiskit budget disclosed, exit code `0`)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (41 tests passed across Sessions 2–4, exit code `0`)
  - Schema validations across all Session 4 schemas and report JSON/JSONL files: Passed (exit code `0`)
  - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`, Python-free `cargo test --lib --test cli --test compiler_end_to_end`, and full `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet` (`1,401` passed): Passed (exit code `0`)
- **Exit Gate Proof**: Generated & verified 1,080 distinct deterministic canonical cases across 8 circuit families, 4 QCC targets (`linear`, `ring`, `linear-generic`, `ring-generic`), widths `1..7`, and explicit exhaustive vs sampled modes; proved oracle sensitivity across 17 injected fault families (`40/40`); and completed semantically verified, fairness-disclosed QCC-vs-Qiskit benchmark baseline identifying concrete optimization/routing gaps for M9–M10. Next unit: **M7 Session 5**.

### M7 Session 5 — Required CI oracle integration & M7 (v0.2) final acceptance (Phases 69 & 70)
- **Completion Date**: 2026-11-12
- **Milestone**: Milestone M7 (`M7` / v0.2) — **ACCEPTED**
- **Files Created**: `.github/workflows/independent-oracle.yml`, `tools/independent_oracle/ci_check.py`, `tools/independent_oracle/tests/test_oracle_session5.py`, `docs/project-state/reports/m7-session-5.md`.
- **Files Modified**: `tools/independent_oracle/README.md`, `README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/m7-session-5.md`](./reports/m7-session-5.md)
- **Verification Commands Executed**:
  - `.venv/bin/python3 tools/independent_oracle/ci_check.py --stage all --benchmark-trials 2`: Passed (`core` + `campaign`, `1,080/1,080` corpus PASS, `40/40` faults caught, `18` workloads $\times$ `2` trials benchmark PASS, exit code `0`)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (**45 tests passed**, 0 failed, exit code `0`)
  - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`, Python-free `cargo test --lib --test cli --test compiler_end_to_end` (`172` passed), and full `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet` (`1,401` passed): Passed (exit code `0`)
- **Exit Gate Proof**: Integrated isolated two-stage `.github/workflows/independent-oracle.yml` (`oracle-core-gate` and `oracle-campaign-and-baseline`) preserving existing Python-free Rust CI cleanly, enforcing pinned dependencies, 7 Draft 2020-12 schemas, QASM3 compat checks, 10 canonical cases, 1,080-case corpus regeneration & execution, 17-family fault injection, benchmark schema/semantic validation, and 30-day failure diagnostic artifacts; published consolidated M7 v0.2 acceptance report and README benchmark summary. Next authorized unit: **Milestone M8 (Phase 71)** (stopped at M7 acceptance).

### Phase 71 — Per-pass diagnostic and report schema
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M8 (`M8`)
- **Files Created**: `src/compiler/pass_diagnostic.rs`, `tools/independent_oracle/schema/pass_diagnostic.schema.json`, `tools/independent_oracle/tests/test_pass_diagnostic_schema.py`, `tests/compiler/pass_diagnostic.rs`, `docs/pass-report-schema.md`, `docs/project-state/reports/phase-071.md`.
- **Files Modified**: `src/compiler/mod.rs`, `src/lib.rs`, `tests/compiler_end_to_end.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-071.md`](./reports/phase-071.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --test compiler_end_to_end pass_diagnostic`: Passed (13/13 passed, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (57 tests passed, code 0)
- **Exit Gate Proof**: Versioned diagnostic report schema `qcc.pass_diagnostic` (v1.0.0, Draft 2020-12) and strongly typed Rust data structures implemented. Established deterministic representation fingerprints via SHA-256 with explicit invariant `semantic_equivalence_guaranteed == false`. Enforced truthful 4-state verification semantics (`passed`, `failed`, `not_run`, `unsupported`) with provenance separation (`internal` vs `independent_oracle`) and explicit measurement availability classification (`available`, `not_measured`, `unavailable`, `not_applicable`) without fabricating zero defaults. All schema invariant validations, serialization tests, and Python jsonschema unit tests pass with zero warnings. Milestone M8 Phase 72 is unblocked.

### Phase 72 — Pass-manager instrumentation and verification hooks
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M8 (`M8`)
- **Files Created**: `tests/compiler/pass_instrumentation.rs`, `docs/project-state/reports/phase-072.md`.
- **Files Modified**: `src/compiler/pass_diagnostic.rs`, `src/compiler/mod.rs`, `src/compiler/config.rs`, `src/compiler/result.rs`, `src/compiler/pipeline.rs`, `src/opt/context.rs`, `src/opt/pass.rs`, `src/opt/pass_manager.rs`, `src/opt/statistics.rs`, `src/optimizer/config.rs`, `src/optimizer/report.rs`, `src/optimizer/pipeline.rs`, `src/optimizer/passes/*`, `tests/compiler_end_to_end.rs`, `docs/pass-report-schema.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-072.md`](./reports/phase-072.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --test compiler_end_to_end pass_instrumentation`: Passed (13/13 passed, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast`: Passed (1,427 workspace tests passed across all 41 test targets, 0 failed, 0 ignored, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (61 tests passed, code 0)
- **Exit Gate Proof**: Instrumented `PassManager` and `OptimizerPipeline` execution boundaries with truthful `PassDiagnosticReport` emission matching schema v1.0.0. Implemented independently configurable controls (`basic()`, `standard()`, `full()`, `with_metrics()`, `with_fingerprints()`, `with_verification()`). Implemented opt-in `PassVerificationHookConfig` executing in-tree `EquivalenceChecker` with bounded statevector simulation (`max_qubits`) and `Internal` provenance without altering compiler semantics or swallowing pass errors. Transformation accounting captures exact node `OperationId` additions, removals, in-place rewrites, and scalar global phase delta. Preserved prior successful diagnostic records upon pass failure. Truthfully recorded `Skipped` status for disabled passes and O0 execution with `not_applicable` post-measurements and `not_run` verification. Measured instrumentation overhead across all 6 configurations: basic records (+3.1%), basic + metrics (+5.5%), fingerprints (+84.9%), standard (+89.9%), verification (+526.8%). Explicitly documented uncovered monolithic whole-circuit stage engines in `CompilerPipeline::compile`. Reconciled all 1,427 Rust tests and 61 Python tests. Phase 72 refinement complete, awaiting user acceptance review before starting Phase 73.

### Phase 73 — Diagnostic CLI flags and inspect/explain tooling
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M8 (`M8`) — **ACCEPTED & COMPLETED**
- **Files Created**: `docs/project-state/reports/phase-073.md`.
- **Files Modified**: `src/cli/compile.rs`, `src/compiler/pass_diagnostic.rs`, `src/compiler/mod.rs`, `src/ir/serialize.rs`, `src/lib.rs`, `tests/cli.rs`, `docs/pass-report-schema.md`, `README.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-073.md`](./reports/phase-073.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --test cli`: Passed (45/45 passed, including 15 new Phase 73 CLI integration tests, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast`: Passed (1,442 workspace tests passed across all 41 test targets, 0 failed, 0 ignored, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (61 Python tests passed, code 0)
  - `Draft202012Validator.validate(pass_report)`: Passed (100% adherence to `pass_diagnostic.schema.json` across `basic`, `standard`, `full`, and `O0` modes)
- **Exit Gate Proof**: Successfully integrated Phase 71/72 per-pass diagnostics into the QCC command-line interface. Added `--pass-report <FILE>` exporting structured JSON arrays conforming to `qcc.pass_diagnostic` v1.0.0; `--inspect-pass <ID>` displaying comprehensive human-readable inspection records with execution order, configuration, before/after metrics, transformation accounting, fingerprints, and verification evidence (gracefully listing available IDs upon missing passes and reporting all executions sequentially for multi-iteration passes); `--pass-summary` printing concise formatted terminal ASCII execution summary tables to `stderr`; `--pass-diagnostics <MODE>` presets (`basic`, `metrics`, `fingerprints`, `standard`, `full`) defaulting to lightweight `basic` mode when diagnostics are requested; additive toggles (`--pass-metrics`, `--pass-fingerprints`, `--pass-verify`); and `-O, --opt-level <0|1|2|3>`. Enforced strict Unix stream separation with clean `stdout` for compiled circuits and diagnostic telemetry directed to `stderr`. Replaced randomized `HashMap` metadata with sorted `BTreeMap` in `SerializedCircuit` ensuring bit-for-bit repeatable SHA-256 circuit fingerprints across separate process invocations. Reconciled 1,442 Rust tests and 61 Python tests. Milestone M8 (Phases 71–73) is complete. Milestone M9 (Phase 74) is unblocked.

### Phase 74 — Multi-candidate routing configuration and objectives
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M9 (`M9`)
- **Files Created**: `src/routing/search/mod.rs`, `src/routing/search/error.rs`, `src/routing/search/objective.rs`, `src/routing/search/candidate.rs`, `src/routing/search/config.rs`, `tests/routing/search_config.rs`, `docs/routing-search.md`, `docs/project-state/reports/phase-074.md`.
- **Files Modified**: `src/routing/mod.rs`, `src/routing/pathfinding.rs`, `src/compiler/config.rs`, `src/compiler/pipeline.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-074.md`](./reports/phase-074.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --test routing`: Passed (148/148 passed, including 14 Phase 74 tests, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast`: Passed (1,458 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out across 40 test targets, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (61 Python tests passed, code 0)
- **Exit Gate Proof**: Established strongly typed, bounded configuration contracts for multi-candidate routing search over SABRE seeds and initial layouts. Defined evaluation objectives (`Single`, `Lexicographic`, `Weighted`) over measurable circuit properties (`SwapCount`, `CxCount`, `TwoQubitGateCount`, `TotalGateCount`, `CircuitDepth`, `EstimatedDuration`) with fail-closed validation for uncalibrated hardware timing models adhering to the Explicit Unknown Principle. Decoupled hard feasibility constraints (`TargetLegality`, `SemanticEquivalence`, `AncillaZeroState`) from soft ranking scores and enforced all three as strictly mandatory. Validated explicit layouts against target physical capacity fail-closed during configuration validation. Bounded combinatorial search with `HARD_MAX_CANDIDATE_LIMIT = 256` and configurable timeout/swap budgets. Provided deterministic candidate identity tracking via `CandidateIdentity` and `CandidateDescriptor` directly embedding full 64-hex SHA-256 router options hash and target hash in candidate IDs. Integrated `RouterChoice::MultiCandidate` into `CompilerPipeline` maintaining 100% backward compatibility with single-candidate baseline routing. All 148 routing integration tests, 1,458 workspace Rust tests, and 61 Python oracle tests passed with zero errors and zero warnings. Phase 74 is complete and accepted.

### Phase 75 — Seeded multi-run SABRE candidate generation
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M9 (`M9`)
- **Files Created**: `src/routing/search/runner.rs`, `tests/routing/multi_seed.rs`, `docs/project-state/reports/phase-075.md`.
- **Files Modified**: `src/routing/sabre/bidirectional.rs`, `src/routing/pathfinding.rs`, `src/routing/greedy.rs`, `src/routing/search/candidate.rs`, `src/routing/search/config.rs`, `src/routing/search/runner.rs`, `src/routing/search/error.rs`, `src/routing/search/mod.rs`, `src/routing/mod.rs`, `src/verification/layout_equivalence.rs`, `src/verification/mod.rs`, `src/compiler/pipeline.rs`, `src/lib.rs`, `tests/routing.rs`, `docs/routing-search.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-075.md`](./reports/phase-075.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0)
  - `cargo test --test routing`: Passed (170/170 passed, including 22 new Phase 75 tests, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast`: Passed (1,480 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out across 40 test targets + 1 doc test, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (61 Python tests passed, code 0)
- **Exit Gate Proof**: Implemented bounded multi-run SABRE candidate generation and execution engine (`MultiCandidateSearchRunner`) over derived seeds and layout heuristics. Enforced candidate isolation through cloning inputs and keeping target references immutable. Extended SABRE reporting with exact cumulative iterations and SWAPs across forward, reverse, and refinement passes, clamping candidate budgets dynamically before each run. Implemented cooperative timeout evaluation (`budget.timeout_ms`) between candidate runs. Preserved partial resource consumption on routing failures. Implemented independent three-property validation records (`TargetLegality`, `SemanticEquivalence`, `AncillaZeroState`) truthfully reporting `CheckStatus::Unexecuted` and `is_fully_validated() == false` for $> 16$ qubits without fabricating proofs. Preserved full execution reports in `RoutingSearchExecutionError` across all failure modes (`Aborted`, `InvalidBaselineFallback`, `AllCandidatesFailed`, `NoValidCandidates`). Implemented provisional candidate selection API (`select_provisional_candidate()`) returning selections embedding stable 64-hex SHA-256 router and target hashes in `candidate_id`. Maintained 100% backward-compatible fast path for single-candidate configurations in `CompilerPipeline`. All 170 routing integration tests, 1,480 workspace Rust tests, and 61 Python oracle tests passed with zero errors and zero warnings. Phase 75 is complete and accepted.

### Phase 76 — Deterministic candidate ranking and selection
- **Completion Date**: 2026-11-13
- **Milestone**: Milestone M9 (`M9`)
- **Files Created**: `src/routing/search/evaluator.rs`, `src/routing/search/ranking.rs`, `tests/routing/candidate_selection.rs`, `docs/project-state/reports/phase-076.md`.
- **Files Modified**: `src/routing/search/mod.rs`, `src/routing/search/objective.rs`, `src/routing/search/evaluator.rs`, `src/routing/search/ranking.rs`, `src/routing/search/runner.rs`, `src/target/query.rs`, `tests/routing.rs`, `tests/routing/candidate_selection.rs`, `tests/routing/multi_seed.rs`, `docs/routing-search.md`, `docs/project-state/progress.md`.
- **Report**: [`docs/project-state/reports/phase-076.md`](./reports/phase-076.md)
- **Verification Commands Executed**:
  - `cargo fmt --check`: Passed (code 0)
  - `cargo clippy --all-targets -- -D warnings`: Passed (code 0)
  - `cargo doc --no-deps`: Passed (code 0, zero warnings)
  - `cargo test --test routing`: Passed (198/198 passed, including 28 Phase 76 tests, code 0)
  - `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast`: Passed (1,508 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out across 41 test targets, code 0)
  - `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: Passed (61 Python tests passed, code 0)
- **Exit Gate Proof**: Successfully implemented deterministic candidate evaluation, ranking, Pareto dominance analysis, tie-breaking, and explainability reporting (`RankingEngine`, `RankingReport`, `CandidateMetricsSnapshot`, `ObjectiveEvaluator`). Enforced mandatory hard constraints (`TargetLegality`, `SemanticEquivalence`, `AncillaZeroState`) as strict non-negotiable filters; invalid or unexecuted candidates are immediately disqualified from ranking and selection. Implemented strict `EstimatedDuration` semantics under the Explicit Unknown Principle, failing closed when target timing or gate calibrations are incomplete. Implemented Deb fast non-dominated sorting partitioning eligible candidates into Pareto fronts ($F_0, F_1, \dots$). Established an immutable deterministic tie-breaking hierarchy: Pareto front index ($F_0 < F_1 < \dots$, strictly preferring non-dominated candidates), objective score, designated baseline preference (`is_baseline == true`), and canonical `candidate_id` alphabetical string comparison. Retained full candidate evaluation tables in compile reports with explicit per-candidate selection and rejection reasons. Updated `FallbackPolicy::BestValidSoFar` to choose the highest-ranked candidate according to Phase 76 ranking while preserving `DeterministicBaseline` selection of the designated baseline. Preserved 100% backward compatibility for single-candidate execution in `CompilerPipeline`. Completed comprehensive acceptance audit addressing weighted normalization contracts, weight sum validation overflow, numerical term multiplication/accumulation overflow safety (`NonFiniteScore`), Pareto incomparability, makespan DAG concurrency, `TimingModel` resolution in `TargetQuery`, error report preservation, and determinism. All 198 routing tests, 1,508 workspace Rust tests, and 61 Python oracle tests passed with zero errors and zero warnings. Phase 76 is complete and ready for acceptance.








