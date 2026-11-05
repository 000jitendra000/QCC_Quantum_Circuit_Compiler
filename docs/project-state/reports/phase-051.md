# Phase Completion Report: Phase 51 — SABRE Heuristic, Candidate Scoring and Decay

## 1. Executive Summary
- **Phase Number**: Phase 51
- **Exact Title**: SABRE heuristic, candidate scoring and decay
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Status**: **Completed**
- **Completion Date**: 2026-11-05
- **Exit Gate Satisfied**: **YES** (Heuristic fixture rankings and repeatability tests pass)

---

## 2. Implementation Overview

Phase 51 implements the complete mathematical SABRE heuristic formulation (ASPLOS 2019, Li et al.) across four modular components in `src/routing/sabre/`:

1. **Candidate SWAP Generation ([`src/routing/sabre/candidate.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/candidate.rs))**:
   - `CandidateSwap` represents a canonical physical SWAP between adjacent physical qubits $(u, v)$ where $u < v$.
   - `CandidateGenerator` identifies front-layer operations whose physical operands have distance $> 1$, generating candidate SWAPs from coupling edges incident to either operand.
   - All candidates are deduplicated via `BTreeSet` and returned in strictly deterministic order. Front-layer operations that are already adjacent require no candidate SWAPs.

2. **Decay Table & Anti-Oscillation Tracking ([`src/routing/sabre/decay.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/decay.rs))**:
   - `DecayTable` tracks dynamic decay multipliers $\text{decay}(p) \ge 1.0$ per physical qubit.
   - Recording a candidate SWAP increments the decay factors of both $u$ and $v$ by configurable $\delta$ (default $0.001$).
   - Computes candidate decay multipliers as $\text{decay\_factor}(u, v) = \max(\text{decay}(u), \text{decay}(v))$.
   - Supports reset to $1.0$ upon front-layer gate completion (forward routing progress).

3. **Explainable Score Breakdown & Tie-Breaking ([`src/routing/sabre/scoring.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/scoring.rs))**:
   - `ScoreComponents` exposes the detailed breakdown of candidate evaluations:
     $$H_F(\pi') = \frac{1}{|F|} \sum_{g \in F} D(\pi'(g_1), \pi'(g_2))$$
     $$H_E(\pi') = \begin{cases} \frac{1}{|E|} \sum_{g \in E} D(\pi'(g_1), \pi'(g_2)) & |E| > 0 \\ 0.0 & |E| = 0 \end{cases}$$
     $$\text{Cost}(\pi') = H_F(\pi') + W \cdot H_E(\pi')$$
     $$\text{Score}(\text{SWAP}) = \max(\text{decay}(u), \text{decay}(v)) \times \text{Cost}(\pi')$$
   - Deterministic tie-breaking hierarchy:
     1. `total_score` ascending (within $10^{-12}$ float precision).
     2. `front_cost` $H_F$ ascending.
     3. `lookahead_cost` $H_E$ ascending.
     4. `decay_factor` ascending.
     5. Canonical candidate operands $(u, v)$ lexicographically ascending.

4. **Heuristic Orchestrator ([`src/routing/sabre/heuristic.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/sabre/heuristic.rs))**:
   - `SabreHeuristic` coordinates candidate generation, hypothetical layout evaluation (ensuring active compiler state is never mutated during scoring), lookahead weighting $W$, decay factors, and ranking.
   - Evaluates disconnected physical topologies using `TopologyDistances::disconnected_penalty()`.
   - Validates configuration parameters ($W \ge 0, \delta \ge 0$, finite).

---

## 3. Verification & Test Evidence

### Integration Test Suite ([`tests/routing/sabre_heuristic.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/routing/sabre_heuristic.rs))
- `test_candidate_swap_generation_and_deduplication`: Verifies deduplicated canonical candidate generation around front-layer operands.
- `test_no_candidates_generated_when_all_front_gates_adjacent`: Verifies clean termination when front-layer gates can execute directly.
- `test_hand_calculated_front_and_lookahead_scores`: Evaluates hand-calculated 5-qubit line fixtures for $H_F, H_E, W=0.5, \text{Cost}, \text{decay}$, and total score.
- `test_lookahead_weights_alter_rankings`: Verifies $W=0.0$ (greedy front-layer only) vs $W=2.0$ (heavy lookahead) alters candidate rankings.
- `test_decay_penalties_penalize_repeated_swaps`: Verifies decay increases score and breaks oscillation loops, restoring scores after reset.
- `test_deterministic_tie_breaking_symmetric_topology`: Evaluates a symmetric star topology where candidate scores tie, verifying deterministic resolution via $(u, v)$ lexicographical order.
- `test_disconnected_topology_handling`: Verifies penalty distance application for disconnected physical components.
- `test_invalid_heuristic_config_rejected`: Validates fail-closed handling for negative/NaN weights and decay deltas.
- `test_scoring_does_not_mutate_frontier_state_or_layout`: Confirms active layout, frontier, and lookahead state remain 100% immutable during candidate scoring.
- `test_repeatability_identical_runs_produce_identical_scores`: Verifies 10 repeated evaluations yield identical scores and rankings.

### Pipeline Test Results
- `cargo fmt --check`: Passed (0 diffs)
- `cargo clippy --all-targets -- -D warnings`: Passed (0 warnings)
- `cargo test --test routing`: Passed (**93 / 93 tests**)
- `cargo test --all`: Passed (**1,188 / 1,188 tests** across all 35 test binaries, 0 failed, 0 ignored)
- `cargo doc --no-deps`: Passed (0 warnings)
- `cargo run --example bell && cargo run --example qasm`: Passed
- `cargo bench --bench routing_baseline`: Passed (8 benchmark fixtures verified for 100% deterministic reproducibility)

---

## 4. Acceptance Checklist
- [x] Canonical candidate SWAP generation around front-layer gates implemented.
- [x] Front-layer normalized distance scoring ($H_F$) implemented.
- [x] Extended-lookahead normalized distance scoring ($H_E$) with configurable weight $W$ implemented.
- [x] Anti-oscillation decay tracking and reset policy implemented.
- [x] Deterministic tie-breaking hierarchy implemented.
- [x] Explainable score breakdown struct implemented.
- [x] Real compiler/router state remains immutable during scoring.
- [x] Exit gate satisfied with automated tests.
- [x] Zero warnings across clippy and doc tests.
