# Phase Completion Report: Phase 20 — Standard Gate Statement Parsing

## 1. Summary
- **Phase Number**: Phase 20
- **Exact Phase Title**: Standard gate statement parsing
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity Pair Programming Session

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/frontend/parser/gates.rs` — Standard gate statement parser (`GateParser`), gate signatures registry (`GateSignature`, `SUPPORTED_GATES`), case-sensitive lookup (`lookup_gate_signature`), case-mismatch suggestion helper (`find_similar_gate`), parameter-first syntax handling (`rx(theta) q[1]`), parameter and operand arity validation, and semicolon error recovery.
  - `src/frontend/parser/operands.rs` — Qubit and classical bit operand parser (`OperandParser`), parsing scalar and indexed array references (`q` and `q[k]`), comma-separated operand lists, trailing comma rejection, and fail-closed bounds/type validation against [`RegisterTable`].
  - `tests/qasm3/gates.rs` — Integration test suite containing 14 comprehensive test categories for gate statement parsing and semantic bounds.
  - `docs/project-state/plans/phase-020.md` — Phase 20 bounded implementation plan.
  - `docs/project-state/reports/phase-020.md` — This phase completion report.
- **Modified Files**:
  - `src/frontend/parser/mod.rs` — Re-exported `gates` and `operands` modules and public parser types.
  - `src/frontend/adapter.rs` — Integrated `GateParser` and `OperandParser` into `OpenQasmFrontend` and `Parser`, replacing monolithic prototypes with modular parser invocations.
  - `src/frontend/mod.rs` — Re-exported `GateParser`, `GateSignature`, `OperandParser`, `SUPPORTED_GATES`, `lookup_gate_signature`, and `find_similar_gate`.
  - `src/lib.rs` — Exposed new gate and operand parser types in the public library root.
  - `tests/qasm3.rs` — Registered `gates` integration test submodule alongside `declarations`.
  - `docs/project-state/progress.md` — Updated project ledger marking Phase 20 complete.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**: `Gate parser tests pass for documented subset.`
- **Verification Command Executed**:
  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets && cargo doc --no-deps
  ```
- **Execution Log / Output**:
  ```text
  running 28 tests (tests/qasm3.rs)
  test declarations::test_correct_typed_id_assignment_and_stable_identity ... ok
  test declarations::test_declaration_order_preservation ... ok
  test declarations::test_duplicate_register_names_rejected ... ok
  test declarations::test_malformed_declarations_diagnostics ... ok
  test declarations::test_multiple_quantum_and_classical_registers ... ok
  test declarations::test_out_of_bounds_indices_rejected ... ok
  test declarations::test_quantum_classical_type_mismatches_rejected ... ok
  test declarations::test_scalar_and_array_declarations ... ok
  test declarations::test_scalar_vs_array_indexing_mismatches_rejected ... ok
  test declarations::test_source_span_correctness ... ok
  test declarations::test_undeclared_register_references_rejected ... ok
  test declarations::test_valid_first_and_last_indices ... ok
  test declarations::test_version_and_include_parsing ... ok
  test declarations::test_zero_sized_declarations_rejected ... ok
  test gates::test_cx_operand_order_preservation ... ok
  test gates::test_gate_operand_register_resolution_and_bounds ... ok
  test gates::test_gate_statement_source_spans ... ok
  test gates::test_incorrect_capitalization_rejected_with_suggestion ... ok
  test gates::test_missing_semicolon_and_trailing_comma_rejected ... ok
  test gates::test_multiple_gate_statements_with_whitespace_and_comments ... ok
  test gates::test_parameter_arity_and_malformed_lists ... ok
  test gates::test_parameterized_gate_expression_preservation ... ok
  test gates::test_parse_all_supported_gates ... ok
  test gates::test_phase19_declarations_regression ... ok
  test gates::test_qubit_operand_arity_too_few_and_too_many ... ok
  test gates::test_qubit_operand_handling_scalar_and_indexed ... ok
  test gates::test_swap_and_cz_operand_order_preservation ... ok
  test gates::test_unknown_gate_rejected ... ok
  test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  Total workspace tests: 389 passed; 0 failed; 0 ignored; 0 filtered out.
  ```
- **Exit Gate Pass Confirmation**: Verified. Gate parser tests pass for the entire documented subset: all supported MVP gates (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `rx`, `ry`, `rz`, `cx`, `cz`, `swap`), case-sensitivity enforcement, arity validation, operand ordering preservation, register bounds checks, and parameter expression AST preservation.

## 4. Test Suite Summary
- **Unit & Integration Tests Added**: 14 tests in `tests/qasm3/gates.rs`:
  1. `test_parse_all_supported_gates`: Verifies all 14 supported gates parse cleanly into `AstGateCall`.
  2. `test_parameterized_gate_expression_preservation`: Verifies parameter expressions (`rx(theta)`, `ry(0.5 * pi)`, `rz(-pi / 4.0)`) retain full AST structure without lossy eager conversion.
  3. `test_qubit_operand_handling_scalar_and_indexed`: Verifies scalar and indexed array operands across 1-qubit and 2-qubit gates.
  4. `test_cx_operand_order_preservation`: Validates $CX$ operand 0 is control and operand 1 is target (`cx q[0], q[1]` vs `cx q[1], q[0]`).
  5. `test_swap_and_cz_operand_order_preservation`: Validates written operand order preservation for symmetric gates ($SWAP$, $CZ$).
  6. `test_unknown_gate_rejected`: Validates unknown gates (`foo q[0];`) are rejected with `E0020`.
  7. `test_incorrect_capitalization_rejected_with_suggestion`: Validates case sensitivity (`H`, `CX`, `Rx`, `SWAP` rejected with notes suggesting `h`, `cx`, `rx`, `swap`).
  8. `test_qubit_operand_arity_too_few_and_too_many`: Validates operand count bounds (`h;`, `h q[0], q[1];`, `cx q[0];`, `cx q[0], q[1], q[2];`) rejected with `E0022`.
  9. `test_parameter_arity_and_malformed_lists`: Validates parameter count checks (`rx q[0];`, `rx() q[0];`, `rx(1, 2) q[0];`, `h(1.0) q[0];`, `rx(0.5 q[0];`) rejected with `E0021` or `E0001`.
  10. `test_gate_operand_register_resolution_and_bounds`: Validates register table bounds checks ($k = n$, $k > n$, undeclared names, kind mismatches) within gate calls (`E0012`, `E0013`, `E0014`).
  11. `test_gate_statement_source_spans`: Validates exact line and 1-indexed column span accuracy for gate statements, parameters, and operands.
  12. `test_multiple_gate_statements_with_whitespace_and_comments`: Validates resilient parsing across comments, empty lines, and indentation.
  13. `test_missing_semicolon_and_trailing_comma_rejected`: Validates syntax error handling on missing semicolons and trailing commas (`E0001`).
  14. `test_phase19_declarations_regression`: Validates seamless interoperation between Phase 19 declarations and Phase 20 gate statements.
- **Regression Check**: All 375 previous tests continue to pass without regression, totaling 389 passing tests.

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Little-endian qubit indexing preserved ($q_0$ is LSB).
- [x] CX operand ordering preserved: operand 0 is control, operand 1 is target.
- [x] SWAP and CZ written operand sequence preserved.
- [x] Explicit global phase preserved without loss.
- [x] Parameter expressions preserved without eager lossy floating-point evaluation.
- [x] Case-sensitive gate names strictly enforced.
- [x] Fail-closed bounds checking ($0 \le k < n$) via `RegisterTable`.
- [x] Pure frontend independence: `src/frontend/` has zero dependencies or imports of `crate::ir` or `crate::quantum`.
- [x] Scope bounded strictly to Phase 20.

## 6. Known Limitations & Technical Debt
- Full parameter expression parsing with general classical variables and full operator precedence is deferred to Phase 21.
- Measurement (`= measure`, `measure ->`), reset, and barrier statement parsing into dedicated AST nodes is deferred to Phase 22.
- AST-to-QccIR lowering is deferred to Phase 25.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
