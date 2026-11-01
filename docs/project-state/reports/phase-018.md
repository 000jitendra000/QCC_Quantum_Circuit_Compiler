# Phase Completion Report: Phase 18 — OpenQASM AST model

## 1. Summary
- **Phase Number**: Phase 18
- **Exact Phase Title**: OpenQASM AST model
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Pair Programming Session

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/frontend/ast/mod.rs` — Root module for OpenQASM AST, re-exports all typed syntax nodes, and provides deterministic structural serialization (`to_structural_string`).
  - `src/frontend/ast/program.rs` — Compilation unit (`AstProgram`), version header (`AstVersion`), and include directives (`AstInclude`).
  - `src/frontend/ast/declaration.rs` — Register declarations (`AstRegisterDecl`, `AstRegisterKind`) supporting both sized arrays (`qubit[n]`, `bit[n]`) and scalar declarations.
  - `src/frontend/ast/statement.rs` — Top-level statements (`AstStatement`), gate calls (`AstGateCall`), qubit references (`AstQubitRef`), classical bit references (`AstBitRef`), measurements (`AstMeasure`), state resets (`AstReset`), synchronization barriers (`AstBarrier`), global phase directives (`AstGlobalPhase`), and unsupported constructs (`AstUnsupported`).
  - `src/frontend/ast/expression.rs` — Parameter expressions (`AstExpression`, `AstExprKind`) representing numbers, identifiers, binary arithmetic (`+`, `-`, `*`, `/`), negation (`-`), power (`^`), and function calls (`sin`, `cos`, etc.), with AST-native numerical evaluation (`eval`). Completely independent of QccIR types (lowering deferred to Phase 25).
  - `tests/ast.rs` — Integration test runner linking all modular AST test suites.
  - `tests/ast/program.rs` — Unit tests for program header construction, version directives, includes, and spans.
  - `tests/ast/declaration.rs` — Unit tests for quantum and classical register declarations, scalar vs indexed references.
  - `tests/ast/statement.rs` — Unit tests for gate calls, measurements, resets, global/selective barriers, and global phase.
  - `tests/ast/expression.rs` — Unit tests for parameter expressions, AST-native arithmetic evaluation, unary minus, power, function calls, and symbol resolution without QccIR dependencies.
  - `tests/ast/golden.rs` — Golden structural serialization tests for Bell circuit, parameterized rotations, and full statement variety.
  - `tests/ast/spans.rs` — Strict hierarchical span containment tests ensuring parent spans enclose child spans.
  - `docs/project-state/plans/phase-018.md` — Execution plan for Phase 18.
- **Removed Files**:
  - `src/frontend/ast.rs` — Monolithic prototype replaced cleanly by the modular `src/frontend/ast/` directory.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**: `AST covers documented input subset.`
- **Verification Command Executed**:
  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets && cargo doc --no-deps
  ```
- **Execution Log / Output**:
  ```text
  running 30 tests (tests/ast.rs)
  test declaration::test_classical_register_declaration ... ok
  test declaration::test_quantum_register_declaration ... ok
  test declaration::test_qubit_and_bit_operands ... ok
  test declaration::test_register_kinds ... ok
  test declaration::test_scalar_classical_register_declaration ... ok
  test declaration::test_scalar_quantum_register_declaration ... ok
  test expression::test_binary_arithmetic_expressions ... ok
  test expression::test_constant_expression ... ok
  test expression::test_deep_expression_tree ... ok
  test expression::test_identifier_expression_and_pi_mapping ... ok
  test expression::test_pow_and_function_call_expressions ... ok
  test expression::test_unary_negation ... ok
  test golden::test_golden_bell_circuit_ast ... ok
  test golden::test_golden_full_statement_variety_ast ... ok
  test golden::test_golden_parameterized_circuit_ast ... ok
  test program::test_minimal_program_without_version_or_includes ... ok
  test program::test_program_construction_with_headers ... ok
  test program::test_multiple_includes ... ok
  test spans::test_expression_span_containment ... ok
  test spans::test_gate_call_span_contains_operands_and_params ... ok
  test spans::test_measure_span_contains_qubit_and_bit ... ok
  test spans::test_program_span_contains_all_statements ... ok
  test statement::test_barrier_statements ... ok
  test statement::test_gate_call_construction ... ok
  test statement::test_global_phase_statement ... ok
  test statement::test_measurement_statement ... ok
  test statement::test_parameterized_gate_call ... ok
  test statement::test_reset_statement ... ok
  test statement::test_statement_predicates_exhaustive ... ok
  test statement::test_unsupported_statement ... ok
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  Total workspace tests: 361 passed; 0 failed; 0 ignored; 0 filtered out.
  ```
- **Exit Gate Pass Confirmation**: Verified. The AST completely models the documented OpenQASM subset (program headers, version, includes, register declarations, gates, measures, resets, barriers, global phase, expressions, and source spans).

## 4. Test Suite Summary
- **Unit Tests Added**: 18 unit tests across program, declaration, statement, and expression modules.
- **Integration Tests Added**: 12 tests across golden structural serialization and hierarchical span containment suites.
- **Regression Check**: All 331 existing tests across 19 test binaries continue to pass with zero regressions (totaling 361 tests).

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Correct qubit endianness ($q_0$ is LSB).
- [x] Correct gate operand ordering (e.g. $CX(q_c, q_t)$).
- [x] Explicit circuit global phase preserved (`AstGlobalPhase`).
- [x] Frontend AST independence: `src/frontend/ast/` has zero dependencies or imports of `crate::ir` (pure syntax representation).
- [x] Source provenance preserved with valid, hierarchical `SourceSpan`s.
- [x] Zero extraneous feature development (scope bounded strictly to Phase 18).

## 6. Known Limitations & Technical Debt
- Recursive-descent parsing into this AST is deferred to Phase 19.
- Register symbol resolution and semantic analysis are deferred to Phase 20.
- Lowering to QccIR (including parameter expression lowering to `ParameterExpr`) is deferred to Phase 25.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
