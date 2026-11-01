# Phase Implementation Plan: Phase 2 — Quantum Semantics and Gate Definitions

## 1. Metadata
- **Phase Number**: Phase 2
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Relative Effort**: M
- **Prerequisites Satisfied**: Phase 1 (Verified complete, commit `b7754bd`)
- **Source Document Page**: Page 6 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Freeze the mathematical conventions and define the initial gate catalog with verified matrices and inverses before statevector simulation or graph IR code accumulates.
- **Explicit Inclusions**:
  - `src/quantum/parameter.rs`: Angle representation with normalization, negation, arithmetic, and numeric global phase representation.
  - `src/quantum/semantics.rs`: Little-endian statevector indexing ($q_0$ is LSB), Dirac basis notation formatting ($|q_{n-1} \dots q_0\rangle$), column-vector convention, chronological multiplication order ($B \cdot A$), tensor product order, control/target operand order, separate measurement bitstring formatting, and semantic operation classification (`Unitary` vs `NonUnitary`).
  - `src/quantum/gate.rs`: Gate enum (X, Y, Z, H, S, Sdg, T, Tdg, Rx, Ry, Rz, CX, CZ, SWAP) with explicit arity, parameter count, operand roles, and representable inverses (`gate.inverse()`).
  - `src/quantum/gate_matrix.rs`: Concrete $2 \times 2$ and $4 \times 4$ complex matrices using `num_complex::Complex64`, matrix multiplication, adjoint ($U^\dagger$), trace, and unitarity checking ($U^\dagger U = I$).
  - `src/quantum/mod.rs`: Module exports.
  - `docs/semantics.md`: Full architectural and mathematical specification of conventions, Dirac notation, matrix definitions, and tensor rules.
  - `tests/quantum_semantics.rs`: Comprehensive test suite including exact matrix checks, proptest unitarity ($U^\dagger U = I$), inverse composition, rotation identities, endianness, operand orders, and global phase.
- **Explicit Exclusions**:
  - No statevector allocation or simulation kernels (deferred to Phase 3–4).
  - No QccIR DAG or petgraph dependencies (deferred to Phase 6).
  - No OpenQASM parser or emitter (deferred to Phases 9 & 15+).
  - No optimizer passes (deferred to Milestone M3).

## 3. Architecture & Semantic Invariants Checklist
- [x] Stable Rust only (zero nightly flags).
- [x] Logical $q[0]$ is the least-significant statevector bit (LSB).
- [x] Computational basis states displayed as $|q_{n-1} \dots q_0\rangle$.
- [x] Statevector index $i = \sum_{k=0}^{n-1} q_k \cdot 2^k$.
- [x] Statevectors are column vectors; chronological operations act by left-multiplication ($B \cdot A$).
- [x] Tensor product ordering consistent with bit indexing.
- [x] For $CX(q_c, q_t)$: operand 0 is control, operand 1 is target.
- [x] Measurement result bit formatting defined separately from statevector indexing.
- [x] Numeric circuit-level global phase $e^{i\phi}$ defined and preserved.
- [x] Unitary and non-unitary operations categorized explicitly.

## 4. Step-by-Step Implementation Sequence
1. Update `Cargo.toml` with `num-complex = "0.4"` and update `Cargo.lock`.
2. Implement `src/quantum/parameter.rs` (`Angle`, `GlobalPhase`).
3. Implement `src/quantum/semantics.rs` (endianness, index mapping, basis formatting, measurement formatting, operation categories, Kronecker tensor product).
4. Implement `src/quantum/gate_matrix.rs` (`Matrix2x2`, `Matrix4x4`, adjoints, matrix products, unitarity checks, and exact gate matrices).
5. Implement `src/quantum/gate.rs` (`Gate` enum, arity, parameters, inverses, and matrix dispatch).
6. Implement `src/quantum/mod.rs` and update `src/lib.rs`.
7. Author `docs/semantics.md` with complete mathematical documentation.
8. Implement comprehensive tests in `tests/quantum_semantics.rs`.
9. Run formatting, Clippy, and tests.
10. Update progress ledger and author completion report.

## 5. Expected Files and Modules
- `src/quantum/mod.rs`
- `src/quantum/gate.rs`
- `src/quantum/gate_matrix.rs`
- `src/quantum/semantics.rs`
- `src/quantum/parameter.rs`
- `tests/quantum_semantics.rs`
- `docs/semantics.md`
- `docs/project-state/plans/phase-002.md`
- `docs/project-state/reports/phase-002.md`
- `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit tests:
  - Exact matrix definitions for all 14 gates (X, Y, Z, H, S, Sdg, T, Tdg, Rx, Ry, Rz, CX, CZ, SWAP).
  - Unitarity checks for fixed gates.
  - Inverse composition for all gates.
  - Rotation identities ($R(0) = I$, $R(2\pi) = -I$, $R(\pi) = -i \sigma$, angle addition $R(\theta_1)R(\theta_2) = R(\theta_1+\theta_2)$).
  - Endianness and basis label formatting.
  - CX operand order discrimination ($CX(q_0, q_1) \neq CX(q_1, q_0)$).
  - Global phase composition.
- Property tests:
  - Proptest unitarity of $R_x(\theta), R_y(\theta), R_z(\theta)$ for arbitrary $\theta \in [-100, 100]$.
  - Proptest inverse composition $R(\theta) \cdot R(-\theta) = I$.
  - Proptest basis index and bit round-trip consistency.
- Commands:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`

## 7. Exit Gate Verification Target
> **EXIT GATE**: Semantic convention document and gate matrix tests pass.
