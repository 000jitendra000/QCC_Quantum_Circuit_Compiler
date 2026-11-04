//! Integration tests for Phase 46: Initial Placement Algorithms.
//!
//! Validates:
//! 1. Logical interaction graph construction and edge weight tracking.
//! 2. Topology distance metrics across Line, Ring, Grid, Star, and Irregular topologies.
//! 3. Identity placement and capacity validation.
//! 4. Random-seeded placement reproducibility.
//! 5. Degree-based placement matching interaction hubs to topology hubs.
//! 6. Interaction-weighted placement directly minimizing weighted physical distances.
//! 7. Deterministic placement scoring and best-of selection.
//! 8. Injective layout properties and ADR-0001 unused physical qubit tracking.

use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::routing::placement::{
    place_best_of, place_circuit, place_circuit_with_coupling, InteractionGraph, PlacementError,
    PlacementStrategy, TopologyDistances,
};
use qcc::target::connectivity::{CouplingGraph, Directionality};
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

// =========================================================================
// 1. Logical Interaction Graph Tests
// =========================================================================

#[test]
fn test_interaction_graph_construction_and_weights() {
    let mut b = CircuitBuilder::new("interactions_test");
    let q = b.add_quantum_register("q", 4).unwrap();

    // 3 interactions between q[0] and q[1]
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[0]).unwrap();

    // 1 interaction between q[1] and q[2]
    b.cz(q[1], q[2]).unwrap();

    // 2 interactions between q[2] and q[3]
    b.swap(q[2], q[3]).unwrap();
    b.swap(q[2], q[3]).unwrap();

    // Single-qubit gates (should not create interaction edges)
    b.h(q[0]).unwrap();
    b.x(q[3]).unwrap();

    let circuit = b.finish().unwrap();
    let graph = InteractionGraph::from_circuit(&circuit);

    assert_eq!(graph.num_qubits(), 4);
    assert_eq!(graph.edge_weight(QubitId(0), QubitId(1)), 3);
    assert_eq!(graph.edge_weight(QubitId(1), QubitId(2)), 1);
    assert_eq!(graph.edge_weight(QubitId(2), QubitId(3)), 2);
    assert_eq!(graph.edge_weight(QubitId(0), QubitId(3)), 0);

    assert_eq!(graph.total_interactions(), 6);
    assert_eq!(graph.edges().len(), 3);

    // Degree queries
    assert_eq!(graph.unweighted_degree(QubitId(1)), 2); // connected to q[0] and q[2]
    assert_eq!(graph.weighted_degree(QubitId(1)), 4); // 3 + 1
    assert_eq!(graph.unweighted_degree(QubitId(0)), 1);
    assert_eq!(graph.weighted_degree(QubitId(0)), 3);

    // Sorted edges
    let sorted_edges = graph.sorted_edges_by_weight_descending();
    assert_eq!(sorted_edges[0], ((QubitId(0), QubitId(1)), 3));
    assert_eq!(sorted_edges[1], ((QubitId(2), QubitId(3)), 2));
    assert_eq!(sorted_edges[2], ((QubitId(1), QubitId(2)), 1));
}

// =========================================================================
// 2. Topology Distance Tests Across Required Topologies
// =========================================================================

#[test]
fn test_topology_distances_line() {
    let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);
    let dist = TopologyDistances::from_coupling_graph(&coupling);

    assert!(dist.is_connected());
    assert_eq!(dist.diameter(), 3);

    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
        Some(1)
    );
    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(3)),
        Some(3)
    );
    assert_eq!(
        dist.distance(PhysicalQubitId::new(1), PhysicalQubitId::new(2)),
        Some(1)
    );
}

#[test]
fn test_topology_distances_ring() {
    let coupling = CouplingGraph::ring(5, Directionality::Undirected);
    let dist = TopologyDistances::from_coupling_graph(&coupling);

    assert!(dist.is_connected());
    assert_eq!(dist.diameter(), 2);

    // 0 is directly adjacent to 4 in a ring of 5
    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(4)),
        Some(1)
    );
    // Shortest path between 0 and 2 is 2 (0-1-2)
    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(2)),
        Some(2)
    );
}

#[test]
fn test_topology_distances_grid() {
    let coupling = CouplingGraph::grid_2d(2, 3, Directionality::Undirected);
    let dist = TopologyDistances::from_coupling_graph(&coupling);

    assert!(dist.is_connected());
    assert_eq!(dist.num_qubits(), 6);
    // Corners (0,0) and (1,2) have Manhattan distance = 1 + 2 = 3
    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(5)),
        Some(3)
    );
    // Internal node (row 0, col 1) has index 1, connected to 0, 2, 4
    assert_eq!(dist.degree(PhysicalQubitId::new(1)), 3);
}

#[test]
fn test_topology_distances_star() {
    let coupling = CouplingGraph::star(5, 0, Directionality::Undirected);
    let dist = TopologyDistances::from_coupling_graph(&coupling);

    assert!(dist.is_connected());
    assert_eq!(dist.degree(PhysicalQubitId::new(0)), 4); // Hub has degree 4
    assert_eq!(dist.degree(PhysicalQubitId::new(1)), 1); // Satellites have degree 1

    // Distance between any two satellites via hub is 2
    assert_eq!(
        dist.distance(PhysicalQubitId::new(1), PhysicalQubitId::new(2)),
        Some(2)
    );
    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
        Some(1)
    );
}

#[test]
fn test_topology_distances_irregular() {
    // Irregular T-shaped topology:
    // 0 - 1 - 2
    //     |
    //     3
    //     |
    //     4
    let mut coupling = CouplingGraph::new(5, Directionality::Undirected);
    coupling.add_edge(0u32, 1u32).unwrap();
    coupling.add_edge(1u32, 2u32).unwrap();
    coupling.add_edge(1u32, 3u32).unwrap();
    coupling.add_edge(3u32, 4u32).unwrap();

    let dist = TopologyDistances::from_coupling_graph(&coupling);
    assert!(dist.is_connected());
    assert_eq!(dist.degree(PhysicalQubitId::new(1)), 3); // Hub
    assert_eq!(dist.degree(PhysicalQubitId::new(3)), 2);
    assert_eq!(dist.degree(PhysicalQubitId::new(0)), 1);
    assert_eq!(dist.degree(PhysicalQubitId::new(2)), 1);
    assert_eq!(dist.degree(PhysicalQubitId::new(4)), 1);

    assert_eq!(
        dist.distance(PhysicalQubitId::new(0), PhysicalQubitId::new(4)),
        Some(3)
    ); // 0-1-3-4
}

// =========================================================================
// 3. Identity Placement Tests
// =========================================================================

#[test]
fn test_identity_placement_valid() {
    let mut b = CircuitBuilder::new("id_circ");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let coupling = CouplingGraph::linear_chain(5, Directionality::Undirected);
    let result =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Identity).unwrap();

    assert_eq!(result.strategy, PlacementStrategy::Identity);
    assert_eq!(result.logical_qubits, 3);
    assert_eq!(result.physical_qubits, 5);

    assert_eq!(
        result.layout.get_physical(QubitId(0)),
        Some(PhysicalQubitId::new(0))
    );
    assert_eq!(
        result.layout.get_physical(QubitId(1)),
        Some(PhysicalQubitId::new(1))
    );
    assert_eq!(
        result.layout.get_physical(QubitId(2)),
        Some(PhysicalQubitId::new(2))
    );

    // ADR-0001: unused physical qubits p[3], p[4]
    let unused = result.layout.unused_physical_qubits();
    assert_eq!(
        unused,
        vec![PhysicalQubitId::new(3), PhysicalQubitId::new(4)]
    );
}

#[test]
fn test_identity_placement_capacity_mismatch_fails() {
    let mut b = CircuitBuilder::new("big_circ");
    let q = b.add_quantum_register("q", 6).unwrap();
    b.x(q[0]).unwrap();
    let circuit = b.finish().unwrap();

    let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);
    let err =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Identity).unwrap_err();

    match err {
        PlacementError::InsufficientCapacity {
            required,
            available,
        } => {
            assert_eq!(required, 6);
            assert_eq!(available, 4);
        }
        other => panic!("expected InsufficientCapacity, got {other:?}"),
    }
}

// =========================================================================
// 4. Random-Seeded Placement Reproducibility Tests
// =========================================================================

#[test]
fn test_random_seeded_placement_reproducibility() {
    let mut b = CircuitBuilder::new("rand_circ");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let coupling = CouplingGraph::grid_2d(3, 3, Directionality::Undirected); // 9 physical qubits

    let seed = 0xDEAD_BEEF_CAFE_BABE;
    let res1 = place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Random { seed })
        .unwrap();

    let res2 = place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Random { seed })
        .unwrap();

    // Identical seed must produce the exact same layout mapping and score
    for i in 0..4 {
        assert_eq!(
            res1.layout.get_physical(QubitId(i)),
            res2.layout.get_physical(QubitId(i))
        );
    }
    assert_eq!(res1.score, res2.score);

    // Different seed should generally produce a different layout
    let res3 = place_circuit_with_coupling(
        &circuit,
        &coupling,
        PlacementStrategy::Random { seed: seed + 1 },
    )
    .unwrap();

    let mut differences = 0;
    for i in 0..4 {
        if res1.layout.get_physical(QubitId(i)) != res3.layout.get_physical(QubitId(i)) {
            differences += 1;
        }
    }
    assert!(
        differences > 0,
        "different seeds should produce distinct mappings"
    );
}

// =========================================================================
// 5. Degree-Based Placement Tests
// =========================================================================

#[test]
fn test_degree_based_placement_on_star_topology() {
    // Circuit where logical q[2] is the central interaction hub:
    // q[2] interacts with q[0], q[1], q[3], q[4]
    let mut b = CircuitBuilder::new("star_circuit");
    let q = b.add_quantum_register("q", 5).unwrap();
    b.cx(q[2], q[0]).unwrap();
    b.cx(q[2], q[1]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    b.cx(q[2], q[4]).unwrap();
    let circuit = b.finish().unwrap();

    // Target is a star topology with physical hub at p[3]
    let coupling = CouplingGraph::star(5, 3, Directionality::Undirected);

    let result =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Degree).unwrap();

    // The degree-based placer MUST map the logical hub q[2] directly to the physical hub p[3]!
    assert_eq!(
        result.layout.get_physical(QubitId(2)),
        Some(PhysicalQubitId::new(3)),
        "logical star hub q[2] must be placed on physical star hub p[3]"
    );

    // All interactions should be placed with physical distance 1 (perfect score!)
    assert_eq!(result.score.max_distance, 1);
    assert_eq!(result.score.weighted_distance_sum, 4);
    assert!(result.score.is_perfect());
}

// =========================================================================
// 6. Interaction-Weighted Placement Tests
// =========================================================================

#[test]
fn test_interaction_weighted_placement_optimizes_heavy_edges() {
    // Circuit on 4 qubits:
    // - q[0] and q[1] interact 100 times (super heavy edge!)
    // - q[1] and q[2] interact 10 times
    // - q[2] and q[3] interact 1 time
    let mut b = CircuitBuilder::new("weighted_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();

    for _ in 0..100 {
        b.cx(q[0], q[1]).unwrap();
    }
    for _ in 0..10 {
        b.cx(q[1], q[2]).unwrap();
    }
    b.cx(q[2], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    // Target is a linear chain: 0 - 1 - 2 - 3
    let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);

    let result =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Weighted).unwrap();

    let p0 = result.layout.get_physical(QubitId(0)).unwrap();
    let p1 = result.layout.get_physical(QubitId(1)).unwrap();
    let p2 = result.layout.get_physical(QubitId(2)).unwrap();
    let p3 = result.layout.get_physical(QubitId(3)).unwrap();

    let dist = TopologyDistances::from_coupling_graph(&coupling);

    // Heaviest pair (q[0], q[1]) MUST be at distance 1
    assert_eq!(
        dist.distance(p0, p1),
        Some(1),
        "heaviest interacting pair (100 cx) must be placed at distance 1"
    );

    // Second heaviest pair (q[1], q[2]) should also be at distance 1
    assert_eq!(
        dist.distance(p1, p2),
        Some(1),
        "second heaviest pair (10 cx) must be placed at distance 1"
    );

    // Third pair (q[2], q[3]) should also be at distance 1
    assert_eq!(
        dist.distance(p2, p3),
        Some(1),
        "third pair (1 cx) must be placed at distance 1"
    );

    // Sequential chain in placement minimizes the total cost to 111!
    assert_eq!(result.score.weighted_distance_sum, 111);
    assert!(result.score.is_perfect());
}

#[test]
fn test_weighted_vs_identity_scoring_comparison() {
    // Circuit where logical indices are reverse of physical linear connectivity:
    // (q[0], q[3]) interact 50 times
    // (q[3], q[1]) interact 20 times
    let mut b = CircuitBuilder::new("cross_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    for _ in 0..50 {
        b.cx(q[0], q[3]).unwrap();
    }
    for _ in 0..20 {
        b.cx(q[3], q[1]).unwrap();
    }
    let circuit = b.finish().unwrap();

    let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);

    let res_identity =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Identity).unwrap();
    let res_weighted =
        place_circuit_with_coupling(&circuit, &coupling, PlacementStrategy::Weighted).unwrap();

    // Under identity: q[0] -> p[0], q[3] -> p[3] (distance 3).
    // Cost = 50 * 3 + 20 * 2 = 190.
    assert_eq!(res_identity.score.weighted_distance_sum, 190);

    // Under weighted: q[0] and q[3] are placed adjacent (distance 1).
    // Cost should be significantly lower!
    assert!(
        res_weighted.score.weighted_distance_sum < res_identity.score.weighted_distance_sum,
        "weighted placement must outperform identity placement on cross interactions"
    );
    assert!(res_weighted.score < res_identity.score);
}

// =========================================================================
// 7. Best-Of Selection Tests
// =========================================================================

#[test]
fn test_place_best_of_selection() {
    let mut b = CircuitBuilder::new("best_of_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let mut target = Target::new("chain_target", 4);
    let desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(
            qcc::target::connectivity::OperationConnectivity::undirected_two_qubit([
                (PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
                (PhysicalQubitId::new(1), PhysicalQubitId::new(2)),
                (PhysicalQubitId::new(2), PhysicalQubitId::new(3)),
            ]),
        )
        .build();
    target.add_operation(desc);

    let strategies = vec![
        PlacementStrategy::Identity,
        PlacementStrategy::Random { seed: 1 },
        PlacementStrategy::Random { seed: 42 },
        PlacementStrategy::Degree,
        PlacementStrategy::Weighted,
    ];

    let mut all_results = Vec::new();
    for &s in &strategies {
        let res = place_circuit(&circuit, &target, s).unwrap();
        all_results.push(res);
    }

    let min_score = all_results.iter().map(|r| r.score).min().unwrap();

    // Verify place_best_of selects a placement matching the minimum score
    let best = place_best_of(&circuit, &target, &strategies).unwrap();
    assert_eq!(best.score, min_score);
}

// =========================================================================
// 8. Injective Layout and ADR-0001 Compliance
// =========================================================================

#[test]
fn test_placement_produces_injective_layout_with_adr0001_ancillae() {
    let mut b = CircuitBuilder::new("ancilla_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    // 8 physical qubits
    let coupling = CouplingGraph::linear_chain(8, Directionality::Undirected);

    let strategies = [
        PlacementStrategy::Identity,
        PlacementStrategy::Random { seed: 12345 },
        PlacementStrategy::Degree,
        PlacementStrategy::Weighted,
    ];

    for s in strategies {
        let res = place_circuit_with_coupling(&circuit, &coupling, s).unwrap();
        let layout = &res.layout;

        // Injective forward and reverse lookups
        assert_eq!(layout.num_logical(), 3);
        assert_eq!(layout.num_physical(), 8);
        assert!(layout.is_fully_assigned());
        assert!(layout.validate().is_ok());

        // 5 unused physical qubits
        let unused = layout.unused_physical_qubits();
        assert_eq!(unused.len(), 5);

        for &p in &unused {
            assert!(layout.is_physical_idle(p));
            assert_eq!(layout.get_logical(p), None);
        }
    }
}

// =========================================================================
// 9. Edge Cases: Zero Qubits, Single Qubits, Zero Interactions
// =========================================================================

#[test]
fn test_placement_circuit_with_only_single_qubit_gates() {
    let mut b = CircuitBuilder::new("single_q_only");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.x(q[1]).unwrap();
    b.z(q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);

    for s in [
        PlacementStrategy::Identity,
        PlacementStrategy::Degree,
        PlacementStrategy::Weighted,
    ] {
        let res = place_circuit_with_coupling(&circuit, &coupling, s).unwrap();
        assert_eq!(res.logical_qubits, 3);
        assert_eq!(res.score.total_interactions, 0);
        assert_eq!(res.score.weighted_distance_sum, 0);
        assert!(res.score.is_perfect());
    }
}
