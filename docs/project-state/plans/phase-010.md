# Phase 10 Implementation Plan: Global-Phase-Aware Unitary Equivalence Checker

## Objective
Build the formal unitary equivalence verification oracle for QCC. The checker certifies that candidate circuits preserve unitary action up to exactly **one single common global phase** $\phi \in [0, 2\pi)$ across all amplitudes and test states:
$$B|\psi\rangle \approx e^{i\theta} A|\psi\rangle$$
This oracle serves as the foundational correctness gate for all subsequent logical optimization passes (Milestone M3) and synthesis/routing passes.

---

## 1. Scope & Modules

### Files to Create:
- `src/verification/mod.rs`: Verification module root exporting tolerance, phase estimation, and unitary equivalence checker.
- `src/verification/tolerance.rs`: `VerificationTolerances` specifying distinct tolerances for statevector amplitude, matrix reconstruction, phase coherence, zero-amplitude thresholds, and input state normalization.
- `src/verification/global_phase.rs`: Robust common global-phase estimator using collective Hilbert-Schmidt aggregate overlap evidence, unit-modulus phase extraction, and zero-amplitude handling.
- `src/verification/unitary.rs`: `EquivalenceChecker`, `EquivalenceVerdict`, `VerificationMethod`, `MismatchDiagnostic`, and `VerificationError`. Implements computational-basis matrix comparison ($n \le \text{max\_matrix\_qubits}$) and randomized arbitrary-state testing.
- `src/testkit/mod.rs`: Testkit module root exporting random statevector and random circuit generators.
- `src/testkit/random_circuit.rs`: Haar-random normalized complex state generator (via Box-Muller transform on `SplitMix64`) and random unitary circuit builder.
- `tests/equivalence.rs` and `tests/equivalence/`: Comprehensive test suite covering known equivalent circuits, relative-phase bugs, inequivalent circuits, numerical boundary edge cases, and strict non-unitary operation rejection.
- `docs/project-state/reports/phase-010.md`: Phase completion report.

### Files to Modify:
- `src/simulator/executor.rs`: Add `execute_with_initial_state` allowing DAG evolution on arbitrary normalized input statevectors.
- `src/errors.rs`: Add `QccError::Verification(#[from] VerificationError)`.
- `src/lib.rs`: Expose `verification` and `testkit` modules and top-level verification types.
- `docs/project-state/progress.md`: Update progress ledger upon verified completion.

---

## 2. Mathematical Contracts & Invariants

1. **One Single Common Global Phase**:
   - $B|\psi_k\rangle = e^{i\theta} A|\psi_k\rangle$ must hold with the **exact same scalar $\theta$** across all $k$ test states and all amplitude components.
   - Independent phase selection per test state or basis column is strictly prohibited.
2. **Component-Wise Amplitude Verification**:
   - Aggregate overlap $|S|/m$ is used only to derive $\theta$.
   - Component-wise check $|b_{k,j} - e^{i\theta} a_{k,j}| \le \epsilon_{\text{state}}$ is mandatory for every amplitude; aggregate overlap alone can never certify equivalence.
3. **Robust Zero-Amplitude Handling**:
   - Amplitudes with magnitude below $\epsilon_{\text{threshold}}$ are not divided directly.
   - Phase angle deviations $|\text{angle\_diff}(\theta_{\text{local}}, \theta)| \le \epsilon_{\text{phase}}$ are checked only for amplitudes with significant magnitude.
4. **Strict Non-Unitary Rejection**:
   - Any circuit containing measurement (`Operation::Measure`), active reset (`Operation::Reset`), or classical computation/feedback (`Operation::Classical`) is rejected with `VerificationError::NonUnitaryOperation`.
   - Barriers (`Operation::Barrier`) act as scheduling constraints with unitary operator $I$ and are safely treated as semantic identity no-ops on statevectors.
5. **No Measurement Distribution Equivalence**:
   - Probability equality $P(|k\rangle) = |\alpha_k|^2$ is rejected as an equivalence criterion; statevector amplitudes must match up to $e^{i\theta}$.
6. **Bounded Memory in Tiny Matrix Reconstruction**:
   - Full column-by-column matrix reconstruction is bounded to $n \le 5$ qubits ($2^5 = 32$ columns) to prevent exponential memory consumption.

---

## 3. Step-by-Step Implementation Sequence

1. Add `execute_with_initial_state` to `src/simulator/executor.rs`.
2. Implement `src/verification/tolerance.rs`.
3. Implement `src/verification/global_phase.rs`.
4. Implement `src/verification/unitary.rs`.
5. Implement `src/verification/mod.rs`.
6. Implement `src/testkit/random_circuit.rs` and `src/testkit/mod.rs`.
7. Update `src/errors.rs` and `src/lib.rs`.
8. Create `tests/equivalence.rs` and tests under `tests/equivalence/`.
9. Run all verification checks (`fmt`, `clippy`, `test`, `build`, `doc`, `examples`).
10. Draft completion report in `docs/project-state/reports/phase-010.md`.
11. Update progress ledger in `docs/project-state/progress.md`.
12. Commit changes and present results.
