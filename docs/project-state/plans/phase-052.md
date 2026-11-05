# Phase Implementation Plan: Phase 52 — SABRE Bidirectional Routing and Layout Refinement

## 1. Metadata
- **Phase Number**: Phase 52
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: **XL**
- **Prerequisites Satisfied**: Phase 51 verified and complete ([`docs/project-state/reports/phase-051.md`](./reports/phase-051.md))
- **Source Document Page**: Page 27 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

---

## 2. Objective & Scope Boundaries
- **Objective**: Implement bidirectional SABRE routing: reverse circuit traversal to derive an improved initial layout, forward SABRE routing, and iterative layout-refinement passes preserving exact initial/final permutation semantics and ADR-0001 strict ancilla policy.
- **Explicit Inclusions**:
  - `src/routing/sabre/bidirectional.rs`:
    - Reverse circuit construction ($C_{rev}$) inverting dependency DAG order and phase without mutating the original logical circuit.
    - Full forward and reverse SABRE routing engine executing physical SWAP insertions via `SabreHeuristic`, `CandidateGenerator`, and `DecayTable`.
    - Detailed `SabreRouterReport` and configuration `SabreRouterConfig`.
  - `src/routing/sabre/layout_refinement.rs`:
    - Iterative layout refinement coordinating forward and reverse passes over configurable rounds.
    - Explicit tracking of candidate initial layouts, intermediate permutations, and round-by-round cost comparisons using Phase 49 `RoutingMetrics`.
    - Objective-based acceptance rule (accepting refinements that improve composite routing cost or SWAP count).
    - Unambiguous layout semantics across forward/reverse boundaries.
  - `tests/routing/sabre_bidirectional.rs`:
    - Hand-calculated circuit fixtures on line, ring, star, and grid topologies.
    - Exact permutation and inverse-permutation tracking.
    - Improved initial layout derivation proving SWAP reduction.
    - Layout refinement acceptance and rejection behavior.
    - ADR-0001 strict ancilla zero-state preservation on partially occupied devices.
    - Verification using `LayoutEquivalenceChecker`.
    - 100% deterministic repeatability across identical runs.
- **Explicit Exclusions**:
  - Comparative benchmark sweeps and automated router pipelines (Phase 53).

---

## 3. Mathematical Formulation & Permutation Conventions

### 3.1 Layout Representation & Coordinate Systems
- A layout $L$ maps logical program qubits $Q = \{q_0, \dots, q_{n-1}\}$ to physical hardware qubits $P = \{p_0, \dots, p_{m-1}\}$ ($m \ge n$):
  $$L: Q \to P, \quad L(q) = p$$
- `LayoutMap::get_physical(q)` returns $p = L(q)$.
- `LayoutMap::get_logical(p)` returns $q = L^{-1}(p)$ for occupied physical qubits.
- A physical SWAP $(u, v)$ interchanges the occupants of physical qubits $u$ and $v$:
  $$L' = \text{swap}_{u, v} \circ L$$
  where $L'(q) = v$ if $L(q) = u$, $L'(q) = u$ if $L(q) = v$, and $L'(q) = L(q)$ otherwise.

### 3.2 Forward Routing Permutation
- Let $C$ be the forward logical circuit with operations $g_1, g_2, \dots, g_k$.
- Routing begins at time $t = 0$ with initial layout $L_{\text{initial}}$.
- During routing, physical SWAPs $S_1, S_2, \dots, S_w$ are inserted along coupling edges.
- Routing terminates at time $t = T$ with final layout:
  $$L_{\text{final}} = S_w \circ \dots \circ S_2 \circ S_1 \circ L_{\text{initial}}$$

### 3.3 Reverse Circuit Traversal & Initial Layout Derivation
- The reversed circuit $C_{rev}$ has operations $g_k, \dots, g_2, g_1$ in reversed dependency order (successors become predecessors).
- Reverse routing starts at time $t = T$ with layout $L_{\text{start}}^{rev}$ (e.g. $L_{\text{final}}$ of a prior forward pass).
- During reverse routing, physical SWAPs $R_1, \dots, R_v$ are inserted to bring operands of reversed gates adjacent.
- Reverse routing terminates at time $t = 0$ with reverse final layout:
  $$L_{\text{end}}^{rev} = R_v \circ \dots \circ R_1 \circ L_{\text{start}}^{rev}$$
- Because the early operations of $C$ ($g_1, g_2, \dots$) are the *final* operations resolved in $C_{rev}$, the logical operands of $g_1$ have just been brought adjacent in $L_{\text{end}}^{rev}$.
- Therefore, $L_{\text{end}}^{rev}$ is directly a candidate initial layout for forward routing:
  $$L_{\text{initial}}^{\text{candidate}} = L_{\text{end}}^{rev}$$
  **Critical Convention**: No permutation inversion ($L \to L^{-1}$) is performed; $L_{\text{end}}^{rev}$ maps each logical qubit $q$ to the physical location where it ended at $t = 0$.

### 3.4 Iterative Refinement Loop
For rounds $r = 1, \dots, R$:
1. **Forward Pass**: Route $C$ from $L_{\text{initial}}^{(r)}$, yielding routed circuit $C_{\text{routed}}^{(r)}$, final layout $L_{\text{final}}^{(r)}$, and metrics $M^{(r)}$.
2. **Reverse Pass**: Route $C_{rev}$ from $L_{\text{final}}^{(r)}$, yielding reverse final layout $L_{\text{rev\_final}}^{(r)}$.
3. **Candidate Generation**: Set next round's candidate initial layout $L_{\text{initial}}^{(r+1)} = L_{\text{rev\_final}}^{(r)}$.
4. **Acceptance Rule**: If $M^{(r)}$ improves composite cost (or SWAP count on tie), accept and update the best-seen result.

---

## 4. Architectural Invariants
- **Stable Rust Only**: Zero nightly compiler features.
- **Circuit Immutability**: Original logical circuit $C$ is never mutated during reverse analysis.
- **Routing Provenance**: Every physical operation references its logical origin; inserted SWAPs record explicit causality.
- **ADR-0001 Strict Ancilla Zero-State**: Physical qubits not occupied by logical qubits begin in $|0\rangle$ and must return to $|0\rangle$ at circuit termination. Verified via `LayoutEquivalenceChecker`.
- **Target Legality**: Routed physical circuits must pass `TargetLegalityValidator`.
- **Deterministic Repeatability**: Identical inputs and configurations produce bit-for-bit identical results.

---

## 5. Step-by-Step Implementation Sequence
1. Implement `src/routing/sabre/layout_refinement.rs`:
   - `RefinementRoundRecord`, `RefinementSummary`, and refinement decision engine.
2. Implement `src/routing/sabre/bidirectional.rs`:
   - `reverse_circuit` helper constructing $C_{rev}$.
   - Core SABRE routing pass (`route_single_pass`).
   - `SabreRouterConfig`, `SabreRouterReport`, and `SabreBidirectionalRouter`.
3. Export new modules in `src/routing/sabre/mod.rs`, `src/routing/mod.rs`, and `src/lib.rs`.
4. Implement `tests/routing/sabre_bidirectional.rs` covering all 11 required test categories.
5. Run complete test and verification suite.
6. Update `docs/project-state/progress.md` and write completion report `docs/project-state/reports/phase-052.md`.
7. Commit Phase 52 changes with git.

