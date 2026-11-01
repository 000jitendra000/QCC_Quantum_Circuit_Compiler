# Phase 17 Implementation Plan: QASM Source Buffer and Lexer

## Objective
Implement a reliable, bounded OpenQASM source lexer that converts supported source text into a token stream with accurate source spans, independent of parsing and AST/QccIR construction.

---

## 1. Scope & Modules

### Files to Create:
- `src/frontend/token.rs`: Typed `TokenKind` enum and `Token` representation with `byte_start`, `byte_end`, and 1-indexed `SourceSpan`.
- `src/frontend/lexer.rs`: Standalone `Lexer`, `LexerLimits`, and `LexResult` with full edge-case coverage and configurable resource limits.
- `tests/lexer.rs`: Integration test runner for the lexer.
- `tests/lexer/golden.rs`: Golden token sequences for Bell state and standard circuits.
- `tests/lexer/tokens.rs`: Verification of all keywords, punctuation, delimiters, and operators.
- `tests/lexer/numeric.rs`: Edge cases for integer, decimal, leading/trailing dot, scientific notation, and malformed numeric literals.
- `tests/lexer/comments.rs`: Line comments (`//`), block comments (`/* ... */`), CRLF/LF line endings, and multiline span integrity.
- `tests/lexer/strings.rs`: Include path strings and unterminated string diagnostics.
- `tests/lexer/unicode.rs`: Multi-byte Unicode identifiers, `'π'` recognition, and column counting.
- `tests/lexer/limits.rs`: Resource limit enforcement (`E0101`, `E0102`, `E0103`).
- `tests/lexer/fuzz_smoke.rs`: Proptest-based fuzzing and property tests over arbitrary Unicode inputs and malformed corpora.
- `docs/project-state/plans/phase-017.md`: Implementation plan.
- `docs/project-state/reports/phase-017.md`: Completion report.

### Files to Modify:
- `src/frontend/source.rs`: Added `byte_slice` method for byte-offset substring extraction.
- `src/frontend/adapter.rs`: Replaced temporary private lexer with standalone `Lexer` and `Token` from `src/frontend/{lexer, token}.rs`.
- `src/frontend/mod.rs`: Re-exported `Token`, `TokenKind`, `Lexer`, `LexerLimits`, and `LexResult`.
- `src/lib.rs`: Re-exported lexer and token items.
- `docs/project-state/progress.md`: Update progress ledger marking Phase 17 complete (`17 / 105`).

---

## 2. Invariants & Architectural Contracts

1. **Parser Independence**:
   - The lexer strictly scans characters into tokens; it performs zero AST construction, gate classification, or QccIR lowering.
2. **Deterministic Termination & Bounded Resources**:
   - Every scanning branch strictly advances `cursor` by at least 1 byte or character. Infinite loops are impossible.
   - Resource limits (`max_source_bytes`, `max_tokens`, `max_token_bytes`) are enforced and emit structured diagnostics rather than panicking.
3. **1-Indexed SourceSpan Accuracy**:
   - Every token retains a valid `SourceSpan` satisfying `start_line >= 1`, `start_col >= 1`, `start_line <= end_line`, and `start <= end`.
   - Unicode character column arithmetic correctly tracks multi-byte UTF-8 sequences.
4. **Fail-Closed Diagnostics**:
   - Unterminated comments, unterminated strings, invalid numeric exponents, and unknown characters produce localized `DiagnosticLevel::Error` compiler diagnostics.

---

## 3. Ordered Implementation Sequence

1. Extend `SourceBuffer` in `src/frontend/source.rs` with `byte_slice`.
2. Define `TokenKind` and `Token` in `src/frontend/token.rs`.
3. Implement `Lexer`, `LexerLimits`, and `LexResult` in `src/frontend/lexer.rs`.
4. Update `src/frontend/adapter.rs` to consume the standalone lexer.
5. Export types in `src/frontend/mod.rs` and `src/lib.rs`.
6. Implement modular integration and property tests in `tests/lexer/` and `tests/lexer.rs`.
7. Verify all checks (`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo doc --no-deps`, examples, Python validation).
8. Author completion report `docs/project-state/reports/phase-017.md` and update `docs/project-state/progress.md`.
9. Commit with message: `feat(phase-17): implement OpenQASM source lexer`.
