# ADR-0005: Selection of Commutative Two-Qubit Cancellation for Milestone M10

## Status
**Accepted** (13 November 2026 — Phase 78)

## Context
Roadmap v3.1 Milestone M10 (Phases 78–81) executes **Evidence-Selected Optimizer Improvements**.
Under Roadmap Governance Rule 8 ("Evidence-Selected Optimization"):
> *"Do not begin M10 optimizer improvements before M7/M9 benchmark evidence identifies a concrete gap."*

In Phase 70 fair benchmarking against Qiskit across 18 canonical workloads (90 paired trials under identical bases, coupling graphs, and trial counts):
- **Two-Qubit (CX) Count**: QCC won **0**, Qiskit won **13**, with **5 ties**.
- **Critical-Path Depth**: QCC won **1**, Qiskit won **15**, with **2 ties**.
- **Total Gate Count**: QCC won **2**, Qiskit won **15**, with **1 tie**.

QCC's primary competitive deficit is physical two-qubit gate count.

Source-level inspection of QCC's optimizer pipeline (`src/optimizer/pipeline.rs`) and pass contracts revealed that QCC's standard logical optimizer (`OptimizationLevel::O2`) includes `IdentityEliminationPass`, `InverseCancellationPass`, `RotationMergePass`, and `OneQubitFusionPass`.

In `InverseCancellationPass` (`src/optimizer/passes/cancellation.rs`), candidate cancellation pairs are strictly required to be **wire-adjacent on all shared quantum wires simultaneously**. If two identical, self-inverse gates ($CX(c, t) \dots CX(c, t)$) are separated by operations that mathematically commute with control or target, `O2` fails to cancel them.

Conversely, the experimental `CommutationReorderPass` in `O3` attempts speculative adjacent node swaps in topological order. In corpus evaluations, this pass caused depth and gate-count regressions (e.g. `c0037` depth degraded from 19 to 21, gates increased from 57 to 58) while incurring a **$6.8\times$ compile-time penalty** due to pass-manager loop oscillation.

Across the canonical 1,080-case verified corpus (`corpus_v1.jsonl`), **278 commutative CX/CZ cancellation pairs exist across 154 cases** ($14.3\%$ of the corpus), representing up to **556 physical CX gates**.

## Decision

QCC formally selects **Commutative Two-Qubit (CX/CZ) Cancellation** as the single bounded optimizer improvement for Milestone M10 (Phases 79–81):

1. **Dedicated Pass Implementation**:
   Implement a new isolated pass (`CommutativeCancellationPass`) operating directly on the QccIR dependency DAG.
2. **Forward Reachability Traversal**:
   Instead of speculatively bubbling gates through topological node swaps, the pass performs a targeted forward reachability scan along quantum wires starting from each candidate two-qubit gate $G_1 \in \{CX(c, t), CZ(a, b)\}$:
   - For $CX(c, t)$: Scans forward along wire $c$ validating that all intervening operations commute with $G_1$'s control action ($R_z(\theta), Z, S, S^\dagger, T, T^\dagger$, other $CX(c, \cdot)$ sharing control $c$, $CZ(c, \cdot)$), and scans forward along wire $t$ validating that all intervening operations commute with $G_1$'s target action ($R_x(\theta), X$, other $CX(\cdot, t)$ sharing target $t$).
   - For $CZ(a, b)$: Scans forward along both wires validating that all intervening operations commute with $CZ$ ($R_z(\theta), Z, S, S^\dagger, T, T^\dagger$, other $CZ$s).
   - If a matching self-inverse gate $G_2$ is reached along all operand wires without encountering any non-commuting operation, non-unitary operation (`Measure`, `Reset`), synchronization `Barrier`, or classical hazard boundary: atomically removes $G_1$ and $G_2$ from the DAG via transactional rewriting, reconnecting intervening wires directly.
3. **Strict Semantic & Global Phase Preservation**:
   Because $G \cdot G^\dagger = I$, cancellation carries **zero net global phase change** ($\Delta \phi = 0$). No floating-point angle normalization is involved.
4. **Guaranteed Monotonicity**:
   Every rewrite removes exactly 2 gates from the circuit. The pass is guaranteed never to increase total gate count or critical-path depth.
5. **Execution Order and Opt-In Gating**:
   In Phase 80, the pass is initially implemented as an opt-in pass (`OptimizerConfig.enable_commutative_cancellation`). Upon passing the Phase 81 independent oracle acceptance gate across all 1,080 corpus cases, it will be promoted to run in standard logical optimization (`OptimizationLevel::O2`).

## Alternatives Considered and Rejected

1. **Commutative Same-Axis Rotation Merging**:
   - *Consideration*: Merging $R_z(q, \theta_1) \dots R_z(q, \theta_2)$ across commuting operations on wire $q$ occurs in 2,179 pairs across 527 cases.
   - *Rejection Reason*: On native superconducting targets with $\{R_z, SX, CX\}$ primitives, $R_z$ rotations are implemented as zero-duration virtual frame changes (`duration: 0.0 ns`). Merging $R_z$ rotations yields **zero physical makespan reduction**, whereas eliminating CX gates cuts $150\text{ ns}$ per gate ($83.4\,\mu\text{s}$ total). Moreover, rotation merging carries floating-point angle sum tolerances and $4\pi$ global-phase tracking risks, whereas CX cancellation carries zero phase risk.
2. **Tuning Topological Commutation Reordering (`CommutationReorderPass`)**:
   - *Consideration*: Improving lookahead windows in the existing `O3` pass.
   - *Rejection Reason*: Speculative topological bubbling inherently risks breaking single-qubit fusion windows and increasing circuit depth (as demonstrated on `c0037`). It incurs quadratic compile-time scaling without guaranteeing convergence.
3. **Terminal / Virtual SWAP Elision (`ElidePermutations`)**:
   - *Consideration*: Eliminating terminal SWAP gates by permuting output layout maps saves up to 9 CX gates in QFT circuits.
   - *Rejection Reason*: Modifying logical circuit output permutations breaks `OptimizerPipeline`'s strict unitary equivalence contract (`check_equivalence(initial, circuit)`). SWAP elision is a layout/routing boundary capability, not a logical optimizer DAG rewrite.
4. **Two-Qubit Block Optimization (KAK / Cartan Decomposition)**:
   - *Consideration*: Optimal $SU(4)$ synthesis can reduce arbitrary 2-qubit blocks to $\le 3$ CX gates.
   - *Rejection Reason*: **Strictly prohibited** by QCC Non-Negotiable Project Invariants (Rule 10: *"KAK, MPS, and remote provider adapters remain deferred unless the user explicitly changes the project direction"*).
5. **Symmetric Gate Operand Normalization**:
   - *Consideration*: Cancelling $CZ(a, b) \cdot CZ(b, a)$ with reversed operands.
   - *Rejection Reason*: Zero instances exist in the 1,080-case verified corpus.

## Implementation Consequences & Bounds

1. **Module Location**: New pass module `src/optimizer/passes/commutative_cancellation.rs`.
2. **Rewrite Infrastructure**: Must use centralized transactional rewrite helpers (`CircuitTransaction::remove_operation`) with automatic dependency reconnection, cycle validation, and rollback. Direct petgraph mutation across passes is prohibited.
3. **Telemetry & Auditability**: Telemetry recorded via `PassDiagnosticReport` conforming to `qcc.pass_diagnostic` schema v1.0.0, reporting pairs examined, CX pairs cancelled, CZ pairs cancelled, and execution duration.
4. **Scope Boundaries**: Strictly bounded to two-qubit self-inverse gates ($CX, CZ$) across verified commuting operations. Does not reorder other operations, synthesize non-unitary blocks, or modify gate parameters.

## Required Tests

1. **Rule-Level Commutation Proofs**: Test all valid and invalid pairs against mathematically exact $4 \times 4$ unitary matrix definitions.
2. **Wire Adjacency Negative Cases**: Verify that non-commuting operations (e.g. $R_y$ on target wire, $R_x$ on control wire, or $CX$ with mismatched operands) strictly prevent cancellation.
3. **Boundary Protection**: Verify that `Measure`, `Reset`, `Barrier`, and classical dependencies (RAW, WAR, WAW) strictly terminate reachability traversal.
4. **Multi-Wire Interleaving**: Test overlapping independent CX pairs to ensure dependency reconnection preserves DAG acyclicity and execution order.
5. **Full Physical Oracle Validation**: Validate all 1,080 corpus cases through the independent Qiskit statevector oracle with 100% pass rate.

