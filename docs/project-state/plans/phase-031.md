# Implementation Plan — Phase 31: Single-Qubit Euler Decomposition

## 1. Phase Metadata
- **Phase**: Phase 31
- **Title**: Single-qubit Euler decomposition
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`)
- **Effort Rating**: **XL**
- **Prerequisites**: Phase 10 (Equivalence checking) and Phase 30 (Same-axis rotation merging) — verified and complete.

---

## 2. Objective
Decompose arbitrary $2 \times 2$ unitary matrices into a canonical Euler rotation form while cleanly extracting and preserving the explicit scalar global phase, and reconstructing the exact unitary matrix within strict documented numerical tolerances.

---

## 3. Mathematical Foundations & Conventions
- **Matrix Class**: Arbitrary $U \in U(2)$ where $U^\dagger U = I$ within tolerance $\epsilon_{\text{matrix}} = 10^{-10}$.
- **Determinant & Global Phase Factorization**:
  $$\det(U) = e^{i\Phi}, \quad \Phi = \text{Arg}(\det(U)) \in (-\pi, \pi]$$
  Extract scalar global phase $\alpha = \Phi / 2$.
  Normalize to $SU(2)$: $V = e^{-i\alpha} U$, so $\det(V) = 1$.
- **Selected Canonical Euler Basis**: $Z\text{-}Y\text{-}Z$ (with optional support for $Z\text{-}X\text{-}Z$):
  $$V = R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$$
  $$U = e^{i\alpha} \cdot (R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda))$$
- **Multiplication Order & Column-Vector Convention**:
  - In QCC, statevectors are column vectors and operations act by left multiplication.
  - If a circuit applies gate 1 first, gate 2 second, gate 3 third, the net operator is $U_3 \cdot U_2 \cdot U_1$.
  - Therefore, the Euler factorization $V = R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$ corresponds to:
    1. First apply $R_z(\lambda)$
    2. Second apply $R_y(\theta)$
    3. Third apply $R_z(\phi)$
- **Angle Domains & Degeneracies**:
  - $\theta \in [0, \pi]$: $\theta = 2 \cdot \text{atan2}(|v_{10}|, |v_{00}|)$, clamped to $[0, \pi]$.
  - If $|v_{10}| \le \epsilon_{\text{gimbal}}$ (polar singularity $\theta = 0$):
    - $\theta = 0$, $\lambda = 0$, $\phi = 2 \cdot \text{Arg}(v_{11}) = -2 \cdot \text{Arg}(v_{00})$.
  - If $|v_{00}| \le \epsilon_{\text{gimbal}}$ (polar singularity $\theta = \pi$):
    - $\theta = \pi$, $\lambda = 0$, $\phi = 2 \cdot \text{Arg}(v_{10})$.
  - Non-singular case:
    - $S = -\text{Arg}(v_{00}) = \text{Arg}(v_{11}) = (\phi + \lambda) / 2$.
    - $D = \text{Arg}(v_{10}) = (\phi - \lambda) / 2$.
    - $\phi = S + D$, $\lambda = S - D$.
- **Double-Cover Rotation Phase Tracking**:
  - Normalizing angles modulo $2\pi$ into $[0, 2\pi)$ flips the sign of $R_z$ by $(-1)$ for each $2\pi$ shift because $R(\theta + 2\pi) = -R(\theta) = e^{i\pi} R(\theta)$.
  - Any odd number of $2\pi$ shifts across $\phi$ and $\lambda$ adds $+\pi$ to the extracted global phase $\alpha$, ensuring the reconstructed matrix $e^{i\alpha} V$ remains identical to the original matrix $U$ within machine precision.
- **Tolerances**:
  - Matrix unitarity and reconstruction tolerance: $\epsilon_{\text{matrix}} = 1.0 \times 10^{-10}$.
  - Degeneracy/gimbal tolerance: $\epsilon_{\text{gimbal}} = 1.0 \times 10^{-12}$.

---

## 4. Key Deliverables
1. **Matrix2x2 Extensions** (`src/quantum/gate_matrix.rs`):
   - `det(&self) -> Complex64`: Exact determinant of $2 \times 2$ matrix.
   - `scale(&self, factor: Complex64) -> Self`: Scalar multiplication.
   - `euler_decompose(&self, basis: EulerBasis)` convenience method.
2. **Euler Decomposition Module** (`src/decomposition/mod.rs` & `src/decomposition/euler.rs`):
   - `EulerBasis`: Enum `Zyz` (default) and `Zxz`.
   - `EulerDecomposition`: Struct containing `basis`, `phi: Angle`, `theta: Angle`, `lambda: Angle`, `global_phase: GlobalPhase`.
   - `decompose_euler(matrix: &Matrix2x2, basis: EulerBasis, tolerance: f64) -> Result<EulerDecomposition, DecompositionError>`.
   - `decompose_euler_zyz(matrix: &Matrix2x2)` and `decompose_euler_zxz(matrix: &Matrix2x2)`.
   - Methods: `reconstruct() -> Matrix2x2`, `to_su2_matrix() -> Matrix2x2`, `angles()`, `angles_rad()`, `to_operations(qubit: QubitId) -> Vec<Operation>`.
   - `DecompositionError`: Struct with `NonUnitaryMatrix`, `NonFiniteElement`.
3. **Crate Re-Exports** (`src/lib.rs` & `src/decomposition/mod.rs`):
   - Re-export `EulerBasis`, `EulerDecomposition`, `DecompositionError`, `decompose_euler`, `decompose_euler_zyz`, `decompose_euler_zxz`.
4. **Integration Test Suite** (`tests/euler.rs`):
   - Canonical single-qubit gates: $I, X, Y, Z, H, S, S^\dagger, T, T^\dagger$.
   - Arbitrary continuous single-qubit rotations: $R_x(\theta), R_y(\theta), R_z(\theta)$.
   - Singular and degenerate configurations: $\theta = 0$, $\theta = \pi$, near-zero elements.
   - Polar boundaries and negative angles.
   - Random Haar-distributed unitary matrices generated via QR/Gram-Schmidt of Gaussian matrices.
   - Parameter-space coverage via `proptest`.
   - Global phase extraction tests: matrices with pure global phase shifts $e^{i\alpha} U$.
   - Phase-aware unitary equivalence tests via Phase 10 `check_equivalence`.
   - Non-unitary matrix rejection tests.
5. **Phase Documentation**:
   - `docs/project-state/plans/phase-031.md`: Implementation plan.
   - `docs/project-state/reports/phase-031.md`: Completion report with test evidence.
   - `docs/project-state/progress.md`: Ledger update.

---

## 5. Scope Boundaries
- Do not begin Phase 32 (One-qubit gate fusion).
- Do not implement multi-qubit synthesis or KAK decomposition.
- Do not discard or absorb global phase into unobservable factors.
- Preserve all existing Phase 1–30 tests without modification.
- Stable Rust only.

---

## 6. Verification Protocol
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell` and `cargo run --example qasm`
7. Independent OpenQASM 3 parser validation.
