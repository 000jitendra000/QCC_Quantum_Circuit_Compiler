# Phase Completion Report: Phase 4 — Two-Qubit and Controlled-Gate Kernels

## 1. Summary
- **Phase Number**: Phase 4
- **Exact Phase Title**: Two-qubit and controlled-gate kernels
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Assistant (Gemini 3.8 Flash)

## 2. Deliverables & Files Changed
- **New Files Created**:
  - `src/simulator/kernels/two_qubit.rs`:
    - `AmplitudeQuartet` struct: local 4-amplitude representation $(i_{00}, i_{10}, i_{01}, i_{11})$.
    - `QuartetIterator`: zero-allocation exact-size iterator covering all $2^{n-2}$ quartets with bit manipulation.
    - `quartet_indices(num_qubits, op0, op1)`: validation and iterator factory.
    - `apply_4x4_kernel`: generic in-place $4 \times 4$ unitary matrix application in $\mathcal{O}(2^n)$ time.
    - `apply_swap`: in-place qubit state exchange.
    - `apply_two_qubit_gate_kernel`: matrix dispatch for two-qubit gates.
  - `src/simulator/kernels/controlled.rs`:
    - `apply_controlled_2x2_kernel`: controlled single-qubit unitary matrix kernel.
    - `apply_cx`: in-place Controlled-NOT kernel with explicit control-target order.
    - `apply_cz`: in-place Controlled-Z phase flip kernel.
  - `tests/simulator_two_qubit.rs`: 18 comprehensive tests covering truth tables, control/target discrimination, non-adjacent qubits, Bell-state preparation, randomized norm preservation, and comparison with an independent dense full-system reference operator.
  - `docs/project-state/plans/phase-004.md`: Phase 4 implementation plan.
  - `docs/project-state/reports/phase-004.md`: This completion report.
- **Modified Files**:
  - `src/simulator/error.rs`: Added `SimulatorError::IdenticalQubitOperands { qubit: usize }`.
  - `src/simulator/kernels/mod.rs`: Re-exported two-qubit and controlled modules and functions.
  - `src/simulator/mod.rs`: Re-exported `two_qubit` and `controlled` submodules.
  - `src/simulator/statevector.rs`: Added two-qubit methods to `StateVector`:
    - `apply_matrix4x4`
    - `apply_gate_2q`
    - `apply_cx`
    - `apply_cz`
    - `apply_swap`
    - `apply_controlled_2x2`
  - `docs/project-state/progress.md`: Updated progress ledger reflecting Phase 4 completion.

## 3. Exit Gate Verification Evidence
- **Exit Gate Statement**: `Two-qubit kernels pass basis and randomized superposition tests.`
- **Verification Commands Executed**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo doc --no-deps
  cargo run -- --version
  ```
- **Execution Log / Output**:
  - `cargo fmt --check`: Clean exit (code `0`).
  - `cargo clippy --all-targets -- -D warnings`: Clean exit (code `0`, zero warnings across all targets).
  - `cargo test`: Clean exit (code `0`). **114 tests passed, 0 failed, 0 ignored**:
    - 49 unit tests in `src/lib.rs` (diagnostics, errors, gate matrices, parameter algebra, semantics, and single/two-qubit simulator kernels).
    - 18 integration and property tests in `tests/simulator_two_qubit.rs` (exhaustive quartet partitioning, CX truth table in both orientations, CZ, SWAP, Bell state, non-adjacent operands, proptest norm preservation, and dense full-system reference comparison).
    - 19 integration and property tests in `tests/simulator_single_qubit.rs`.
    - 23 tests in `tests/quantum_semantics.rs`.
    - 5 tests in `tests/smoke.rs`.
  - `cargo doc --no-deps`: Clean exit (code `0`, zero warnings).
  - `cargo run -- --version`: Clean exit (code `0`, output: `qcc 0.1.0`).
- **Exit Gate Pass Confirmation**: **PASSED**.

## 4. Test Suite Summary
- **Quartet Indexing Coverage**:
  - `test_quartet_partition_exactness_exhaustive`: Exhaustively tested all distinct pairs $(q_A, q_B)$ across registers from 2 to 5 qubits. Verified that every basis state belongs to exactly one quartet, with $2^{n-2}$ quartets per pair, and verified correct bit weights ($q_A$ has weight 1, $q_B$ has weight 2).
- **CX Truth Table & Control/Target Discrimination**:
  - `test_cx_truth_table_c0_t1`: Verified $CX(q_0, q_1)$ basis mappings: $|00\rangle \to |00\rangle$, $|01\rangle \to |11\rangle$, $|10\rangle \to |10\rangle$, $|11\rangle \to |01\rangle$.
  - `test_cx_truth_table_c1_t0`: Verified reversed orientation $CX(q_1, q_0)$ basis mappings: $|00\rangle \to |00\rangle$, $|01\rangle \to |01\rangle$, $|10\rangle \to |11\rangle$, $|11\rangle \to |10\rangle$.
  - `test_cx_orientation_discrimination_on_same_state`: Confirmed that applying $CX(q_0, q_1)$ vs $CX(q_1, q_0)$ to input $|01\rangle$ produces completely different orthogonal outputs ($|11\rangle$ vs $|01\rangle$).
- **CZ & SWAP Actions**:
  - `test_cz_truth_table_and_symmetry`: Verified $CZ$ negates only $|11\rangle$ and satisfies $CZ(q_0, q_1) \equiv CZ(q_1, q_0)$.
  - `test_cz_superposition`: Verified phase flip on uniform superposition state.
  - `test_swap_basis_states`: Verified $|01\rangle \leftrightarrow |10\rangle$ exchange with $|00\rangle, |11\rangle$ preserved.
  - `test_swap_non_adjacent_in_multi_qubit_register`: Verified SWAP between $q_1$ and $q_3$ in a 4-qubit register with other qubits untouched.
- **Bell State Synthesis**:
  - `test_bell_state_phi_plus`: Prepared $|\Phi^+\rangle = (|00\rangle + |11\rangle)/\sqrt{2}$ using $H(q_0)$ followed by $CX(q_0, q_1)$. Verified amplitudes $\alpha_0 = \alpha_3 = 1/\sqrt{2}$ and $\alpha_1 = \alpha_2 = 0$.
  - `test_bell_state_in_3_qubit_register_non_adjacent`: Synthesized Bell state between non-adjacent qubits $q_0$ and $q_2$ in a 3-qubit register, leaving $q_1$ in $|0\rangle$.
- **Error Handling**:
  - `test_identical_operands_error`: Verified structured error `SimulatorError::IdenticalQubitOperands` when $q_0 == q_1$.
  - `test_target_out_of_bounds_errors`: Verified `SimulatorError::TargetOutOfBounds`.
  - `test_apply_single_qubit_gate_with_2q_kernel`: Verified `SimulatorError::UnsupportedGate`.
- **Property-Based Testing (Proptest)**:
  - `prop_norm_preservation_cx_random_states`: Verified norm preservation ($\||\psi\rangle\| = 1.0 \pm 10^{-6}$) on random normalized states for $CX$ on arbitrary operand pairs.
  - `prop_norm_preservation_cz_random_states`: Verified norm preservation for $CZ$.
  - `prop_norm_preservation_swap_random_states`: Verified norm preservation for $SWAP$.
- **Dense Full-System Matrix Reference Comparison**:
  - Built test-only reference functions `build_dense_full_system_operator` and `dense_matrix_vector_mult`.
  - `test_dense_reference_comparison_cx_cz_swap`: Compared in-place kernels against the dense $2^n \times 2^n$ matrix across $n \in [2, 4]$, all operand pairs, both orientations, and all gates. Amplitudes matched to $< 10^{-10}$.
  - `prop_dense_reference_equivalence_on_random_states`: Proptest verifying amplitude equivalence against the independent dense operator on random normalized states.

## 5. Architectural Invariants Verified
- [x] Stable Rust only (zero nightly flags).
- [x] Little-endian qubit indexing ($q_0$ is LSB with bit weight 1).
- [x] Column vectors and left multiplication ($B \cdot A$).
- [x] Operand ordering: operand 0 is control, operand 1 is target in $CX$.
- [x] Quartet indexing partitions all $2^n$ amplitudes into $2^{n-2}$ disjoint quartets.
- [x] No dense full-system matrix is allocated or constructed in production simulator code ($\mathcal{O}(2^n)$ runtime, $\mathcal{O}(1)$ space).
- [x] Scope adhered strictly to Phase 4 boundaries (no terminal measurement sampling yet).

## 6. Known Limitations & Technical Debt
- Phase 4 implements two-qubit kernels. Terminal measurement sampling (Born rule probabilities, CDF binary search, and seeded RNG) is scheduled for Phase 5.

## 7. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
