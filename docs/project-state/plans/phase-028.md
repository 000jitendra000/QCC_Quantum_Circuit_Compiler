# Implementation Plan — Phase 28: Identity-Gate Elimination

## 1. Phase Metadata
- **Phase**: Phase 28
- **Title**: Identity-gate elimination
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`)
- **Effort Rating**: **S**
- **Prerequisites**: Phases 10, 12–14, and 27 (completed and verified).

---

## 2. Objective
Remove operations that have no quantum or classical effect from QccIR circuits while strictly preserving circuit semantics, circuit-level global phase, and DAG dependency invariants.

---

## 3. Scope Boundaries & Invariants
- **Stable Rust Only**: Zero nightly flags; strict compiler and clippy compliance.
- **Explicit Identity Detection**: Unambiguously identify explicit identity gates (`id`, `i`).
- **Mathematically Proven Zero Rotations**:
  - $R_x(\theta)$, $R_y(\theta)$, $R_z(\theta)$ eliminated only when parameter evaluates to $0 \pmod{4\pi}$ (exact matrix equality, phase adjustment $0$).
  - Rotations evaluated to $2\pi (2k+1) \equiv 2\pi \pmod{4\pi}$ eliminated only when global phase tracking is enabled, applying the required $+\pi$ scalar global phase adjustment ($R(\theta) = -I = e^{i\pi} I$).
  - Never eliminate unresolved symbolic parameter expressions (e.g., `ParameterExpr::Symbolic`).
  - Explicit tolerance policy: `const DEFAULT_ANGLE_TOLERANCE: f64 = 1e-12`.
- **Strict Preservation of Non-Removable Operations**:
  - `Operation::Barrier`: synchronization ordering preserved.
  - `Operation::Reset`: state reset $|0\rangle$ preserved.
  - `Operation::Measure`: projective measurement preserved.
  - `Operation::Classical`: classical compute and wire hazards preserved.
- **Transactional Graph Rewriting**:
  - All removals staged through `CircuitTransaction::remove_operation`.
  - Automatic dependency wire reconnection and cycle validation.
  - Atomic commit or rollback upon invariant failure.
- **Semantic Domain**:
  - Default `SemanticDomain::GeneralStructural` to safely process both pure unitary circuits and circuits with measurements/resets.
- **Preserve Unitary Equivalence**:
  - Verified with `UnitaryEquivalenceChecker` asserting equivalence up to a single common global phase across all basis vectors.

---

## 4. Key Deliverables
1. **Optimizer Subsystem Pass Implementation**:
   - `src/optimizer/passes/mod.rs`: Passes module declaration and re-exports.
   - `src/optimizer/passes/identity.rs`:
     - `IdentityEliminationPass`: Implements `CompilerPass` trait.
     - `IdentityEliminationConfig`: Configurable tolerance, $2\pi$ elimination toggle, and symbol environment.
     - `IdentityEliminationStats`: Detailed execution statistics (examined, removed, preserved, phase adjustments).
     - `RotationIdentityKind`: Strongly typed classification for rotation angle values.
   - `src/optimizer/mod.rs` & `src/lib.rs`: Expose new pass types and configuration.
2. **IR, Builder, Simulator, & Frontend Integration**:
   - `src/ir/validation.rs`: Recognize `"id"` and `"i"` in `canonical_gate_signature`.
   - `src/simulator/operation_dispatch.rs`: Execute `"id"` / `"i"` as no-op identity in statevector kernels to allow unitary equivalence checking on circuits before pass execution.
   - `src/builder/circuit_builder.rs`: Convenience `id` / `id_gate` methods on `CircuitBuilder`.
   - `src/frontend/parser/gates.rs` & `src/frontend/lowering/operations.rs`: Parse and lower `id` gate into `Operation::custom_gate("id", ...)`.
   - `src/backend/openqasm/emitter.rs`: Emit `id` operations seamlessly.
3. **Automated Test Suite**:
   - `tests/optimizer.rs`: Integration test runner root for optimizer passes.
   - `tests/optimizer/identity.rs`:
     - Explicit identity gate removal (`id`, `i`).
     - Zero rotation elimination ($R_x(0)$, $R_y(0)$, $R_z(0)$).
     - $4\pi$ periodic rotation elimination ($\pm 4\pi, 8\pi$).
     - $2\pi$ periodic rotation elimination with global phase update ($\pm 2\pi, 6\pi$).
     - Configurable refusal of $2\pi$ rotations when global phase update is disabled.
     - Preservation of non-zero rotations ($0.1, \pi/2$).
     - Strict preservation of unresolved symbolic rotations ($R_x(\theta)$).
     - Strict preservation of non-removable operations (barriers, resets, measurements, classical ops).
     - DAG wire dependency reconnection and topological validity.
     - Property-based testing with Proptest for randomized angles and tolerance bounds.
     - Randomized circuit equivalence testing: inserting identities into random circuits, eliminating them, and asserting unitary equivalence with `check_equivalence`.
     - Behavioral sampling tests for non-unitary circuits containing identities.
4. **Documentation & Ledger**:
   - `docs/project-state/plans/phase-028.md`
   - `docs/project-state/reports/phase-028.md`
   - Update `docs/project-state/progress.md`.

---

## 5. Verification Gate
- `cargo fmt --check` passes cleanly.
- `cargo clippy --all-targets --all-features -- -D warnings` reports zero warnings.
- `cargo test --all` passes 100% of tests.
- `cargo build --all-targets` passes without warnings.
- `cargo doc --no-deps` completes successfully.
- Examples `cargo run --example bell` and `cargo run --example qasm` execute cleanly.
