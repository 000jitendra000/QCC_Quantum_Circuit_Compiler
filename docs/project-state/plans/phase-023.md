# Phase Plan: Phase 23 — Semantic Analysis and Diagnostics

## 1. Metadata
- **Phase Number**: Phase 23
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 19, Phase 20, Phase 21, Phase 22 completed and verified in `docs/project-state/progress.md`.
- **Source Document Page**: Page 16 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Reject semantically invalid OpenQASM programs before lowering.
- **Explicit Inclusions**:
  - `src/diagnostics/qasm.rs`: Standardized diagnostic codes (`E0010`–`E0032`) and diagnostic construction helpers.
  - `src/frontend/semantic/symbols.rs`: Semantic symbol table (`SymbolTable`) wrapping the authoritative `RegisterTable` and tracking parameter symbol bindings.
  - `src/frontend/semantic/checks.rs`: Comprehensive semantic checks across AST nodes: register declarations, gate calls (names, parameter arity, qubit arity, operand references, duplicate multi-qubit operands, parameter domains/unresolved symbols), measurement operands (source quantum, destination classical, size matching), reset operand typing, and barrier operand typing.
  - `src/frontend/semantic/mod.rs`: Top-level `SemanticAnalyzer` driver and semantic analysis result types.
  - `tests/qasm3/semantic.rs`: Comprehensive semantic negative corpus and positive validation tests.
- **Explicit Exclusions**:
  - No AST-to-QccIR lowering (deferred to Phase 25).
  - No unsupported language extensions (classical control flow, custom gates deferred to Phase 24/later).
  - No modification to core QccIR or simulator.
  - Pure frontend isolation: `src/frontend/` must never import `crate::ir` or `crate::quantum`.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero `#![feature]` flags).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] CX operand order preserved: operand 0 is control, operand 1 is target.
- [x] Stable diagnostic codes attached (`E0010`–`E0032`).
- [x] Distinct semantic errors from lexical and syntactic errors.
- [x] Register identity delegated to existing authoritative `RegisterTable`.
- [x] Gate signatures reused from existing registry (`SUPPORTED_GATES`).
- [x] Fail-closed semantic rejection: invalid programs cannot produce valid output.
- [x] Pure frontend isolation: zero dependencies on `crate::ir` or `crate::quantum`.

## 4. Step-by-Step Implementation Sequence
1. Create `src/diagnostics/qasm.rs` defining standard diagnostic codes and helper constructors. Re-export in `src/diagnostics/mod.rs`.
2. Implement `src/frontend/semantic/symbols.rs` defining `SymbolTable` delegating to `RegisterTable`.
3. Implement `src/frontend/semantic/checks.rs` with fine-grained semantic checks for all AST statement kinds.
4. Implement `src/frontend/semantic/mod.rs` with `SemanticAnalyzer` driver, result bundling, and convenience methods.
5. Expose `semantic` in `src/frontend/mod.rs` and `src/lib.rs`.
6. Implement `tests/qasm3/semantic.rs` covering all required negative test cases and positive baseline tests. Register in `tests/qasm3.rs`.
7. Execute full verification pipeline: fmt, clippy, tests, build, doc, examples, python openqasm3 check.
8. Update progress ledger and completion report, then commit as `feat(phase-23): implement semantic analysis and diagnostics`.

## 5. Expected Files and Modules
- New files:
  - `src/diagnostics/qasm.rs`
  - `src/frontend/semantic/mod.rs`
  - `src/frontend/semantic/symbols.rs`
  - `src/frontend/semantic/checks.rs`
  - `tests/qasm3/semantic.rs`
  - `docs/project-state/plans/phase-023.md`
  - `docs/project-state/reports/phase-023.md`
- Modified files:
  - `src/diagnostics/mod.rs`
  - `src/frontend/mod.rs`
  - `src/frontend/adapter.rs`
  - `src/lib.rs`
  - `tests/qasm3.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Negative tests:
  - Undefined register (`E0012`).
  - Out-of-range bit/index (`E0014`).
  - Wrong gate arity (`E0021`, `E0022`).
  - Invalid measurement destination (`E0013`).
  - Invalid measurement source (`E0013`).
  - Unresolved parameter (`E0031`).
  - Parameter-count mismatches (`E0021`).
  - Invalid parameter domains (division by zero `E0030`, NaN/inf `E0024`/`E0032`).
  - Reset operand errors (`E0013`).
  - Duplicate qubit operands in multi-qubit gates (`E0023`).
  - Whole-register measurement size mismatch (`E0025`).
  - Source-span accuracy for diagnostics.
- Positive tests:
  - Valid programs analyze cleanly and return valid `SymbolTable`.
  - Bound parameters evaluate correctly.
  - Standard mathematical constant `pi` evaluates without binding.
- Regressions:
  - All existing 419 workspace tests pass.

## 7. Exit Gate Verification Target
> **EXIT GATE CRITERIA**: Semantic negative corpus passes.
> - Command to execute: `cargo test --test qasm3`
> - Expected outcome: Clean pass with zero failures and zero warnings.
