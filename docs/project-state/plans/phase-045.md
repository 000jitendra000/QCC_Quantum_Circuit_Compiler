# Phase Plan: Phase 45 — Layout-Aware Equivalence

## 1. Metadata
- **Phase Number**: Phase 45
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 10 (Unitary equivalence checker) and Phase 44 (Logical/physical layout model) completed.
- **Source Document Page**: Page 25 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Implement layout-aware semantic equivalence verification comparing logical reference circuits against routed physical circuits accounting for initial layout ($L_{\text{initial}}$), final layout ($L_{\text{final}}$), and ADR-0001 strict ancilla zero-state preservation over the complete $2^n$-dimensional physical statevector up to one common scalar global phase.
- **Explicit Inclusions**:
  - Module `src/verification/layout_equivalence.rs`.
  - State embedding helper `embed_logical_state` embedding logical $2^m$ statevectors into full $2^n$ physical registers with unmapped physical qubits strictly set to $|0\rangle$.
  - Injective little-endian bit placement: logical qubit $q_j$ (bit $j$) maps to physical qubit $p_k$ (bit $k$).
  - `LayoutEquivalenceChecker` struct and `check_layout_equivalence` API.
  - Complete $2^n$-dimensional physical comparison enforcing ADR-0001 (ancilla leakage or entanglement fails verification).
  - Common global phase estimation across all test states.
  - Computational basis mode ($m \le \text{max\_matrix\_qubits}$) and deterministic seeded randomized states mode ($m > \text{max\_matrix\_qubits}$).
  - Re-exports in `src/verification/mod.rs` and `src/lib.rs`.
- **Explicit Exclusions**:
  - Do NOT implement initial placement algorithms (Phase 46).
  - Do NOT implement routing algorithms (Phases 47–52).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero nightly flags).
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB, $p_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Explicit circuit global phase verified against one single common global phase across all basis states.
- [x] ADR-0001 strict ancilla zero-state policy: Every physical qubit not occupied by a logical qubit starts in $|0\rangle$ and must return to $|0\rangle$. Complete $2^n$ physical statevector comparison.
- [x] Layout permutations applied exactly once; no double-inversion.
- [x] No petgraph NodeIndex exposure; strictly uses stable typed identifiers.

## 4. Step-by-Step Implementation Sequence
1. **Extend VerificationError (`src/verification/unitary.rs`)**:
   - Add `Layout(#[from] LayoutError)` variant for clean error propagation.
2. **State Embedding Helper (`src/verification/layout_equivalence.rs`)**:
   - Implement `embed_logical_state` mapping $2^m$ logical state to $2^n$ physical state with little-endian bit mapping and zero-initialized ancillae.
3. **Layout-Aware Equivalence Checker (`src/verification/layout_equivalence.rs`)**:
   - Implement `LayoutEquivalenceChecker` with computational basis and randomized state evaluation.
   - Implement `check_layout_equivalence` free function.
4. **Re-exports (`src/verification/mod.rs`, `src/lib.rs`)**:
   - Re-export `embed_logical_state`, `LayoutEquivalenceChecker`, `check_layout_equivalence`.
5. **Integration Tests (`tests/equivalence/layout.rs`, `tests/equivalence.rs`)**:
   - Identity mapping.
   - Routed circuit with inserted SWAP.
   - Changed final permutation.
   - Random valid layouts.
   - Deliberately incorrect final layouts (must fail).
   - Unused physical qubit initialization and final $|0\rangle$ requirement.
   - Leakage into unused physical qubit (must fail).
   - Entanglement with unused physical qubit (must fail).
   - Global-phase-sensitive cases.
   - Little-endian multi-qubit permutation checks.
6. **Full Project Verification**:
   - Format, Clippy, test, build, doc, examples, independent OpenQASM parser.
7. **Report & Commit**:
   - Write `docs/project-state/reports/phase-045.md`, update progress ledger, commit `feat(phase-45): implement layout-aware equivalence`.

## 5. Expected Files and Modules
- New files:
  - `src/verification/layout_equivalence.rs`
  - `tests/equivalence/layout.rs`
  - `docs/project-state/plans/phase-045.md`
  - `docs/project-state/reports/phase-045.md`
- Existing files to modify:
  - `src/verification/unitary.rs`
  - `src/verification/mod.rs`
  - `src/lib.rs`
  - `tests/equivalence.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit and integration tests covering all required test scenarios.
- Regression testing across all existing 35 test binaries.

## 7. Exit Gate Verification Target
- Correctly routed circuits pass layout-aware semantic comparison.
- Incorrect final permutations fail.
- Unused physical qubits satisfy mandatory $|0\rangle$ ancilla policy.

