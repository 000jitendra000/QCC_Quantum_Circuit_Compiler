# Phase Plan: Phase 18 — OpenQASM AST model

## 1. Metadata
- **Phase Number**: Phase 18
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 17 completed and verified in ledger ([`docs/project-state/reports/phase-017.md`](../reports/phase-017.md))
- **Source Document Page**: Page 14 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Represent supported OpenQASM syntax in a modular frontend AST, separately from QccIR and internal circuit operations.
- **Explicit Inclusions**:
  - Reorganize AST definitions from monolithic `src/frontend/ast.rs` into modular submodules:
    - `src/frontend/ast/mod.rs` (module definitions, re-exports, and structural golden serialization)
    - `src/frontend/ast/program.rs` (`AstProgram`, `AstVersion`, `AstInclude`)
    - `src/frontend/ast/declaration.rs` (`AstRegisterDecl`, `AstRegisterKind`)
    - `src/frontend/ast/statement.rs` (`AstStatement`, `AstGateCall`, `AstQubitRef`, `AstBitRef`, `AstMeasure`, `AstReset`, `AstBarrier`, `AstGlobalPhase`, `AstUnsupported`)
    - `src/frontend/ast/expression.rs` (`AstExpression`, `AstExprKind`, arithmetic operators, function calls, and conversions)
  - Full source-span attachment to all relevant AST nodes (`span(&self) -> SourceSpan`).
  - Deterministic structural representation / golden serialization (`to_structural_string(&self)`).
  - Integration test suite under `tests/ast/` and `tests/ast.rs`.
- **Explicit Exclusions**:
  - Do NOT implement full OpenQASM recursive-descent parsing (Phase 19).
  - Do NOT implement register symbol table resolution or bounds checking (Phase 19–20).
  - Do NOT lower AST directly to QccIR (Phase 25).
  - Do NOT restructure workspace crate boundaries (Stage 1 consolidated crate policy).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (MSRV 1.80.0, zero `#![feature]` flags).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Explicit circuit global phase tracked and preserved (`AstGlobalPhase`).
- [x] AST does not leak internal QccIR types or parser-specific constructs.
- [x] Every accepted construct has an exact source-preserving AST representation with valid source spans.

## 4. Step-by-Step Implementation Sequence
1. **Refactor AST Structure into Submodules**:
   - Create `src/frontend/ast/program.rs` containing `AstProgram`, `AstVersion`, and `AstInclude`.
   - Create `src/frontend/ast/declaration.rs` containing `AstRegisterDecl` and `AstRegisterKind`.
   - Create `src/frontend/ast/statement.rs` containing `AstStatement`, `AstGateCall`, `AstQubitRef`, `AstBitRef`, `AstMeasure`, `AstReset`, `AstBarrier`, `AstGlobalPhase`, `AstUnsupported`.
   - Create `src/frontend/ast/expression.rs` containing `AstExpression`, `AstExprKind`, constructors, and arithmetic helpers.
   - Create `src/frontend/ast/mod.rs` re-exporting all public AST types and providing `to_structural_string` for golden serialization.
   - Remove legacy `src/frontend/ast.rs`.
2. **Verify Backward Compatibility**:
   - Ensure all re-exports in `src/frontend/mod.rs` and `src/lib.rs` remain unbroken.
   - Run `cargo check --all-targets` to confirm `src/frontend/adapter.rs` and existing tests compile seamlessly.
3. **Implement Integration Tests**:
   - `tests/ast/program.rs`: Version header, include statements, and program span bounds.
   - `tests/ast/declaration.rs`: Sized/scalar quantum and classical register declarations.
   - `tests/ast/statement.rs`: Gate calls, measurement syntax, reset, barriers, global phase, unsupported constructs.
   - `tests/ast/expression.rs`: Expression trees, operator precedence, binary operations, unary negation, function calls.
   - `tests/ast/golden.rs`: Deterministic structural serialization of complete ASTs.
   - `tests/ast/spans.rs`: Strict hierarchical span enclosing assertions.
   - `tests/ast.rs`: Root runner linking all `tests/ast/` modules.
4. **Run Verification & Quality Suite**:
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all-targets`
   - `cargo doc --no-deps`
   - Run example binaries and Python reference validation.

## 5. Expected Files and Modules
- New files to create:
  - `src/frontend/ast/mod.rs`
  - `src/frontend/ast/program.rs`
  - `src/frontend/ast/declaration.rs`
  - `src/frontend/ast/statement.rs`
  - `src/frontend/ast/expression.rs`
  - `tests/ast.rs`
  - `tests/ast/mod.rs` (or referenced submodules)
  - `tests/ast/program.rs`
  - `tests/ast/declaration.rs`
  - `tests/ast/statement.rs`
  - `tests/ast/expression.rs`
  - `tests/ast/golden.rs`
  - `tests/ast/spans.rs`
- Files to remove:
  - `src/frontend/ast.rs` (replaced by `src/frontend/ast/`)
- Existing files to modify:
  - `src/frontend/mod.rs` (update documentation / exports if needed)
  - `docs/project-state/progress.md` (progress update upon verification)

## 6. Verification & Test Plan
- Unit tests:
  - Construct program AST with version and multiple includes.
  - Construct sized and scalar register declarations.
  - Construct gate calls with multi-qubit operands and parameterized angles.
  - Construct measure, reset, barrier, and global phase statements.
  - Construct deep expression trees (arithmetic, functions, symbols).
  - Verify span coverage across all nodes and children.
- Golden tests:
  - Structural golden serialization of Bell state program, parameterized rotation program, and full statement variety program.
- Regressions:
  - Full suite of 331 existing tests must pass with zero regressions.

## 7. Exit Gate Verification Target
> **EXIT GATE**: AST covers documented input subset.
- Command to execute: `cargo test`
- Expected outcome: Clean pass with zero failures and zero warnings across all test targets.
