# Phase 40 Completion Report — Native Single-Qubit Synthesis

## 1. Executive Summary

- **Phase Number**: Phase 40
- **Phase Title**: Native single-qubit synthesis
- **Milestone**: Milestone M4: Generic Target and Basis Translation
- **Exit Gate**: Random unitary synthesis tests and target legality checks pass (**PASSED**)
- **Baseline Git Checkpoint**: `3d6751c` (`feat(phase-39): add verified fixed decomposition templates`)
- **Status**: Completed and fully verified with 100% test pass across all targets.

Phase 40 implements native single-qubit unitary synthesis for the Quantum Circuit Compiler (QCC). It provides a uniform interface to synthesize arbitrary $2 \times 2$ unitary operators $U \in U(2)$ into sequences of hardware-native single-qubit instructions supported by a generic `Target`.

Key architectural achievements in this phase:
1. **Target-Aware Synthesis Interface (`SingleQubitSynthesis` / `SingleQubitSynthesizer`)**:
   Provides queryable single-qubit synthesis routines parameterized by `Target` and `PhysicalQubitId`. Queries `Target` capabilities to determine supported and preferred native single-qubit instruction families.
2. **Canonical Basis Representations**:
   - Generic $Z\text{-}Y\text{-}Z$ Euler representation ($R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$).
   - $Z\text{-}X\text{-}Z$ Euler representation ($R_z(\beta) \cdot R_x(\gamma) \cdot R_z(\delta)$).
   - Superconducting-native $RZ\text{-}SX$ representation ($R_z(\phi_2) \cdot SX \cdot R_z(\theta') \cdot SX \cdot R_z(\lambda')$) where $SX = \sqrt{X}$.
   - Generic 3-parameter single-qubit unitary representation ($U_3(\theta, \phi, \lambda)$).
3. **Rigorous Global Phase & Spinor Accounting**:
   Preserves circuit global phase explicitly: $U_{\text{target}} = e^{i\alpha} \prod_{k} U_k$. Accounts for $SU(2)$ double-cover spinor sign flips ($e^{i\pi} = -1$) when normalizing rotation angles into canonical $[0, 2\pi)$ ranges.
4. **Deterministic Gimbal-Lock & Singularity Handling**:
   Resolves Euler singularities at $\theta = 0$ and $\theta = \pi$ deterministically by fixing redundant degrees of freedom ($\lambda = 0$).
5. **Hardware Legality Verification**:
   Every emitted target operation is verified against `Target::check_operation_legality` before output sequence creation.

---

## 2. Key Deliverables & Architecture

### 2.1 Target Single-Qubit Basis Model (`src/target/synthesis_basis.rs`)

- **`SingleQubitBasis` Enum**:
  - `SingleQubitBasis::Zyz`: Native gates `rz` and `ry`.
  - `SingleQubitBasis::Zxz`: Native gates `rz` and `rx`.
  - `SingleQubitBasis::RzSx`: Native gates `rz` and `sx` (standard for superconducting transmon architectures).
  - `SingleQubitBasis::U3`: Native generic 3-parameter gate `u3` or `u`.
- **Capability Queries**:
  - `basis.is_supported_by(target, qubit)`: Checks whether all required operations for the basis family are declared and legal for the target on the given physical qubit.
  - `target.supported_single_qubit_bases(qubit)`: Returns all single-qubit bases legal on `qubit`.
  - `target.preferred_single_qubit_basis(qubit)`: Returns the highest-priority native single-qubit basis for `qubit` (ordered `RzSx` $\to$ `Zyz` $\to$ `Zxz` $\to$ `U3`).

### 2.2 Mathematical Decomposition & Identities

#### 1. $Z\text{-}Y\text{-}Z$ Euler Angle Extraction
Any $U \in U(2)$ is factored as $U = e^{i\alpha} V$ where $\det(V) = 1$ ($V \in SU(2)$).
$$V = \begin{pmatrix} v_{00} & v_{01} \\ v_{10} & v_{11} \end{pmatrix} = \begin{pmatrix} \cos\frac{\theta}{2} e^{-i(\phi+\lambda)/2} & -\sin\frac{\theta}{2} e^{-i(\phi-\lambda)/2} \\ \sin\frac{\theta}{2} e^{i(\phi-\lambda)/2} & \cos\frac{\theta}{2} e^{i(\phi+\lambda)/2} \end{pmatrix}$$
- $\theta = 2 \operatorname{atan2}(|v_{10}|, |v_{00}|) \in [0, \pi]$.
- Non-singular case ($0 < \theta < \pi$):
  - $\phi + \lambda = 2 \arg(v_{11})$
  - $\phi - \lambda = 2 \arg(v_{10})$
  - $\phi = \frac{(\phi+\lambda) + (\phi-\lambda)}{2}$, $\lambda = \frac{(\phi+\lambda) - (\phi-\lambda)}{2}$.
- Singularities (Gimbal lock):
  - $\theta = 0$ ($|v_{10}| = 0$): $v_{00} = e^{-i(\phi+\lambda)/2}$. Set $\lambda = 0$, $\phi = -2 \arg(v_{00})$.
  - $\theta = \pi$ ($|v_{00}| = 0$): $v_{10} = e^{i(\phi-\lambda)/2}$. Set $\lambda = 0$, $\phi = 2 \arg(v_{10})$.

#### 2. $RZ\text{-}SX$ Identity
Using $SX = \sqrt{X} = e^{i\pi/4} R_x(\pi/2)$ and $R_y(\theta) = R_z(\pi/2) \cdot R_x(\theta) \cdot R_z(-\pi/2)$, the single-qubit $Y$-rotation decomposes into:
$$R_y(\theta) = e^{i\pi/2} \left( R_z(\pi) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \right)$$
Combining with outer $R_z$ rotations in execution order (left-multiplication $B \cdot A$):
$$U_{\text{target}} = e^{i(\alpha + \pi/2)} \left( R_z(\phi + \pi) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \cdot R_z(\lambda) \right)$$

#### 3. Spinor Periodicity & Angle Normalization
Single-qubit rotation gates $R_P(\theta) = \exp(-i \frac{\theta}{2} P)$ exhibit $4\pi$ periodicity:
$$R_P(\theta + 2\pi) = -R_P(\theta) = e^{i\pi} R_P(\theta)$$
To maintain exact numerical matrix equivalence ($< 10^{-14}$) when normalizing angles into $[0, 2\pi)$, QCC tracks modulo $4\pi$ spinor winding:
- `normalize_angle_with_spinor_shift(rad) -> (canonical_angle, shift_count)`
- If the total shift count $\sum k_i$ across gates is odd, an exact $+e^{i\pi}$ phase factor is added to the global phase $\alpha$.

### 2.3 Synthesis Interface & Engine (`src/synthesis/single_qubit.rs`)

- **`SingleQubitSynthesis` Trait**:
  ```rust
  pub trait SingleQubitSynthesis {
      fn synthesize(
          &self,
          target: &Target,
          qubit: PhysicalQubitId,
          matrix: &Matrix2x2,
      ) -> Result<SynthesizedOneQubitSequence, SynthesisError>;

      fn synthesize_with_basis(
          &self,
          target: &Target,
          qubit: PhysicalQubitId,
          matrix: &Matrix2x2,
          basis: SingleQubitBasis,
      ) -> Result<SynthesizedOneQubitSequence, SynthesisError>;
  }
  ```
- **`SynthesizedOneQubitSequence`**:
  Encapsulates synthesized operations along with:
  - `operations(&self) -> &[Operation]`
  - `physical_qubit(&self) -> PhysicalQubitId`
  - `global_phase(&self) -> f64` (in radians, $[0, 2\pi)$)
  - `basis(&self) -> SingleQubitBasis`
  - `reconstruct_matrix(&self) -> Matrix2x2`
  - `verify_equivalence(&self, original: &Matrix2x2, tolerance: f64) -> Result<(), SynthesisError>`
  - `verify_legality(&self, target: &Target) -> Result<(), SynthesisError>`
  - `into_circuit(&self) -> Circuit` (builds an executable QccIR circuit)
- **`SynthesisError`**:
  - `Decomposition(EulerDecompositionError)`
  - `UnsupportedBasis { basis, qubit }`
  - `NoSupportedBasis { qubit }`
  - `QubitOutOfBounds { qubit, capacity }`
  - `Legality(LegalityError)`
  - `VerificationFailed { max_diff, tolerance }`

### 2.4 Simulator Kernel Additions (`src/simulator/operation_dispatch.rs` & `src/quantum/gate_matrix.rs`)

- Added `matrix_sx() -> Matrix2x2` ($SX = \frac{1}{2} \begin{pmatrix} 1+i & 1-i \\ 1-i & 1+i \end{pmatrix}$).
- Added `matrix_u3(theta, phi, lambda) -> Matrix2x2`.
- Added statevector simulation kernel dispatch for `"sx"`, `"u3"`, and `"u"`, enabling direct statevector simulation and Phase 10 equivalence verification on synthesized circuits.

---

## 3. Verification & Test Evidence

### 3.1 Synthesis Integration Test Suite (`tests/synthesis.rs`, 45 Tests Total, 12 New Single-Qubit Tests)

1. `single_qubit::test_random_unitary_reconstruction_rz_sx`:
   Generates 50 Haar-random $2 \times 2$ unitaries using SeededRng with Gaussian normal random variables (Box-Muller), synthesizes each into $RZ\text{-}SX$ sequences, and verifies matrix reconstruction matches within $10^{-12}$.
2. `single_qubit::test_random_unitary_reconstruction_zyz`:
   Synthesizes 50 Haar-random unitaries into $Z\text{-}Y\text{-}Z$ sequences and verifies matrix reconstruction within $10^{-12}$.
3. `single_qubit::test_random_unitary_reconstruction_zxz`:
   Synthesizes 50 Haar-random unitaries into $Z\text{-}X\text{-}Z$ sequences and verifies matrix reconstruction within $10^{-12}$.
4. `single_qubit::test_random_unitary_reconstruction_u3`:
   Synthesizes 50 Haar-random unitaries into native $U3$ sequences and verifies matrix reconstruction within $10^{-12}$.
5. `single_qubit::test_exact_global_phase_sensitivity`:
   Tests synthetic unitaries with intentional global phase offsets ($e^{i \cdot 0.35} X$, $e^{-i \cdot 1.2} H$). Verifies global phase is captured exactly without affecting relative matrix action.
6. `single_qubit::test_euler_singularities_and_canonical_gates`:
   Tests synthesis on singular canonical gates ($I, X, Y, Z, H, S, T$) across all 4 bases. Verifies smooth singularity handling with zero NaN/divergence.
7. `single_qubit::test_angle_normalization_and_periodicity`:
   Tests unitaries parameterized with out-of-range angles ($\pm 10\pi$). Verifies angles are cleanly mapped to $[0, 2\pi)$ and spinor phase parity correctly adjusts global phase.
8. `single_qubit::test_unsupported_target_rejection`:
   Constructs a target supporting only `rx`, attempts $RZ\text{-}SX$ and $Z\text{-}Y\text{-}Z$ synthesis, and verifies structured `UnsupportedBasis` error.
9. `single_qubit::test_qubit_out_of_bounds_rejection`:
   Attempts synthesis on a qubit index exceeding physical capacity, verifying structured `QubitOutOfBounds` error.
10. `single_qubit::test_non_unitary_matrix_rejection`:
    Attempts synthesis on singular or non-unitary matrices, verifying rejection.
11. `single_qubit::test_preferred_basis_selection`:
    Verifies that a multi-basis target automatically chooses $RZ\text{-}SX$ over generic Euler forms.
12. `single_qubit::test_into_circuit_unitary_equivalence`:
    Converts synthesized sequences directly into executable `Circuit` DAGs, evaluates them with the statevector simulator, and verifies equivalence against the analytical matrix using Phase 10 equivalence verification.

### 3.2 Target Capability Queries (`tests/target_capabilities.rs` & `src/target/synthesis_basis.rs`)

- Tests `SingleQubitBasis::is_supported_by` across synthetic targets (`SyntheticCxTarget`, `SyntheticMixedTarget`).
- Validates that target capabilities and basis support agree with declared target operations.

---

## 4. Exit Gate Verification Checklist

| Exit Gate Requirement | Status | Evidence |
|:----------------------|:------:|:---------|
| Stable Rust (zero nightly flags) | **PASSED** | Compiled and verified with stable toolchain |
| Generic $Z\text{-}Y\text{-}Z$ representation | **PASSED** | Implemented and verified in `single_qubit::test_random_unitary_reconstruction_zyz` |
| Native $RZ\text{-}SX$ target path | **PASSED** | Implemented and verified in `single_qubit::test_random_unitary_reconstruction_rz_sx` |
| Angle normalization & spinor phase shifts | **PASSED** | Verified in `single_qubit::test_angle_normalization_and_periodicity` |
| Deterministic singularity handling | **PASSED** | Verified in `single_qubit::test_euler_singularities_and_canonical_gates` |
| Global phase returned separately | **PASSED** | $U = e^{i\alpha} \prod U_k$ verified in `test_exact_global_phase_sensitivity` |
| Target legality verified on emitted ops | **PASSED** | `target.check_operation_legality` evaluated in synthesizer |
| Random unitary synthesis tests pass | **PASSED** | 50 Haar-random tests per basis family pass ($< 10^{-12}$) |
| Zero clippy warnings with `-D warnings` | **PASSED** | `cargo clippy --all-targets -- -D warnings` clean |
| Zero formatting discrepancies | **PASSED** | `cargo fmt --check` clean |
| Full regression test pass | **PASSED** | All 1022+ tests pass with zero failures |
