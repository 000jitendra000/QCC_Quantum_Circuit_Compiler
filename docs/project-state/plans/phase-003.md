# Phase Implementation Plan: Phase 3 — Single-Qubit In-Place Statevector Kernels

## 1. Metadata
- **Phase Number**: Phase 3
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Relative Effort**: M
- **Prerequisites Satisfied**: Phase 2 (Verified complete, commit `d0f7aa5`)
- **Source Document Page**: Page 7 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Implement the reference in-place statevector simulator core for single-qubit gates operating on amplitude pairs without full-system matrix allocation.
- **Explicit Inclusions**:
  - `src/simulator/statevector.rs`: `StateVector` holding $2^n$ `Complex64` amplitudes.
  - `src/simulator/error.rs`: `SimulatorError` hierarchy with typed error categories.
  - `src/simulator/kernels/single_qubit.rs`: Generic $2 \times 2$ in-place amplitude-pair kernel and gate-specific dispatches (X, Y, Z, H, S, Sdg, T, Tdg, Rx, Ry, Rz).
  - `src/simulator/kernels/mod.rs` & `src/simulator/mod.rs`: Module exports and facade methods.
  - Integration into `src/errors.rs` and `src/lib.rs`.
  - `tests/simulator_single_qubit.rs`: Comprehensive test suite including basis state initialization, gate truth tables, rotation gates, arbitrary target indices, proptest norm preservation, and error handling.
- **Explicit Exclusions**:
  - No two-qubit kernels (CX, CZ, SWAP amplitude quartets are Phase 4).
  - No terminal measurement probability sampling (Phase 5).
  - No DAG QccIR execution adapter (Phase 8).
  - No dense $2^n \times 2^n$ matrix allocation.

## 3. Architecture & Semantic Invariants Checklist
- [x] Stable Rust only (zero nightly flags).
- [x] Little-endian qubit indexing ($q_0$ is LSB with bitmask $1 \ll 0$).
- [x] In-place $\mathcal{O}(2^n)$ kernel execution: pair $(i_0, i_1)$ visited exactly once.
- [x] Arbitrary valid target qubit $q \in [0, n-1]$ supported.
- [x] Zero dense $2^n \times 2^n$ global matrix allocations.
- [x] Explicit validation: user amplitudes rejected if not normalized; never silently normalized.
- [x] Resource limits enforced prior to allocation ($16 \cdot 2^n$ bytes).

## 4. Step-by-Step Implementation Sequence
1. Implement `src/simulator/error.rs` (`SimulatorError`).
2. Update `src/errors.rs` with `QccError::Simulator`.
3. Implement `src/simulator/statevector.rs` (`StateVector`, basis initialization, custom amplitudes, norm calculation, resource limits).
4. Implement `src/simulator/kernels/single_qubit.rs` (generic $2 \times 2$ strided amplitude-pair kernel and single-qubit gate kernels).
5. Implement `src/simulator/kernels/mod.rs` and `src/simulator/mod.rs`.
6. Update `src/lib.rs` to expose `simulator` module.
7. Implement `tests/simulator_single_qubit.rs`.
8. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
9. Document completion in `docs/project-state/reports/phase-003.md` and update `docs/project-state/progress.md`.
10. Commit with `feat(phase-3): implement single-qubit statevector kernels`.

## 5. Expected Files and Modules
- `src/simulator/mod.rs`
- `src/simulator/statevector.rs`
- `src/simulator/error.rs`
- `src/simulator/kernels/mod.rs`
- `src/simulator/kernels/single_qubit.rs`
- `tests/simulator_single_qubit.rs`
- `docs/project-state/plans/phase-003.md`
- `docs/project-state/reports/phase-003.md`
- `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit tests:
  - $|0\dots 0\rangle$ and basis-state initialization ($|0\dots 0\rangle, |1\dots 1\rangle$, etc.).
  - Dimension and basis index bounds validation.
  - Rejection of unnormalized or non-power-of-two custom amplitudes.
  - Action of $X, Y, Z, H, S, S^\dagger, T, T^\dagger$ on computational basis states.
  - $H |0\rangle = \frac{|0\rangle + |1\rangle}{\sqrt{2}}$ and $H^2 = I$.
  - Action on arbitrary non-zero target qubits ($q_1, q_2, \dots$).
  - Action of $R_x(\theta), R_y(\theta), R_z(\theta)$ on basis states.
- Property tests:
  - Norm preservation ($\|U |\psi\rangle\| = 1$) under every single-qubit gate across arbitrary valid target indices on random normalized complex statevectors.
  - Invertibility: applying gate then its inverse restores the exact original statevector within floating-point tolerance.

## 7. Exit Gate Verification Target
> **EXIT GATE**: Randomized one-qubit tests pass for every valid target index.
