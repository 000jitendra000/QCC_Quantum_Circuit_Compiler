//! Integration and property tests for QccIR transactional DAG rewrite infrastructure.

use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::edge::{DependencyKind, WireDependency};
use qcc::ir::error::IrError;
use qcc::ir::node::OperationId;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::rewrite::{
    check_convexity, expand_operation, fuse_operations, insert_after, insert_at_end,
    insert_at_start, insert_before, remove_operation, replace_operation, replace_sequence,
    CircuitTransaction, RewriteError,
};
use qcc::ir::validation::validate_circuit;
use qcc::ir::wire::{ClassicalBitId, QubitId, WireId};

fn create_test_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("test_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q.qubit(0).unwrap()).unwrap();
    builder
        .cx(q.qubit(0).unwrap(), q.qubit(1).unwrap())
        .unwrap();
    builder.x(q.qubit(1).unwrap()).unwrap();
    builder.z(q.qubit(2).unwrap()).unwrap();
    builder
        .measure(q.qubit(0).unwrap(), c.bit(0).unwrap())
        .unwrap();

    builder.finish().unwrap()
}

#[test]
fn test_insert_after_quantum_wire() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    // Original order: op#1: H(0), op#2: CX(0, 1), op#3: X(1), op#4: Z(2), op#5: Measure(0, 0)
    let h_id = OperationId(1);
    let s_gate = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };

    let new_id = insert_after(&mut circuit, h_id, s_gate).expect("insertion should succeed");
    assert!(circuit.graph.contains_operation(new_id));
    assert_eq!(circuit.num_operations(), original.num_operations() + 1);

    // Verify provenance points to target
    let node = circuit.graph.get_node(new_id).expect("node exists");
    assert_eq!(node.provenance, vec![h_id]);

    // Verify dependencies on qubit 0: H(0) -> S(0) -> CX(0, 1)
    let preds = circuit.graph.predecessors(new_id);
    assert_eq!(preds.len(), 1);
    assert_eq!(preds[0].0, h_id);
    assert_eq!(preds[0].1.wire, WireId::Quantum(QubitId(0)));

    let succs = circuit.graph.successors(new_id);
    assert_eq!(succs.len(), 1);
    assert_eq!(succs[0].0, OperationId(2)); // CX(0, 1)

    // Verify circuit validation passes
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_insert_before_quantum_wire() {
    let mut circuit = create_test_circuit();
    let cx_id = OperationId(2); // CX(0, 1)
    let y_gate = Operation::Gate {
        gate_name: "y".to_string(),
        qubits: vec![QubitId(1)],
        parameters: Vec::new(),
    };

    let new_id = insert_before(&mut circuit, cx_id, y_gate).expect("insert_before should succeed");
    assert!(circuit.graph.contains_operation(new_id));

    // Y is on qubit 1 before CX(0, 1)
    // On qubit 1: Y(1) has no predecessors, and its successor is CX(0, 1)
    let succs = circuit.graph.successors(new_id);
    assert!(succs.iter().any(|(s, _)| *s == cx_id));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_insert_at_start_and_end() {
    let mut circuit = create_test_circuit();
    let orig_count = circuit.num_operations();

    let start_gate = Operation::Gate {
        gate_name: "x".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    let start_id = insert_at_start(&mut circuit, start_gate).unwrap();

    let end_gate = Operation::Gate {
        gate_name: "z".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    let end_id = insert_at_end(&mut circuit, end_gate).unwrap();

    assert_eq!(circuit.num_operations(), orig_count + 2);

    // Verify wire dependency placement:
    // start_gate is on q[0] before H(0) [id 1]
    let preds_start = circuit.graph.predecessors(start_id);
    assert!(
        preds_start.is_empty(),
        "start_id should have no predecessors on q[0]"
    );
    let succs_start = circuit.graph.successors(start_id);
    assert!(
        succs_start.iter().any(|(s, _)| *s == OperationId(1)),
        "start_id must precede H(0) on q[0]"
    );

    // end_gate is on q[0] after Measure(0, 0) [id 5]
    let preds_end = circuit.graph.predecessors(end_id);
    assert!(
        preds_end.iter().any(|(p, _)| *p == OperationId(5)),
        "end_id must succeed Measure(0, 0) on q[0]"
    );
    let succs_end = circuit.graph.successors(end_id);
    assert!(
        succs_end.is_empty(),
        "end_id should have no successors on q[0]"
    );

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_remove_operation_wire_reconnection() {
    let mut circuit = create_test_circuit();

    // Original: H(0) [id 1] -> CX(0, 1) [id 2] -> Measure(0, 0) [id 5] on wire q[0]
    // And on wire q[1]: CX(0, 1) [id 2] -> X(1) [id 3]
    let cx_id = OperationId(2);
    let removed_op = remove_operation(&mut circuit, cx_id).expect("remove should succeed");

    assert!(matches!(removed_op, Operation::Gate { ref gate_name, .. } if gate_name == "cx"));
    assert!(!circuit.graph.contains_operation(cx_id));

    // Verify wire reconnection:
    // On q[0], H(0) [id 1] should now directly connect to Measure(0, 0) [id 5]
    let succs_h = circuit.graph.successors(OperationId(1));
    assert!(
        succs_h
            .iter()
            .any(|(s, dep)| *s == OperationId(5) && dep.wire == WireId::Quantum(QubitId(0))),
        "H(0) should directly connect to Measure on q[0]"
    );

    // On q[1], X(1) [id 3] no longer has CX(0, 1) as predecessor
    let preds_x = circuit.graph.predecessors(OperationId(3));
    assert!(
        preds_x.is_empty(),
        "X(1) should have no predecessors on q[1]"
    );

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_remove_independent_operation_non_interference() {
    let mut circuit = create_test_circuit();

    // Z(2) is on qubit 2, completely independent of q[0] and q[1]
    let z_id = OperationId(4);
    let removed = remove_operation(&mut circuit, z_id).unwrap();
    assert!(matches!(removed, Operation::Gate { ref gate_name, .. } if gate_name == "z"));

    // Verify dependencies on q[0] and q[1] are completely unaffected
    let preds_cx = circuit.graph.predecessors(OperationId(2));
    assert_eq!(preds_cx.len(), 1);
    assert_eq!(preds_cx[0].0, OperationId(1)); // H(0) -> CX(0, 1)

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_replace_operation_preserves_position_and_records_provenance() {
    let mut circuit = create_test_circuit();

    let h_id = OperationId(1);
    let rx_gate = Operation::Gate {
        gate_name: "rx".to_string(),
        qubits: vec![QubitId(0)],
        parameters: vec![ParameterExpr::from(1.57)],
    };

    let new_id = replace_operation(&mut circuit, h_id, rx_gate).expect("replace should succeed");
    assert!(!circuit.graph.contains_operation(h_id));
    assert!(circuit.graph.contains_operation(new_id));

    let node = circuit.graph.get_node(new_id).unwrap();
    assert_eq!(node.provenance, vec![h_id]);

    // Position check: new node connects to CX(0, 1)
    let succs = circuit.graph.successors(new_id);
    assert!(succs.iter().any(|(s, _)| *s == OperationId(2)));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_expand_operation_1_to_n_decomposition() {
    let mut builder = CircuitBuilder::new("swap_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder
        .swap(q.qubit(0).unwrap(), q.qubit(1).unwrap())
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    let swap_id = OperationId(1);

    // Expand SWAP(0, 1) into 3 CX gates: CX(0, 1), CX(1, 0), CX(0, 1)
    let cx1 = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(0), QubitId(1)],
        parameters: Vec::new(),
    };
    let cx2 = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(1), QubitId(0)],
        parameters: Vec::new(),
    };
    let cx3 = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(0), QubitId(1)],
        parameters: Vec::new(),
    };

    let new_ids = expand_operation(&mut circuit, swap_id, vec![cx1, cx2, cx3])
        .expect("expansion should succeed");

    assert_eq!(new_ids.len(), 3);
    assert_eq!(circuit.num_operations(), 3);
    assert!(!circuit.graph.contains_operation(swap_id));

    for &id in &new_ids {
        let node = circuit.graph.get_node(id).unwrap();
        assert_eq!(node.provenance, vec![swap_id]);
    }

    // Verify dependency chaining between the 3 CX gates
    // new_ids[0] -> new_ids[1] -> new_ids[2]
    let succs_0 = circuit.graph.successors(new_ids[0]);
    assert!(succs_0.iter().any(|(s, _)| *s == new_ids[1]));

    let succs_1 = circuit.graph.successors(new_ids[1]);
    assert!(succs_1.iter().any(|(s, _)| *s == new_ids[2]));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_expand_operation_1_to_0_decomposition() {
    let mut circuit = create_test_circuit();
    let orig_count = circuit.num_operations();
    let z_id = OperationId(4);

    let new_ids =
        expand_operation(&mut circuit, z_id, Vec::new()).expect("1-to-0 expansion should succeed");
    assert!(new_ids.is_empty());
    assert_eq!(circuit.num_operations(), orig_count - 1);
    assert!(!circuit.graph.contains_operation(z_id));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_fuse_operations_convex_valid() {
    let mut builder = CircuitBuilder::new("fuse_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q.qubit(0).unwrap()).unwrap(); // op#1
    builder.x(q.qubit(0).unwrap()).unwrap(); // op#2
    builder
        .cx(q.qubit(0).unwrap(), q.qubit(1).unwrap())
        .unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();

    let targets = [OperationId(1), OperationId(2)];
    assert!(check_convexity(&circuit.graph, &targets).is_ok());

    let fused_gate = Operation::Gate {
        gate_name: "rx".to_string(),
        qubits: vec![QubitId(0)],
        parameters: vec![ParameterExpr::from(std::f64::consts::PI)],
    };

    let fused_id =
        fuse_operations(&mut circuit, &targets, fused_gate).expect("convex fusion should succeed");

    assert!(!circuit.graph.contains_operation(OperationId(1)));
    assert!(!circuit.graph.contains_operation(OperationId(2)));
    assert!(circuit.graph.contains_operation(fused_id));
    assert_eq!(circuit.num_operations(), 2);

    let node = circuit.graph.get_node(fused_id).unwrap();
    assert_eq!(node.provenance, targets.to_vec());

    // Fused node must precede CX(0, 1) [op#3]
    let succs = circuit.graph.successors(fused_id);
    assert!(succs.iter().any(|(s, _)| *s == OperationId(3)));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_fuse_operations_rejects_non_convex_set() {
    let mut builder = CircuitBuilder::new("non_convex_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q.qubit(0).unwrap()).unwrap(); // op#1
    builder
        .cx(q.qubit(0).unwrap(), q.qubit(1).unwrap())
        .unwrap(); // op#2 (intermediate)
    builder.x(q.qubit(0).unwrap()).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    // Targets: {op#1, op#3}. Intermediate op#2 lies on path 1 -> 2 -> 3 and is not in targets!
    let targets = [OperationId(1), OperationId(3)];

    // 1. check_convexity directly detects violation
    let conv_res = check_convexity(&circuit.graph, &targets);
    assert!(conv_res.is_err());
    match conv_res.unwrap_err() {
        RewriteError::InvalidFusion { reason } => {
            assert!(reason.contains("targets are not convex"));
            assert!(reason.contains("op#2"));
        }
        other => panic!("expected InvalidFusion, got {:?}", other),
    }

    // 2. fuse_operations rejects and rolls back completely
    let fused_op = Operation::Gate {
        gate_name: "z".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    let fuse_res = fuse_operations(&mut circuit, &targets, fused_op);
    assert!(fuse_res.is_err());

    // Verify 100% rollback: circuit must be strictly equal to original
    assert_eq!(circuit, original);
}

#[test]
fn test_transaction_rollback_on_out_of_bounds_qubit() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    let invalid_op = Operation::Gate {
        gate_name: "x".to_string(),
        qubits: vec![QubitId(99)], // total declared is 3 (0..2)
        parameters: Vec::new(),
    };

    let res = insert_after(&mut circuit, OperationId(1), invalid_op);
    assert!(res.is_err());
    match res.unwrap_err() {
        RewriteError::Ir(IrError::InvalidQubit { qubit, total }) => {
            assert_eq!(qubit, 99);
            assert_eq!(total, 3);
        }
        other => panic!("expected InvalidQubit, got {:?}", other),
    }

    // Complete rollback verification
    assert_eq!(circuit, original);
}

#[test]
fn test_transaction_rollback_on_missing_target_operation() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    let non_existent = OperationId(999);
    let op = Operation::Gate {
        gate_name: "h".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };

    let res = insert_after(&mut circuit, non_existent, op.clone());
    assert!(matches!(
        res.unwrap_err(),
        RewriteError::OperationNotFound { id } if id == non_existent
    ));
    assert_eq!(circuit, original);

    let res_rep = replace_operation(&mut circuit, non_existent, op.clone());
    assert!(matches!(
        res_rep.unwrap_err(),
        RewriteError::OperationNotFound { id } if id == non_existent
    ));
    assert_eq!(circuit, original);

    let res_rem = remove_operation(&mut circuit, non_existent);
    assert!(matches!(
        res_rem.unwrap_err(),
        RewriteError::OperationNotFound { id } if id == non_existent
    ));
    assert_eq!(circuit, original);
}

#[test]
fn test_transaction_rollback_on_duplicate_qubits() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    let invalid_cx = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(0), QubitId(0)], // duplicate operand
        parameters: Vec::new(),
    };

    let res = insert_after(&mut circuit, OperationId(1), invalid_cx);
    assert!(res.is_err());
    assert_eq!(circuit, original);
}

#[test]
fn test_transaction_multi_step_commit_and_rollback() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    // 1. Transaction dropped without commit -> rollback
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        let op = Operation::Gate {
            gate_name: "x".to_string(),
            qubits: vec![QubitId(0)],
            parameters: Vec::new(),
        };
        tx.insert_at_start(op).unwrap();
        assert_eq!(tx.circuit().num_operations(), original.num_operations() + 1);
        tx.rollback();
    }
    assert_eq!(circuit, original);

    // 2. Transaction with multiple mutations committed atomically
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        let h_id = OperationId(1);
        let s_op = Operation::Gate {
            gate_name: "s".to_string(),
            qubits: vec![QubitId(0)],
            parameters: Vec::new(),
        };
        let s_id = tx.insert_after(h_id, s_op).unwrap();

        // remove the original Z(2) [op#4]
        let z_id = OperationId(4);
        tx.remove_operation(z_id).unwrap();

        // adjust global phase
        tx.add_global_phase(ParameterExpr::from(0.5));

        let report = tx.commit().expect("transaction commit should succeed");
        assert_eq!(report.added, vec![s_id]);
        assert_eq!(report.removed, vec![z_id]);
    }

    assert_eq!(circuit.global_phase(), &ParameterExpr::from(0.5));
    assert!(!circuit.graph.contains_operation(OperationId(4)));
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_classical_wire_hazards_raw_war_waw() {
    let mut builder = CircuitBuilder::new("classical_hazard_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    // op#1: Measure q[0] -> c[0] (Write c[0])
    builder
        .measure(q.qubit(0).unwrap(), c.bit(0).unwrap())
        .unwrap();

    let mut circuit = builder.finish().unwrap();

    // Let's test adding classical read (RAW), classical write (WAW, WAR)
    // op#2: Classical op reading c[0] (RAW from op#1)
    let read_op = Operation::Classical {
        name: "test_read".to_string(),
        reads: vec![ClassicalBitId(0)],
        writes: Vec::new(),
    };
    let read_id = insert_at_end(&mut circuit, read_op).unwrap();

    // Verify RAW edge from op#1 -> op#2 on c[0]
    let preds_read = circuit.graph.predecessors(read_id);
    assert_eq!(preds_read.len(), 1);
    assert_eq!(preds_read[0].0, OperationId(1));
    assert_eq!(
        preds_read[0].1,
        WireDependency::new(
            WireId::Classical(ClassicalBitId(0)),
            DependencyKind::ClassicalRaw
        )
    );

    // op#3: Classical op writing c[0] (WAW from op#1, WAR from op#2)
    let write_op = Operation::Classical {
        name: "test_write".to_string(),
        reads: Vec::new(),
        writes: vec![ClassicalBitId(0)],
    };
    let write_id = insert_at_end(&mut circuit, write_op).unwrap();

    let preds_write = circuit.graph.predecessors(write_id);
    assert_eq!(preds_write.len(), 2);
    assert!(
        preds_write
            .iter()
            .any(|(p, dep)| *p == OperationId(1) && dep.kind == DependencyKind::ClassicalWaw),
        "WAW hazard from op#1 to op#3"
    );
    assert!(
        preds_write
            .iter()
            .any(|(p, dep)| *p == read_id && dep.kind == DependencyKind::ClassicalWar),
        "WAR hazard from op#2 to op#3"
    );

    assert!(validate_circuit(&circuit).is_ok());

    // Now remove op#2 (the reader) and verify hazards update cleanly:
    // WAW remains from op#1 to op#3, but WAR from op#2 is gone
    remove_operation(&mut circuit, read_id).unwrap();
    let preds_write_after = circuit.graph.predecessors(write_id);
    assert_eq!(preds_write_after.len(), 1);
    assert_eq!(preds_write_after[0].0, OperationId(1));
    assert_eq!(preds_write_after[0].1.kind, DependencyKind::ClassicalWaw);

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_barrier_boundary_dependency_preservation() {
    let mut builder = CircuitBuilder::new("barrier_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q.qubit(0).unwrap()).unwrap(); // op#1
    builder
        .barrier(&[q.qubit(0).unwrap(), q.qubit(1).unwrap()])
        .unwrap(); // op#2 (barrier)
    builder.x(q.qubit(0).unwrap()).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();

    // Verify initial barrier dependency
    let preds_3 = circuit.graph.predecessors(OperationId(3));
    assert_eq!(preds_3.len(), 1);
    assert_eq!(preds_3[0].0, OperationId(2));
    assert_eq!(preds_3[0].1.kind, DependencyKind::BarrierBoundary);

    // Insert gate before barrier
    let y_op = Operation::Gate {
        gate_name: "y".to_string(),
        qubits: vec![QubitId(1)],
        parameters: Vec::new(),
    };
    let y_id = insert_before(&mut circuit, OperationId(2), y_op).unwrap();

    // Barrier should now depend on y_id on q[1] with BarrierBoundary
    let preds_barrier = circuit.graph.predecessors(OperationId(2));
    assert!(preds_barrier
        .iter()
        .any(|(p, dep)| *p == y_id && dep.kind == DependencyKind::BarrierBoundary));

    assert!(validate_circuit(&circuit).is_ok());
}

proptest! {
    #[test]
    fn prop_random_rewrite_sequence_always_validates(
        seed in 0u64..1000u64,
        steps in 1usize..20usize,
    ) {
        let mut builder = CircuitBuilder::new("prop_circuit");
        let q = builder.add_quantum_register("q", 3).unwrap();
        let c = builder.add_classical_register("c", 2).unwrap();

        builder.h(q.qubit(0).unwrap()).unwrap();
        builder.cx(q.qubit(0).unwrap(), q.qubit(1).unwrap()).unwrap();
        builder.rz(q.qubit(1).unwrap(), ParameterExpr::from(0.5)).unwrap();
        builder.measure(q.qubit(1).unwrap(), c.bit(0).unwrap()).unwrap();

        let mut circuit = builder.finish().unwrap();
        let mut rng = seed;

        for _ in 0..steps {
            let choice = rng % 4;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);

            let order = circuit.graph.deterministic_topological_order().unwrap();
            if order.is_empty() {
                break;
            }

            let target = order[(rng as usize) % order.len()];

            match choice {
                0 => {
                    // insert_after
                    let target_qubit = QubitId((rng % 3) as usize);
                    let op = Operation::Gate {
                        gate_name: "x".to_string(),
                        qubits: vec![target_qubit],
                        parameters: Vec::new(),
                    };
                    let _ = insert_after(&mut circuit, target, op);
                }
                1 => {
                    // replace
                    let target_qubit = QubitId((rng % 3) as usize);
                    let op = Operation::Gate {
                        gate_name: "h".to_string(),
                        qubits: vec![target_qubit],
                        parameters: Vec::new(),
                    };
                    let _ = replace_operation(&mut circuit, target, op);
                }
                2 => {
                    // remove if more than 1 op left
                    if circuit.num_operations() > 1 {
                        let _ = remove_operation(&mut circuit, target);
                    }
                }
                3 => {
                    // expand into 2 gates
                    let q0 = QubitId((rng % 3) as usize);
                    let op1 = Operation::Gate {
                        gate_name: "s".to_string(),
                        qubits: vec![q0],
                        parameters: Vec::new(),
                    };
                    let op2 = Operation::Gate {
                        gate_name: "z".to_string(),
                        qubits: vec![q0],
                        parameters: Vec::new(),
                    };
                    let _ = expand_operation(&mut circuit, target, vec![op1, op2]);
                }
                _ => unreachable!(),
            }

            // Invariant: after every rewrite (successful or rolled back),
            // the circuit is strictly valid and acyclic.
            prop_assert!(validate_circuit(&circuit).is_ok());
            prop_assert!(!circuit.graph.is_cyclic());
        }
    }
}

#[test]
fn test_failed_transaction_does_not_consume_ids() {
    let mut circuit = create_test_circuit();
    let initial_next_id = circuit.next_operation_id();
    let initial_next_seq = circuit.next_sequence_number();
    let original = circuit.clone();

    // 1. Transaction that fails during a staging step
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        let invalid_op = Operation::Gate {
            gate_name: "x".to_string(),
            qubits: vec![QubitId(999)],
            parameters: Vec::new(),
        };
        let res = tx.insert_after(OperationId(1), invalid_op);
        assert!(res.is_err());
        tx.rollback();
    }
    assert_eq!(circuit, original);
    assert_eq!(circuit.next_operation_id(), initial_next_id);
    assert_eq!(circuit.next_sequence_number(), initial_next_seq);

    // 2. Transaction that performs valid staging but is dropped without commit
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        let valid_op = Operation::Gate {
            gate_name: "z".to_string(),
            qubits: vec![QubitId(0)],
            parameters: Vec::new(),
        };
        let staged_id = tx.insert_after(OperationId(1), valid_op).unwrap();
        // Inside candidate, next_operation_id was incremented
        assert_eq!(
            tx.circuit().next_operation_id(),
            OperationId(staged_id.0 + 1)
        );
        // Drop tx without commit
    }
    // Underlying circuit must NOT have consumed any ID or sequence number
    assert_eq!(circuit, original);
    assert_eq!(circuit.next_operation_id(), initial_next_id);
    assert_eq!(circuit.next_sequence_number(), initial_next_seq);

    // 3. Now perform a valid rewrite and verify it receives initial_next_id
    let valid_op = Operation::Gate {
        gate_name: "z".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    let new_id = insert_after(&mut circuit, OperationId(1), valid_op).unwrap();
    assert_eq!(
        new_id, initial_next_id,
        "Freshly committed ID should equal initial unconsumed next_id"
    );
}

#[test]
fn test_reset_operation_rewrites_preserve_dependencies() {
    let mut builder = CircuitBuilder::new("reset_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q.qubit(0).unwrap()).unwrap(); // op#1
    builder.reset(q.qubit(0).unwrap()).unwrap(); // op#2
    builder.x(q.qubit(0).unwrap()).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();

    let reset_id = OperationId(2);

    // Verify initial chain: H(0) -> Reset(0) -> X(0)
    let preds_reset = circuit.graph.predecessors(reset_id);
    assert_eq!(preds_reset.len(), 1);
    assert_eq!(preds_reset[0].0, OperationId(1));

    let succs_reset = circuit.graph.successors(reset_id);
    assert_eq!(succs_reset.len(), 1);
    assert_eq!(succs_reset[0].0, OperationId(3));

    // 1. Insert S(0) after Reset(0)
    let s_op = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    let s_id = insert_after(&mut circuit, reset_id, s_op).unwrap();

    // Reset(0) -> S(0) -> X(0)
    let succs_reset_after = circuit.graph.successors(reset_id);
    assert_eq!(succs_reset_after.len(), 1);
    assert_eq!(succs_reset_after[0].0, s_id);

    let preds_x = circuit.graph.predecessors(OperationId(3));
    assert_eq!(preds_x.len(), 1);
    assert_eq!(preds_x[0].0, s_id);

    // 2. Remove Reset(0) and verify wire reconnection: H(0) -> S(0)
    remove_operation(&mut circuit, reset_id).unwrap();
    let preds_s = circuit.graph.predecessors(s_id);
    assert_eq!(preds_s.len(), 1);
    assert_eq!(preds_s[0].0, OperationId(1));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_multi_qubit_insertion_preserves_wire_independence() {
    let mut builder = CircuitBuilder::new("disjoint_wires");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q.qubit(0).unwrap()).unwrap(); // op#1
    builder.x(q.qubit(0).unwrap()).unwrap(); // op#2
    builder.z(q.qubit(1).unwrap()).unwrap(); // op#3
    builder.y(q.qubit(1).unwrap()).unwrap(); // op#4
    builder.s(q.qubit(2).unwrap()).unwrap(); // op#5 (independent on q[2])
    let mut circuit = builder.finish().unwrap();

    // Insert CX(0, 1) after H(0) [op#1]
    let cx_op = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(0), QubitId(1)],
        parameters: Vec::new(),
    };
    let cx_id = insert_after(&mut circuit, OperationId(1), cx_op).unwrap();

    // Verify q[2] [op#5] remains completely independent: 0 predecessors, 0 successors
    assert_eq!(circuit.graph.predecessors(OperationId(5)).len(), 0);
    assert_eq!(circuit.graph.successors(OperationId(5)).len(), 0);

    // Verify CX(0, 1) has predecessor H(0) on q[0]
    let preds_cx = circuit.graph.predecessors(cx_id);
    assert!(preds_cx
        .iter()
        .any(|(p, dep)| *p == OperationId(1) && dep.wire == WireId::Quantum(QubitId(0))));

    // Verify X(0) [op#2] has predecessor CX(0, 1) on q[0]
    let preds_x = circuit.graph.predecessors(OperationId(2));
    assert!(preds_x
        .iter()
        .any(|(p, dep)| *p == cx_id && dep.wire == WireId::Quantum(QubitId(0))));

    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_multi_step_transaction_failure_complete_atomicity() {
    let mut circuit = create_test_circuit();
    let original = circuit.clone();

    let mut tx = CircuitTransaction::begin(&mut circuit);

    // Step 1: Valid mutation
    let s_op = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![QubitId(0)],
        parameters: Vec::new(),
    };
    tx.insert_after(OperationId(1), s_op).unwrap();

    // Step 2: Valid global phase
    tx.add_global_phase(ParameterExpr::from(1.23));

    // Step 3: Invalid mutation (out of bounds qubit)
    let invalid_op = Operation::Gate {
        gate_name: "x".to_string(),
        qubits: vec![QubitId(55)],
        parameters: Vec::new(),
    };
    let res = tx.insert_after(OperationId(1), invalid_op);
    assert!(res.is_err());

    // Dropping tx without commit: verify original circuit is completely untouched
    tx.rollback();

    assert_eq!(circuit, original);
    assert_eq!(circuit.next_operation_id(), original.next_operation_id());
    assert_eq!(
        circuit.next_sequence_number(),
        original.next_sequence_number()
    );
    assert_eq!(circuit.global_phase(), original.global_phase());
    assert_eq!(circuit.metadata, original.metadata);
    assert_eq!(circuit.num_operations(), original.num_operations());
}

#[test]
fn test_replace_sequence_m_to_n() {
    let mut builder = CircuitBuilder::new("replace_seq");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap(); // op#1
    builder.x(q[0]).unwrap(); // op#2
    builder.h(q[0]).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();

    let targets = [OperationId(1), OperationId(2), OperationId(3)];
    let replacement = vec![
        Operation::custom_gate("rz", vec![QubitId(0)], vec![ParameterExpr::constant(1.57)]),
        Operation::custom_gate("ry", vec![QubitId(0)], vec![ParameterExpr::constant(0.78)]),
    ];

    let new_ids = replace_sequence(&mut circuit, &targets, replacement).unwrap();
    assert_eq!(new_ids.len(), 2);
    assert_eq!(circuit.num_operations(), 2);

    for node in circuit.graph.ordered_nodes().unwrap() {
        assert_eq!(node.provenance, targets.to_vec());
    }
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_replace_sequence_m_to_zero() {
    let mut builder = CircuitBuilder::new("replace_zero");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap(); // op#1
    builder.h(q[0]).unwrap(); // op#2
    let mut circuit = builder.finish().unwrap();

    let targets = [OperationId(1), OperationId(2)];
    let new_ids = replace_sequence(&mut circuit, &targets, vec![]).unwrap();
    assert_eq!(new_ids.len(), 0);
    assert_eq!(circuit.num_operations(), 0);
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_replace_sequence_rejects_non_convex() {
    let mut builder = CircuitBuilder::new("replace_non_convex");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap(); // op#1
    builder.cx(q[0], q[1]).unwrap(); // op#2
    builder.h(q[0]).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    // {op#1, op#3} is non-convex due to intermediate op#2
    let targets = [OperationId(1), OperationId(3)];
    let res = replace_sequence(
        &mut circuit,
        &targets,
        vec![Operation::custom_gate("z", vec![QubitId(0)], vec![])],
    );
    assert!(res.is_err());
    assert_eq!(circuit, original);
    assert_eq!(circuit.next_operation_id(), original.next_operation_id());
    assert_eq!(
        circuit.next_sequence_number(),
        original.next_sequence_number()
    );
}

#[test]
fn test_replace_sequence_m_to_1() {
    let mut builder = CircuitBuilder::new("replace_to_1");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap(); // op#1
    builder.x(q[0]).unwrap(); // op#2
    builder.h(q[0]).unwrap(); // op#3
    let mut circuit = builder.finish().unwrap();

    let targets = [OperationId(1), OperationId(2), OperationId(3)];
    let replacement = vec![Operation::custom_gate("z", vec![QubitId(0)], vec![])];

    let new_ids = replace_sequence(&mut circuit, &targets, replacement).unwrap();
    assert_eq!(new_ids.len(), 1);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.get_node(new_ids[0]).unwrap();
    assert_eq!(node.provenance, targets.to_vec());
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_replace_sequence_m_to_3() {
    let mut builder = CircuitBuilder::new("replace_to_3");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap(); // op#1
    builder.x(q[0]).unwrap(); // op#2
    let mut circuit = builder.finish().unwrap();

    let targets = [OperationId(1), OperationId(2)];
    let replacement = vec![
        Operation::custom_gate("rz", vec![QubitId(0)], vec![ParameterExpr::constant(1.0)]),
        Operation::custom_gate("ry", vec![QubitId(0)], vec![ParameterExpr::constant(2.0)]),
        Operation::custom_gate("rz", vec![QubitId(0)], vec![ParameterExpr::constant(3.0)]),
    ];

    let new_ids = replace_sequence(&mut circuit, &targets, replacement).unwrap();
    assert_eq!(new_ids.len(), 3);
    assert_eq!(circuit.num_operations(), 3);

    for &id in &new_ids {
        let node = circuit.graph.get_node(id).unwrap();
        assert_eq!(node.provenance, targets.to_vec());
    }
    assert!(validate_circuit(&circuit).is_ok());
}

#[test]
fn test_replace_sequence_validation_failure_leaves_circuit_intact() {
    let mut builder = CircuitBuilder::new("replace_val_fail");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();
    let original = circuit.clone();

    let targets = [OperationId(1), OperationId(2)];
    // Invalid operation referencing out-of-bounds qubit 99
    let invalid_op = Operation::custom_gate("x", vec![QubitId(99)], vec![]);
    let res = replace_sequence(&mut circuit, &targets, vec![invalid_op]);
    assert!(res.is_err());

    // Circuit must remain completely unmodified
    assert_eq!(circuit, original);
    assert_eq!(circuit.next_operation_id(), original.next_operation_id());
    assert_eq!(
        circuit.next_sequence_number(),
        original.next_sequence_number()
    );
    assert_eq!(circuit.num_operations(), original.num_operations());
}
