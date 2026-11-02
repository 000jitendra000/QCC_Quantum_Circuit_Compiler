# Implementation Plan — Phase 30: Same-Axis Rotation Merging

## 1. Phase Metadata
- **Phase**: Phase 30
- **Title**: Same-axis rotation merging
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`)
- **Effort Rating**: **M**
- **Prerequisites**: Phase 29 (completed and verified).

---

## 2. Objective
Combine adjacent single-qubit rotations ($R_x$, $R_y$, or $R_z$) acting on the same qubit wire in QccIR dependency DAGs while strictly preserving circuit quantum semantics, including explicit circuit-level global phase.

---

## 3. Scope Boundaries & Invariants
- **Stable Rust Only**: Zero nightly compiler flags; strict clippy compliance without warnings.
- **Strict Single-Phase Bounding**: Implement Phase 30 only; do not start Phase 31 (Euler decomposition) or Phase 32 (gate fusion).
- **Rotation Gate Families**:
  - $R_x(\theta)$, $R_y(\theta)$, $R_z(\theta)$ single-qubit rotation gates.
  - Commutation and merging apply only along the **same rotation axis** on the **same qubit wire**.
  - Rotations along different axes ($[X, Y] \neq 0$, etc.) do not commute and must not be merged.
- **DAG Wire Adjacency & Intervening Dependencies**:
  - Merge candidates must be direct wire successors along `WireId::Quantum(q)`.
  - Never cross intervening gates on the same wire, multi-qubit gates touching the wire, barriers, measurements, resets, or classical dependencies.
  - Operations on disjoint wires commute in the DAG and do not block adjacency on independent wires.
- **Angle Normalization Periodicity & Global Phase**:
  - Single-qubit rotations have an exact $4\pi$ matrix identity periodicity ($R(\theta + 4\pi) = R(\theta)$).
  - Shifting an angle by an odd multiple of $2\pi$ introduces a $-I$ factor ($R(\theta + 2\pi) = -R(\theta) = e^{i\pi} R(\theta)$).
  - Normalization to canonical $[0, 2\pi)$ tracks this distinction by applying a $+\pi$ global-phase adjustment to the circuit.
  - Exact zero identities ($\theta \equiv 0 \pmod{4\pi}$) are eliminated with zero global-phase adjustment.
  - Phase zero identities ($\theta \equiv 2\pi \pmod{4\pi}$) are eliminated with a $+\pi$ global-phase adjustment.
- **Symbolic Parameters**:
  - Unbound symbolic variables are never assumed to be zero or constants.
  - Runtime environment (`symbol_env`) evaluates bound symbols.
  - Unbound algebraic opposites ($p$ and $-p$) safely cancel to zero.
  - Under `PreserveUnresolved`, unbound symbolic parameters remain unchanged as separate gates.
  - Under `CombineSymbolic`, symbolic parameters combine algebraically into `ParameterExpr::Add`.
- **Transactional DAG Rewriting**:
  - Atomic staged mutations via `CircuitTransaction::fuse_operations` and `CircuitTransaction::remove_operation`.
  - Fused operations record original operation IDs in provenance.
  - DAG acyclicity validated on commit; complete rollback on failure.

---

## 4. Key Deliverables
1. **Angle Normalization & Periodicity Module**:
   - `src/quantum/angle.rs`:
     - `normalize_rotation_angle`: Normalizes continuous angles with $4\pi$ exact and $2\pi$ phase tracking.
     - `RotationNormalizedAngle`: Structured output with angle, phase correction, and identity classification.
     - `RotationNormalizationMode`: `TwoPiWithPhase` (canonical) and `FourPiExact`.
   - `src/quantum/mod.rs` & `src/lib.rs`: Expose angle normalization types and functions.
2. **Rotation Merge Compiler Pass**:
   - `src/optimizer/passes/rotation_merge.rs`:
     - `RotationMergePass`: Implements `CompilerPass` trait.
     - `RotationMergeConfig`: Tolerance, maximum iterations, identity elimination flags, normalization mode, symbolic policy, symbol environment.
     - `RotationMergeStats`: Telemetry statistics tracking merged pairs, fused operations, eliminated identities, phase adjustments, iterations.
   - `src/optimizer/passes/mod.rs` & `src/optimizer/mod.rs`: Expose pass types.
3. **Automated Test Suite**:
   - `tests/optimizer/rotation_merge.rs`:
     - Numeric compositions for $R_x$, $R_y$, and $R_z$.
     - Random numeric angles verified with Phase 10 `check_equivalence`.
     - $4\pi$ periodicity (exact zero identity elimination) and $2\pi$ global-phase adjustment (phase zero).
     - Negative angles, boundary periodicity, and non-zero angle normalization.
     - Sequential rotation chains merging across fixed-point iterations.
     - Symbolic parameter expressions: algebraic opposites cancellation, bound symbol environments, `CombineSymbolic`, and `PreserveUnresolved`.
     - Intervening operations on the same wire blocking merge ($H$, $CX$, different axes).
     - Non-unitary boundaries blocking merge (`Barrier`, `Measure`, `Reset`).
     - Operations on unrelated wires commuting in DAG without blocking merge.
     - Provenance tracking, stable operation IDs, and transactional rollback.
     - Telemetry statistics verification.
     - Property-based tests (`proptest`) and randomized circuit equivalence over 30 trials.
   - `tests/optimizer.rs`: Module registration.

---

## 5. Verification Plan
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell` and `cargo run --example qasm`
7. Independent OpenQASM 3 reference parser validation on emitted artifacts.
