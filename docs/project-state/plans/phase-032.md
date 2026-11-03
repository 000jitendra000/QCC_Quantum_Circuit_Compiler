# Implementation Plan — Phase 32: One-Qubit Gate Fusion

## 1. Phase Metadata
- **Phase**: Phase 32
- **Title**: One-qubit gate fusion
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`)
- **Effort Rating**: **L**
- **Prerequisites**: Phase 13 (Transactional DAG rewrites), Phase 30 (Same-axis rotation merging), Phase 31 (Single-qubit Euler decomposition) — verified and complete.

---

## 2. Objective
Fuse adjacent one-qubit operations on the same quantum wire in QccIR dependency DAGs into an equivalent canonical operation sequence using local matrix composition and Phase 31 Euler decomposition, while strictly preserving quantum semantics, DAG convexity, and explicit circuit-level global phase.

---

## 3. Mathematical Foundations & Contracts
- **Fusion Window Adjacency Rules**:
  - Identify maximal consecutive single-qubit unitary operations on the same quantum wire $q$ ($k \ge 2$).
  - Strictly follow direct wire edges (`WireId::Quantum(q)`).
  - Never reorder operations or commute operations across other gates to construct fusion windows.
  - Multi-qubit gates ($CX, CZ, SWAP$) touch multiple wires and cannot participate in single-qubit windows on either wire.
  - Side-effect, non-unitary, and classical hazard boundaries (`Barrier`, `Measure`, `Reset`, `Classical`) strictly terminate fusion windows.
  - Operations with unresolved symbolic parameters that cannot evaluate to concrete matrices terminate fusion windows.
- **Matrix Composition Order (Column-Vector Convention)**:
  - In QCC, quantum operations act by left multiplication on column statevectors.
  - For sequential execution of operations $N_1, N_2, \dots, N_k$ on wire $q$, the composite operator is:
    $$M = U_k \cdot U_{k-1} \cdot \dots \cdot U_2 \cdot U_1$$
  - Evaluated incrementally as $M \leftarrow U_i \cdot M$ for each node $N_i$ from $i = 1$ to $k$.
  - Preserving this exact multiplication order is verified by order-sensitive non-commuting tests.
- **Phase-Preserving Resynthesis via Phase 31 Euler Decomposition**:
  - Decompose $M \in U(2)$ into scalar global phase $\alpha$ and canonical special unitary $V \in SU(2)$:
    $$M = e^{i\alpha} V, \quad V = R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$$
  - Add $\alpha$ directly to the circuit's explicit global phase:
    $$\phi_{\text{circuit}} \leftarrow (\phi_{\text{circuit}} + \alpha) \pmod{2\pi}$$
  - Simplify redundant identity rotations from the resynthesized Euler sequence:
    - Omit $R(\theta)$ when $\theta \equiv 0 \pmod{4\pi}$.
    - Omit $R(\theta)$ and add $+\pi$ phase when $\theta \equiv 2\pi \pmod{4\pi}$.
    - An identity block ($M = I$) simplifies to 0 operations.
    - A pure Z-rotation simplifies to 1 operation ($R_z$).
    - A pure Y-rotation simplifies to 1 operation ($R_y$).
    - General non-identity blocks simplify to at most 3 operations ($R_z \to R_y \to R_z$).
- **Fixed-Point Convergence & Cycle Prevention**:
  - If a sequence of operations is already in canonical Euler form with identical parameters, resynthesis is a no-op and skipped, ensuring deterministic pass termination.
- **Transactional Rewriting (Phase 13)**:
  - Atomic $M$-to-$N$ replacement via `CircuitTransaction::replace_sequence`.
  - DAG convexity is validated before replacement.
  - Parent provenance of all original operations in the window is attached to each resynthesized operation.
  - In case of rewrite failure, transaction rollback preserves the original circuit completely unmodified.

---

## 4. Key Deliverables
1. **DAG Rewrite Sequence Helper** (`src/ir/rewrite/replace.rs` & `src/ir/rewrite/mod.rs`):
   - `stage_replace_sequence()` and `CircuitTransaction::replace_sequence()` for clean $M$-to-$N$ atomic replacements with convexity checking and provenance tracking.
2. **One-Qubit Gate Fusion Pass** (`src/optimizer/passes/one_qubit_fusion.rs`):
   - `OneQubitFusionPass`: Implements `CompilerPass` trait.
   - `OneQubitFusionConfig`: Configuration options (tolerance, max iterations, Euler basis, min/max window size, identity elimination flag, symbol environment).
   - `OneQubitFusionStats`: Telemetry metrics tracking `windows_examined`, `windows_fused`, `operations_removed`, `operations_added`, `global_phase_adjustments`, `windows_rejected`, and `iterations`.
3. **Module Registration & Re-Exports** (`src/optimizer/passes/mod.rs`, `src/optimizer/mod.rs`, `src/lib.rs`):
   - Expose `OneQubitFusionPass`, `OneQubitFusionConfig`, `OneQubitFusionStats`, and `DEFAULT_FUSION_TOLERANCE`.
4. **Integration Test Suite** (`tests/optimizer/fusion.rs` registered in `tests/optimizer.rs`):
   - Fixed discrete gate sequences ($H \to H = I$, $H \to X \to H = Z$, $S \to S = Z$, $T \to T \to T \to T = Z$).
   - Mixed rotation and discrete gate sequences ($R_x \to R_y \to H \to Z$).
   - Multiplication order sensitivity verification.
   - Global phase accumulation checks across single and multiple wires.
   - Identity block elimination (resynthesis to 0 operations).
   - Near-singular and near-zero angle resynthesis.
   - Boundary tests: `Barrier`, `Measure`, `Reset`, and `Classical` terminating fusion windows.
   - Two-qubit gate ($CX$) boundary separation.
   - Disjoint wire commutation and independent fusion.
   - Transactional rollback on error and provenance verification.
   - Phase 10 unitary equivalence checks across all fused circuits using a single common global phase.
5. **Phase Documentation**:
   - `docs/project-state/plans/phase-032.md`: Implementation plan.
   - `docs/project-state/reports/phase-032.md`: Completion report with test proof.
   - `docs/project-state/progress.md`: Progress ledger update.

---

## 5. Scope Boundaries
- Do not start Phase 33 (Commutation analysis rules).
- Do not reorder operations across multi-qubit gates or independent operations to create fusion opportunities.
- Do not cross side-effect or non-unitary boundaries (`Barrier`, `Measure`, `Reset`, `Classical`).
- Do not silently discard global phase.
- Preserve all existing Phase 1–31 tests intact.
- Stable Rust only.

---

## 6. Verification Protocol
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell` and `cargo run --example qasm`
7. Independent OpenQASM 3 reference parser validation.

