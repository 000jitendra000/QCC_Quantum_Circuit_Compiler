# Phase Plan: Phase 20 — Standard Gate Statement Parsing

## 1. Metadata
- **Phase Number**: Phase 20
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **M**
- **Prerequisites Satisfied**: Phase 18 (AST model) and Phase 19 (register declaration parsing & symbol table) completed and verified in ledger.
- **Source Document Page**: Page 15 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Parse supported standard OpenQASM gate statements into the existing frontend AST, preserving gate identity, parameter expressions without lossy eager evaluation, source spans, and ordered operands.
- **Explicit Inclusions**:
  - Supported MVP gates:
    - Single-qubit fixed: `x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`
    - Parameterized single-qubit: `rx`, `ry`, `rz`
    - Two-qubit entangling: `cx`, `cz`, `swap`
  - Case-sensitive gate name validation and lookup (`GateSignature`).
  - Parameter-first parsing: `rx(theta) q[1]`, preserving `AstExpression` trees.
  - Basic arity validation:
    - Parameter counts (0 for fixed gates, 1 for rotation gates; reject extra/missing).
    - Qubit operand counts (1 for single-qubit gates, 2 for two-qubit gates; reject too few or too many).
  - Operand order preservation (exact written order: `cx q[c], q[t]` operand 0 is control, operand 1 is target; `swap q[a], q[b]` preserved).
  - Operand resolution and fail-closed validation via Phase 19 `RegisterTable` (bounds $0 \le k < n$, scalar vs array indexing, undeclared registers).
  - Source-located diagnostics with standardized codes:
    - `E0001`: Syntax errors (missing semicolon, malformed parameters/operands).
    - `E0003`: Unsupported 3-qubit gates (`ccx`, `cswap`).
    - `E0004`: Unsupported derived gates (`u1`, `u2`, `u3`, `p`, etc.).
    - `E0020`: Unknown gate name (with case-sensitivity note if capitalized like `H` or `CX`).
    - `E0021`: Gate parameter arity mismatch (extra or missing parameter list).
    - `E0022`: Gate qubit operand arity mismatch (too few or too many operands).
- **Explicit Exclusions**:
  - General parameter expression grammar/precedence extensions beyond existing `AstExpression` support (Phase 21).
  - Measurement, reset, and barrier parsing (Phase 22).
  - Semantic analysis beyond basic arity and register resolution (Phase 23).
  - AST-to-QccIR lowering (Phase 25).
  - Adding gates outside the documented MVP set.
  - Pure frontend independence: `src/frontend/` does not import `crate::ir`.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] CX operand order invariant: Operand 0 is control, Operand 1 is target.
- [x] SWAP and CZ operand order invariant: Preserve written sequence in AST.
- [x] Parameter expressions preserved without eager lossy float conversion.
- [x] Case-sensitive gate names enforced; no silent case normalization.
- [x] Fail-closed bounds checking via `RegisterTable`.
- [x] Pure frontend AST independence: zero `crate::ir` imports in `src/frontend/`.

## 4. Step-by-Step Implementation Sequence
1. **Operand Parser Module (`src/frontend/parser/operands.rs`)**:
   - Implement `OperandParser` to parse single qubit references (`q` or `q[k]`) and comma-separated operand lists.
   - Resolve operands using `RegisterTable`, returning structured diagnostics for undeclared registers, bounds violations, and type mismatches.
2. **Gate Parser Module (`src/frontend/parser/gates.rs`)**:
   - Implement `GateSignature` defining parameter and qubit operand requirements for all MVP gates.
   - Implement `lookup_gate_signature` and `find_similar_gate` for case-sensitivity suggestions.
   - Implement `GateParser` supporting parameter-first syntax (`gate(params) qubits;`), arity verification, and error recovery.
3. **Module Wiring & Adapter Integration**:
   - Expose `gates` and `operands` in `src/frontend/parser/mod.rs` and `src/frontend/mod.rs`.
   - Update `OpenQasmFrontend` / `Parser` in `src/frontend/adapter.rs` to delegate gate parsing and operand parsing to the new modular parsers.
4. **Comprehensive Test Suite (`tests/qasm3/gates.rs` & `tests/qasm3.rs`)**:
   - Add tests covering all 14 required verification scenarios.
   - Verify regression suite across all existing targets.

## 5. Expected Files and Modules
- New files:
  - `src/frontend/parser/gates.rs`
  - `src/frontend/parser/operands.rs`
  - `tests/qasm3/gates.rs`
  - `docs/project-state/plans/phase-020.md`
  - `docs/project-state/reports/phase-020.md`
- Modified files:
  - `src/frontend/parser/mod.rs`
  - `src/frontend/adapter.rs`
  - `src/frontend/mod.rs`
  - `src/lib.rs`
  - `tests/qasm3.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit and integration tests in `tests/qasm3/gates.rs`:
  1. Parsing each supported gate (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `rx`, `ry`, `rz`, `cx`, `cz`, `swap`).
  2. Parameterized forms (`rx(theta) q[1]`, `ry(0.5 * pi) q[0]`, `rz(-pi / 4.0) q[2]`).
  3. Correct AST gate names and parameter expression preservation.
  4. Single-qubit and two-qubit operand handling.
  5. CX control/target order preservation (`cx q[0], q[1]` vs `cx q[1], q[0]`).
  6. SWAP and CZ operand order preservation (`swap q[0], q[1]` vs `swap q[1], q[0]`).
  7. Unknown gate names (`foo q[0];` rejected with `E0020`).
  8. Incorrect capitalization and case sensitivity (`H q[0];`, `CX q[0], q[1];`, `Rx(0.5) q[0];`).
  9. Too few and too many operands (`h;`, `h q[0], q[1];`, `cx q[0];`, `cx q[0], q[1], q[2];`).
  10. Missing or malformed parameter lists (`rx q[0];`, `rx() q[0];`, `rx(1.0, 2.0) q[0];`, `h(1.0) q[0];`).
  11. Undeclared registers and out-of-bounds indices (`h unknown_q[0];`, `cx q[0], q[5];`).
  12. Source-span correctness for gate statements, parameters, and operands.
  13. Multiple gate statements in one program with comments and whitespace.
  14. Regression tests ensuring Phase 19 declarations and register resolution continue to pass.
- Verification commands:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all-targets`
  - `cargo build`
  - `cargo doc --no-deps`
  - Milestone 1 examples (`bell`, `qasm`) and python3 `openqasm3` reference parser.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Gate parser tests pass for documented subset.
- Command to execute: `cargo test --all-targets`
- Expected outcome: All tests pass with zero errors and zero warnings.
