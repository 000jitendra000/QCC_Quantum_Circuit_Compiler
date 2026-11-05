//! Hand-calculated DAG fixtures and integration tests for SABRE Frontier Analysis (Phase 50).
//!
//! Validates:
//! - Linear dependency chains in front layer and lookahead sets.
//! - Independent gates all entering the front layer.
//! - Topologically later operations blocked by dependencies staying out of the front layer.
//! - Multi-qubit DAG patterns with transparent single-qubit gate advancement.
//! - Logical-to-physical operand resolution under non-identity layout mappings.
//! - Anti-stale layout mutation updates across front layer and lookahead sets.
//! - Lookahead budget ($W$) and max-depth truncation constraints.
//! - Direct physical target executability detection on front-layer gates.

use std::collections::BTreeSet;

use qcc::builder::CircuitBuilder;
use qcc::ir::node::OperationId;
use qcc::ir::wire::QubitId;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::sabre::{LookaheadConfig, LookaheadSet, SabreFrontier, SabreFrontierState};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

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
// 1. Linear Dependencies Tests
// ============================================================================

#[test]
fn test_linear_dependencies_front_layer_and_lookahead() {
    // Circuit on 2 qubits: CX(0, 1) -> CX(0, 1) -> CX(0, 1)
    let mut b = CircuitBuilder::new("linear_cx");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap(); // op#1
    b.cx(q[0], q[1]).unwrap(); // op#2
    b.cx(q[0], q[1]).unwrap(); // op#3
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(2, 2).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // 1. Initial State:
    // Only op#1 has in-degree 0 => Front Layer = [op#1]
    assert_eq!(state.front_layer().len(), 1);
    let f0 = &state.front_layer().operations()[0];
    assert_eq!(f0.logical_operands(), (QubitId(0), QubitId(1)));
    assert_eq!(
        f0.physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );

    // Lookahead set must contain op#2 (layer 1) and op#3 (layer 2)
    assert_eq!(state.lookahead().len(), 2);
    assert_eq!(state.lookahead().operations()[0].layer, 1);
    assert_eq!(state.lookahead().operations()[1].layer, 2);

    // 2. Retire op#1:
    let op1_id = f0.id;
    state.retire_operation(&circuit, op1_id).unwrap();

    // Front Layer is now [op#2]
    assert_eq!(state.front_layer().len(), 1);
    let f1 = &state.front_layer().operations()[0];
    assert_ne!(f1.id, op1_id);

    // Lookahead set is now [op#3] (now at layer 1 relative to new front layer)
    assert_eq!(state.lookahead().len(), 1);
    assert_eq!(state.lookahead().operations()[0].layer, 1);

    // 3. Retire op#2:
    let op2_id = f1.id;
    state.retire_operation(&circuit, op2_id).unwrap();

    // Front Layer is now [op#3], lookahead is empty
    assert_eq!(state.front_layer().len(), 1);
    let f2 = &state.front_layer().operations()[0];
    assert_eq!(state.lookahead().len(), 0);

    // 4. Retire op#3:
    let op3_id = f2.id;
    state.retire_operation(&circuit, op3_id).unwrap();

    assert!(state.front_layer().is_empty());
    assert!(state.lookahead().is_empty());
    assert!(state.is_fully_completed());
}

// ============================================================================
// 2. Independent Gates Tests
// ============================================================================

#[test]
fn test_independent_gates_all_enter_front_layer() {
    // 4 qubits:
    // CX(0, 1) [op#1] and CX(2, 3) [op#2] are completely independent (disjoint wires).
    // CX(0, 1) [op#3] depends on op#1.
    // CX(2, 3) [op#4] depends on op#2.
    let mut b = CircuitBuilder::new("independent_cx");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap(); // op#1
    b.cx(q[2], q[3]).unwrap(); // op#2
    b.cx(q[0], q[1]).unwrap(); // op#3
    b.cx(q[2], q[3]).unwrap(); // op#4
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Both op#1 and op#2 have in-degree 0, so both MUST be in the front layer!
    assert_eq!(state.front_layer().len(), 2);
    let front_ids: Vec<_> = state
        .front_layer()
        .operations()
        .iter()
        .map(|op| op.id)
        .collect();

    // Lookahead contains op#3 and op#4
    assert_eq!(state.lookahead().len(), 2);

    // Retire op#1: op#3 becomes ready and enters front layer alongside op#2
    let op1_id = front_ids[0];
    state.retire_operation(&circuit, op1_id).unwrap();

    assert_eq!(state.front_layer().len(), 2);
    assert!(state.front_layer().contains(front_ids[1])); // op#2 still present
    assert!(!state.front_layer().contains(op1_id));

    // Lookahead now contains only op#4
    assert_eq!(state.lookahead().len(), 1);
}

// ============================================================================
// 3. Topologically Later Non-Ready Operations
// ============================================================================

#[test]
fn test_topologically_later_operations_not_ready() {
    // 4 qubits:
    // CX(0, 1) [op#1]
    // CX(2, 3) [op#2]
    // CX(1, 2) [op#3] depends on BOTH op#1 (wire 1) AND op#2 (wire 2)!
    // CX(0, 3) [op#4] depends on BOTH op#1 (wire 0) AND op#2 (wire 3)!
    let mut b = CircuitBuilder::new("diamond_cx");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap(); // op#1
    b.cx(q[2], q[3]).unwrap(); // op#2
    b.cx(q[1], q[2]).unwrap(); // op#3
    b.cx(q[0], q[3]).unwrap(); // op#4
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Front layer must only contain op#1 and op#2
    assert_eq!(state.front_layer().len(), 2);
    let op1_id = state.front_layer().operations()[0].id;
    let op2_id = state.front_layer().operations()[1].id;

    // op#3 and op#4 are in lookahead, NOT in front layer
    assert_eq!(state.lookahead().len(), 2);
    let op3_id = state.lookahead().operations()[0].id;
    let op4_id = state.lookahead().operations()[1].id;

    assert!(!state.front_layer().contains(op3_id));
    assert!(!state.front_layer().contains(op4_id));

    // Retire ONLY op#1:
    // op#3 and op#4 still have unsatisfied in-degree on wire 2 and 3 from op#2!
    state.retire_operation(&circuit, op1_id).unwrap();

    // Front layer must have ONLY op#2! Neither op#3 nor op#4 can enter yet!
    assert_eq!(state.front_layer().len(), 1);
    assert_eq!(state.front_layer().operations()[0].id, op2_id);
    assert!(!state.front_layer().contains(op3_id));
    assert!(!state.front_layer().contains(op4_id));

    // Now retire op#2:
    // Now both op#3 and op#4 have all dependencies satisfied!
    state.retire_operation(&circuit, op2_id).unwrap();

    assert_eq!(state.front_layer().len(), 2);
    assert!(state.front_layer().contains(op3_id));
    assert!(state.front_layer().contains(op4_id));
    assert!(state.lookahead().is_empty());
}

// ============================================================================
// 4. Multi-Qubit DAG with Interleaved Single-Qubit Gates
// ============================================================================

#[test]
fn test_multi_qubit_dag_with_interleaved_single_qubit_gates() {
    // 3 qubits:
    // H(0) [1Q]
    // H(1) [1Q]
    // CX(0, 1) [2Q] depends on H(0), H(1)
    // Z(0) [1Q] depends on CX(0, 1)
    // CX(0, 2) [2Q] depends on Z(0)
    let mut b = CircuitBuilder::new("interleaved_1q_2q");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.h(q[1]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.z(q[0]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 3).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Initial state:
    // H(0) and H(1) were single-qubit gates and were transparently satisfied.
    // Therefore, Front Layer immediately contains CX(0, 1)!
    assert_eq!(state.front_layer().len(), 1);
    assert_eq!(
        state.front_layer().operations()[0].logical_operands(),
        (QubitId(0), QubitId(1))
    );

    // Lookahead set contains CX(0, 2), reached across the intervening Z(0) gate!
    assert_eq!(state.lookahead().len(), 1);
    assert_eq!(
        state.lookahead().operations()[0].logical_operands(),
        (QubitId(0), QubitId(2))
    );
}

// ============================================================================
// 5. Non-Identity Layout Mapping
// ============================================================================

#[test]
fn test_non_identity_layout_mapping() {
    let mut b = CircuitBuilder::new("layout_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    // Permutation layout: q0 -> p2, q1 -> p0, q2 -> p1
    let layout = LayoutMap::from_permutation(&[2, 0, 1]).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Front layer operation is CX(q0, q1):
    // Physical mapping: q0 -> p2, q1 -> p0 => (p2, p0)
    assert_eq!(state.front_layer().len(), 1);
    let f = &state.front_layer().operations()[0];
    assert_eq!(f.logical_operands(), (QubitId(0), QubitId(1)));
    assert_eq!(
        f.physical_operands(),
        (PhysicalQubitId::new(2), PhysicalQubitId::new(0))
    );

    // Lookahead operation is CX(q1, q2):
    // Physical mapping: q1 -> p0, q2 -> p1 => (p0, p1)
    assert_eq!(state.lookahead().len(), 1);
    let la = &state.lookahead().operations()[0];
    assert_eq!(la.logical_operands(), (QubitId(1), QubitId(2)));
    assert_eq!(
        la.physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );
}

// ============================================================================
// 6. Candidate Layout Mutation Updates & Anti-Stale Prevention
// ============================================================================

#[test]
fn test_layout_mutation_updates_front_and_lookahead_no_stale_mapping() {
    let mut b = CircuitBuilder::new("stale_prevention");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    assert_eq!(state.layout_version(), 1);

    // Initial mapping under identity:
    // Front: CX(q0, q1) => (p0, p1)
    // Lookahead: CX(q0, q2) => (p0, p2)
    assert_eq!(
        state.front_layer().operations()[0].physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );
    assert_eq!(
        state.lookahead().operations()[0].physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(2))
    );

    // Candidate SWAP: swap physical qubits 0 and 3
    // Logical q0 moves from p0 to p3
    state
        .apply_physical_swap(PhysicalQubitId::new(0), PhysicalQubitId::new(3))
        .unwrap();

    assert_eq!(state.layout_version(), 2);

    // Verify Front layer operands immediately updated to (p3, p1) — NO STALE (p0, p1)
    assert_eq!(
        state.front_layer().operations()[0].physical_operands(),
        (PhysicalQubitId::new(3), PhysicalQubitId::new(1))
    );

    // Verify Lookahead operands immediately updated to (p3, p2) — NO STALE (p0, p2)
    assert_eq!(
        state.lookahead().operations()[0].physical_operands(),
        (PhysicalQubitId::new(3), PhysicalQubitId::new(2))
    );

    // Another candidate SWAP: swap physical qubits 1 and 2
    // Logical q1 moves to p2, logical q2 moves to p1
    state
        .apply_physical_swap(PhysicalQubitId::new(1), PhysicalQubitId::new(2))
        .unwrap();

    assert_eq!(state.layout_version(), 3);

    // Front: CX(q0, q1) => (p3, p2)
    assert_eq!(
        state.front_layer().operations()[0].physical_operands(),
        (PhysicalQubitId::new(3), PhysicalQubitId::new(2))
    );
    // Lookahead: CX(q0, q2) => (p3, p1)
    assert_eq!(
        state.lookahead().operations()[0].physical_operands(),
        (PhysicalQubitId::new(3), PhysicalQubitId::new(1))
    );
}

// ============================================================================
// 7. Lookahead Budget (W) and Max Depth Truncation
// ============================================================================

#[test]
fn test_lookahead_budget_truncation() {
    // 2 qubits with 6 sequential CX gates: op#1 to op#6
    let mut b = CircuitBuilder::new("long_chain");
    let q = b.add_quantum_register("q", 2).unwrap();
    for _ in 0..6 {
        b.cx(q[0], q[1]).unwrap();
    }
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(2, 2).unwrap();

    // Budget W = 2
    let config = LookaheadConfig::with_max_size(2);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    assert_eq!(state.front_layer().len(), 1); // 1st gate is in front layer
                                              // Lookahead must be strictly truncated to max_size = 2
    assert_eq!(state.lookahead().len(), 2);
    assert_eq!(state.lookahead().operations()[0].layer, 1);
    assert_eq!(state.lookahead().operations()[1].layer, 2);
}

#[test]
fn test_lookahead_max_depth_truncation() {
    let mut b = CircuitBuilder::new("depth_chain");
    let q = b.add_quantum_register("q", 2).unwrap();
    for _ in 0..6 {
        b.cx(q[0], q[1]).unwrap();
    }
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(2, 2).unwrap();

    // Budget W = 10 (ample), but max_depth = 1
    let config = LookaheadConfig::with_max_size(10).with_max_depth(1);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    assert_eq!(state.front_layer().len(), 1);
    // Only Layer 1 gates are included; layer 2+ are truncated by max_depth
    assert_eq!(state.lookahead().len(), 1);
    assert_eq!(state.lookahead().operations()[0].layer, 1);
}

// ============================================================================
// 8. Direct Physical Executability Detection on Target
// ============================================================================

#[test]
fn test_target_executable_detection() {
    // Line target: 0 - 1 - 2 - 3 - 4
    let target = make_linear_target("line5", 5);

    // Two independent operations on disjoint qubit pairs:
    // CX(0, 1) is directly adjacent on target (0 - 1)
    // CX(2, 4) is distance-2 on target (not adjacent: 2 - 3 - 4)
    let mut b = CircuitBuilder::new("exec_check");
    let q = b.add_quantum_register("q", 5).unwrap();
    b.cx(q[0], q[1]).unwrap(); // op#1
    b.cx(q[2], q[4]).unwrap(); // op#2
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(5, 5).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Both gates are ready in the DAG (disjoint qubits):
    assert_eq!(state.front_layer().len(), 2);

    // But only CX(0, 1) is directly executable on the hardware target without SWAPs!
    let directly_executable = state.front_layer().executable_on_target(&target);
    assert_eq!(directly_executable.len(), 1);
    assert_eq!(
        directly_executable[0].logical_operands(),
        (QubitId(0), QubitId(1))
    );
}

// ============================================================================
// 9. Audit Item 1: Lookahead Traversal Semantics
// ============================================================================

#[test]
fn test_lookahead_does_not_mutate_real_frontier() {
    let mut b = CircuitBuilder::new("immutability_check");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.h(q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 3).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let frontier = SabreFrontier::new(&circuit).unwrap();

    // Snapshot frontier properties before lookahead build
    let exec_before: Vec<_> = frontier.executable_operations().copied().collect();
    let completed_before = frontier.completed_count();
    let remaining_before = frontier.remaining_operations_count();
    let in_degrees_before: Vec<_> = circuit
        .graph
        .deterministic_topological_order()
        .unwrap()
        .into_iter()
        .map(|id| (id, frontier.remaining_in_degree(id)))
        .collect();

    // Build extended lookahead set
    let lookahead = LookaheadSet::build(&frontier, &circuit, &layout, &config).unwrap();
    assert!(!lookahead.is_empty());

    // Verify frontier was NOT mutated in any way
    let exec_after: Vec<_> = frontier.executable_operations().copied().collect();
    assert_eq!(exec_before, exec_after);
    assert_eq!(completed_before, frontier.completed_count());
    assert_eq!(remaining_before, frontier.remaining_operations_count());
    for (id, deg) in in_degrees_before {
        assert_eq!(deg, frontier.remaining_in_degree(id));
    }
}

#[test]
fn test_lookahead_never_skips_unresolved_dependency_across_single_qubit_gates() {
    // 3 qubits:
    // op#1: CX(0, 1) [front layer]
    // op#2: CX(1, 2) [depends on op#1 on wire 1]
    // op#3: H(0)     [depends on op#1 on wire 0]
    // op#4: X(0)     [depends on op#3 on wire 0]
    // op#5: CX(0, 2) [depends on op#4 on wire 0 AND op#2 on wire 2!]
    let mut b = CircuitBuilder::new("no_skip_dep");
    let q = b.add_quantum_register("q", 3).unwrap();
    let op1 = b.cx(q[0], q[1]).unwrap();
    let op2 = b.cx(q[1], q[2]).unwrap();
    let _op3 = b.h(q[0]).unwrap();
    let _op4 = b.x(q[0]).unwrap();
    let op5 = b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 3).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    // Front layer contains only op#1
    assert_eq!(state.front_layer().len(), 1);
    assert_eq!(state.front_layer().operations()[0].id, op1);

    // Lookahead set must contain op#2 in Layer 1, and op#5 in Layer 2.
    // op#5 CANNOT appear in Layer 1, because its dependency on op#2 (wire 2)
    // was not satisfied by merely traversing through H(0) and X(0)!
    let lookahead = state.lookahead();
    assert_eq!(lookahead.len(), 2);

    let layer1_ops = lookahead.operations_in_layer(1);
    assert_eq!(layer1_ops.len(), 1);
    assert_eq!(layer1_ops[0].id, op2);

    let layer2_ops = lookahead.operations_in_layer(2);
    assert_eq!(layer2_ops.len(), 1);
    assert_eq!(layer2_ops[0].id, op5);

    assert_eq!(lookahead.get(op5).unwrap().layer, 2);
}

#[test]
fn test_front_layer_and_lookahead_are_strictly_disjoint() {
    let mut b = CircuitBuilder::new("disjoint_check");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[0], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    while !state.front_layer().is_empty() {
        let front_ids: BTreeSet<OperationId> = state
            .front_layer()
            .operations()
            .iter()
            .map(|op| op.id)
            .collect();
        let lookahead_ids: BTreeSet<OperationId> = state
            .lookahead()
            .operations()
            .iter()
            .map(|op| op.id)
            .collect();

        assert!(
            front_ids.is_disjoint(&lookahead_ids),
            "Front layer and lookahead set must be strictly disjoint! Overlap: {:?}",
            front_ids.intersection(&lookahead_ids).collect::<Vec<_>>()
        );

        let next_op = state.front_layer().operations()[0].id;
        state.retire_operation(&circuit, next_op).unwrap();
    }
}

#[test]
fn test_lookahead_limits_determinism() {
    let mut b = CircuitBuilder::new("determinism_check");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap(); // Front layer
    b.cx(q[0], q[2]).unwrap(); // Branch 1
    b.cx(q[1], q[3]).unwrap(); // Branch 2
    b.cx(q[2], q[3]).unwrap(); // Downstream
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(2); // Restrictive budget

    // Repeated builds from clean state must produce identical lookahead sets
    let state1 = SabreFrontierState::new(&circuit, layout.clone(), config.clone()).unwrap();
    let state2 = SabreFrontierState::new(&circuit, layout, config).unwrap();

    let ops1: Vec<_> = state1
        .lookahead()
        .operations()
        .iter()
        .map(|op| (op.id, op.layer))
        .collect();
    let ops2: Vec<_> = state2
        .lookahead()
        .operations()
        .iter()
        .map(|op| (op.id, op.layer))
        .collect();

    assert_eq!(ops1, ops2);
    assert_eq!(ops1.len(), 2);
}

// ============================================================================
// 10. Audit Item 2: Frontier Dependency Progression
// ============================================================================

#[test]
fn test_frontier_mark_executed_and_dependency_progression() {
    let mut b = CircuitBuilder::new("mark_executed_progression");
    let q = b.add_quantum_register("q", 3).unwrap();
    let op1 = b.cx(q[0], q[1]).unwrap();
    let op2 = b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let mut frontier = SabreFrontier::new(&circuit).unwrap();

    assert!(frontier.is_executable(op1));
    assert!(!frontier.is_executable(op2));
    assert_eq!(frontier.remaining_in_degree(op2), Some(1));

    // mark_executed alias works identically to advance
    let unblocked = frontier.mark_executed(&circuit, op1).unwrap();
    assert_eq!(unblocked, vec![op2]);

    assert!(frontier.is_completed(op1));
    assert!(frontier.is_executable(op2));
    assert_eq!(frontier.remaining_in_degree(op2), Some(0));

    let unblocked2 = frontier.mark_executed(&circuit, op2).unwrap();
    assert!(unblocked2.is_empty());
    assert!(frontier.is_fully_completed());
}

#[test]
fn test_frontier_classical_raw_war_waw_dependencies() {
    let mut b = CircuitBuilder::new("classical_hazards");
    let q = b.add_quantum_register("q", 2).unwrap();
    let c = b.add_classical_register("c", 1).unwrap();

    // Writer 1 (RAW source, WAW source)
    let w1 = b.measure(q[0], c[0]).unwrap();
    // Readers 1 & 2 (RAW from w1, WAR to w2)
    let r1 = b.classical("reader1", &[c[0]], &[]).unwrap();
    let r2 = b.classical("reader2", &[c[0]], &[]).unwrap();
    // Writer 2 (WAW from w1, WAR from r1 and r2)
    let w2 = b.measure(q[1], c[0]).unwrap();
    let circuit = b.finish().unwrap();

    let mut frontier = SabreFrontier::new(&circuit).unwrap();

    // Initially: only w1 has in-degree 0
    assert!(frontier.is_executable(w1));
    assert!(!frontier.is_executable(r1));
    assert!(!frontier.is_executable(r2));
    assert!(!frontier.is_executable(w2));
    assert_eq!(frontier.remaining_in_degree(r1), Some(1));
    assert_eq!(frontier.remaining_in_degree(r2), Some(1));
    assert_eq!(frontier.remaining_in_degree(w2), Some(3)); // WAW from w1 + WAR from r1 + WAR from r2

    // Advance w1: satisfies RAW to r1 and r2; decrements w2 WAW dependency
    let unblocked = frontier.advance(&circuit, w1).unwrap();
    assert_eq!(unblocked, vec![r1, r2]);
    assert!(frontier.is_executable(r1));
    assert!(frontier.is_executable(r2));
    assert!(!frontier.is_executable(w2));
    assert_eq!(frontier.remaining_in_degree(w2), Some(2)); // remaining: WAR from r1, WAR from r2

    // Advance r1: w2 WAR dependency decrements, but still waiting on r2
    let unblocked_r1 = frontier.advance(&circuit, r1).unwrap();
    assert!(unblocked_r1.is_empty());
    assert!(!frontier.is_executable(w2));
    assert_eq!(frontier.remaining_in_degree(w2), Some(1));

    // Advance r2: w2 final WAR dependency satisfied => w2 becomes executable
    let unblocked_r2 = frontier.advance(&circuit, r2).unwrap();
    assert_eq!(unblocked_r2, vec![w2]);
    assert!(frontier.is_executable(w2));

    frontier.advance(&circuit, w2).unwrap();
    assert!(frontier.is_fully_completed());
}

#[test]
fn test_frontier_cannot_execute_twice_and_invalid_transitions() {
    let mut b = CircuitBuilder::new("invalid_transitions");
    let q = b.add_quantum_register("q", 2).unwrap();
    let op1 = b.h(q[0]).unwrap();
    let op2 = b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let mut frontier = SabreFrontier::new(&circuit).unwrap();

    // 1. Advance op#1 normally
    frontier.advance(&circuit, op1).unwrap();
    assert!(frontier.is_completed(op1));

    // 2. Attempt to advance op#1 a second time: must fail cleanly
    let err_twice = frontier.advance(&circuit, op1);
    assert!(err_twice.is_err());
    // op2 in-degree must NOT be decremented again
    assert_eq!(frontier.remaining_in_degree(op2), Some(0));

    // 3. Attempt to advance non-existent operation ID
    let err_unknown = frontier.advance(&circuit, OperationId(9999));
    assert!(err_unknown.is_err());

    // 4. In a new frontier, attempt to advance op2 before op1 is ready
    let mut fresh_frontier = SabreFrontier::new(&circuit).unwrap();
    let err_not_ready = fresh_frontier.advance(&circuit, op2);
    assert!(err_not_ready.is_err());
    assert!(!fresh_frontier.is_completed(op2));
    assert_eq!(fresh_frontier.remaining_in_degree(op2), Some(1));
}

// ============================================================================
// 11. Audit Item 3: Layout-Change Invalidation
// ============================================================================

#[test]
fn test_nontrivial_physical_swap_updates_front_and_lookahead() {
    // 4 qubits:
    // op#1: CX(0, 1) -> Front Layer
    // op#2: CX(1, 2) -> Lookahead Layer 1
    // op#3: CX(2, 3) -> Lookahead Layer 2
    let mut b = CircuitBuilder::new("swap_invalidation");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(4, 4).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    assert_eq!(state.layout_version(), 1);

    // Initial identity mapping (q_i -> p_i):
    // Front: CX(q0, q1) -> (p0, p1)
    let f_before = &state.front_layer().operations()[0];
    assert_eq!(f_before.logical_operands(), (QubitId(0), QubitId(1)));
    assert_eq!(
        f_before.physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1))
    );

    // Lookahead:
    // op#2: CX(q1, q2) -> (p1, p2)
    // op#3: CX(q2, q3) -> (p2, p3)
    let la_ops_before = state.lookahead().operations();
    assert_eq!(
        la_ops_before[0].physical_operands(),
        (PhysicalQubitId::new(1), PhysicalQubitId::new(2))
    );
    assert_eq!(
        la_ops_before[1].physical_operands(),
        (PhysicalQubitId::new(2), PhysicalQubitId::new(3))
    );

    // Apply nontrivial physical SWAP between physical qubits 1 and 3:
    // Logical q1 (formerly at p1) moves to p3
    // Logical q3 (formerly at p3) moves to p1
    // Logical q0 (at p0) and q2 (at p2) are unchanged.
    state
        .apply_physical_swap(PhysicalQubitId::new(1), PhysicalQubitId::new(3))
        .unwrap();

    assert_eq!(state.layout_version(), 2);

    // Verify Front Layer immediately reflects new layout:
    // CX(q0, q1) must now map to (p0, p3)
    let f_after = &state.front_layer().operations()[0];
    assert_eq!(
        f_after.physical_operands(),
        (PhysicalQubitId::new(0), PhysicalQubitId::new(3))
    );

    // Verify Lookahead set immediately reflects new layout:
    // op#2: CX(q1, q2) must now map to (p3, p2)
    // op#3: CX(q2, q3) must now map to (p2, p1)
    let la_ops_after = state.lookahead().operations();
    assert_eq!(
        la_ops_after[0].physical_operands(),
        (PhysicalQubitId::new(3), PhysicalQubitId::new(2))
    );
    assert_eq!(
        la_ops_after[1].physical_operands(),
        (PhysicalQubitId::new(2), PhysicalQubitId::new(1))
    );
}

#[test]
fn test_failed_layout_mutation_leaves_state_unmodified() {
    let mut b = CircuitBuilder::new("failed_mutation");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = LayoutMap::identity(3, 3).unwrap();
    let config = LookaheadConfig::with_max_size(10);
    let mut state = SabreFrontierState::new(&circuit, layout, config).unwrap();

    let version_before = state.layout_version();
    let front_before = state.front_layer().clone();
    let lookahead_before = state.lookahead().clone();
    let layout_before = state.layout().clone();

    // 1. Attempt out-of-bounds physical swap (p0 <-> p99)
    let err_swap = state.apply_physical_swap(PhysicalQubitId::new(0), PhysicalQubitId::new(99));
    assert!(err_swap.is_err());

    // Verify state was NOT partially modified:
    assert_eq!(state.layout_version(), version_before);
    assert_eq!(state.front_layer(), &front_before);
    assert_eq!(state.lookahead(), &lookahead_before);
    assert_eq!(state.layout(), &layout_before);

    // 2. Attempt update_layout with invalid/partial mapping missing qubit 2
    let partial_layout =
        LayoutMap::from_slice(2, &[PhysicalQubitId::new(0), PhysicalQubitId::new(1)]).unwrap();
    let err_update = state.update_layout(partial_layout);
    assert!(err_update.is_err());

    // Verify state remains 100% pristine
    assert_eq!(state.layout_version(), version_before);
    assert_eq!(state.front_layer(), &front_before);
    assert_eq!(state.lookahead(), &lookahead_before);
    assert_eq!(state.layout(), &layout_before);
}
