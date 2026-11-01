# Phase Completion Report: Phase 15 — OpenQASM Parser Dependency Evaluation

## 1. Summary
- **Phase Number**: Phase 15
- **Exact Phase Title**: OpenQASM parser dependency evaluation
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Assistant (Google DeepMind)

---

## 2. Deliverables & Files Changed

### New Files Created:
- `docs/research/qasm-parser-evaluation.md`: Exhaustive evaluation report covering:
  - Inventory of candidates: `oq3_lexer`, `oq3_parser`, `oq3_syntax`, `oq3_semantics`, `openqasm3[parser]==1.0.1`, and a custom handwritten restricted parser.
  - License, MSRV, and dependency footprint findings from primary sources.
  - Upstream maintenance status (noting `Qiskit/openqasm3_parser` operates in "reduced maintenance mode").
  - 10-program parser evaluation corpus and empirical test results.
  - Comparison matrix across 9 key architectural criteria.
  - Risks, trade-offs, and fallback architecture.
- `docs/adr/ADR-0002-qasm-parser.md`: Formal Architecture Decision Record documenting:
  - Selection of **Strategy 2 (Handwritten Restricted Parser)** encapsulated behind a `Frontend` trait boundary.
  - Rationale and rejection of `oq3_*` (reduced maintenance, `rowan <= 0.15.15` lock-in, lossless CST mismatch) and Python bridge (runtime dependency, subprocess overhead, poor syntax diagnostics).
  - Explicit specification of QCC's supported OpenQASM 3 subset vs unsupported features cleanly rejected with errors.
  - Dual license (`Apache-2.0 OR MIT`), MSRV 1.80.0, and zero external dependency footprint.
  - Fallback strategy and reconsideration triggers.
- `docs/decisions/ADR-0002-qasm-parser.md`: Synchronized copy in repository decision records.
- `experiments/oq3-evaluation/`:
  - `Cargo.toml` & `Cargo.lock`: Standalone, isolated evaluation harness.
  - `src/main.rs`: Rust evaluation program analyzing feature requirements and AST adaptation complexity.
  - `evaluate_corpus.py`: Python evaluation runner using pinned `openqasm3[parser]==1.0.1`.
  - `py_parser_results.json`: Empirical parsing results, statement counts, AST node types, and diagnostic output.
  - `corpus/`: 10 representative OpenQASM 3 test files:
    - `bell.qasm` (Bell state preparation and terminal measurement)
    - `registers.qasm` (Quantum and classical 1D registers and scalar declarations)
    - `standard_gates.qasm` (Standard 1Q and 2Q gates)
    - `parameterized_gates.qasm` (Rotations, phase gates, arithmetic expressions, gphase)
    - `measurement.qasm` (Assignment syntax and arrow syntax)
    - `reset.qasm` (Active quantum state resets)
    - `barrier.qasm` (Global and selective barriers)
    - `invalid_syntax.qasm` (Missing semicolons, corrupted statements)
    - `malformed_operands.qasm` (Duplicate 2Q operands, out-of-bounds indices)
    - `unsupported_features.qasm` (While loops, pulse defcal blocks, extern functions)
- `docs/project-state/plans/phase-015.md`: Phase implementation plan.
- `docs/project-state/reports/phase-015.md`: This completion report.

### Modified Files:
- `.gitignore`: Added `target/` and `**/target/` to ignore experiment build artifacts.
- `docs/project-state/progress.md`: Updated progress ledger marking Phase 15 complete (`15 / 105`).

---

## 3. Architecture & Decision Highlights

### 3.1 The Chosen Strategy: Handwritten Restricted Parser
- Implements an in-tree, lightweight, recursive-descent parser (Phases 16–20) tailored to QCC's canonical OpenQASM 3 subset.
- 100% pure stable Rust with MSRV 1.80.0 and zero foreign runtime dependencies.
- Native integration with QCC's 1-indexed `SourceSpan` and `Diagnostic` infrastructure.
- Rejects unsupported syntax (e.g. pulse-level calibration, arbitrary classical control flow) early at the parse boundary with helpful, line/column-accurate error messages.

### 3.2 Strict Frontend Adapter Boundary (Phase 16)
- The parser remains completely isolated behind a generic `Frontend` trait and `SourceBuffer` abstraction in `crates/qcc-frontend-qasm3`.
- QccIR remains 100% agnostic to parser implementation details; foreign AST types never leak into compiler core passes.

### 3.3 Role of Python Reference Parser
- `openqasm3[parser]==1.0.1` is maintained strictly as an out-of-process acceptance verification oracle in CI to confirm that QCC's *emitted* QASM passes standard reference validation.
- It will never be a runtime or build dependency of the compiler executable.

---

## 4. Exit Gate Verification Evidence

- **Exit Gate Statement**: `ADR documents selected parser, supported subset, license and fallback.`
- **Verification Evidence**:
  - `ADR-0002` documents:
    - Selected strategy: Handwritten restricted recursive-descent parser behind `Frontend` boundary.
    - Supported subset: Version headers, includes, 1D qubit/bit registers, canonical 1Q/2Q/3Q gates, parameterized rotations, measurement (assignment and arrow), reset, barrier, and global phase.
    - Unsupported subset rejected with errors: `defcal`, arbitrary classical loops, externs, timing delays.
    - License: Apache-2.0 OR MIT; MSRV: 1.80.0; Dependencies: zero added to production core.
    - Fallback strategy: Pluggable parser adapter via `Frontend` trait without changing QccIR or optimizer.
  - Reproducible evidence generated:
    - `experiments/oq3-evaluation/evaluate_corpus.py` executed across 10 corpus files.
    - `experiments/oq3-evaluation/py_parser_results.json` captured concrete statement counts, AST structures, and diagnostic behaviors.
    - `experiments/oq3-evaluation/src/main.rs` built and verified offline with 0 warnings.
  - Full project verification passed:
    - `cargo fmt --check`: 0 diffs.
    - `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings, 0 errors.
    - `cargo test --all-targets`: All 298 tests passed.
    - `cargo build`: Succeeded.
    - `cargo doc --no-deps`: Built with 0 warnings.
    - `cargo run --example bell`: Executed and verified.
    - `cargo run --example qasm`: Executed and verified.

---

## 5. Next Steps

- Phase 15 is complete.
- Next phase is **Phase 16: QASM frontend adapter boundary** (`crates/qcc-frontend-qasm3/`).
- Do NOT begin Phase 16 until explicitly requested by the user.
