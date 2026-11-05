# Phase Completion Report: Phase 51 — SABRE Heuristic, Candidate Scoring and Decay

## 1. Executive Summary
- **Phase Number**: Phase 51
- **Exact Title**: SABRE heuristic, candidate scoring and decay
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Status**: **Completed**
- **Completion Date**: 2026-11-05
- **Exit Gate Satisfied**: **YES** (Heuristic fixture rankings and repeatability tests pass)

---

## 2. Mathematical Formulation & Publication Comparison

Phase 51 implements the complete mathematical SABRE heuristic formulation (ASPLOS 2019, Li et al.) across four modular components in `src/routing/sabre/`:

1. **Normalized Front-Layer Distance**:
   $$H_F(\pi') = \frac{1}{|F|} \sum_{g=(q_1, q_2) \in F} D(\pi'(q_1), \pi'(q_2))$$
   - Evaluated on hypothetical layout $\pi' = \text{swap}(\pi, u, v)$ without mutating active compiler state.
   - Normalized by $|F|$; safely returns $0.0$ if $|F| = 0$.

2. **Normalized Extended-Lookahead Distance**:
   $$H_E(\pi') = \begin{cases} \frac{1}{|E|} \sum_{g=(q_1, q_2) \in E} D(\pi'(q_1), \pi'(q_2)) & |E| > 0 \\ 0.0 & |E| = 0 \end{cases}$$
   - Evaluated on hypothetical layout $\pi'$ across lookahead set $E$, discoverable by transparently skipping completed single-qubit gates.
   - Avoids division by zero when $|E| = 0$.

3. **Combined Basic Cost**:
   $$\text{Cost}(\pi') = H_F(\pi') + W \cdot H_E(\pi')$$
   - Configurable lookahead weight multiplier $W \ge 0.0$ (default $W = 0.5$, matching Li et al.).
   - Validated against negative, NaN, and infinite values.

4. **Decay-Scaled Candidate Score**:
   $$\text{Score}(\text{SWAP}) = \max(\text{decay}(u), \text{decay}(v)) \times \text{Cost}(\pi')$$
   - Initial physical decay $\text{decay}(p) = 1.0$.
   - Selected candidate SWAP $(u, v)$ increments $\text{decay}(u) \leftarrow \text{decay}(u) + \delta$ and $\text{decay}(v) \leftarrow \text{decay}(v) + \delta$ transactionally.
   - Front-layer forward progress resets all physical decay factors to $1.0$.

### Canonical Publication vs. QCC Adaptations
- **Shortest Path Distance**: Li et al. assume an idealized, fully-connected coupling graph. QCC models target topologies using precomputed BFS all-pairs shortest paths on undirected coupling graphs, with explicit disconnected component detection and mathematically justified penalties.
- **Deterministic 5-Tier Total Order Hierarchy**: Li et al. leave tie-breaking unspecified. QCC implements a strict, transitive 5-tier comparison hierarchy using IEEE 754-2008 `total_cmp` to ensure 100% repeatable compilation.
- **Canonical Undirected Pairs**: QCC enforces canonical $u < v$ physical qubit IDs, deduplicating symmetric edges and eliminating redundant evaluations.
- **Explainability**: `ScoreComponents` exposes full numerical breakdowns ($H_F, H_E, W, \text{decay\_factor}, \text{total\_score}$) for introspection and compiler diagnostics.

---

## 3. Implementation Overview

1. **Candidate SWAP Generation ([`src/routing/sabre/candidate.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/candidate.rs))**:
   - `CandidateSwap` represents a canonical physical SWAP between adjacent physical qubits $(u, v)$ where $u < v$.
   - `CandidateGenerator` identifies front-layer operations whose physical operands have distance $> 1$, generating candidate SWAPs from coupling edges incident to either operand.
   - All candidates are deduplicated via `BTreeSet` and returned in strictly deterministic order. Operations already adjacent or residing in disconnected components do not generate redundant SWAPs.

2. **Decay Table & Anti-Oscillation Tracking ([`src/routing/sabre/decay.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/decay.rs))**:
   - `DecayTable` tracks dynamic decay multipliers $\text{decay}(p) \ge 1.0$ per physical qubit.
   - Transactional `record_swap`: bounds-checks both $u$ and $v$ prior to modification, returning `Err(RoutingError::QubitOutOfBounds)` with zero partial modification if either index is invalid.
   - Computes candidate decay multipliers as $\text{decay\_factor}(u, v) = \max(\text{decay}(u), \text{decay}(v))$.
   - Supports reset to $1.0$ upon front-layer gate completion (forward routing progress).

3. **Explainable Score Breakdown & 5-Tier Deterministic Total Ordering ([`src/routing/sabre/scoring.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/scoring.rs))**:
   - `ScoreComponents` exposes detailed metrics and implements `Ord` via exact IEEE 754-2008 [`f64::total_cmp`]. Epsilon-based float equality is strictly prohibited inside the sorting comparator to avoid violating transitivity ($A \approx B \land B \approx C \not\implies A \approx C$).
   - 5-Tier Deterministic Comparison Hierarchy:
     1. `total_score` ascending via [`f64::total_cmp`] (primary optimization goal: smaller score preferred).
     2. `front_cost` ascending via [`f64::total_cmp`] (secondary tie-breaker: prefer immediate front-layer distance reduction).
     3. `lookahead_cost` ascending via [`f64::total_cmp`] (tertiary tie-breaker: prefer superior lookahead distance positioning).
     4. `decay_factor` ascending via [`f64::total_cmp`] (quaternary tie-breaker: prefer less frequently swapped physical qubits).
     5. Candidate physical operands `(u, v)` lexicographically ascending via [`CandidateSwap::cmp`] (quinary tie-breaker: deterministic symmetry breaking).

4. **Heuristic Orchestrator & Fast-Fail Disconnected Handling ([`src/routing/sabre/heuristic.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/heuristic.rs))**:
   - `SabreHeuristic` coordinates candidate generation, hypothetical layout evaluation (ensuring active compiler state is never mutated during scoring), lookahead weighting $W$, decay factors, and ranking.
   - Disconnected penalty distance is mathematically justified: $\text{penalty} = \max(N, 1) \times 1000 \gg N - 1$ (diameter of any connected component).
   - Fast-fail on disconnected front-layer gates: returns `Err(RoutingError::DisconnectedTopology { src, dst })` immediately when no routing path exists, preventing infinite oscillation loops.
   - Validates configuration parameters ($W \ge 0, \delta \ge 0$, finite).

---

## 4. Verification & Test Evidence

### Integration Test Suite ([`tests/routing/sabre_heuristic.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/routing/sabre_heuristic.rs))
1. `test_candidate_swap_generation_and_deduplication`: Verifies deduplicated canonical candidate generation around front-layer operands.
2. `test_no_candidates_generated_when_all_front_gates_adjacent`: Verifies clean termination when front-layer gates can execute directly.
3. `test_hand_calculated_front_and_lookahead_scores`: Evaluates hand-calculated 5-qubit line fixtures for $H_F, H_E, W=0.5, \text{Cost}, \text{decay}$, and total score.
4. `test_lookahead_weights_alter_rankings`: Verifies $W=0.0$ (greedy front-layer only) vs $W=2.0$ (heavy lookahead) alters candidate rankings.
5. `test_decay_penalties_penalize_repeated_swaps`: Verifies decay increases score and breaks oscillation loops, restoring scores after reset.
6. `test_deterministic_tie_breaking_symmetric_topology`: Evaluates a symmetric star topology where candidate scores tie, verifying deterministic resolution via $(u, v)$ lexicographical order.
7. `test_disconnected_topology_handling`: Verifies penalty distance application for disconnected physical components and fast-fail with `DisconnectedTopology`.
8. `test_invalid_heuristic_config_rejected`: Validates fail-closed handling for negative/NaN weights and decay deltas.
9. `test_scoring_does_not_mutate_frontier_state_or_layout`: Confirms active layout, frontier, and lookahead state remain 100% immutable during candidate scoring.
10. `test_repeatability_identical_runs_produce_identical_scores`: Verifies 10 repeated evaluations yield identical scores and rankings.
11. `test_comparator_strict_transitivity_and_total_order`: Verifies mathematical transitivity ($A \le B \land B \le C \implies A \le C$) on near-equal float scores under `total_cmp`.
12. `test_deterministic_5_tier_tie_breaking_hierarchy`: Verifies each tier of the 5-tier comparison hierarchy in isolation.
13. `test_comparator_handles_nan_and_inf_safely`: Verifies safe ordering under `total_cmp` with NaN and infinity.
14. `test_disconnected_topology_isolated_subgraph_success`: Verifies routing succeeds when circuits reside entirely within a connected component of a disconnected topology.
15. `test_disconnected_lookahead_gate_penalty_scoring`: Verifies lookahead gates spanning disconnected components cleanly receive penalty distance.
16. `test_decay_transactional_failure_does_not_modify_state`: Verifies out-of-bounds swap recording fails cleanly without partial modification.
17. `test_decay_unrelated_qubits_remain_unaffected`: Verifies repeated swapping of $(1, 2)$ leaves qubits 0, 3, 4 strictly unaffected.
18. `test_decay_reset_qubit`: Verifies resetting individual physical qubits and out-of-bounds error handling.

### Pipeline Test Results
- `cargo fmt --check`: Passed (0 diffs)
- `cargo clippy --all-targets -- -D warnings`: Passed (0 warnings)
- `cargo test --test routing`: Passed (**101 / 101 tests**, including all 18 SABRE heuristic tests)
- `cargo test --all`: Passed (**1,196 / 1,196 tests** across all 35 test binaries, 0 failed, 0 ignored)
- `cargo doc --no-deps`: Passed (0 warnings)
- `cargo run --example bell && cargo run --example qasm`: Passed
- `cargo bench --bench routing_baseline`: Passed (8 benchmark fixtures verified for 100% deterministic reproducibility)

---

## 5. Acceptance Checklist
- [x] Canonical candidate SWAP generation around front-layer gates implemented.
- [x] Front-layer normalized distance scoring ($H_F$) implemented.
- [x] Extended-lookahead normalized distance scoring ($H_E$) with configurable weight $W$ implemented.
- [x] Anti-oscillation decay tracking and reset policy implemented.
- [x] 5-Tier deterministic total ordering hierarchy implemented with exact `total_cmp`.
- [x] Explainable score breakdown struct implemented.
- [x] Mathematically justified disconnected topology penalty ($\max(N, 1) \times 1000 \gg N - 1$) and fast-fail error handling implemented.
- [x] Transactional bounds-checked decay updates preventing partial state corruption implemented.
- [x] Real compiler/router state remains immutable during scoring.
- [x] Exit gate satisfied with automated tests.
- [x] Zero warnings across clippy and doc tests.