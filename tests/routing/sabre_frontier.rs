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

use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::sabre::{LookaheadConfig, SabreFrontierState};
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
