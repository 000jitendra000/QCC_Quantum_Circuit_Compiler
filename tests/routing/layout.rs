//! Integration tests for Phase 44: Logical/Physical Layout Model.

use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::routing::{CircuitLayout, Layout, LayoutError, LogicalQubitId};
use qcc::target::model::Target;
use qcc::target::operation::PhysicalQubitId;

#[test]
fn test_identity_layouts_and_nontrivial_permutations() {
    // 1. Identity layout
    let id_layout = Layout::identity(4, 4).expect("failed to create identity layout");
    assert_eq!(id_layout.num_logical(), 4);
    assert_eq!(id_layout.num_physical(), 4);
    assert_eq!(id_layout.num_assigned(), 4);
    assert!(id_layout.is_bijective());
    assert!(id_layout.is_fully_assigned());

    for i in 0..4 {
        assert_eq!(
            id_layout.get_physical(QubitId(i)),
            Some(PhysicalQubitId(i as u32))
        );
        assert_eq!(
            id_layout.get_logical(PhysicalQubitId(i as u32)),
            Some(QubitId(i))
        );
    }
    assert!(id_layout.validate().is_ok());

    // 2. Nontrivial permutation: [2, 0, 3, 1]
    let perm = vec![2, 0, 3, 1];
    let perm_layout = Layout::from_permutation(&perm).expect("failed to create permutation layout");
    assert_eq!(perm_layout.num_logical(), 4);
    assert_eq!(perm_layout.num_physical(), 4);
    assert!(perm_layout.is_bijective());

    assert_eq!(
        perm_layout.get_physical(QubitId(0)),
        Some(PhysicalQubitId(2))
    );
    assert_eq!(
        perm_layout.get_physical(QubitId(1)),
        Some(PhysicalQubitId(0))
    );
    assert_eq!(
        perm_layout.get_physical(QubitId(2)),
        Some(PhysicalQubitId(3))
    );
    assert_eq!(
        perm_layout.get_physical(QubitId(3)),
        Some(PhysicalQubitId(1))
    );

    assert_eq!(
        perm_layout.get_logical(PhysicalQubitId(0)),
        Some(QubitId(1))
    );
    assert_eq!(
        perm_layout.get_logical(PhysicalQubitId(1)),
        Some(QubitId(3))
    );
    assert_eq!(
        perm_layout.get_logical(PhysicalQubitId(2)),
        Some(QubitId(0))
    );
    assert_eq!(
        perm_layout.get_logical(PhysicalQubitId(3)),
        Some(QubitId(2))
    );
    assert!(perm_layout.validate().is_ok());

    // 3. Permutation reached via successive physical SWAPs from identity
    let mut swapper = Layout::identity(4, 4).unwrap();
    // swap p[0] and p[2]: q[0]->p[2], q[2]->p[0]
    swapper
        .swap_physical(PhysicalQubitId(0), PhysicalQubitId(2))
        .unwrap();
    // swap p[1] and p[3]: q[1]->p[3], q[3]->p[1]
    swapper
        .swap_physical(PhysicalQubitId(1), PhysicalQubitId(3))
        .unwrap();
    // swap p[0] and p[3]: q[2]->p[3], q[1]->p[0]
    swapper
        .swap_physical(PhysicalQubitId(0), PhysicalQubitId(3))
        .unwrap();

    // Now: q[0]->p[2], q[1]->p[0], q[2]->p[3], q[3]->p[1] -> Exactly matches perm!
    assert_eq!(swapper, perm_layout);
}

#[test]
fn test_partial_mappings_where_logical_qubits_occupy_part_of_device() {
    // 3 logical qubits mapped onto a 7-qubit device
    let mut layout = Layout::new(3, 7);
    layout.assign(QubitId(0), PhysicalQubitId(1)).unwrap();
    layout.assign(QubitId(1), PhysicalQubitId(4)).unwrap();
    layout.assign(QubitId(2), PhysicalQubitId(5)).unwrap();

    assert_eq!(layout.num_logical(), 3);
    assert_eq!(layout.num_physical(), 7);
    assert_eq!(layout.num_assigned(), 3);
    assert_eq!(layout.num_unused_physical(), 4);
    assert!(layout.is_fully_assigned());
    assert!(!layout.is_bijective());

    assert_eq!(
        layout.assigned_logical_qubits(),
        vec![QubitId(0), QubitId(1), QubitId(2)]
    );
    assert_eq!(
        layout.assigned_physical_qubits(),
        vec![PhysicalQubitId(1), PhysicalQubitId(4), PhysicalQubitId(5)]
    );

    // Unused physical qubits: p[0], p[2], p[3], p[6]
    let idle = layout.unused_physical_qubits();
    assert_eq!(
        idle,
        vec![
            PhysicalQubitId(0),
            PhysicalQubitId(2),
            PhysicalQubitId(3),
            PhysicalQubitId(6)
        ]
    );

    assert!(layout.is_physical_idle(PhysicalQubitId(0)));
    assert!(!layout.is_physical_idle(PhysicalQubitId(1)));
    assert!(layout.is_physical_idle(PhysicalQubitId(2)));
    assert!(layout.is_physical_idle(PhysicalQubitId(3)));
    assert!(!layout.is_physical_idle(PhysicalQubitId(4)));
    assert!(!layout.is_physical_idle(PhysicalQubitId(5)));
    assert!(layout.is_physical_idle(PhysicalQubitId(6)));

    // Migration into idle ancilla: SWAP p[1] with idle p[2]
    layout
        .swap_physical(PhysicalQubitId(1), PhysicalQubitId(2))
        .unwrap();
    assert_eq!(layout.get_physical(QubitId(0)), Some(PhysicalQubitId(2)));
    assert!(layout.is_physical_idle(PhysicalQubitId(1)));
    assert!(!layout.is_physical_idle(PhysicalQubitId(2)));
    assert!(layout.validate().is_ok());
}

#[test]
fn test_forward_and_reverse_lookup_roundtrips() {
    let layout = Layout::from_mappings(
        3,
        5,
        [
            (QubitId(0), PhysicalQubitId(4)),
            (QubitId(1), PhysicalQubitId(2)),
            (QubitId(2), PhysicalQubitId(0)),
        ],
    )
    .expect("failed to construct mapping");

    // Logical -> Physical -> Logical
    for &q in &layout.assigned_logical_qubits() {
        let p = layout
            .get_physical(q)
            .expect("assigned logical must have physical");
        let roundtrip_q = layout
            .get_logical(p)
            .expect("assigned physical must have logical");
        assert_eq!(q, roundtrip_q);
    }

    // Physical -> Logical -> Physical
    for &p in &layout.assigned_physical_qubits() {
        let q = layout
            .get_logical(p)
            .expect("assigned physical must have logical");
        let roundtrip_p = layout
            .get_physical(q)
            .expect("assigned logical must have physical");
        assert_eq!(p, roundtrip_p);
    }

    // Unmapped queries
    assert_eq!(layout.get_physical(QubitId(99)), None);
    assert_eq!(layout.get_logical(PhysicalQubitId(1)), None);
    assert_eq!(layout.get_logical(PhysicalQubitId(3)), None);
}

#[test]
fn test_duplicate_physical_assignments_and_invalid_mappings() {
    let mut layout = Layout::new(3, 4);
    layout.assign(QubitId(0), PhysicalQubitId(2)).unwrap();

    // 1. Duplicate physical assignment
    let err1 = layout
        .assign(QubitId(1), PhysicalQubitId(2))
        .expect_err("should reject duplicate physical qubit");
    match err1 {
        LayoutError::DuplicatePhysicalAssignment {
            physical,
            existing_logical,
            new_logical,
        } => {
            assert_eq!(physical, PhysicalQubitId(2));
            assert_eq!(existing_logical, QubitId(0));
            assert_eq!(new_logical, QubitId(1));
        }
        other => panic!("expected DuplicatePhysicalAssignment, got {other:?}"),
    }

    // 2. Duplicate logical assignment
    let err2 = layout
        .assign(QubitId(0), PhysicalQubitId(3))
        .expect_err("should reject duplicate logical qubit");
    match err2 {
        LayoutError::DuplicateLogicalAssignment {
            logical,
            existing_physical,
            new_physical,
        } => {
            assert_eq!(logical, QubitId(0));
            assert_eq!(existing_physical, PhysicalQubitId(2));
            assert_eq!(new_physical, PhysicalQubitId(3));
        }
        other => panic!("expected DuplicateLogicalAssignment, got {other:?}"),
    }

    // 3. Logical qubit out of bounds
    let err3 = layout
        .assign(QubitId(5), PhysicalQubitId(0))
        .expect_err("should reject logical out of bounds");
    match err3 {
        LayoutError::LogicalQubitOutOfBounds { qubit, capacity } => {
            assert_eq!(qubit, QubitId(5));
            assert_eq!(capacity, 3);
        }
        other => panic!("expected LogicalQubitOutOfBounds, got {other:?}"),
    }

    // 4. Physical qubit out of bounds
    let err4 = layout
        .assign(QubitId(1), PhysicalQubitId(10))
        .expect_err("should reject physical out of bounds");
    match err4 {
        LayoutError::PhysicalQubitOutOfBounds { qubit, capacity } => {
            assert_eq!(qubit, PhysicalQubitId(10));
            assert_eq!(capacity, 4);
        }
        other => panic!("expected PhysicalQubitOutOfBounds, got {other:?}"),
    }
}

#[test]
fn test_serialization_roundtrips_and_invalid_serialized_layouts() {
    let layout = Layout::from_mappings(
        3,
        6,
        [
            (QubitId(0), PhysicalQubitId(5)),
            (QubitId(1), PhysicalQubitId(1)),
            (QubitId(2), PhysicalQubitId(3)),
        ],
    )
    .unwrap();

    // 1. Valid JSON round-trip
    let json = layout.to_json().expect("serialization must succeed");
    let recovered = Layout::from_json(&json).expect("deserialization must succeed");
    assert_eq!(layout, recovered);

    // 2. Reject JSON with duplicate physical assignment
    let corrupt_duplicate = r#"{
        "num_logical": 3,
        "num_physical": 6,
        "mappings": [[0, 2], [1, 2]]
    }"#;
    let err1 = Layout::from_json(corrupt_duplicate).expect_err("should reject duplicate physical");
    match err1 {
        LayoutError::DeserializationError(msg) => {
            assert!(msg.contains("duplicate") || msg.contains("occupied"));
        }
        other => panic!("expected DeserializationError, got {other:?}"),
    }

    // 3. Reject JSON with physical qubit exceeding capacity
    let corrupt_oob = r#"{
        "num_logical": 2,
        "num_physical": 3,
        "mappings": [[0, 0], [1, 99]]
    }"#;
    let err2 = Layout::from_json(corrupt_oob).expect_err("should reject out of bounds");
    match err2 {
        LayoutError::DeserializationError(msg) => {
            assert!(msg.contains("out of bounds"));
        }
        other => panic!("expected DeserializationError, got {other:?}"),
    }

    // 4. Reject malformed JSON
    let malformed = "{ this is not json }";
    assert!(Layout::from_json(malformed).is_err());
}

#[test]
fn test_layouts_with_different_logical_and_physical_capacities() {
    // 1. Logical > Physical is rejected for identity
    let err_capacity = Layout::identity(5, 3).expect_err("cannot fit 5 qubits on 3");
    match err_capacity {
        LayoutError::CapacityExceeded { required, capacity } => {
            assert_eq!(required, 5);
            assert_eq!(capacity, 3);
        }
        other => panic!("expected CapacityExceeded, got {other:?}"),
    }

    // 2. Physical > Logical is supported
    let partial = Layout::identity(2, 8).unwrap();
    assert_eq!(partial.num_logical(), 2);
    assert_eq!(partial.num_physical(), 8);
    assert_eq!(partial.num_assigned(), 2);
    assert_eq!(partial.num_unused_physical(), 6);

    // 3. CircuitLayout mismatch detection
    let l1 = Layout::identity(2, 4).unwrap();
    let l2 = Layout::identity(3, 4).unwrap();
    let err_mismatch = CircuitLayout::with_layouts(l1.clone(), l2, l1)
        .expect_err("should reject mismatched logical capacity");
    match err_mismatch {
        LayoutError::CapacityMismatch { expected, actual } => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 3);
        }
        other => panic!("expected CapacityMismatch, got {other:?}"),
    }
}

#[test]
fn test_logical_and_physical_identities_distinctness() {
    let q = QubitId(2);
    let p = PhysicalQubitId(2);
    let alias_q: LogicalQubitId = q;

    // Formatting is distinct
    assert_eq!(q.to_string(), "q[2]");
    assert_eq!(alias_q.to_string(), "q[2]");
    assert_eq!(p.to_string(), "p[2]");

    // Internal representations are distinct
    assert_eq!(q.0, 2usize);
    assert_eq!(p.0, 2u32);
    assert_eq!(p.index(), 2usize);

    // Type system prevents interchanging them directly
    let mut layout = Layout::new(3, 3);
    layout.assign(q, p).unwrap();
    assert_eq!(layout.get_physical(q), Some(p));

    // Trying to pass PhysicalQubitId to get_physical would be a compile error!
    // layout.get_physical(p); // does not compile!
}

#[test]
fn test_circuit_layout_tracking_across_routing_swaps() {
    // 3 logical qubits on 5 physical qubits
    let mut cl = CircuitLayout::identity(3, 5).expect("failed to create circuit layout");

    // L_initial: q[0]->p[0], q[1]->p[1], q[2]->p[2], idle: p[3], p[4]
    assert_eq!(
        cl.initial().get_physical(QubitId(0)),
        Some(PhysicalQubitId(0))
    );
    assert_eq!(
        cl.initial().get_physical(QubitId(1)),
        Some(PhysicalQubitId(1))
    );
    assert_eq!(
        cl.initial().get_physical(QubitId(2)),
        Some(PhysicalQubitId(2))
    );
    assert_eq!(
        cl.initial().unused_physical_qubits(),
        vec![PhysicalQubitId(3), PhysicalQubitId(4)]
    );

    // Apply router SWAP: SWAP(p[0], p[1])
    cl.apply_physical_swap(PhysicalQubitId(0), PhysicalQubitId(1))
        .unwrap();

    // Apply router SWAP: SWAP(p[1], p[3]) - moves logical qubit into idle ancilla!
    cl.apply_physical_swap(PhysicalQubitId(1), PhysicalQubitId(3))
        .unwrap();

    // Verify L_initial is strictly preserved
    assert_eq!(
        cl.initial().get_physical(QubitId(0)),
        Some(PhysicalQubitId(0))
    );
    assert_eq!(
        cl.initial().get_physical(QubitId(1)),
        Some(PhysicalQubitId(1))
    );
    assert_eq!(
        cl.initial().get_physical(QubitId(2)),
        Some(PhysicalQubitId(2))
    );

    // Verify L_final correctly tracks the mutations:
    // q[0] was at p[0], swapped with p[1] -> at p[1], then swapped with p[3] -> at p[3]!
    assert_eq!(
        cl.final_layout().get_physical(QubitId(0)),
        Some(PhysicalQubitId(3))
    );
    // q[1] was at p[1], swapped with p[0] -> at p[0]!
    assert_eq!(
        cl.final_layout().get_physical(QubitId(1)),
        Some(PhysicalQubitId(0))
    );
    // q[2] untouched at p[2]!
    assert_eq!(
        cl.final_layout().get_physical(QubitId(2)),
        Some(PhysicalQubitId(2))
    );

    // Unused physical qubits in final layout: p[1] and p[4]
    assert_eq!(
        cl.final_layout().unused_physical_qubits(),
        vec![PhysicalQubitId(1), PhysicalQubitId(4)]
    );

    // Serialization roundtrip of CircuitLayout
    let json = cl.to_json().unwrap();
    let recovered = CircuitLayout::from_json(&json).unwrap();
    assert_eq!(cl, recovered);
}

#[test]
fn test_target_and_circuit_integration() {
    let target = Target::new("test_coupling", 5);

    let mut builder = CircuitBuilder::new("test_circ");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let cl = CircuitLayout::identity_for_circuit(&circuit, &target).unwrap();
    assert_eq!(cl.initial().num_logical(), 3);
    assert_eq!(cl.initial().num_physical(), 5);
    assert_eq!(cl.initial().num_assigned(), 3);
    assert_eq!(cl.initial().num_unused_physical(), 2);
    assert!(cl.validate().is_ok());
}
