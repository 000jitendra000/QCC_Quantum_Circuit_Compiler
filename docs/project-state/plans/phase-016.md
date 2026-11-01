# Phase 16 Implementation Plan: OpenQASM Frontend Adapter Boundary

## Objective
Establish a stable, parser-independent frontend adapter boundary for OpenQASM 3.
The frontend interface must isolate parsing technology from the rest of QCC, wrapping the handwritten restricted parser selected in ADR-0002 behind a generic `Frontend` interface.

---

## 1. Scope & Modules

### Files to Create:
- `src/frontend/mod.rs`: Frontend module root re-exporting adapter, AST, error, and source buffer types.
- `src/frontend/source.rs`: `SourceBuffer` abstraction providing immutable source storage, line start indexing, and exact 1-indexed `SourceSpan` calculation.
- `src/frontend/ast.rs`: Frontend-owned typed AST representing the reconciled OpenQASM 3 subset with source spans on all nodes.
- `src/frontend/errors.rs`: `FrontendError` and `FrontendResult<T>` bundling optional output with structured `Diagnostic`s.
- `src/frontend/adapter.rs`: Abstract `Frontend` trait and canonical `OpenQasmFrontend` implementation encapsulating internal private tokenizer and recursive-descent parser.
- `tests/frontend_adapter.rs`: Integration test suite validating spans, AST generation across the corpus, error diagnostics, unsupported feature rejections, and parser swappability via `MockFrontend`.
- `docs/project-state/plans/phase-016.md`: Phase plan.
- `docs/project-state/reports/phase-016.md`: Completion report.

### Files to Modify:
- `src/errors.rs`: Add `QccError::Frontend(#[from] FrontendError)`.
- `src/lib.rs`: Expose `pub mod frontend;` and re-export core frontend boundary types.
- `docs/project-state/progress.md`: Update progress ledger marking Phase 16 complete (`16 / 105`).

---

## 2. Invariants & Architectural Contracts

1. **Strict Parser Decoupling**:
   - Zero parser-internal tokens, syntax events, or third-party parser types may leak into QccIR or public compiler-core APIs.
   - All parser interactions occur through `Frontend::parse(&buffer) -> FrontendResult<AstProgram>`.
2. **1-Indexed SourceSpan Contract**:
   - Every token and AST node preserves valid, 1-indexed line and column `SourceSpan`s.
   - Multiline spans, empty lines, and Unicode columns (`π`) are correctly calculated without panics.
3. **Fail-Closed Unsupported Rejection**:
   - Non-supported constructs (`while`, `for`, `defcal`, `extern`, `delay`, `ccx`, `cswap`, `u3`, `p`) are rejected early with structured compiler diagnostics (`DiagnosticLevel::Error`).
   - OpenQASM 2 reserved keywords (`qreg`, `creg`) are detected with clear advice recommending `qubit[size]` and `bit[size]`.
4. **Error Recovery & Multi-Diagnostics**:
   - The parser recovers from statement-level errors by skipping to the next statement delimiter (`;`), discovering multiple diagnostics in a single compilation pass.
5. **Stage 1 Packaging Boundary**:
   - Conforms to Stage 1 Consolidated Package (`src/frontend/`), preparing a clean path for multi-crate extraction in Milestone M3 (`crates/qcc-frontend-qasm3`).

---

## 3. Ordered Implementation Sequence

1. Implement `SourceBuffer` in `src/frontend/source.rs` with line start indexing, character column arithmetic, and span extraction.
2. Implement `FrontendError` and `FrontendResult<T>` in `src/frontend/errors.rs`.
3. Implement `AstProgram`, `AstStatement`, and subnodes in `src/frontend/ast.rs`.
4. Implement `Frontend` trait and `OpenQasmFrontend` in `src/frontend/adapter.rs`.
5. Connect subsystem in `src/frontend/mod.rs`, `src/errors.rs`, and `src/lib.rs`.
6. Implement comprehensive tests in `tests/frontend_adapter.rs`.
7. Verify formatting (`cargo fmt --check`), linting (`cargo clippy --all-targets --all-features -- -D warnings`), tests (`cargo test --all-targets`), and documentation (`cargo doc --no-deps`).
8. Author completion report `docs/project-state/reports/phase-016.md` and update `docs/project-state/progress.md`.
9. Commit with message: `feat(phase-16): establish OpenQASM frontend adapter boundary`.
