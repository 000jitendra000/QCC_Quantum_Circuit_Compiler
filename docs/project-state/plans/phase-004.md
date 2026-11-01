# Implementation Plan: Phase 4 — Two-Qubit and Controlled-Gate Kernels

## 1. Context & Objectives
- **Phase Number**: Phase 4
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Primary Goal**: Implement two-qubit operations directly over amplitude quartets with explicit operand order:
  - Support arbitrary distinct valid qubit indices (including non-adjacent qubits).
  - In-place $\mathcal{O}(2^n)$ application with $\mathcal{O}(1)$ auxiliary space.
  - No dense full-system operator allocation in production code.
  - Implement $CX$ (operand 0 = control, operand 1 = target), $CZ$, $SWAP$, and generic $4 \times 4$ unitaries.
  - Rigorous verification including Bell-state synthesis, randomized norm preservation, and comparison with an independent dense reference simulator in tiny tests.

## 2. Invariants & Conventions
1. **Little-Endian Qubit Indexing**: $q_0$ is LSB with bit weight $2^0 = 1$; $q_k$ has bit weight $2^k$.
2. **Column Vectors & Left Multiplication**: Statevectors are column vectors; operators act by left multiplication ($B \cdot A$).
3. **Operand Ordering**: For gate $U(q_A, q_B)$, operand 0 is $q_A$ and operand 1 is $q_B$.
   - Local basis index $k = \text{bit}(q_A) \cdot 1 + \text{bit}(q_B) \cdot 2 \in \{0, 1, 2, 3\}$.
   - In $CX(q_{\text{ctrl}}, q_{\text{targ}})$, operand 0 is control, operand 1 is target.
4. **Quartet Indexing Strategy**:
   - For distinct operands $q_A, q_B$, let $q_{\text{low}} = \min(q_A, q_B)$ and $q_{\text{high}} = \max(q_A, q_B)$.
   - For $k \in 0 \dots 2^{n-2} - 1$, construct base index $i$ by inserting 0s at bits $q_{\text{low}}$ and $q_{\text{high}}$.
   - Form quartet: $i_{00} = i$, $i_{10} = i \mid (1 \ll q_A)$, $i_{01} = i \mid (1 \ll q_B)$, $i_{11} = i \mid (1 \ll q_A) \mid (1 \ll q_B)$.
   - Guaranteed to cover all $2^n$ basis amplitudes in $2^{n-2}$ disjoint quartets.
5. **No Dense Operator in Production**: Only tiny tests may construct full-system matrices for comparative validation.

## 3. Ordered Implementation Steps
1. **Error Infrastructure**: Add `IdenticalQubitOperands { qubit: usize }` to [`SimulatorError`](src/simulator/error.rs).
2. **Quartet & 4x4 Kernels (`src/simulator/kernels/two_qubit.rs`)**:
   - Define `AmplitudeQuartet` struct.
   - Implement `quartet_indices(num_qubits, q_a, q_b)`.
   - Implement `apply_4x4_kernel(amplitudes, num_qubits, op0, op1, matrix: &Matrix4x4)`.
   - Implement `apply_swap(amplitudes, num_qubits, q0, q1)`.
   - Implement `apply_two_qubit_gate_kernel(amplitudes, num_qubits, op0, op1, gate: &Gate)`.
3. **Controlled Gate Kernels (`src/simulator/kernels/controlled.rs`)**:
   - Implement `apply_controlled_2x2_kernel(amplitudes, num_qubits, control, target, matrix: &Matrix2x2)`.
   - Implement `apply_cx(amplitudes, num_qubits, control, target)`.
   - Implement `apply_cz(amplitudes, num_qubits, q0, q1)`.
4. **Expose Kernels & Facade**:
   - Update `src/simulator/kernels/mod.rs` and `src/simulator/mod.rs`.
   - Add two-qubit methods to `StateVector` in `src/simulator/statevector.rs` (`apply_matrix4x4`, `apply_cx`, `apply_cz`, `apply_swap`, `apply_gate_2q`).
5. **Integration & Property Testing (`tests/simulator_two_qubit.rs`)**:
   - CX truth table in both orientations ($CX(q_0, q_1)$ vs $CX(q_1, q_0)$).
   - CZ phase flips and superposition.
   - SWAP state exchange.
   - Bell state preparation: $H(q_0)$ then $CX(q_0, q_1)$ yielding $(|00\rangle + |11\rangle)/\sqrt{2}$.
   - Arbitrary and non-adjacent qubit positions in multi-qubit registers.
   - Proptest for norm preservation across random normalized states for $CX$, $CZ$, and $SWAP$.
   - Independent dense full-system matrix reference comparison in tiny tests ($n=2, 3, 4$).
   - Error handling for identical and out-of-bounds operands.
6. **Verification Suite**:
   - Run `cargo fmt --check`.
   - Run `cargo clippy --all-targets -- -D warnings`.
   - Run `cargo test`.
7. **Reporting & Progress Update**:
   - Generate `docs/project-state/reports/phase-004.md`.
   - Update `docs/project-state/progress.md`.
   - Create Git commit checkpoint `feat(phase-4): implement two-qubit statevector kernels`.
