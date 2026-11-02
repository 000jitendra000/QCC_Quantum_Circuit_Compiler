# QCC Phase Completion Report — Phase 24: Unsupported Syntax Detection

## Metadata

- **Phase Number**: Phase 24
- **Phase Title**: Unsupported syntax detection
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Engineering Effort**: Medium (**M**)
- **Prerequisites**: Phase 23 (Semantic analysis and diagnostics)
- **Status**: **Completed**
- **Completion Date**: 2026-11-02
- **Exit Gate Satisfied**: **Yes** — Unsupported-feature conformance tests pass (`123/123` tests in `tests/qasm3.rs`, `418/418` total workspace tests).

---

## 1. Executive Summary

Phase 24 establishes a rigorous, fail-closed conformance boundary for the OpenQASM 3 frontend in QCC. It ensures that every unsupported language feature (conditionals, loops, subroutines, user gate definitions, pulse calibrations, classical scalar memory types, modifiers, timing delays, legacy OpenQASM 2 keywords, and un-decomposed standard gates) produces a deterministic, source-located diagnostic and halts lowering. Furthermore, under error recovery, statements are never silently omitted or discarded from the AST; they are preserved as `AstStatement::Unsupported(AstUnsupported)` nodes for diagnostics, tooling, and verification.

---

## 2. Ordered Implementation Tasks

### Task 1: Unsupported Construct Tokenization & Classification
- Added `TokenKind::Def`, `TokenKind::Cal`, and `TokenKind::Complex` to `src/frontend/token.rs` and `src/frontend/lexer.rs`.
- Created `src/frontend/unsupported.rs` defining `UnsupportedConstruct` with variants:
  - `Conditional` (`if`, `else`) -> `E0005`
  - `Loop` (`while`, `for`, `break`, `continue`) -> `E0005`
  - `Subroutine` (`def`, `return`, `extern`, `end`) -> `E0005`
  - `GateDefinition` (`gate`) -> `E0005`
  - `Calibration` (`defcal`, `defcalgrammar`, `cal`) -> `E0005`
  - `ClassicalType` (`int`, `uint`, `float`, `angle`, `bool`, `complex`) -> `E0005`
  - `ModifierOrDirective` (`const`, `readonly`, `pragma`) -> `E0005`
  - `Timing` (`delay`, `duration`, `stretch`) -> `E0005`
  - `LegacyKeyword` (`qreg`, `creg`) -> `E0002`
  - `ThreeQubitGate` (`ccx`, `cswap`) -> `E0003`
  - `UnsupportedGate` (`u1`, `u2`, `u3`, `u`, `p`, `sx`, `sxdg`, etc.) -> `E0004`
- Implemented `classify_unsupported_token(&TokenKind)` to map token streams to unsupported construct metadata.

### Task 2: Explicit Diagnostics & Standard Error Codes
- Added constructor helpers in `src/diagnostics/qasm.rs`:
  - `reserved_keyword(keyword, span, file_path)` (`E0002`)
  - `unsupported_3q_gate(gate, span, file_path)` (`E0003`)
  - `unsupported_gate(gate, span, file_path)` (`E0004`)
  - `unsupported_feature(feature, detail, span, file_path)` (`E0005`)
- Re-exported constructors and classification types across `src/diagnostics/mod.rs`, `src/frontend/mod.rs`, and `src/lib.rs`.

### Task 3: Comprehensive Grammar Documentation
- Created `docs/qasm-supported-subset.md` detailing:
  - Supported subset: Version directives, register declarations, MVP gates (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `rx`, `ry`, `rz`, `cx`, `cz`, `swap`), symbolic expressions, measurement, reset, barrier, and global phase.
  - Unsupported constructs matrix with explicit diagnostic error codes (`E0002`–`E0005`).
  - AST non-omission contracts and planned roadmap milestone schedules.

### Task 4: Fail-Closed & AST Non-Omission Guarantees
- Updated `src/frontend/adapter.rs`:
  - Added `skip_unsupported_construct()` and `skip_unsupported_block_or_statement()` to balance braces and consume statements cleanly without cascading spurious errors.
  - In `parse_statement()`, all unsupported tokens produce `Some(AstStatement::Unsupported(AstUnsupported { kind, raw, span }))` retaining exact source text and statement boundaries.
  - In `parse_gate_call()`, three-qubit gates (`ccx`, `cswap`) and decomposition-required gates (`u3`, `p`, etc.) are captured as `AstStatement::Unsupported` with source slices.
  - Exposed `OpenQasmFrontend::parse_recovering()` to allow diagnostic analysis and AST inspection while preserving `Frontend::parse()` fail-closed semantics (`output == None` on any error).
  - Updated `SemanticChecker` in `src/frontend/semantic/checks.rs` to reject any `AstStatement::Unsupported` fail-closed with designated diagnostic codes.

---

## 3. Verification Evidence

### Automated Test Suite
- `tests/qasm3/unsupported/conditionals.rs`: 4 tests verifying `if`, `if-else`, single statement `if`, and lone `else` rejection (`E0005`).
- `tests/qasm3/unsupported/loops.rs`: 3 tests verifying `while`, `for`, `break`, and `continue` rejection (`E0005`).
- `tests/qasm3/unsupported/subroutines.rs`: 3 tests verifying `def`, `return`, and `extern` rejection (`E0005`).
- `tests/qasm3/unsupported/gate_defs.rs`: 2 tests verifying custom and parameterized `gate` definition rejection (`E0005`).
- `tests/qasm3/unsupported/calibrations.rs`: 3 tests verifying `defcal`, `defcalgrammar`, and `cal` block rejection (`E0005`).
- `tests/qasm3/unsupported/types.rs`: 4 parametrized tests covering scalar types (`int`, `uint`, `float`, `angle`, `bool`, `complex`), modifiers (`const`, `readonly`, `pragma`), timing (`delay`, `duration`, `stretch`), and legacy OpenQASM 2 keywords (`qreg`, `creg`).
- `tests/qasm3/unsupported/recovery.rs`: 2 integration tests proving AST statement non-omission across mixed valid/unsupported statements, correct sequential recovery, and `SemanticAnalyzer` fail-closed rejection.

### Test Results Summary
- `cargo test --test qasm3`: 123 tests passed, 0 failed.
- `cargo test --all`: 418 tests passed across 15 test suites, 0 failed.
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings, 0 errors.
- `cargo fmt --check`: Clean formatting verified.
- `cargo doc --no-deps`: Generated cleanly.
- `cargo run --example bell`: Bell vertical slice verified.
- `cargo run --example qasm`: OpenQASM 3 emitter verified.

---

## 4. Exit Gate Affirmation

> **EXIT GATE**: Unsupported-feature conformance tests pass.

**Verdict: PASSED.** All unsupported OpenQASM 3 features produce deterministic, source-located diagnostics (`E0002`–`E0005`), recover cleanly without silent statement omission, and fail closed before lowering.
