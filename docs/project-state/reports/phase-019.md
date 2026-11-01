# Phase Completion Report: Phase 19 — QASM register declaration parsing

## 1. Summary
- **Phase Number**: Phase 19
- **Exact Phase Title**: QASM register declaration parsing
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Pair Programming Session

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/frontend/parser/mod.rs` — Root module for the handwritten recursive-descent frontend parser, exporting declaration parsing and register symbol tables.
  - `src/frontend/parser/registers.rs` — Symbol table and typed ID registry (`RegisterTable`, `QuantumRegisterId`, `ClassicalRegisterId`, `QubitId`, `BitId`, `ResolvedQubit`, `ResolvedBit`), duplicate name checking across namespaces, static array bounds checking ($0 \le k < n$), scalar vs array reference validation, and declaration order preservation.
  - `src/frontend/parser/declarations.rs` — Declaration parsing submodule (`DeclarationParser`) handling `OPENQASM <version>;`, `include "<filename>";`, and `qubit`/`bit` declarations (both scalar and indexed arrays).
  - `tests/qasm3.rs` — Integration test runner linking OpenQASM 3 integration test modules.
  - `tests/qasm3/declarations.rs` — Comprehensive integration tests covering all 13 declaration parsing and register resolution test cases.
  - `docs/project-state/plans/phase-019.md` — Execution plan for Phase 19.
  - `docs/project-state/reports/phase-019.md` — This phase completion report.
- **Modified Files**:
  - `src/frontend/mod.rs` — Re-exported parser module and public types (`RegisterTable`, `QuantumRegisterId`, `ClassicalRegisterId`, `QubitId`, `BitId`, `ResolvedQubit`, `ResolvedBit`).
  - `src/frontend/adapter.rs` — Integrated `RegisterTable` symbol table and declaration parsing into `OpenQasmFrontend` and `Parser`.
  - `src/lib.rs` — Re-exported frontend parser and register types for public consumption.
  - `docs/project-state/progress.md` — Updated ledger to reflect completion of Phase 19.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**: `Register syntax and semantic bounds tests pass.`
- **Verification Command Executed**:
  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets && cargo doc --no-deps
  ```
- **Execution Log / Output**:
  ```text
  running 14 tests (tests/qasm3.rs)
  test declarations::test_bounds_exact_size_rejected ... ok
  test declarations::test_bounds_exceeded_rejected ... ok
  test declarations::test_cross_namespace_duplicate_rejected ... ok
  test declarations::test_declaration_order_preserved ... ok
  test declarations::test_duplicate_classical_register_rejected ... ok
  test declarations::test_duplicate_quantum_register_rejected ... ok
  test declarations::test_indexing_scalar_rejected ... ok
  test declarations::test_missing_semicolon_in_declaration ... ok
  test declarations::test_multiple_registers_quantum_and_classical ... ok
  test declarations::test_source_span_correctness ... ok
  test declarations::test_undeclared_register_reference_rejected ... ok
  test declarations::test_unindexed_array_reference_rejected ... ok
  test declarations::test_version_and_include_parsing ... ok
  test declarations::test_zero_size_register_rejected ... ok
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  Total workspace tests: 375 passed; 0 failed; 0 ignored; 0 filtered out.
  ```
- **Exit Gate Pass Confirmation**: Verified. Register syntax parsing (`OPENQASM`, `include`, `qubit`, `bit`, sized arrays, and scalars), semantic symbol table tracking, duplicate name detection across namespaces, zero/invalid size rejection, and strict $[0, n)$ bounds checks pass with zero failures.

## 4. Test Suite Summary
- **Unit & Integration Tests Added**: 14 tests in `tests/qasm3/declarations.rs` covering:
  - Version declaration and multiple include directives (`test_version_and_include_parsing`)
  - Multiple quantum and classical registers (`test_multiple_registers_quantum_and_classical`)
  - Zero-size register rejection (`test_zero_size_register_rejected`)
  - Duplicate quantum and classical register detection (`test_duplicate_quantum_register_rejected`, `test_duplicate_classical_register_rejected`)
  - Cross-namespace duplicate name collision detection (`test_cross_namespace_duplicate_rejected`)
  - Undeclared register resolution rejection (`test_undeclared_register_reference_rejected`)
  - Off-by-one boundary rejection ($k = n$) (`test_bounds_exact_size_rejected`)
  - Out-of-bounds index rejection ($k > n$) (`test_bounds_exceeded_rejected`)
  - Scalar vs array indexing validation (`test_indexing_scalar_rejected`, `test_unindexed_array_reference_rejected`)
  - Declaration order preservation across iterations (`test_declaration_order_preserved`)
  - Accurate line/column source spans on syntax errors (`test_missing_semicolon_in_declaration`, `test_source_span_correctness`)
- **Regression Check**: All 361 previous tests across 20 test targets continue to pass without regression, bringing total passing tests to 375.

## 5. Architectural Invariants Verified
- [x] Stable Rust toolchain only (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Correct qubit endianness ($q_0$ is LSB).
- [x] Correct gate operand ordering (e.g. $CX(q_c, q_t)$).
- [x] Explicit circuit global phase preserved.
- [x] Pure frontend independence: `src/frontend/parser/` has zero dependencies or imports of `crate::ir`.
- [x] Fail-closed bounds checking: strict $0 \le k < n$ verification for all indexed references.
- [x] Stable, typed IDs: `QuantumRegisterId`, `ClassicalRegisterId`, `QubitId`, `BitId` distinguish quantum from classical registers and logical entities.
- [x] Zero extraneous feature development (scope adhered strictly to Phase 19: gate parsing deferred to Phase 20, parameter expression parsing deferred to Phase 21, lowering deferred to Phase 25).

## 6. Known Limitations & Technical Debt
- Standard gate statement parsing is deferred to Phase 20.
- Parameter expression parsing is deferred to Phase 21.
- Measurement, reset, and barrier parsing into statements is deferred to Phase 22.
- Lowering frontend AST and resolved registers into QccIR `Circuit` is deferred to Phase 25.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
