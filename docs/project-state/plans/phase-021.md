# Phase Plan: Phase 21 — Parameter Expression Parser

## 1. Metadata
- **Phase Number**: Phase 21
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 18 (AST model), Phase 19 (register resolution), Phase 20 (gate statement parser) verified and completed in ledger.
- **Source Document Page**: Page 15 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Implement a bounded OpenQASM parameter-expression parser in `src/frontend/parser/expressions.rs` supporting numeric constants, `pi`, symbolic identifiers, unary signs (`+`, `-`), parentheses, and arithmetic (`+`, `-`, `*`, `/`) with correct precedence and left associativity. Preserve symbolic expressions in the AST without lossy eager evaluation, and detect division by zero and unresolved identifiers explicitly at binding/evaluation time.
- **Explicit Inclusions**:
  - `ExpressionParser` in `src/frontend/parser/expressions.rs`.
  - Pratt parsing with standard operator precedence:
    1. Parentheses and primary expressions (literals, `pi`, identifiers).
    2. Unary prefix signs (`+` and `-`).
    3. Multiplicative operators (`*`, `/`).
    4. Additive operators (`+`, `-`).
  - Left associativity for binary `+`, `-`, `*`, `/`.
  - Preservation of unresolved identifiers as `AstExprKind::Identifier` nodes.
  - Special mathematical recognition of `pi` and `π` in evaluation, resolving to $\pi \approx 3.141592653589793$ without requiring symbol bindings in `env`.
  - Detection of literal division by zero at parse time and evaluation time.
  - Explicit error handling for division by zero after symbol binding and unbound/unresolved symbols during evaluation.
  - Integration with `GateParser` for parameterized gates (`rx`, `ry`, `rz`) and `OpenQasmFrontend`.
  - Source span tracking for all expression nodes.
- **Explicit Exclusions**:
  - Full classical language expressions (booleans, bitwise ops, comparisons, control flow).
  - Arbitrary user-defined function parsing.
  - Classical variable declaration statements.
  - AST-to-QccIR lowering (deferred to Phase 25).
  - Measurement, reset, barrier parsing (deferred to Phase 22).
  - Optimization or simulation engine changes.
  - Zero QccIR dependencies in `src/frontend/`.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Explicit circuit global phase tracked and preserved.
- [x] Parameter expressions preserved without eager lossy floating-point evaluation.
- [x] Precedence hierarchy: Parens/Primary > Unary `+`/`-` > `*`/`/` > `+`/`-`.
- [x] Left associativity for binary `+`, `-`, `*`, `/`.
- [x] `pi` treated as mathematical constant $\pi$, not an unresolved symbol.
- [x] Fail-closed error reporting on literal division by zero and unbound symbols at evaluation time (never silently default to 0.0).
- [x] Pure frontend AST independence: zero `crate::ir` or `crate::quantum` imports in `src/frontend/`.

## 4. Step-by-Step Implementation Sequence
1. **Expression Parser Implementation (`src/frontend/parser/expressions.rs`)**:
   - Implement `ExpressionParser<'a>` over `SourceBuffer` and `&[Token]`.
   - Implement Pratt operator precedence parsing with explicit binding powers.
   - Implement literal division by zero detection.
   - Implement structured evaluation `evaluate_expression` and `ExpressionEvaluationError`.
2. **Parser Module Wiring & Integration**:
   - Expose `expressions` in `src/frontend/parser/mod.rs`, `src/frontend/mod.rs`, and `src/lib.rs`.
   - Update `GateParser` in `src/frontend/parser/gates.rs` to delegate parameter expression parsing to `ExpressionParser`.
   - Update `Parser` in `src/frontend/adapter.rs` to use `ExpressionParser` for `gphase`.
3. **Comprehensive Test Suite (`tests/qasm3/expressions.rs` & `tests/qasm3.rs`)**:
   - Add integration tests covering all 18 required categories.
4. **Verification & Regression Checks**:
   - Format, Clippy with zero warnings, full test suite execution, doc check, Bell and QASM examples, and Python reference parser verification.

## 5. Expected Files and Modules
- New files:
  - `src/frontend/parser/expressions.rs`
  - `tests/qasm3/expressions.rs`
  - `docs/project-state/plans/phase-021.md`
  - `docs/project-state/reports/phase-021.md`
- Modified files:
  - `src/frontend/parser/mod.rs`
  - `src/frontend/parser/gates.rs`
  - `src/frontend/adapter.rs`
  - `src/frontend/mod.rs`
  - `src/lib.rs`
  - `tests/qasm3.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Integration tests in `tests/qasm3/expressions.rs`:
  1. Integer and floating-point literals.
  2. `pi` and symbolic identifiers.
  3. Unary positive and negative signs (`+theta`, `-theta`, `--theta`).
  4. Parenthesized expressions.
  5. Addition and subtraction.
  6. Multiplication and division.
  7. Precedence (`theta * 2 + pi / 4`).
  8. Left associativity (`a - b - c`, `a / b / c`).
  9. Nested expressions (`(theta + pi) / 2`).
  10. Symbolic preservation when symbols are unresolved.
  11. Numerical evaluation with symbol environment (`env.insert("theta", ...)`).
  12. Literal division by zero rejection.
  13. Division by zero after symbol binding.
  14. Unresolved identifiers at binding/evaluation time.
  15. Malformed expressions and missing operands/parentheses.
  16. Hierarchical source-span correctness.
  17. Parameterized gate statement integration (`rx`, `ry`, `rz`).
  18. Regression tests for Phase 19 declarations and Phase 20 gate statements.
- Quality verification commands:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all-targets`
  - `cargo build`
  - `cargo doc --no-deps`
  - Milestone 1 examples and python3 `openqasm3` parser validation.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Expressions round-trip or evaluate with explicit error behavior.
- Command to execute: `cargo test --all-targets`
- Expected outcome: All tests pass with zero errors and zero warnings.
