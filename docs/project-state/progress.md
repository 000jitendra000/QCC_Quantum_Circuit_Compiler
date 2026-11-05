# QCC Implementation Progress Ledger

```text
Project: QCC
Repository: QCC_Quantum_Circuit_Compiler
Roadmap: 105 phases
Project foundation documentation: Complete
Current implementation phase: Phase 52
Completed implementation phases: Phase 1 through Phase 52 (52 / 105)
```

## Current Checkpoint

- **Current Active Phase**: **Phase 52 — Bidirectional SABRE and layout refinement**
- **Implementation Status**: **Completed**
- **Completed Phases**: `52 / 105` (Phases 1 through 52 verified and complete)
- **Milestone M5 Progress**: `9 / 10` phases completed (In Progress)
- **Next Action**: Execute Phase 53 implementation (Router validation and comparative benchmarks, Milestone M5) according to [phases-044-053.md](../roadmap/phases-044-053.md#phase-053) upon explicit user request.

---

## Milestone Progress Overview

| Milestone | Code | Phases | Completed | Status |
|---|:---:|:---:|:---:|:---:|
| **Milestone M1: Bell Vertical Slice** | `M1` | 1–11 | 11 / 11 | **Completed** |
| **Milestone M2: Compiler Foundations & QASM** | `M2` | 12–27 | 16 / 16 | **Completed** |
| **Milestone M3: Verified Logical Optimizer** | `M3` | 28–36 | 9 / 9 | **Completed** |
| **Milestone M4: Generic Target & Basis Translation** | `M4` | 37–43 | 7 / 7 | **Completed** |
| **Milestone M5: Logical-to-Physical Mapping & Routing** | `M5` | 44–53 | 9 / 10 | In Progress |
| **Milestone M6: Scheduling & Integrated Compiler** | `M6` | 54–59 | 0 / 6 | Not Started |
| **Milestone M7: Dynamic Simulation & Non-Unitary** | `M7` | 60–67 | 0 / 8 | Not Started |
| **Milestone M8: Differential Validation & Interop** | `M8` | 68–82 | 0 / 15 | Not Started |
| **Milestone M9: Developer Experience & Release** | `M9` | 83–89 | 0 / 7 | Not Started |
| **Advanced Research Track** | `Research` | 90–105 | 0 / 16 | Not Started |

---

## Detailed Phase Status Ledger (1–105)

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
| 53 | Router validation and comparative benchmarks | `M5` | **XL** | **Not Started** | — | Pending |
| 54 | Target timing and resource model | `M6` | **L** | **Not Started** | — | Pending |
| 55 | ASAP scheduler | `M6` | **L** | **Not Started** | — | Pending |
| 56 | ALAP scheduler | `M6` | **L** | **Not Started** | — | Pending |
| 57 | Resource-constrained scheduling | `M6` | **XL** | **Not Started** | — | Pending |
| 58 | Schedule serialization and visualization data | `M6` | **M** | **Not Started** | — | Pending |
| 59 | Integrated compilation pipeline | `M6` | **XL** | **Not Started** | — | Pending |
| 60 | Mid-circuit measurement semantics | `M7` | **L** | **Not Started** | — | Pending |
| 61 | Trajectory simulator | `M7` | **XL** | **Not Started** | — | Pending |
| 62 | Exact branch enumeration | `M7` | **XL** | **Not Started** | — | Pending |
| 63 | Reset semantics | `M7` | **L** | **Not Started** | — | Pending |
| 64 | Non-unitary semantic verification | `M7` | **XL** | **Not Started** | — | Pending |
| 65 | Density-matrix reference simulator | `M7` | **XL** | **Not Started** | — | Pending |
| 66 | Basic noise channel framework | `M7` | **XL** | **Not Started** | — | Pending |
| 67 | Simulator performance and resource limits | `M7` | **L** | **Not Started** | — | Pending |
| 68 | Qiskit Aer differential testing | `M8` | **L** | **Not Started** | — | Pending |
| 69 | Cirq and qsim differential testing | `M8` | **M** | **Not Started** | — | Pending |
| 70 | Stim stabilizer validation | `M8` | **M** | **Not Started** | — | Pending |
| 71 | Unified differential test harness | `M8` | **L** | **Not Started** | — | Pending |
| 72 | Provider-neutral backend API | `M8` | **L** | **Not Started** | — | Pending |
| 73 | Local simulator backend adapter | `M8` | **M** | **Not Started** | — | Pending |
| 74 | OpenQASM execution artifact and dry run | `M8` | **M** | **Not Started** | — | Pending |
| 75 | Provider-neutral remote job lifecycle | `M8` | **L** | **Not Started** | — | Pending |
| 76 | Credential and secret handling | `M8` | **M** | **Not Started** | — | Pending |
| 77 | IBM Quantum adapter | `M8` | **XL** | **Not Started** | — | Pending |
| 78 | AWS Braket adapter | `M8` | **XL** | **Not Started** | — | Pending |
| 79 | Additional provider adapters | `M8` | **XL per adapter** | **Not Started** | — | Pending |
| 80 | Remote execution result normalization | `M8` | **L** | **Not Started** | — | Pending |
| 81 | Microsoft QIR interoperability | `M8` | **XL** | **Not Started** | — | Pending |
| 82 | Backend capability matrix and preflight | `M8` | **M** | **Not Started** | — | Pending |
| 83 | Complete CLI command architecture | `M9` | **L** | **Not Started** | — | Pending |
| 84 | AST/DAG/QccIR inspection | `M9` | **M** | **Not Started** | — | Pending |
| 85 | Circuit and topology visualization | `M9` | **L** | **Not Started** | — | Pending |
| 86 | Compilation and optimization reports | `M9` | **M** | **Not Started** | — | Pending |
| 87 | Reproducibility manifests | `M9` | **M** | **Not Started** | — | Pending |
| 88 | Benchmarking framework | `M9` | **L** | **Not Started** | — | Pending |
| 89 | Documentation, packaging and release engineering | `M9` | **L** | **Not Started** | — | Pending |
| 90 | Calibration-aware cost model | `Research` | **XL** | **Not Started** | — | Pending |
| 91 | User-defined gate AST and semantics | `Research` | **L** | **Not Started** | — | Pending |
| 92 | User-defined gate expansion | `Research` | **L** | **Not Started** | — | Pending |
| 93 | Control-flow graph representation | `Research` | **XL** | **Not Started** | — | Pending |
| 94 | Classical expressions and conditions | `Research` | **L** | **Not Started** | — | Pending |
| 95 | Conditional branch lowering | `Research` | **XL** | **Not Started** | — | Pending |
| 96 | Loop representation and validation | `Research` | **XL** | **Not Started** | — | Pending |
| 97 | Dynamic CFG execution | `Research` | **XL** | **Not Started** | — | Pending |
| 98 | KAK mathematical foundations | `Research` | **XL** | **Not Started** | — | Pending |
| 99 | KAK numerical decomposition | `Research` | **XL** | **Not Started** | — | Pending |
| 100 | KAK target-specific synthesis | `Research` | **XL** | **Not Started** | — | Pending |
| 101 | MPS simulation foundations | `Research` | **XL** | **Not Started** | — | Pending |
| 102 | MPS gate updates and truncation | `Research` | **XL** | **Not Started** | — | Pending |
| 103 | MPS benchmarking and simulator selection | `Research` | **L** | **Not Started** | — | Pending |
| 104 | Advanced optimizer research | `Research` | **XL** | **Not Started** | — | Pending |
| 105 | Final architecture audit and research roadmap | `Research` | **L** | **Not Started** | — | Pending |

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





