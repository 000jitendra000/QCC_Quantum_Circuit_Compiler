# Implementation Plan — Phase 29: Inverse-Gate Cancellation

## 1. Phase Metadata
- **Phase**: Phase 29
- **Title**: Inverse-gate cancellation
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`)
- **Effort Rating**: **M**
- **Prerequisites**: Phase 28 (completed and verified).

---

## 2. Objective
Cancel adjacent inverse operations on the same dependency wires in QccIR circuits, while strictly preserving circuit semantics, DAG invariants, explicit circuit global phase, operation provenance, and stable semantic IDs.

---

## 3. Scope Boundaries & Invariants
- **Stable Rust Only**: Zero nightly flags; strict compiler and clippy compliance.
- **Strict Single-Phase Bounding**: Implement Phase 29 only; do not start Phase 30 (rotation merging).
- **Exact Unitary Inverses**:
  - Discrete self-inverse gates ($U^2 = I$): single-qubit `H`, `X`, `Y`, `Z`; two-qubit `CX`, `CZ`, `SWAP`.
  - Discrete adjoint pairs ($U \cdot U^\dagger = I$): `S`-`Sdg`, `Sdg`-`S`, `T`-`Tdg`, `Tdg`-`T`.
  - Parameterized continuous rotation pairs: $R_k(\theta) \cdot R_k(-\theta) = I$ along axis $k \in \{x, y, z\}$. Angles must satisfy $(\theta_1 + \theta_2) \equiv 0 \pmod{4\pi}$ (matrix equality $R(0) = I$).
  - Never merge general rotations $\theta_1 + \theta_2 \neq 0 \pmod{4\pi}$ (Phase 30 scope).
- **Exact Ordered Operands & Multi-Wire Dependency Safety**:
  - Enforce exact ordered operand equality for every gate: candidate pairs are recognized only when `partner.qubits() == node.qubits()`.
  - Directional gates must match operands exactly: `CX(c, t)` and `CX(c, t)` cancel, but `CX(a, b)` and `CX(b, a)` **NEVER** cancel ($CX(a, b) \cdot CX(b, a) \neq I$).
  - Symmetric gates (`CZ`, `SWAP`) with reversed operands (e.g. `CZ(0, 1)` and `CZ(1, 0)`) do not form candidate pairs.
  - Multi-qubit gates must be wire-adjacent along **all** shared wires simultaneously with no intervening operations on any shared wire.
- **Intervening Dependency Boundaries**:
  - **Never cancel across** `Barrier`, `Measure`, `Reset`, or `Classical` operations.
  - An intervening operation on even one shared wire blocks cancellation of multi-qubit pairs.
  - Operations on disjoint wires commute in the DAG and do not block wire-adjacency on independent wires.
- **Transactional Rewriting & Invariants**:
  - Removals staged atomically through `CircuitTransaction::remove_operation`.
  - Dependency wires reconnected automatically and DAG acyclicity verified upon transaction commit.
  - Full rollback on failure with zero modification to target circuit.
  - Existing unremoved nodes retain their stable `OperationId`s and provenance.
  - Unchanged circuit-level global phase ($U^\dagger \cdot U = I$ carries zero global phase change).
- **Deterministic Statistics & Telemetry**:
  - Record candidate pairs examined, inverse pairs cancelled, candidate pairs rejected, iteration count, and categorization (single-qubit, two-qubit, rotation).

---

## 4. Key Deliverables
1. **Pass Implementation**:
   - `src/optimizer/passes/cancellation.rs`:
     - `InverseCancellationPass`: Implements `CompilerPass` trait.
     - `InverseCancellationConfig`: Tolerance, maximum iterations, symbol environment.
     - `InverseCancellationStats`: Examined, cancelled, rejected, single/two-qubit/rotation counts, iterations.
   - `src/optimizer/algebra/inverses.rs`:
     - Enhance `are_inverses_with_config` supporting configurable tolerance and evaluation environments.
   - `src/optimizer/passes/mod.rs` & `src/lib.rs`: Expose new pass types and configuration.
2. **Automated Integration Tests**:
   - `tests/optimizer/cancellation.rs`:
     - Single-qubit self-inverse cancellations (`H-H`, `X-X`, `Y-Y`, `Z-Z`).
     - Discrete inverse pair cancellations (`S-Sdg`, `Sdg-S`, `T-Tdg`, `Tdg-T`).
     - Two-qubit self-inverse cancellations (`CX-CX`, `CZ-CZ`, `SWAP-SWAP`).
     - Rejection of reversed operands for symmetric gates (`CZ(0,1)`-`CZ(1,0)`, `SWAP(0,1)`-`SWAP(1,0)`).
     - Non-cancelling reversed directional gates (`CX(0,1)`-`CX(1,0)`).
     - Continuous rotation inverses ($R_x, R_y, R_z$) modulo $4\pi$.
     - Rejection of $R_x(\pi) \cdot R_x(\pi)$ (which equals $-I$, not $I$).
     - Symbolic inverse pair cancellations and rejection of distinct symbols.
     - Cascaded cancellations ($X \to H \to H \to X \implies \emptyset$).
     - Odd-chain reductions ($H \to H \to H \implies H$).
     - Independence of operations on disjoint wires ($H(0) \to X(1) \to H(0) \implies X(1)$).
     - Cancellation blocking by `Barrier`, `Measure`, `Reset`.
     - Multi-qubit partial blocking by intervening operations on either wire.
     - Unitary equivalence verification against Phase 10 checker.
     - Randomized property testing with 50 generated circuits with inserted inverse pairs.
     - Compiler pass manager execution and statistics verification.
   - `tests/optimizer.rs`: Register cancellation integration test module.

---

## 5. Verification Plan
1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all`
4. `cargo doc --no-deps`
5. `cargo run --example bell` and `cargo run --example qasm`

