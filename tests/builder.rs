//! Integration tests for the public CircuitBuilder API (Phase 7).

use std::f64::consts::PI;

use qcc::builder::{BuilderError, CircuitBuilder, CircuitView};
use qcc::ir::{DependencyKind, Operation, ParameterExpr, QubitId, WireDependency, WireId};
use qcc::quantum::gate::Gate;
use qcc::Angle;

#[test]
fn test_bell_circuit_construction_and_qccir_verification() {
    let mut builder = CircuitBuilder::new("bell");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    let h_id = builder.h(q[0]).unwrap();
    let cx_id = builder.cx(q[0], q[1]).unwrap();
    let m0_id = builder.measure(q[0], c[0]).unwrap();
    let m1_id = builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().expect("Bell circuit must validate");

    assert_eq!(circuit.name, "bell");
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert_eq!(circuit.graph.node_count(), 4);

    // Verify operations in DAG
    assert!(matches!(
        circuit.graph.get_operation(h_id),
        Some(Operation::Gate { gate_name, .. }) if gate_name == "h"
    ));
    assert!(matches!(
        circuit.graph.get_operation(cx_id),
        Some(Operation::Gate { gate_name, qubits, .. }) if gate_name == "cx" && qubits == &[QubitId(0), QubitId(1)]
    ));
    assert!(matches!(
        circuit.graph.get_operation(m0_id),
        Some(Operation::Measure { qubit, destination }) if qubit.0 == 0 && destination.0 == 0
    ));
    assert!(matches!(
        circuit.graph.get_operation(m1_id),
        Some(Operation::Measure { qubit, destination }) if qubit.0 == 1 && destination.0 == 1
    ));

    // Verify edges in QccIR
    let edges = circuit.graph.all_edges();
    let q0_order = WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder);
    let q1_order = WireDependency::new(WireId::Quantum(QubitId(1)), DependencyKind::QuantumOrder);

    assert!(edges.contains(&(h_id, cx_id, q0_order.clone())));
    assert!(edges.contains(&(cx_id, m0_id, q0_order)));
    assert!(edges.contains(&(cx_id, m1_id, q1_order)));

    // Verify deterministic topological order
    let toposort = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(toposort, vec![h_id, cx_id, m0_id, m1_id]);
}

#[test]
fn test_register_declarations_and_bounds_handling() {
    let mut builder = CircuitBuilder::new("registers");

    // Empty registers rejected
    assert_eq!(
        builder.add_quantum_register("q_empty", 0),
        Err(BuilderError::EmptyRegister {
            name: "q_empty".to_string()
        })
    );
    assert_eq!(
        builder.add_classical_register("c_empty", 0),
        Err(BuilderError::EmptyRegister {
            name: "c_empty".to_string()
        })
    );

    // Valid registration
    let q = builder.add_quantum_register("q", 3).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    assert_eq!(q.name(), "q");
    assert_eq!(q.size(), 3);
    assert_eq!(c.name(), "c");
    assert_eq!(c.size(), 2);

    // Duplicate registration rejected
    assert_eq!(
        builder.add_quantum_register("q", 1),
        Err(BuilderError::DuplicateRegister {
            name: "q".to_string()
        })
    );
    assert_eq!(
        builder.add_classical_register("c", 1),
        Err(BuilderError::DuplicateRegister {
            name: "c".to_string()
        })
    );

    // Bounds checking via register handle
    assert!(q.q(0).is_ok());
    assert!(q.q(2).is_ok());
    assert_eq!(
        q.q(3),
        Err(BuilderError::IndexOutOfBounds {
            register: "q".to_string(),
            index: 3,
            size: 3
        })
    );

    // Bounds checking via builder accessor
    assert!(builder.q("q", 1).is_ok());
    assert_eq!(
        builder.q("q", 5),
        Err(BuilderError::IndexOutOfBounds {
            register: "q".to_string(),
            index: 5,
            size: 3
        })
    );
    assert_eq!(
        builder.q("nonexistent", 0),
        Err(BuilderError::UnknownRegister {
            name: "nonexistent".to_string()
        })
    );

    assert!(builder.c("c", 1).is_ok());
    assert_eq!(
        builder.c("c", 4),
        Err(BuilderError::IndexOutOfBounds {
            register: "c".to_string(),
            index: 4,
            size: 2
        })
    );
    assert_eq!(
        builder.c("nonexistent", 0),
        Err(BuilderError::UnknownRegister {
            name: "nonexistent".to_string()
        })
    );
}

#[test]
fn test_cross_builder_handle_rejection() {
    let mut builder_a = CircuitBuilder::new("builder_a");
    let mut builder_b = CircuitBuilder::new("builder_b");

    let qa = builder_a.add_quantum_register("q", 2).unwrap();
    let ca = builder_a.add_classical_register("c", 2).unwrap();

    let qb = builder_b.add_quantum_register("q", 2).unwrap();
    let cb = builder_b.add_classical_register("c", 2).unwrap();

    let id_a = builder_a.builder_id();
    let id_b = builder_b.builder_id();
    assert_ne!(id_a, id_b);

    // 1. Single-qubit gate with foreign handle
    let err_h = builder_a.h(qb[0]);
    assert_eq!(
        err_h,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );

    // 2. Two-qubit gate mixing own handle and foreign handle
    let err_cx = builder_a.cx(qa[0], qb[1]);
    assert_eq!(
        err_cx,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );

    // 3. Measurement with foreign classical bit
    let err_meas_c = builder_a.measure(qa[0], cb[0]);
    assert_eq!(
        err_meas_c,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );

    // 4. Measurement with foreign qubit
    let err_meas_q = builder_a.measure(qb[0], ca[0]);
    assert_eq!(
        err_meas_q,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );

    // 5. Barrier with foreign qubit
    let err_barrier = builder_a.barrier(&[qa[0], qb[0]]);
    assert_eq!(
        err_barrier,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );

    // 6. Reset with foreign qubit
    let err_reset = builder_a.reset(qb[0]);
    assert_eq!(
        err_reset,
        Err(BuilderError::ForeignHandle {
            expected: id_a,
            actual: id_b
        })
    );
}

#[test]
fn test_gate_arity_and_invalid_operands() {
    let mut builder = CircuitBuilder::new("operands");
    let q = builder.add_quantum_register("q", 3).unwrap();

    // CX with identical operands: (q[0], q[0])
    let err_cx = builder.cx(q[0], q[0]);
    assert_eq!(
        err_cx,
        Err(BuilderError::DuplicateOperandQubit {
            operation: "cx".to_string(),
            qubit: q[0].qubit_id()
        })
    );

    // CZ with identical operands
    let err_cz = builder.cz(q[1], q[1]);
    assert_eq!(
        err_cz,
        Err(BuilderError::DuplicateOperandQubit {
            operation: "cz".to_string(),
            qubit: q[1].qubit_id()
        })
    );

    // SWAP with identical operands
    let err_swap = builder.swap(q[2], q[2]);
    assert_eq!(
        err_swap,
        Err(BuilderError::DuplicateOperandQubit {
            operation: "swap".to_string(),
            qubit: q[2].qubit_id()
        })
    );

    // Barrier with duplicate operands
    let err_barrier = builder.barrier(&[q[0], q[1], q[0]]);
    assert_eq!(
        err_barrier,
        Err(BuilderError::DuplicateOperandQubit {
            operation: "barrier".to_string(),
            qubit: q[0].qubit_id()
        })
    );

    // Arity mismatch via generic `gate`
    let err_arity = builder.gate(&Gate::CX, &[q[0]]);
    assert_eq!(
        err_arity,
        Err(BuilderError::GateArityMismatch {
            gate: "cx".to_string(),
            expected: 2,
            actual: 1
        })
    );
}

#[test]
fn test_measurement_dependency_preservation_in_qccir() {
    let mut builder = CircuitBuilder::new("war_raw_waw");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    // Writer 1
    let w1 = builder.measure(q[0], c[0]).unwrap();
    // Readers 1 and 2
    let r1 = builder.classical("reader_1", &[c[0]], &[]).unwrap();
    let r2 = builder.classical("reader_2", &[c[0]], &[]).unwrap();
    // Writer 2
    let w2 = builder.measure(q[1], c[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let edges = circuit.graph.all_edges();

    // RAW edges: w1 -> r1, w1 -> r2
    let raw = WireDependency::new(
        WireId::Classical(c[0].bit_id()),
        DependencyKind::ClassicalRaw,
    );
    assert!(edges.contains(&(w1, r1, raw.clone())));
    assert!(edges.contains(&(w1, r2, raw)));

    // WAW edge: w1 -> w2
    let waw = WireDependency::new(
        WireId::Classical(c[0].bit_id()),
        DependencyKind::ClassicalWaw,
    );
    assert!(edges.contains(&(w1, w2, waw)));

    // WAR edges from BOTH readers to w2
    let war = WireDependency::new(
        WireId::Classical(c[0].bit_id()),
        DependencyKind::ClassicalWar,
    );
    assert!(edges.contains(&(r1, w2, war.clone())));
    assert!(edges.contains(&(r2, w2, war)));
}

#[test]
fn test_parameterized_rotations_retention() {
    let mut builder = CircuitBuilder::new("rotations");
    let q = builder.add_quantum_register("q", 2).unwrap();

    // 1. Literal f64
    let op_rx = builder.rx(q[0], PI / 2.0).unwrap();
    // 2. Angle type
    let op_ry = builder.ry(q[0], Angle::from_radians(0.75)).unwrap();
    // 3. Symbolic string
    let op_rz = builder.rz(q[1], "theta").unwrap();
    // 4. Composite algebraic ParameterExpr
    let composite = ParameterExpr::symbol("alpha") * ParameterExpr::constant(2.0);
    let op_comp = builder.rx(q[1], composite.clone()).unwrap();

    let circuit = builder.finish().unwrap();

    match circuit.graph.get_operation(op_rx).unwrap() {
        Operation::Gate { parameters, .. } => {
            assert_eq!(parameters[0], ParameterExpr::constant(PI / 2.0));
        }
        _ => panic!("Expected Gate"),
    }

    match circuit.graph.get_operation(op_ry).unwrap() {
        Operation::Gate { parameters, .. } => {
            assert_eq!(parameters[0], ParameterExpr::constant(0.75));
        }
        _ => panic!("Expected Gate"),
    }

    match circuit.graph.get_operation(op_rz).unwrap() {
        Operation::Gate { parameters, .. } => {
            assert_eq!(parameters[0], ParameterExpr::symbol("theta"));
        }
        _ => panic!("Expected Gate"),
    }

    match circuit.graph.get_operation(op_comp).unwrap() {
        Operation::Gate { parameters, .. } => {
            assert_eq!(parameters[0], composite);
        }
        _ => panic!("Expected Gate"),
    }
}

#[test]
fn test_global_phase_retention() {
    let mut builder = CircuitBuilder::new("phase");
    let _q = builder.add_quantum_register("q", 1).unwrap();

    builder.set_global_phase(PI / 4.0);
    assert_eq!(builder.global_phase(), &ParameterExpr::constant(PI / 4.0));

    builder.add_global_phase("gamma");
    let expected = ParameterExpr::constant(PI / 4.0) + ParameterExpr::symbol("gamma");
    assert_eq!(builder.global_phase(), &expected);

    let circuit = builder.finish().unwrap();
    assert_eq!(circuit.global_phase(), &expected);
}

#[test]
fn test_reset_and_barrier_construction() {
    let mut builder = CircuitBuilder::new("reset_barrier");
    let q = builder.add_quantum_register("q", 3).unwrap();

    let r_id = builder.reset(q[0]).unwrap();
    let b_id = builder.barrier(&[q[0], q[1], q[2]]).unwrap();

    let circuit = builder.finish().unwrap();

    assert!(matches!(
        circuit.graph.get_operation(r_id),
        Some(Operation::Reset { qubit }) if qubit.0 == 0
    ));
    assert!(matches!(
        circuit.graph.get_operation(b_id),
        Some(Operation::Barrier { qubits }) if qubits.len() == 3
    ));

    // Barrier edge from reset to barrier on q[0]
    let edges = circuit.graph.all_edges();
    let b_dep = WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::BarrierBoundary);
    assert!(edges.contains(&(r_id, b_id, b_dep)));
}

#[test]
fn test_immutable_circuit_view() {
    let mut builder = CircuitBuilder::new("view_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();

    // Inspect view on builder
    let view = builder.view();
    assert_eq!(view.name(), "view_test");
    assert_eq!(view.num_qubits(), 2);
    assert_eq!(view.num_classical_bits(), 1);
    assert_eq!(view.num_operations(), 3);
    assert_eq!(view.num_dependencies(), 2);

    let summary = view.display_summary();
    assert!(summary.contains("Circuit: view_test"));
    assert!(summary.contains("Qubits: 2"));
    assert!(summary.contains("Classical Bits: 1"));

    // Finish circuit and inspect view on finalized Circuit
    let circuit = builder.finish().unwrap();
    let final_view = CircuitView::new(&circuit);
    assert_eq!(final_view.num_operations(), 3);
    let top_ops = final_view.topological_operations().unwrap();
    assert_eq!(top_ops.len(), 3);
}

#[test]
fn test_single_qubit_fixed_gate_suite() {
    let mut builder = CircuitBuilder::new("fixed_gates");
    let q = builder.add_quantum_register("q", 1).unwrap();

    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.tdg(q[0]).unwrap();

    let circuit = builder.finish().unwrap();
    assert_eq!(circuit.graph.node_count(), 8);
    assert_eq!(circuit.graph.edge_count(), 7); // chain of 8 operations on 1 qubit = 7 edges
}
