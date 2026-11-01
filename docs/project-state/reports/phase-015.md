# Phase Completion Report: Phase 15 — OpenQASM Parser Dependency Evaluation

## 1. Summary
- **Phase Number**: Phase 15
- **Exact Phase Title**: OpenQASM parser dependency evaluation
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Assistant (Google DeepMind)

---

## 2. Deliverables & Files Changed

### Deliverables:
- `docs/research/qasm-parser-evaluation.md`: Exhaustive evaluation report covering:
  - Inventory of candidates: `oq3_lexer`, `oq3_parser`, `oq3_syntax`, `oq3_semantics`, `openqasm3[parser]==1.0.1`, and a custom handwritten restricted parser, with primary source links and release dates.
  - Compilation and empirical execution of a minimal Rust harness using `oq3_syntax v0.7.0` and `oq3_semantics v0.7.0` across the test corpus.
  - Upstream maintenance status (documenting that `Qiskit/openqasm3_parser` operates in "reduced maintenance mode").
  - 11-program parser evaluation corpus and empirical comparative results across both Python and Rust parsers.
  - Detailed construct-by-construct diagnosis of rejections (`registers.qasm` reserved keywords, `unsupported_features.qasm` while loop / pulse / extern, `measurement.qasm` arrow syntax).
  - Explicit reconciliation with QccIR gate and parameter capabilities (canonical 1Q/2Q gates, zero 3-qubit claims).
  - Comparison matrix across 8 key architectural criteria.
  - Risks, trade-offs, and fallback architecture.
- `docs/adr/ADR-0002-qasm-parser.md`: Formal Architecture Decision Record documenting:
  - Selection of **Strategy 2 (Handwritten Restricted Parser)** encapsulated behind a `Frontend` trait boundary.
  - Reconciled supported OpenQASM 3 subset matching QccIR `Gate` and `ParameterExpr`.
  - Precise primary source citations and links.
  - Rationale and rejection of `oq3_*` (reduced maintenance, `rowan <= 0.15.15` lock-in, 32 transitive crates, untyped `EXPR_STMT` wrapping) and Python bridge (runtime dependency, subprocess overhead, poor syntax diagnostics).
  - Fallback strategy and conditions triggering reconsideration.
- `docs/decisions/ADR-0002-qasm-parser.md`: Synchronized copy in repository decision records.
- `experiments/oq3-evaluation/`:
  - `Cargo.toml` & `Cargo.lock`: Standalone evaluation harness with `oq3_syntax v0.7.0` and `oq3_semantics v0.7.0`.
  - `src/main.rs`: Compiled Rust harness running `oq3_syntax::SourceFile::parse` across the entire corpus.
  - `rust_parser_results.json`: Empirical results from the Rust `oq3_syntax` parser.
  - `evaluate_corpus.py`: Python evaluation runner using pinned `openqasm3[parser]==1.0.1`.
  - `py_parser_results.json`: Empirical results from Python `openqasm3`.
  - `corpus/`: 11 representative OpenQASM 3 test files:
    - `bell.qasm` (Bell state prep and terminal measurement)
    - `registers.qasm` (1D registers and scalar declarations with valid identifiers)
    - `registers_reserved_keywords.qasm` (Reserved keywords `qreg` and `creg` rejection check)
    - `standard_gates.qasm` (Canonical 1Q and 2Q gates)
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
- Rejects unsupported syntax early at the parse boundary with helpful, line/column-accurate error messages.

### 3.2 Reconciled Supported Subset
- **Canonical Gates**:
  - 1-qubit fixed: `x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`.
  - 1-qubit rotations: `rx(theta)`, `ry(theta)`, `rz(theta)`.
  - 2-qubit: `cx` (ctrl=0, targ=1), `cz`, `swap`.
- **Excluded**:
  - 3-qubit gates (`ccx`, `cswap`) are NOT in QccIR at this milestone and are not supported without explicit decomposition (Milestone M4).
  - Derived gates (`u1`, `u2`, `u3`, `p`, `sx`, `sxdg`), pulse (`defcal`), dynamic classical control (`while`, `for`), and classical types (`int`, `float`, `extern`).

---

## 4. Exit Gate Verification Evidence

- **Exit Gate Statement**: `ADR documents selected parser, supported subset, license and fallback.`
- **Verification Evidence**:
  - `ADR-0002` documents:
    - Selected strategy: Handwritten restricted recursive-descent parser behind `Frontend` boundary.
    - Supported subset reconciled with QccIR: 1Q fixed/rotations, 2Q entanglers, 1D registers, measurements, resets, barriers, and global phase.
    - Unsupported subset rejected with errors: 3-qubit gates, `defcal`, arbitrary classical loops, externs, timing delays.
    - License: Apache-2.0 OR MIT; MSRV: 1.80.0; Dependencies: zero added to production core.
    - Primary source citations and fallback strategy.
  - Reproducible evidence generated:
    - `experiments/oq3-evaluation/evaluate_corpus.py` executed across 11 corpus files.
    - `experiments/oq3-evaluation/src/main.rs` compiled and executed with `oq3_syntax v0.7.0` across 11 corpus files.
    - Both `py_parser_results.json` and `rust_parser_results.json` persisted.
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
