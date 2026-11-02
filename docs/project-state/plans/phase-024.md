# Implementation Plan: Phase 24 — Unsupported Syntax Detection

## 1. Objective & Scope
The objective of Phase 24 is to make the accepted OpenQASM subset explicit, comprehensive, and fail-closed.
Every unsupported OpenQASM 3 feature must produce a deterministic, source-located diagnostic and be preserved in the AST as `AstStatement::Unsupported` rather than being silently skipped, ignored, or partially accepted.

## 2. Prerequisites
- Phase 23 (Semantic analysis and diagnostics): Completed and verified.
- Repository is clean on `main` at commit `a7469d1`.

## 3. Ordered Implementation Tasks
1. **Extend Lexer Tokens**: Add `Def`, `Cal`, `Complex` to `TokenKind` and `Lexer` to recognize all OpenQASM 3 reserved keywords.
2. **Diagnostic Constructors**: Add helper functions in `src/diagnostics/qasm.rs` for `unsupported_feature` (`E0005`), `reserved_keyword` (`E0002`), `unsupported_3q_gate` (`E0003`), and `unsupported_gate` (`E0004`).
3. **Dedicated Unsupported Module**: Create `src/frontend/unsupported.rs` with `UnsupportedConstruct`, token classification, deterministic diagnostic messages, actionable notes, and block/statement recovery logic.
4. **Parser Integration**: Wire `UnsupportedConstruct` into `Parser::parse_statement` in `src/frontend/adapter.rs`. Ensure that whenever an unsupported construct is detected:
   - The diagnostic is emitted with exact source span pointing to the construct.
   - The construct is safely skipped (handling `{ ... }` blocks and optional `else` or terminating `;`).
   - An `AstStatement::Unsupported(AstUnsupported)` node is pushed to the AST statement list, ensuring zero silent omission.
5. **Supported Subset Documentation**: Author `docs/qasm-supported-subset.md`, documenting the exact supported grammar (registers, MVP gates, parameter arithmetic, measurement, reset, barrier, global phase) and all unsupported constructs, failure codes, and rationales.
6. **Comprehensive Test Suite**: Create `tests/qasm3/unsupported/` with submodules covering:
   - Conditionals (`if`, `else`)
   - Loops (`while`, `for`, `in`, `break`, `continue`)
   - Subroutines (`def`, `return`, `extern`)
   - Gate definitions (`gate`)
   - Calibrations (`defcal`, `defcalgrammar`, `cal`)
   - Classical types (`int`, `uint`, `float`, `angle`, `bool`, `duration`, `stretch`, `complex`, `const`, `readonly`, `pragma`, `delay`)
   - Error recovery and non-omission (mixed valid and unsupported statements).
7. **Verification**: Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`, `cargo build --all-targets`, `cargo doc --no-deps`, examples, and Python `openqasm3` reference check.
8. **Completion Report & Ledger**: Record completion in `docs/project-state/reports/phase-024.md` and update `docs/project-state/progress.md`.
9. **Commit**: `feat(phase-24): implement unsupported syntax detection`.
