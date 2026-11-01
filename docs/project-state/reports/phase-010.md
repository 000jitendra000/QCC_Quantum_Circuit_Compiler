# Phase Completion Report: Phase 10 — Global-Phase-Aware Unitary Equivalence Checker

## 1. Summary
- **Phase Number**: Phase 10
- **Exact Phase Title**: Global-phase-aware unitary equivalence checker
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Assistant (Gemini 3.8 Flash)

---

## 2. Deliverables & Files Changed

### New Files Created:
- `src/verification/mod.rs`: Verification subsystem root re-exporting verification types, estimator functions, and tolerances.
- `src/verification/tolerance.rs`: `VerificationTolerances` specifying distinct tolerances for statevector amplitude ($1.0 \times 10^{-10}$), matrix reconstruction ($1.0 \times 10^{-10}$), phase coherence ($1.0 \times 10^{-9}$), amplitude zero-threshold ($1.0 \times 10^{-8}$), and input state normalization ($1.0 \times 10^{-8}$).
- `src/verification/global_phase.rs`: `GlobalPhaseEstimator` implementing Hilbert-Schmidt collective overlap aggregation, unit-modulus phase factor extraction ($e^{i\theta} = S / |S|$), and branch-cut handling ($[-\pi, \pi]$).
- `src/verification/unitary.rs`: `EquivalenceChecker`, `EquivalenceVerdict`, `VerificationMethod`, `MismatchDiagnostic`, and `VerificationError`. Implements exhaustive computational-basis matrix reconstruction ($n \le \text{max\_matrix\_qubits}$) and randomized arbitrary-state testing.
- `src/testkit/mod.rs`: Testkit subsystem root re-exporting random generator functions.
- `src/testkit/random_circuit.rs`: `generate_random_statevector` (Haar-random normalized complex states via Box-Muller on `SplitMix64`) and `generate_random_unitary_circuit`.
- `tests/equivalence.rs`: 23 comprehensive integration and property tests covering known equivalent circuits, gate decompositions, relative-phase bugs, measurement-probability counterexamples, numerical edge cases, and non-unitary operation rejection.
- `docs/project-state/plans/phase-010.md`: Implementation plan.
- `docs/project-state/reports/phase-010.md`: This completion report.

### Modified Files:
- `src/simulator/executor.rs`: Added `execute_with_initial_state` allowing DAG evolution starting from arbitrary normalized input statevectors.
- `src/simulator/mod.rs`: Re-exported `execute_circuit_with_initial_state`.
- `src/errors.rs`: Added `QccError::Verification(#[from] VerificationError)`.
- `src/lib.rs`: Exposed `pub mod testkit;` and `pub mod verification;` and re-exported top-level verification types.
- `docs/project-state/progress.md`: Updated progress ledger reflecting Phase 10 completion.

---

## 3. Exit Gate Verification Evidence

- **Exit Gate Statement**: `Known equivalent and deliberately inequivalent circuits are reliably distinguished.`
- **Verification Commands Executed**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  cargo build
  cargo doc --no-deps
  cargo run --example bell
  cargo run --example qasm
  ```
- **Execution Log / Output**:
  - `cargo fmt --check`: Clean exit (code `0`).
  - `cargo clippy --all-targets --all-features -- -D warnings`: Clean exit (code `0`, zero warnings across all targets and features).
  - `cargo test`: Clean exit (code `0`). **233 tests passed, 0 failed, 0 ignored**:
    - 60 unit tests in `src/lib.rs`.
    - 23 integration tests in `tests/equivalence.rs`.
    - 21 integration tests in `tests/openqasm_emitter.rs`.
    - 15 integration tests in `tests/simulator_dag.rs`.
    - 10 integration tests in `tests/builder.rs`.
    - 19 integration tests in `tests/ir.rs`.
    - 11 integration tests in `tests/measurement.rs`.
    - 9 integration/statistical tests in `tests/sampling.rs`.
    - 18 integration/property tests in `tests/simulator_two_qubit.rs`.
    - 19 integration/property tests in `tests/simulator_single_qubit.rs`.
    - 23 formal semantics tests in `tests/quantum_semantics.rs`.
    - 5 CLI/diagnostic smoke tests in `tests/smoke.rs`.
  - `cargo build`: Clean exit (code `0`).
  - `cargo doc --no-deps`: Clean exit (code `0`, zero warnings).
  - `cargo run --example bell`: Clean exit (code `0`).
  - `cargo run --example qasm`: Clean exit (code `0`).
- **Exit Gate Pass Confirmation**: **PASSED**.

---

## 4. Mathematical Equivalence Condition & Common Global Phase Policy

1. **Equivalence Formulation**:
   Two $n$-qubit circuits $A$ and $B$ are unitarily equivalent if and only if there exists a **single common global phase** $\theta \in (-\pi, \pi]$ such that for all normalized input statevectors $|\psi\rangle$:
   $$B |\psi\rangle \approx e^{i\theta} A |\psi\rangle$$
2. **Strict Phase Invariance**:
   The exact same scalar phase factor $e^{i\theta}$ is enforced across all test inputs and all amplitude components. Selecting an independent phase per test state or per basis column is strictly forbidden.
3. **Measurement Distributions Insufficient**:
   Probability equality $\sum_k |\alpha_k|^2 = \sum_k |\beta_k|^2$ does not establish unitary equivalence. For example, $H$ and $H Z$ produce identical measurement outcome probabilities ($0.5$ on $|0\rangle$ and $0.5$ on $|1\rangle$), but represent orthogonal relative phases; the verifier detects this and flags the circuits as inequivalent.

---

## 5. Phase Estimation & Zero-Amplitude Handling

1. **Hilbert-Schmidt Overlap Aggregation**:
   The candidate phase is derived collectively across all $m$ test states:
   $$S = \sum_{k=1}^m \langle a^{(k)}, b^{(k)} \rangle = \sum_{k=1}^m \sum_{j=0}^{2^n - 1} (a_j^{(k)})^* b_j^{(k)}$$
   For computational basis reconstruction, $S = \operatorname{Tr}(U_A^\dagger U_B)$.
2. **Zero-Amplitude Safety**:
   - The estimator never divides by individual amplitudes that may be zero or near zero.
   - If mean overlap $\frac{|S|}{m} < \epsilon_{\text{threshold}}$, the states are nearly orthogonal, immediately returning a mismatch.
   - The unit-modulus phase factor is computed as $P = e^{i\theta} = \frac{S}{|S|}$, with $\theta = \operatorname{atan2}(\operatorname{Im}(S), \operatorname{Re}(S))$.
   - Local phase coherence $\Delta\theta = |\operatorname{angle\_diff}(\operatorname{arg}(b_j/a_j), \theta)|$ is evaluated strictly on amplitude pairs where $|a_j| > \epsilon_{\text{threshold}}$ and $|b_j| > \epsilon_{\text{threshold}}$.
3. **Mandatory Component-Wise Verification**:
   Even if aggregate overlap $|S|/m$ is near unity ($0.99999$), the component-wise condition $|b_j^{(k)} - e^{i\theta} a_j^{(k)}| \le \epsilon_{\text{state}}$ is checked for every individual amplitude. Localized phase or amplitude bugs cannot be hidden by high aggregate overlap.

---

## 6. Numerical Tolerance Hierarchy

Configured explicitly in `VerificationTolerances`:

| Domain | Parameter | Default Value | Purpose |
|---|:---:|:---:|---|
| **Statevector Amplitudes** | `state_tolerance` | $1.0 \times 10^{-10}$ | Maximum allowable pointwise amplitude error $|b_j - e^{i\theta} a_j|$. |
| **Matrix Reconstruction** | `matrix_tolerance` | $1.0 \times 10^{-10}$ | Maximum allowable element error across reconstructed unitary matrices. |
| **Global Phase Coherence** | `phase_tolerance` | $1.0 \times 10^{-9}$ | Maximum allowable angular phase deviation across non-zero amplitudes. |
| **Zero Threshold** | `amplitude_threshold` | $1.0 \times 10^{-8}$ | Cutoff below which amplitudes are treated as near-zero during phase estimation. |
| **State Normalization** | `norm_tolerance` | $1.0 \times 10^{-8}$ | Maximum allowable deviation of input state norm squared from $1.0$. |

---

## 7. Non-Unitary Rejection & Barrier Semantics

1. **Strict Non-Unitary Rejection**:
   Unitary verification applies strictly to unitary operations. The checker inspects all operations in the circuit graph:
   - `Operation::Measure`: Rejected with `VerificationError::NonUnitaryOperation`.
   - `Operation::Reset`: Rejected with `VerificationError::NonUnitaryOperation`.
   - `Operation::Classical`: Rejected with `VerificationError::NonUnitaryOperation`.
2. **Barrier Semantics**:
   `Operation::Barrier` is an ordering/scheduling constraint with unitary action $I$ ($I^\dagger I = I$). It is explicitly documented and safely treated as a semantic identity no-op during statevector evolution.

---

## 8. Architectural Invariants Verified

- [x] Stable Rust only (zero nightly flags).
- [x] One single common global phase $\theta$ enforced across all basis states and amplitudes.
- [x] In-place $\mathcal{O}(2^n)$ simulator kernels reused (zero dense $2^n \times 2^n$ matrix allocations in production code).
- [x] Full matrix reconstruction bounded to $n \le 5$ qubits to prevent exponential memory consumption.
- [x] Broader systems ($n > 5$) tested using Haar-random normalized complex states.
- [x] Relative-phase bugs and measurement-probability counterexamples strictly detected.
- [x] Non-unitary operations explicitly rejected with structured errors.
- [x] Zero warnings across `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Scope strictly bounded to Phase 10 (no optimizer passes, no gate fusion, no layout routing).

---

## 9. Approval & Ledger Update
- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] Next Phase: Phase 11 — Bell vertical-slice acceptance.
