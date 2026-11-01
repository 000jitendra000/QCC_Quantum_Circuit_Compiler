# Phase 14 Implementation Plan: Gate Algebra and Inverse Registry

## Objective
Establish an authoritative, mathematically verified gate algebra and inverse registry for QCC.
This registry provides the foundation for all future semantics-preserving logical optimizations, gate cancellations, rotation mergings, and basis transformations (Milestone M3), distinguishing exact matrix equalities from equalities up to global phase.

---

## 1. Scope & Modules

### Files to Create:
- `src/optimizer/mod.rs`: Optimizer root module exposing `algebra`.
- `src/optimizer/algebra/mod.rs`: Algebra subsystem root, errors (`AlgebraError`), and public registry interface.
- `src/optimizer/algebra/inverses.rs`: Inverse relationships, self-inverse gates, parameterized gate inversions, and operation inversion.
- `src/optimizer/algebra/identities.rs`: Typed symbolic identity registry, rotation composition rules, $2\pi$ and $4\pi$ periodicity with explicit global phase, and matrix verification helpers.
- `tests/gate_identities.rs`: Independent mathematical verification test suite comparing every registered identity and inverse rule against concrete unitary matrices using `VerificationTolerances` and `proptest`.
- `docs/project-state/reports/phase-014.md`: Completion report.

### Files to Modify:
- `src/opt/mod.rs`: Re-export `pub use crate::optimizer::algebra;`.
- `src/errors.rs`: Add `QccError::Algebra(#[from] AlgebraError)`.
- `src/lib.rs`: Expose `pub mod optimizer;` and re-export core algebra types.
- `docs/project-state/progress.md`: Update progress ledger marking Phase 14 complete.

---

## 2. Invariants & Mathematical Contracts

1. **Left-Multiplication Column-Vector Conventions**:
   - Sequential operations act by left multiplication: if operation $A$ is applied before $B$, the combined operator is $B \cdot A$.
2. **Strict Global-Phase Discrimination**:
   - Exact matrix equality ($U_A = U_B$) must never be confused with equality up to global phase ($U_A = e^{i\phi} U_B$).
   - Phase differences introduced by periodicity (e.g. $R_k(\theta + 2\pi) = e^{i\pi} R_k(\theta) = -R_k(\theta)$) must be explicitly recorded and validated.
   - Exact matrix periodicity for rotations is $4\pi$, whereas $2\pi$ periodicity carries a global phase factor of $e^{i\pi} = -1$.
3. **Little-Endian Multi-Qubit Matrix Ordering**:
   - $q_0$ is LSB (inner factor in standard Kronecker product).
   - In $CX$, Operand 0 is control, Operand 1 is target.
4. **Symbolic Exactness**:
   - No silent numerical approximations of symbolic expressions.
   - Preconditions for identities must be formally checked before claiming equality.
5. **No Optimization Application**:
   - Phase 14 creates the tested mathematical registry only. Circuit rewriting using these identities belongs to Milestone M3.

---

## 3. Step-by-Step Implementation Sequence

1. Create `src/optimizer/algebra/mod.rs` with `AlgebraError` and subsystem exports.
2. Create `src/optimizer/algebra/inverses.rs` with `is_self_inverse`, `invert_gate_name`, `invert_gate`, `invert_operation`, and `are_inverses`.
3. Create `src/optimizer/algebra/identities.rs` with `IdentityKind`, `GateIdentity`, `IdentityRegistry`, parameter normalization, and rotation composition rules.
4. Wire into `src/optimizer/mod.rs`, `src/opt/mod.rs`, `src/errors.rs`, and `src/lib.rs`.
5. Implement comprehensive independent matrix verification test suite in `tests/gate_identities.rs`.
6. Run complete verification suite (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`, examples).
7. Write `docs/project-state/reports/phase-014.md` and update `docs/project-state/progress.md`.
8. Commit with `feat(phase-14): implement gate algebra and inverse registry`.
