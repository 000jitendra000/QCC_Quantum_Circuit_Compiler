# Phase Implementation Plan: Phase 51 — SABRE Heuristic, Candidate Scoring and Decay

## 1. Metadata
- **Phase Number**: Phase 51
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: **XL**
- **Prerequisites Satisfied**: Phase 50 verified and complete ([`docs/project-state/reports/phase-050.md`](./reports/phase-050.md))
- **Source Document Page**: Page 27 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

---

## 2. Objective & Scope Boundaries
- **Objective**: Implement the authoritative SABRE heuristic, candidate SWAP generation, lookahead distance scoring, and decay penalties rather than labeling a greedy heuristic as SABRE.
- **Explicit Inclusions**:
  - `src/routing/sabre/candidate.rs`: `CandidateSwap` and `CandidateGenerator` generating unique, canonical candidate SWAPs on coupling edges adjacent to front-layer operands.
  - `src/routing/sabre/decay.rs`: `DecayTable` maintaining per-physical-qubit decay factors $\ge 1.0$, decay increments $\delta$, and reset/decay policies.
  - `src/routing/sabre/scoring.rs`: `ScoreComponents` exposing explainable metric breakdowns ($H_F, H_E, W, decay\_factor, total\_score$) and deterministic tie-breaking.
  - `src/routing/sabre/heuristic.rs`: `SabreHeuristicConfig` and `SabreHeuristic` coordinating candidate generation, hypothetical layout evaluation, weighted distance calculations, and deterministic ranking.
  - `tests/routing/sabre_heuristic.rs`: Hand-calculated ranking fixtures, candidate deduplication, lookahead weighting, decay updates, tie-breaking, and non-mutating scoring.
- **Explicit Exclusions**:
  - Bidirectional SABRE, reverse DAG routing, and initial layout refinement (Phases 52).
  - Comparative benchmark sweeps and automated router pipelines (Phase 53).

---

## 3. Mathematical Formulation & Architecture Invariants

### 3.1 Candidate SWAP Generation
For each two-qubit gate $g = (q_1, q_2) \in F$ (the active front layer) that is not currently adjacent on the coupling graph:
Let $p_1 = \pi(q_1)$ and $p_2 = \pi(q_2)$.
Candidate physical SWAP edges are generated from coupling edges adjacent to $p_1$ or $p_2$:
$$\mathcal{C} = \bigcup_{g=(q_1, q_2) \in F, D(p_1, p_2) > 1} \left( \{(p_1, v) \mid v \in \mathcal{N}(p_1)\} \cup \{(p_2, u) \mid u \in \mathcal{N}(p_2)\} \right)$$
Canonicalized such that for each pair $\{u, v\}$, $u < v$ by `PhysicalQubitId`. Duplicates are eliminated via `BTreeSet`.

### 3.2 Candidate Scoring Function
For candidate $SWAP(u, v) \in \mathcal{C}$, let $\pi'$ be the hypothetical layout obtained by swapping $u$ and $v$ in layout $\pi$:
$$H_F(\pi') = \frac{1}{|F|} \sum_{g=(q_1, q_2) \in F} D(\pi'(q_1), \pi'(q_2))$$
$$H_E(\pi') = \begin{cases} \frac{1}{|E|} \sum_{g=(q_1, q_2) \in E} D(\pi'(q_1), \pi'(q_2)) & \text{if } |E| > 0 \\ 0.0 & \text{if } |E| == 0 \end{cases}$$
$$Cost(\pi') = H_F(\pi') + W \cdot H_E(\pi')$$
$$Score(SWAP) = \max(decay(u), decay(v)) \times Cost(\pi')$$
where:
- $D(p_1, p_2)$ is the shortest path hop distance on the hardware coupling graph (with disconnected penalty applied for unreachable pairs).
- $W \ge 0$ is the configurable lookahead weight parameter.
- $decay(p) \ge 1.0$ is the decay factor for physical qubit $p$.

### 3.3 Decay Update Policy
- Initialization: $\forall p, decay(p) = 1.0$.
- When $SWAP(u, v)$ is executed: $decay(u) \leftarrow decay(u) + \delta$, $decay(v) \leftarrow decay(v) + \delta$.
- Reset policy: When a gate in $F$ is satisfied and executed, $\forall p, decay(p) \leftarrow 1.0$.

### 3.4 Deterministic Tie-Breaking
When comparing two scored candidates $A$ and $B$:
1. $Score(A) < Score(B)$ (within $10^{-12}$ float precision).
2. $H_F(A) < H_F(B)$.
3. $H_E(A) < H_E(B)$.
4. $decay\_factor(A) < decay\_factor(B)$.
5. Lexicographical order of physical candidate pair: $(A.u, A.v) < (B.u, B.v)$.

---

## 4. Step-by-Step Implementation Sequence
1. Implement `src/routing/sabre/candidate.rs` (`CandidateSwap`, `CandidateGenerator`).
2. Implement `src/routing/sabre/decay.rs` (`DecayTable`).
3. Implement `src/routing/sabre/scoring.rs` (`ScoreComponents`, `ScoredCandidate`).
4. Implement `src/routing/sabre/heuristic.rs` (`SabreHeuristicConfig`, `SabreHeuristic`).
5. Wire exports into `src/routing/sabre/mod.rs`, `src/routing/mod.rs`, and `src/lib.rs`.
6. Implement `tests/routing/sabre_heuristic.rs` with hand-calculated fixtures.
7. Run complete verification (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo doc`).

---

## 5. Expected Files and Modules
- New files to create:
  - `src/routing/sabre/candidate.rs`
  - `src/routing/sabre/decay.rs`
  - `src/routing/sabre/scoring.rs`
  - `src/routing/sabre/heuristic.rs`
  - `tests/routing/sabre_heuristic.rs`
  - `docs/project-state/plans/phase-051.md`
  - `docs/project-state/reports/phase-051.md`
- Existing files to modify:
  - `src/routing/sabre/mod.rs`
  - `src/routing/mod.rs`
  - `src/lib.rs`
  - `tests/routing.rs`
  - `docs/project-state/progress.md`

---

## 6. Verification & Test Plan
- Candidate generation on Line, Grid, and Star targets.
- Duplicate elimination and deterministic canonical ordering.
- Hand-calculated front-layer distance scoring.
- Hand-calculated lookahead distance scoring.
- Varying lookahead weights ($W=0.0, 0.5, 1.0$) altering candidate rankings.
- Decay penalties discouraging repeated swap oscillation.
- Decay reset upon front-layer gate advancement.
- Deterministic tie-breaking on symmetric topologies.
- Disconnected topology handling.
- Immutability of active compiler state during scoring.
- Repeatability tests confirming 100% deterministic results across identical runs.

---

## 7. Exit Gate Verification Target
> **EXIT GATE**: Heuristic fixture rankings and repeatability tests pass.
- Verification command: `cargo test --test routing`
- Workspace suite: `cargo test --all`
- Outcome: Zero warnings, zero errors, all tests passing.
