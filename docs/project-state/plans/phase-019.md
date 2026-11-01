# Phase Plan: Phase 19 — QASM register declaration parsing

## 1. Metadata
- **Phase Number**: Phase 19
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **M**
- **Prerequisites Satisfied**: Phase 18 completed and verified in ledger ([`docs/project-state/reports/phase-018.md`](../reports/phase-018.md))
- **Source Document Page**: Page 14 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Parse and resolve static quantum and classical registers in the OpenQASM 3 frontend, establishing a strongly-typed register symbol table, source declaration order preservation, and semantic bounds checking.
- **Explicit Inclusions**:
  - `src/frontend/parser/mod.rs`: Parser module root exposing declarations and registers.
  - `src/frontend/parser/registers.rs`:
    - Strongly-typed frontend IDs: `QuantumRegisterId`, `ClassicalRegisterId`, `QubitId`, `BitId`.
    - Register entries: `QuantumRegisterEntry`, `ClassicalRegisterEntry`, `RegisterEntry`.
    - Resolved operands: `ResolvedQubit`, `ResolvedBit`.
    - Symbol table: `RegisterTable` with declaration order preservation, duplicate detection (including cross-type collisions), zero-size rejection, and strict index bounds checking (`0..size-1`).
    - Diagnostic generation with structured error codes (`E0010`–`E0015`).
  - `src/frontend/parser/declarations.rs`:
    - Parser routines for version directives, include statements, and register declarations (`qubit[n]`, `qubit`, `bit[n]`, `bit`).
  - Integration with `OpenQasmFrontend` in `src/frontend/adapter.rs` to validate declarations and operand references during parsing.
  - Integration test suite under `tests/qasm3/declarations.rs` and `tests/qasm3.rs`.
- **Explicit Exclusions**:
  - Do NOT implement standard gate statement parsing (Phase 20).
  - Do NOT implement parameter expression parsing beyond existing behavior (Phase 21).
  - Do NOT implement full measurement/barrier parsing refactor (Phase 22).
  - Do NOT lower AST or registers to QccIR (Phase 25).
  - Do NOT modify QccIR, quantum semantics, or statevector kernels.

## 3. Architecture & Semantic Invariants Checklist
- [x] Stable Rust toolchain only (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Pure frontend AST and parser independence: zero dependencies on `crate::ir`.
- [x] Distinct typed IDs for quantum and classical registers.
- [x] Preserve source declaration order independently of hash map iteration order.
- [x] Strict index bounds validation: for register of size $n$, valid indices are $0 \le k < n$.
- [x] Fail-closed validation: zero-sized declarations, duplicate names, and undeclared references produce diagnostic errors.
- [x] Exact source spans attached to all declarations, references, and diagnostics.

## 4. Step-by-Step Implementation Sequence
1. **Implement `src/frontend/parser/registers.rs`**:
   - Define typed IDs: `QuantumRegisterId`, `ClassicalRegisterId`, `QubitId`, `BitId`.
   - Define register entries and resolved operand structs.
   - Define `RegisterTable` with methods for quantum/classical declarations, indexing resolution, duplicate checking, and bounds verification.
   - Define `RegisterError` with conversion to compiler `Diagnostic`.
2. **Implement `src/frontend/parser/declarations.rs`**:
   - Define declaration parser helper routines that validate syntax and populate the `RegisterTable`.
3. **Expose Parser Module**:
   - Create `src/frontend/parser/mod.rs` and re-export types through `src/frontend/mod.rs` and `src/lib.rs`.
4. **Integrate Register Resolution into Frontend Parser**:
   - Update `Parser` in `src/frontend/adapter.rs` to maintain a `RegisterTable`, validate all register declarations, and resolve qubit/bit operands in statements (gates, measures, resets, barriers).
5. **Implement Comprehensive Tests**:
   - `tests/qasm3/declarations.rs` and `tests/qasm3.rs` covering all 13 required test categories.
6. **Mandatory Verification**:
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all-targets`
   - `cargo doc --no-deps`
   - `cargo run --example bell` and `cargo run --example qasm`
   - Independent parser check with `openqasm3`.

## 5. Expected Files and Modules
- New files:
  - `src/frontend/parser/mod.rs`
  - `src/frontend/parser/registers.rs`
  - `src/frontend/parser/declarations.rs`
  - `tests/qasm3.rs`
  - `tests/qasm3/declarations.rs`
  - `docs/project-state/plans/phase-019.md`
  - `docs/project-state/reports/phase-019.md`
- Modified files:
  - `src/frontend/mod.rs`
  - `src/frontend/adapter.rs`
  - `src/lib.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit and integration tests in `tests/qasm3/declarations.rs`:
  - Multiple quantum and classical registers.
  - Sized vs scalar declarations.
  - Correct typed ID assignment and stable identities.
  - Declaration order preservation across iterations.
  - Version header and include parsing.
  - Zero-size declaration rejection (`qubit[0] q;`, `bit[0] c;`).
  - Duplicate register name rejection (within same kind and cross-kind collisions).
  - Undeclared register reference rejection.
  - Valid first (`0`) and last (`n-1`) index resolution.
  - Out-of-bounds rejection (`index == size` and `index > size`).
  - Quantum/classical type mismatch detection.
  - Source-span correctness for declarations and references.
  - Malformed syntax error diagnostics.
- Regressions:
  - All 361 previous tests across all 20 test targets must pass.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Register syntax and semantic bounds tests pass.
- Command to execute: `cargo test --all-targets`
- Expected outcome: All tests pass with zero errors and zero warnings.
