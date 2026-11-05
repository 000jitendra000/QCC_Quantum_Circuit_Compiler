//! Integration tests and hand-calculated fixtures for SABRE Heuristic, Candidate Scoring, and Decay (Phase 51).
//!
//! Validates:
//! - Candidate SWAP generation around front-layer operands.
//! - Duplicate candidate elimination and deterministic canonical ordering.
//! - Hand-calculated front-layer distance score calculations ($H_F$).
//! - Hand-calculated extended-lookahead distance score calculations ($H_E$).
//! - Different lookahead weights ($W = 0.0, 0.5, 1.0$) altering candidate rankings.
//! - Decay factor updates when physical qubits are repeatedly involved in SWAPs.
//! - Decay reset upon front-layer forward progress.
//! - Deterministic tie-breaking on equal scores.
//! - Disconnected topology handling with proper penalty distances.
//! - Scoring does not mutate the active layout, frontier, or lookahead state.
//! - 100% deterministic repeatability across identical runs.

use qcc::builder::CircuitBuilder;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::placement::distance::TopologyDistances;
use qcc::routing::sabre::{
    CandidateGenerator, CandidateSwap, DecayTable, LookaheadConfig, SabreFrontierState,
    SabreHeuristic, SabreHeuristicConfig, ScoreComponents,
};
use qcc::target::connectivity::{CouplingGraph, Directionality, OperationConnectivity};
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

/// Helper to construct a standard linear hardware target (0 - 1 - ... - n-1).
fn make_linear_target(name: &str, num_qubits: usize) -> Target {
    let mut target = Target::new(name, num_qubits);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((
            PhysicalQubitId::new(i as u32),
            PhysicalQubitId::new((i + 1) as u32),
        ));
        edges.push((
            PhysicalQubitId::new((i + 1) as u32),
            PhysicalQubitId::new(i as u32),
        ));
    }
    if !edges.is_empty() {
        let cx_desc = TargetOperationDescriptor::builder("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(150.0)
            .build();
        target.add_operation(cx_desc);
    }

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Candidate SWAP Generation & Deduplication
// ============================================================================

#[test]
fn test_candidate_swap_generation_and_deduplication() {
    // 5-qubit line target: 0 - 1 - 2 - 3 - 4
    let target = make_linear_target("line5", 5);
    let distances = TopologyDistances::from_coupling_graph(&target.combined_coupling_graph());

    // Front-layer operation: CX(q0, q2) mapped to p0 and p2 (distance = 2)
    let mut b = CircuitBuilder::new("cand_gen");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    let candidates =
        CandidateGenerator::generate_candidates(state.front_layer(), &target, &distances);

    // Neighbors of p0: p1 => CandidateSwap(0, 1)
    // Neighbors of p2: p1, p3 => CandidateSwap(1, 2), CandidateSwap(2, 3)
    // Duplicate check: (p0, p1) and (p1, p2) and (p2, p3) are all canonical (u < v)
    assert_eq!(candidates.len(), 3);
    assert_eq!(
        candidates[0],
        CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );
    assert_eq!(
        candidates[1],
        CandidateSwap::new(PhysicalQubitId::new(1), PhysicalQubitId::new(2))
    );
    assert_eq!(
        candidates[2],
        CandidateSwap::new(PhysicalQubitId::new(2), PhysicalQubitId::new(3))
    );
}

#[test]
fn test_no_candidates_generated_when_all_front_gates_adjacent() {
    let target = make_linear_target("line3", 3);
    let distances = TopologyDistances::from_coupling_graph(&target.combined_coupling_graph());

    // Front-layer operation: CX(q0, q1) mapped to p0 and p1 (distance = 1, already adjacent!)
    let mut b = CircuitBuilder::new("already_adjacent");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(2, 3).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    let candidates =
        CandidateGenerator::generate_candidates(state.front_layer(), &target, &distances);

    // No candidates needed when front layer can execute directly
    assert!(candidates.is_empty());
}

// ============================================================================
// 2. Hand-Calculated Score Calculations & Lookahead Weighting
// ============================================================================

#[test]
fn test_hand_calculated_front_and_lookahead_scores() {
    // 5-qubit line target: 0 - 1 - 2 - 3 - 4
    let target = make_linear_target("line5", 5);

    // Circuit:
    // op#1: CX(q0, q2) -> Front layer (p0 to p2, dist=2)
    // op#2: CX(q1, q2) -> Lookahead layer 1 (p1 to p2, dist=1)
    let mut b = CircuitBuilder::new("hand_calc");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let frontier_config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, frontier_config).unwrap();

    // Configure heuristic with W = 0.5, delta = 0.001
    let heuristic_config = SabreHeuristicConfig::new()
        .with_lookahead_weight(0.5)
        .unwrap();
    let heuristic = SabreHeuristic::new(&target, heuristic_config).unwrap();

    // 1. Evaluate Candidate (0, 1):
    // Hypothetical layout: q0->p1, q1->p0, q2->p2
    // Front op CX(q0, q2): D(p1, p2) = 1.0 => H_F = 1.0
    // Lookahead op CX(q1, q2): D(p0, p2) = 2.0 => H_E = 2.0
    // Cost = 1.0 + 0.5 * 2.0 = 2.0
    // Total score = 1.0 * 2.0 = 2.0
    let cand_01 = CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1));
    let score_01 = heuristic.score_candidate(cand_01, &state).unwrap();
    assert_eq!(score_01.front_cost, 1.0);
    assert_eq!(score_01.lookahead_cost, 2.0);
    assert_eq!(score_01.weighted_lookahead_cost, 1.0);
    assert_eq!(score_01.basic_cost, 2.0);
    assert_eq!(score_01.decay_factor, 1.0);
    assert_eq!(score_01.total_score, 2.0);

    // 2. Evaluate Candidate (1, 2):
    // Hypothetical layout: q0->p0, q1->p2, q2->p1
    // Front op CX(q0, q2): D(p0, p1) = 1.0 => H_F = 1.0
    // Lookahead op CX(q1, q2): D(p2, p1) = 1.0 => H_E = 1.0
    // Cost = 1.0 + 0.5 * 1.0 = 1.5
    // Total score = 1.0 * 1.5 = 1.5
    let cand_12 = CandidateSwap::new(PhysicalQubitId::new(1), PhysicalQubitId::new(2));
    let score_12 = heuristic.score_candidate(cand_12, &state).unwrap();
    assert_eq!(score_12.front_cost, 1.0);
    assert_eq!(score_12.lookahead_cost, 1.0);
    assert_eq!(score_12.weighted_lookahead_cost, 0.5);
    assert_eq!(score_12.basic_cost, 1.5);
    assert_eq!(score_12.decay_factor, 1.0);
    assert_eq!(score_12.total_score, 1.5);

    // 3. Evaluate Candidate (2, 3):
    // Hypothetical layout: q0->p0, q1->p1, q2->p3
    // Front op CX(q0, q2): D(p0, p3) = 3.0 => H_F = 3.0
    // Lookahead op CX(q1, q2): D(p1, p3) = 2.0 => H_E = 2.0
    // Cost = 3.0 + 0.5 * 2.0 = 4.0
    // Total score = 1.0 * 4.0 = 4.0
    let cand_23 = CandidateSwap::new(PhysicalQubitId::new(2), PhysicalQubitId::new(3));
    let score_23 = heuristic.score_candidate(cand_23, &state).unwrap();
    assert_eq!(score_23.front_cost, 3.0);
    assert_eq!(score_23.lookahead_cost, 2.0);
    assert_eq!(score_23.weighted_lookahead_cost, 1.0);
    assert_eq!(score_23.basic_cost, 4.0);
    assert_eq!(score_23.decay_factor, 1.0);
    assert_eq!(score_23.total_score, 4.0);

    // Verify Candidate Ranking:
    let ranked = heuristic
        .evaluate_and_rank_candidates(&state, &target)
        .unwrap();
    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].candidate, cand_12); // score 1.5 (best)
    assert_eq!(ranked[1].candidate, cand_01); // score 2.0
    assert_eq!(ranked[2].candidate, cand_23); // score 4.0
}

#[test]
fn test_lookahead_weights_alter_rankings() {
    let target = make_linear_target("line5", 5);

    // Same circuit as above:
    // Front: CX(q0, q2)
    // Lookahead: CX(q1, q2)
    let mut b = CircuitBuilder::new("weight_impact");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let frontier_config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, frontier_config).unwrap();

    // 1. With Lookahead Weight W = 0.0:
    // H_F for (0, 1) is 1.0
    // H_F for (1, 2) is 1.0
    // Both produce identical total_score = 1.0!
    // Tie-breaker uses lookahead_cost H_E:
    // (1, 2) has H_E = 1.0 vs (0, 1) has H_E = 2.0 => (1, 2) wins tie-breaking
    let config_w0 = SabreHeuristicConfig::new()
        .with_lookahead_weight(0.0)
        .unwrap();
    let heuristic_w0 = SabreHeuristic::new(&target, config_w0).unwrap();
    let ranked_w0 = heuristic_w0
        .evaluate_and_rank_candidates(&state, &target)
        .unwrap();
    assert_eq!(ranked_w0[0].total_score, 1.0);
    assert_eq!(ranked_w0[1].total_score, 1.0);
    assert_eq!(
        ranked_w0[0].candidate,
        CandidateSwap::new(PhysicalQubitId::new(1), PhysicalQubitId::new(2))
    );

    // 2. With Large Lookahead Weight W = 2.0:
    // (1, 2): Cost = 1.0 + 2.0 * 1.0 = 3.0
    // (0, 1): Cost = 1.0 + 2.0 * 2.0 = 5.0
    // (2, 3): Cost = 3.0 + 2.0 * 2.0 = 7.0
    let config_w2 = SabreHeuristicConfig::new()
        .with_lookahead_weight(2.0)
        .unwrap();
    let heuristic_w2 = SabreHeuristic::new(&target, config_w2).unwrap();
    let ranked_w2 = heuristic_w2
        .evaluate_and_rank_candidates(&state, &target)
        .unwrap();
    assert_eq!(ranked_w2[0].total_score, 3.0);
    assert_eq!(ranked_w2[1].total_score, 5.0);
    assert_eq!(ranked_w2[2].total_score, 7.0);
}

// ============================================================================
// 3. Decay Penalties & Reset Policy
// ============================================================================

#[test]
fn test_decay_penalties_penalize_repeated_swaps() {
    let target = make_linear_target("line5", 5);

    let mut b = CircuitBuilder::new("decay_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let frontier_config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, frontier_config).unwrap();

    // Use a significant delta = 0.5 to demonstrate clear penalty
    let config = SabreHeuristicConfig::new()
        .with_lookahead_weight(0.5)
        .unwrap()
        .with_decay_delta(0.5)
        .unwrap();
    let mut heuristic = SabreHeuristic::new(&target, config).unwrap();

    let cand_01 = CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1));
    let cand_12 = CandidateSwap::new(PhysicalQubitId::new(1), PhysicalQubitId::new(2));

    // Initially: cand_12 is best (score 1.5 vs cand_01 score 2.0)
    let best_initial = heuristic.best_candidate(&state, &target).unwrap().unwrap();
    assert_eq!(best_initial.candidate, cand_12);
    assert_eq!(best_initial.total_score, 1.5);
    let initial_01_score = heuristic
        .score_candidate(cand_01, &state)
        .unwrap()
        .total_score;
    assert_eq!(initial_01_score, 2.0);

    // Record SWAP(1, 2) repeatedly:
    // decay(1) becomes 1.0 + 0.5 = 1.5
    // decay(2) becomes 1.0 + 0.5 = 1.5
    heuristic.record_swap(cand_12);
    assert_eq!(heuristic.decay().get(PhysicalQubitId::new(1)), 1.5);
    assert_eq!(heuristic.decay().get(PhysicalQubitId::new(2)), 1.5);

    // Now score cand_12: Cost 1.5 * max(decay(1), decay(2)) = 1.5 * 1.5 = 2.25
    // score cand_01: Cost 2.0 * max(decay(0)=1.0, decay(1)=1.5) = 2.0 * 1.5 = 3.0
    let score_12_decay1 = heuristic.score_candidate(cand_12, &state).unwrap();
    assert_eq!(score_12_decay1.total_score, 2.25);

    // Record SWAP(1, 2) once more:
    // decay(1) becomes 2.0, decay(2) becomes 2.0
    heuristic.record_swap(cand_12);

    // Now cand_12 score = Cost 1.5 * 2.0 = 3.0!
    // What if cand_01 only involves qubit 0 (decay=1.0) and qubit 1 (decay=2.0)?
    // Record swap on qubit 1 and 2 again: decay(2) = 2.5
    heuristic.record_swap(cand_12);

    // Now cand_12 has decay_factor = 2.5 => total_score = 1.5 * 2.5 = 3.75!
    let score_12_decay3 = heuristic.score_candidate(cand_12, &state).unwrap();
    assert_eq!(score_12_decay3.decay_factor, 2.5);
    assert_eq!(score_12_decay3.total_score, 3.75);

    // Reset decay policy: when forward progress occurs, reset back to 1.0
    heuristic.reset_decay();
    for p in 0..5 {
        assert_eq!(
            heuristic.decay().get(PhysicalQubitId::new(p)),
            qcc::routing::sabre::BASE_DECAY
        );
    }

    // After reset: cand_12 is restored to 1.5
    let best_after_reset = heuristic.best_candidate(&state, &target).unwrap().unwrap();
    assert_eq!(best_after_reset.candidate, cand_12);
    assert_eq!(best_after_reset.total_score, 1.5);
}

// ============================================================================
// 4. Deterministic Tie-Breaking
// ============================================================================

#[test]
fn test_deterministic_tie_breaking_symmetric_topology() {
    // Star topology: center 0, leaves 1, 2, 3, 4
    let coupling = CouplingGraph::star(5, 0, Directionality::Undirected);

    // Circuit: CX(q1, q2) on leaves 1 and 2 (both distance 2 via center 0)
    let mut b = CircuitBuilder::new("star_tie");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let layout =
        LayoutMap::from_slice(5, &[PhysicalQubitId::new(1), PhysicalQubitId::new(2)]).unwrap();
    let frontier_config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, frontier_config).unwrap();

    let heuristic_config = SabreHeuristicConfig::new();
    let heuristic = SabreHeuristic::from_coupling_graph(&coupling, heuristic_config).unwrap();

    let cands = CandidateGenerator::generate_candidates_with_coupling(
        state.front_layer(),
        &coupling,
        heuristic.distances(),
    );

    // Candidates:
    // From p1: (0, 1)
    // From p2: (0, 2)
    assert_eq!(cands.len(), 2);
    assert_eq!(
        cands[0],
        CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );
    assert_eq!(
        cands[1],
        CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(2))
    );

    let score_01 = heuristic.score_candidate(cands[0], &state).unwrap();
    let score_02 = heuristic.score_candidate(cands[1], &state).unwrap();

    // Symmetrically, swapping 0 and 1 moves q0 to center 0 (distance to q1 at p2 is now 1)
    // Swapping 0 and 2 moves q1 to center 0 (distance to q0 at p1 is now 1)
    // Both produce identical total_score = 1.0!
    assert_eq!(score_01.total_score, score_02.total_score);

    // Deterministic tie-breaker must choose CandidateSwap(0, 1) because (0, 1) < (0, 2) lexicographically!
    assert_eq!(score_01.cmp(&score_02), std::cmp::Ordering::Less);
    assert_eq!(score_02.cmp(&score_01), std::cmp::Ordering::Greater);

    let ranked = vec![score_02.clone(), score_01.clone()];
    let mut sorted = ranked;
    sorted.sort_unstable_by(ScoreComponents::cmp_deterministic);
    assert_eq!(sorted[0].candidate, cands[0]); // (0, 1) wins deterministically
}

// ============================================================================
// 5. Disconnected Topology & Invalid Config Handling
// ============================================================================

#[test]
fn test_disconnected_topology_handling() {
    // Target with two disconnected components: {0, 1} and {2, 3}
    let mut coupling = CouplingGraph::new(4, Directionality::Undirected);
    coupling
        .add_edge(PhysicalQubitId::new(0), PhysicalQubitId::new(1))
        .unwrap();
    coupling
        .add_edge(PhysicalQubitId::new(1), PhysicalQubitId::new(0))
        .unwrap();
    coupling
        .add_edge(PhysicalQubitId::new(2), PhysicalQubitId::new(3))
        .unwrap();
    coupling
        .add_edge(PhysicalQubitId::new(3), PhysicalQubitId::new(2))
        .unwrap();

    let mut b = CircuitBuilder::new("disconnected");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap(); // Front layer
    let circuit = b.finish().unwrap();

    // Logical q0 is at physical 0, logical q1 is at physical 2 (different components!)
    let layout =
        LayoutMap::from_slice(4, &[PhysicalQubitId::new(0), PhysicalQubitId::new(2)]).unwrap();
    let state = SabreFrontierState::new(&circuit, layout, LookaheadConfig::default()).unwrap();

    let heuristic =
        SabreHeuristic::from_coupling_graph(&coupling, SabreHeuristicConfig::default()).unwrap();

    let cand_01 = CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1));
    let score = heuristic.score_candidate(cand_01, &state).unwrap();

    // After swapping 0 and 1: q0 is at p1, q1 is at p2.
    // p1 and p2 are still in disconnected components!
    // Disconnected penalty distance must be applied cleanly:
    let penalty = heuristic.distances().disconnected_penalty();
    assert_eq!(score.front_cost, penalty as f64);
    assert_eq!(score.total_score, penalty as f64);
}

#[test]
fn test_invalid_heuristic_config_rejected() {
    // Negative lookahead weight
    assert!(SabreHeuristicConfig::new()
        .with_lookahead_weight(-0.1)
        .is_err());
    // NaN lookahead weight
    assert!(SabreHeuristicConfig::new()
        .with_lookahead_weight(f64::NAN)
        .is_err());
    // Negative decay delta
    assert!(SabreHeuristicConfig::new().with_decay_delta(-1.0).is_err());
    // Infinite decay delta
    assert!(SabreHeuristicConfig::new()
        .with_decay_delta(f64::INFINITY)
        .is_err());
    // DecayTable invalid delta
    assert!(DecayTable::with_delta(5, -0.01).is_err());
}

// ============================================================================
// 6. Immutability & Repeatability
// ============================================================================

#[test]
fn test_scoring_does_not_mutate_frontier_state_or_layout() {
    let target = make_linear_target("line5", 5);

    let mut b = CircuitBuilder::new("immutability");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let state = SabreFrontierState::new(&circuit, layout, LookaheadConfig::default()).unwrap();

    let version_before = state.layout_version();
    let front_before = state.front_layer().clone();
    let lookahead_before = state.lookahead().clone();
    let layout_before = state.layout().clone();

    let heuristic = SabreHeuristic::new(&target, SabreHeuristicConfig::default()).unwrap();
    let cand = CandidateSwap::new(PhysicalQubitId::new(0), PhysicalQubitId::new(1));

    // Score candidate
    let _score = heuristic.score_candidate(cand, &state).unwrap();

    // Verify state was completely untouched
    assert_eq!(state.layout_version(), version_before);
    assert_eq!(state.front_layer(), &front_before);
    assert_eq!(state.lookahead(), &lookahead_before);
    assert_eq!(state.layout(), &layout_before);
}

#[test]
fn test_repeatability_identical_runs_produce_identical_scores() {
    let target = make_linear_target("line5", 5);

    let mut b = CircuitBuilder::new("repeatability");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 5).unwrap();
    let state = SabreFrontierState::new(&circuit, layout, LookaheadConfig::default()).unwrap();

    let heuristic = SabreHeuristic::new(&target, SabreHeuristicConfig::default()).unwrap();

    // 10 repeated rankings must be byte-for-byte identical
    let baseline = heuristic
        .evaluate_and_rank_candidates(&state, &target)
        .unwrap();

    for _ in 0..10 {
        let run = heuristic
            .evaluate_and_rank_candidates(&state, &target)
            .unwrap();
        assert_eq!(baseline, run);
    }
}
