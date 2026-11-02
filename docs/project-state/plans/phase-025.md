# Phase Plan: Phase 25 — AST-to-QccIR lowering

## 1. Metadata
- **Phase Number**: Phase 25
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phases 6, 7, and 18–24 are completed and verified in [`docs/project-state/progress.md`](../progress.md).
- **Source Document Page**: Page 17 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Lower a semantically validated OpenQASM 3 AST into a canonical QccIR circuit while strictly preserving quantum semantics, operand identity, register allocation order, source statement ordering, circuit global phase, and source-provenance metadata.
- **Explicit Inclusions**:
  - `src/frontend/lowering/registers.rs`: Register mapping and allocation into QccIR quantum and classical registers, resolving single-bit/qubit references and whole-register expansions to typed QccIR `QubitId` and `ClassicalBitId`.
  - `src/frontend/lowering/operations.rs`: Lowering fixed gates (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`), parameterized rotation gates (`rx`, `ry`, `rz`), two-qubit entanglers (`cx`, `cz`, `swap`), measurement (single-bit and whole-register), active reset, synchronization barriers (global and selective), circuit global phase (`gphase`), and parameter expression conversion.
  - `src/frontend/lowering/mod.rs`: `CircuitLowerer` driver orchestrating semantic validation, register allocation, statement lowering, QccIR graph invariant validation, source-provenance preservation in `circuit.metadata`, and structured diagnostic generation.
  - `src/diagnostics/qasm.rs`: Stable lowering error code `E0040` (`QASM_LOWERING_ERROR`) and diagnostic constructor.
  - `tests/qasm3/lowering.rs`: Comprehensive integration test suite comparing OpenQASM-lowered circuits against Rust `CircuitBuilder` circuits via `UnitaryEquivalenceChecker`, verifying register allocation, operand ordering, measurement, reset, barrier dependencies, global phase, graph invariants, and fail-closed negative rejection.
- **Explicit Exclusions**:
  - Do not implement Phase 26 round-trip import/export.
  - Do not expand the supported OpenQASM language subset.
  - Do not move QccIR-specific conversion logic into frontend AST types.
  - Do not bypass semantic analysis or silently lower `AstStatement::Unsupported`.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable.
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Gate operand order: CX operand 0 = control, operand 1 = target.
- [x] Explicit circuit global phase tracked and preserved ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] Classical wire hazards (RAW, WAR, WAW) properly modeled via `WireAccessState`.
- [x] QccIR stable semantic `OperationId` maintained; no raw petgraph indices exposed.
- [x] QccIR graph validated (`circuit.validate()`) with zero invariant errors.
- [x] Fail-closed diagnostics for unsupported and invalid nodes.

## 4. Step-by-Step Implementation Sequence
1. **Diagnostic Support**: Add diagnostic code `QASM_LOWERING_ERROR` (`E0040`) and constructor helper in `src/diagnostics/qasm.rs`.
2. **Register Lowering**: Implement `src/frontend/lowering/registers.rs` mapping AST registers to `Circuit` registers and resolving typed `QubitId`/`ClassicalBitId` operands.
3. **Operation Lowering**: Implement `src/frontend/lowering/operations.rs` converting gates, parameters, measurements, resets, barriers, and global phase to QccIR `Operation` variants.
4. **Driver & Pipeline**: Implement `src/frontend/lowering/mod.rs` with `CircuitLowerer`, integrating with `SemanticAnalyzer`, `Circuit`, and `circuit.validate()`. Re-export in `src/frontend/mod.rs` and `src/lib.rs`.
5. **Frontend Adapter Exposure**: Add `lower` method to `OpenQasmFrontend` in `src/frontend/adapter.rs`.
6. **Tests**: Create `tests/qasm3/lowering.rs` and register it in `tests/qasm3.rs`.
7. **Verification**: Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`, `cargo build --all-targets`, `cargo doc --no-deps`, Bell/QASM examples, and external `openqasm3` parser check.

## 5. Expected Files and Modules
- New files to create:
  - `src/frontend/lowering/mod.rs`
  - `src/frontend/lowering/registers.rs`
  - `src/frontend/lowering/operations.rs`
  - `tests/qasm3/lowering.rs`
- Existing files to modify:
  - `src/diagnostics/qasm.rs`
  - `src/diagnostics/mod.rs`
  - `src/frontend/mod.rs`
  - `src/frontend/adapter.rs`
  - `src/lib.rs`
  - `tests/qasm3.rs`

## 6. Verification & Test Plan
- Builder-vs-QASM Bell circuit equivalence and DAG invariant comparison.
- Parameterized rotation circuit equivalence (`rx`, `ry`, `rz`) with constant and bound parameters.
- Measurement circuit equivalence (single-bit and whole-register, assignment and arrow forms).
- Register allocation, typed ID mapping, and operand order preservation (control vs target in CX).
- Reset and barrier (global and selective) dependency verification.
- Global-phase accumulation and preservation.
- Source-span preservation in `circuit.metadata`.
- QccIR graph invariant validation (`validate_circuit`).
- Negative tests: unsupported constructs, semantic errors, division by zero, unresolved symbols, duplicate operands in CX.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Lowering semantic equivalence passes.
- Verification: `UnitaryEquivalenceChecker` confirms equivalence between OpenQASM-lowered circuits and builder-constructed circuits, and all 21 test targets pass cleanly.

