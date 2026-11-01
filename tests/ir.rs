//! Comprehensive integration test suite for QccIR (Phase 6).

use std::collections::HashMap;
use std::f64::consts::PI;

use proptest::prelude::*;
use qcc::ir::{
    circuit_from_json, circuit_to_json, circuit_to_json_pretty, Circuit, ClassicalBitId,
    DependencyKind, IrError, IrValidationError, Operation, OperationId, ParameterExpr, QubitId,
    WireDependency, WireId,
};
use qcc::quantum::gate::Gate;

#[test]
fn test_registers_and_identifiers() {
    let mut circuit = Circuit::new("test_reg");
    let q_range1 = circuit.add_quantum_register("q", 3).unwrap();
    let q_range2 = circuit.add_quantum_register("ancilla", 2).unwrap();
    assert_eq!(q_range1, 0..3);
    assert_eq!(q_range2, 3..5);
    assert_eq!(circuit.num_qubits(), 5);

    let c_range1 = circuit.add_classical_register("c", 2).unwrap();
    let c_range2 = circuit.add_classical_register("meas", 2).unwrap();
    assert_eq!(c_range1, 0..2);
    assert_eq!(c_range2, 2..4);
    assert_eq!(circuit.num_classical_bits(), 4);

    // Duplicate register rejection
    let dup_q = circuit.add_quantum_register("q", 1);
    assert!(matches!(dup_q, Err(IrError::DuplicateRegister { .. })));

    let dup_c = circuit.add_classical_register("c", 1);
    assert!(matches!(dup_c, Err(IrError::DuplicateRegister { .. })));

    // Register helper methods
    let reg_q = &circuit.quantum_registers[0];
    assert_eq!(reg_q.qubit(0), Some(QubitId(0)));
    assert_eq!(reg_q.qubit(2), Some(QubitId(2)));
    assert_eq!(reg_q.qubit(3), None);

    let reg_c = &circuit.classical_registers[0];
    assert_eq!(reg_c.bit(0), Some(ClassicalBitId(0)));
    assert_eq!(reg_c.bit(1), Some(ClassicalBitId(1)));
    assert_eq!(reg_c.bit(2), None);
}

#[test]
fn test_quantum_wire_dependencies() {
    let mut circuit = Circuit::new("quantum_deps");
    circuit.add_quantum_register("q", 2).unwrap();

    let h0 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    let x0 = circuit
        .add_operation(Operation::gate(&Gate::X, vec![QubitId(0)]))
        .unwrap();
    let cx = circuit
        .add_operation(Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)]))
        .unwrap();

    let preds_cx = circuit.graph.predecessors(cx);
    assert!(preds_cx.contains(&(
        x0,
        WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
    )));

    let succs_h0 = circuit.graph.successors(h0);
    assert!(succs_h0.contains(&(
        x0,
        WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
    )));

    circuit.validate().expect("circuit must be valid");
}

#[test]
fn test_classical_raw_hazard() {
    let mut circuit = Circuit::new("raw_hazard");
    circuit.add_quantum_register("q", 1).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    // Measure q0 -> c0
    let m = circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();
    // Classical op reading c0
    let read_op = circuit
        .add_operation(Operation::classical(
            "check_bit",
            vec![ClassicalBitId(0)],
            vec![],
        ))
        .unwrap();

    let edges = circuit.graph.all_edges();
    assert!(edges.contains(&(
        m,
        read_op,
        WireDependency::new(
            WireId::Classical(ClassicalBitId(0)),
            DependencyKind::ClassicalRaw
        )
    )));

    circuit.validate().expect("circuit must be valid");
}

#[test]
fn test_classical_war_multiple_readers() {
    let mut circuit = Circuit::new("war_multiple_readers");
    circuit.add_quantum_register("q", 2).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    // 1. Initial measurement writing to c0 (Writer 1)
    let writer1 = circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();

    // 2. Reader 1 reads c0
    let reader1 = circuit
        .add_operation(Operation::classical(
            "reader_1",
            vec![ClassicalBitId(0)],
            vec![],
        ))
        .unwrap();

    // 3. Reader 2 reads c0
    let reader2 = circuit
        .add_operation(Operation::classical(
            "reader_2",
            vec![ClassicalBitId(0)],
            vec![],
        ))
        .unwrap();

    // 4. Subsequent measurement writing to c0 (Writer 2)
    let writer2 = circuit
        .add_operation(Operation::measure(QubitId(1), ClassicalBitId(0)))
        .unwrap();

    let edges = circuit.graph.all_edges();

    // RAW edges from writer1 to reader1 and reader2
    let raw_edge = WireDependency::new(
        WireId::Classical(ClassicalBitId(0)),
        DependencyKind::ClassicalRaw,
    );
    assert!(edges.contains(&(writer1, reader1, raw_edge.clone())));
    assert!(edges.contains(&(writer1, reader2, raw_edge)));

    // WAW edge from writer1 to writer2
    let waw_edge = WireDependency::new(
        WireId::Classical(ClassicalBitId(0)),
        DependencyKind::ClassicalWaw,
    );
    assert!(edges.contains(&(writer1, writer2, waw_edge)));

    // WAR edges from BOTH reader1 and reader2 to writer2 (Critical Invariant)
    let war_edge = WireDependency::new(
        WireId::Classical(ClassicalBitId(0)),
        DependencyKind::ClassicalWar,
    );
    assert!(
        edges.contains(&(reader1, writer2, war_edge.clone())),
        "Missing WAR edge from reader1 to writer2"
    );
    assert!(
        edges.contains(&(reader2, writer2, war_edge)),
        "Missing WAR edge from reader2 to writer2"
    );

    circuit.validate().expect("circuit must be valid");
}

#[test]
fn test_classical_waw_hazard() {
    let mut circuit = Circuit::new("waw_hazard");
    circuit.add_quantum_register("q", 2).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    let w1 = circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();
    let w2 = circuit
        .add_operation(Operation::measure(QubitId(1), ClassicalBitId(0)))
        .unwrap();

    let edges = circuit.graph.all_edges();
    assert!(edges.contains(&(
        w1,
        w2,
        WireDependency::new(
            WireId::Classical(ClassicalBitId(0)),
            DependencyKind::ClassicalWaw
        )
    )));

    circuit.validate().expect("circuit must be valid");
}

#[test]
fn test_multi_wire_barrier() {
    let mut circuit = Circuit::new("barrier");
    circuit.add_quantum_register("q", 3).unwrap();

    let h0 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    let h1 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(1)]))
        .unwrap();

    let barrier = circuit
        .add_operation(Operation::barrier(vec![QubitId(0), QubitId(1)]))
        .unwrap();

    let x0 = circuit
        .add_operation(Operation::gate(&Gate::X, vec![QubitId(0)]))
        .unwrap();
    let x1 = circuit
        .add_operation(Operation::gate(&Gate::X, vec![QubitId(1)]))
        .unwrap();

    let b_q0 = WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::BarrierBoundary);
    let b_q1 = WireDependency::new(WireId::Quantum(QubitId(1)), DependencyKind::BarrierBoundary);

    let edges = circuit.graph.all_edges();
    assert!(edges.contains(&(h0, barrier, b_q0.clone())));
    assert!(edges.contains(&(h1, barrier, b_q1.clone())));
    assert!(edges.contains(&(barrier, x0, b_q0)));
    assert!(edges.contains(&(barrier, x1, b_q1)));

    circuit.validate().expect("circuit must be valid");
}

#[test]
fn test_cycle_detection() {
    let mut circuit = Circuit::new("cycle");
    circuit.add_quantum_register("q", 2).unwrap();

    let op1 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    let op2 = circuit
        .add_operation(Operation::gate(&Gate::X, vec![QubitId(0)]))
        .unwrap();

    // Adding reverse edge op2 -> op1 introduces a directed cycle
    let res = circuit.graph.add_dependency_edge(
        op2,
        op1,
        WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder),
    );
    assert_eq!(res, Err(IrError::CycleDetected));
}

#[test]
fn test_validation_gate_arity_and_parameter_mismatch() {
    let mut circuit = Circuit::new("validation_errors");
    circuit.add_quantum_register("q", 2).unwrap();

    // 1. Gate arity mismatch: H gate requires 1 qubit, provided 2
    let bad_h = circuit.graph.add_operation(Operation::custom_gate(
        "h",
        vec![QubitId(0), QubitId(1)],
        vec![],
    ));
    assert!(bad_h.is_ok());
    assert!(matches!(
        circuit.validate(),
        Err(IrValidationError::GateArityMismatch {
            expected: 1,
            actual: 2,
            ..
        })
    ));

    // Clear and test parameter mismatch
    let mut circuit2 = Circuit::new("val_param");
    circuit2.add_quantum_register("q", 2).unwrap();
    // Rx requires 1 parameter, provided 0
    circuit2
        .graph
        .add_operation(Operation::custom_gate("rx", vec![QubitId(0)], vec![]))
        .unwrap();
    assert!(matches!(
        circuit2.validate(),
        Err(IrValidationError::GateParameterMismatch {
            expected: 1,
            actual: 0,
            ..
        })
    ));
}

#[test]
fn test_validation_duplicate_operand_qubits() {
    let mut circuit = Circuit::new("dup_operands");
    circuit.add_quantum_register("q", 2).unwrap();

    // CX on same qubit: (q0, q0)
    let res = circuit.add_operation(Operation::custom_gate(
        "cx",
        vec![QubitId(0), QubitId(0)],
        vec![],
    ));

    assert!(matches!(
        res,
        Err(IrError::Validation(
            IrValidationError::WireDependencyInconsistent { .. }
        ))
    ));
}

#[test]
fn test_deterministic_topological_sort() {
    let mut circuit = Circuit::new("toposort");
    circuit.add_quantum_register("q", 4).unwrap();

    // Add 4 independent gates on distinct qubits
    let op1 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    let op2 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(1)]))
        .unwrap();
    let op3 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(2)]))
        .unwrap();
    let op4 = circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(3)]))
        .unwrap();

    let order = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(order, vec![op1, op2, op3, op4]);
}

#[test]
fn test_global_phase_and_parameter_expr() {
    let mut circuit = Circuit::new("phase_expr");
    circuit.add_quantum_register("q", 1).unwrap();

    let phase = ParameterExpr::constant(PI / 4.0) + ParameterExpr::symbol("alpha");
    circuit.set_global_phase(phase.clone());
    assert_eq!(circuit.global_phase(), &phase);

    let mut env = HashMap::new();
    env.insert("alpha".to_string(), PI / 4.0);
    let eval = phase.eval(&env).unwrap();
    assert!((eval - (PI / 2.0)).abs() < 1e-12);
}

#[test]
fn test_serialization_round_trip() {
    let mut circuit = Circuit::new("bell_and_meas");
    circuit.add_quantum_register("q", 2).unwrap();
    circuit.add_classical_register("c", 2).unwrap();
    circuit.set_global_phase(ParameterExpr::constant(PI / 8.0));
    circuit
        .metadata
        .insert("author".to_string(), "QCC Agent".to_string());

    circuit
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    circuit
        .add_operation(Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)]))
        .unwrap();
    circuit
        .add_operation(Operation::measure(QubitId(0), ClassicalBitId(0)))
        .unwrap();
    circuit
        .add_operation(Operation::measure(QubitId(1), ClassicalBitId(1)))
        .unwrap();

    circuit.validate().expect("circuit must be valid");

    // 1. Serialize to JSON
    let json = circuit_to_json(&circuit).unwrap();
    let pretty_json = circuit_to_json_pretty(&circuit).unwrap();
    assert_eq!(json, pretty_json);

    // Verify key fields in JSON
    assert!(json.contains("\"schema_version\": 1"));
    assert!(json.contains("\"name\": \"bell_and_meas\""));
    assert!(json.contains("\"gate_name\": \"h\""));
    assert!(json.contains("\"gate_name\": \"cx\""));

    // 2. Deserialize from JSON
    let deserialized = circuit_from_json(&json).unwrap();

    // 3. Round-trip semantic equality
    assert_eq!(circuit, deserialized);
    deserialized
        .validate()
        .expect("deserialized circuit must validate");
}

#[test]
fn test_undeclared_qubit_and_classical_bit() {
    let mut circuit = Circuit::new("undeclared_wires");
    circuit.add_quantum_register("q", 2).unwrap();
    circuit.add_classical_register("c", 1).unwrap();

    // Adding op on qubit 2 (only qubits 0, 1 exist)
    let res_q = circuit.add_operation(Operation::gate(&Gate::H, vec![QubitId(2)]));
    assert_eq!(res_q, Err(IrError::InvalidQubit { qubit: 2, total: 2 }));

    // Adding op writing to classical bit 1 (only bit 0 exists)
    let res_c = circuit.add_operation(Operation::measure(QubitId(0), ClassicalBitId(1)));
    assert_eq!(
        res_c,
        Err(IrError::InvalidClassicalBit { bit: 1, total: 1 })
    );
}

#[test]
fn test_gate_catalog_canonical_signatures() {
    let mut circuit = Circuit::new("catalog");
    circuit.add_quantum_register("q", 2).unwrap();

    let all_fixed = [
        Gate::X,
        Gate::Y,
        Gate::Z,
        Gate::H,
        Gate::S,
        Gate::Sdg,
        Gate::T,
        Gate::Tdg,
    ];
    for g in all_fixed {
        circuit
            .add_operation(Operation::gate(&g, vec![QubitId(0)]))
            .unwrap();
    }

    let rotations = [
        Gate::Rx(qcc::Angle::from_radians(0.5)),
        Gate::Ry(qcc::Angle::from_radians(0.5)),
        Gate::Rz(qcc::Angle::from_radians(0.5)),
    ];
    for g in rotations {
        circuit
            .add_operation(Operation::gate(&g, vec![QubitId(0)]))
            .unwrap();
    }

    let entanglers = [Gate::CX, Gate::CZ, Gate::SWAP];
    for g in entanglers {
        circuit
            .add_operation(Operation::gate(&g, vec![QubitId(0), QubitId(1)]))
            .unwrap();
    }

    circuit
        .validate()
        .expect("all canonical gates must validate");
}

#[test]
fn test_operation_display_and_wire_effects() {
    let g = Operation::gate(&Gate::H, vec![QubitId(0)]);
    assert_eq!(g.to_string(), "h q[0]");

    let rx = Operation::custom_gate("rx", vec![QubitId(1)], vec![ParameterExpr::constant(PI)]);
    assert_eq!(rx.to_string(), "rx(3.141592653589793) q[1]");

    let m = Operation::measure(QubitId(0), ClassicalBitId(1));
    assert_eq!(m.to_string(), "measure q[0] -> c[1]");

    let r = Operation::reset(QubitId(2));
    assert_eq!(r.to_string(), "reset q[2]");

    let b = Operation::barrier(vec![QubitId(0), QubitId(1)]);
    assert_eq!(b.to_string(), "barrier q[0] q[1]");

    let c = Operation::classical("add", vec![ClassicalBitId(0)], vec![ClassicalBitId(1)]);
    assert_eq!(c.to_string(), "classical add reads [c[0]] writes [c[1]]");
}

#[test]
fn test_empty_and_single_node_circuits() {
    let empty_circuit = Circuit::new("empty");
    assert!(empty_circuit.graph.is_empty());
    assert_eq!(empty_circuit.graph.node_count(), 0);
    assert_eq!(empty_circuit.graph.edge_count(), 0);
    assert_eq!(
        empty_circuit
            .graph
            .deterministic_topological_order()
            .unwrap(),
        Vec::<OperationId>::new()
    );
    empty_circuit.validate().expect("empty circuit is valid");

    let mut single = Circuit::new("single");
    single.add_quantum_register("q", 1).unwrap();
    let op = single
        .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
        .unwrap();
    assert_eq!(
        single.graph.deterministic_topological_order().unwrap(),
        vec![op]
    );
    single.validate().expect("single node circuit is valid");
}

#[test]
fn test_invalid_json_handling() {
    let bad_json = "not valid json";
    let res = circuit_from_json(bad_json);
    assert!(matches!(res, Err(IrError::SerializationError { .. })));
}

proptest! {
    #[test]
    fn prop_topological_sort_preserves_causality(num_ops in 2usize..15) {
        let mut circuit = Circuit::new("prop_topo");
        circuit.add_quantum_register("q", 2).unwrap();

        for i in 0..num_ops {
            let q = QubitId(i % 2);
            circuit.add_operation(Operation::gate(&Gate::H, vec![q])).unwrap();
        }

        let order = circuit.graph.deterministic_topological_order().unwrap();
        let pos_map: HashMap<OperationId, usize> = order.iter().enumerate().map(|(idx, &id)| (id, idx)).collect();

        for (from, to, _) in circuit.graph.all_edges() {
            let from_pos = pos_map[&from];
            let to_pos = pos_map[&to];
            prop_assert!(from_pos < to_pos, "Topological sort violated: {:?} appeared after {:?}", from, to);
        }
    }

    #[test]
    fn prop_json_serialization_idempotent(num_gates in 1usize..8) {
        let mut circuit = Circuit::new("prop_json");
        circuit.add_quantum_register("q", 2).unwrap();
        circuit.add_classical_register("c", 2).unwrap();

        for i in 0..num_gates {
            if i % 2 == 0 {
                circuit.add_operation(Operation::gate(&Gate::X, vec![QubitId(0)])).unwrap();
            } else {
                circuit.add_operation(Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)])).unwrap();
            }
        }

        let json1 = circuit_to_json(&circuit).unwrap();
        let deserialized1 = circuit_from_json(&json1).unwrap();
        let json2 = circuit_to_json(&deserialized1).unwrap();

        prop_assert_eq!(json1, json2);
        prop_assert_eq!(circuit, deserialized1);
    }
}
