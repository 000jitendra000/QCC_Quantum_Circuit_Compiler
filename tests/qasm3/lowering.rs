//! Integration tests for Phase 25: AST-to-QccIR lowering.
//!
//! Verifies:
//! - Semantic equivalence between OpenQASM-lowered circuits and Rust `CircuitBuilder` circuits
//! - Equivalence checking via `UnitaryEquivalenceChecker`
//! - Register allocation, typed ID mapping, and operand ordering
//! - Single-bit and whole-register measurement lowering
//! - Active reset and barrier (global and selective) lowering
//! - Global phase accumulation and preservation
//! - Source span preservation in circuit metadata
//! - QccIR graph invariant validation
//! - Fail-closed rejection of invalid or unsupported AST constructs

use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;
use std::process::Command;

use qcc::builder::CircuitBuilder;
use qcc::frontend::lowering::{lower_qasm_str, CircuitLowerer};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::OpenQasmFrontend;
use qcc::ir::edge::{DependencyKind, WireDependency};
use qcc::ir::wire::{ClassicalBitId, QubitId, WireId};
use qcc::ir::{Circuit, Operation, ParameterExpr};
use qcc::quantum::ClassicalBitOrder;
use qcc::simulator::{execute_circuit, sample_circuit, ExecutionError, SplitMix64};
use qcc::verification::{check_equivalence, VerificationError};

const TOLERANCE: f64 = 1e-10;

/// Helper parsing and lowering an OpenQASM 3 string into a validated [`Circuit`].
fn lower_success(source: &str, bindings: Option<HashMap<String, f64>>) -> Circuit {
    let res = lower_qasm_str(source, None, bindings);
    assert!(
        res.is_ok(),
        "Lowering failed unexpectedly: {:?}",
        res.diagnostics
    );
    res.output.expect("Circuit must be present on success")
}

// ============================================================================
// 1. Bell Circuit Equivalence: Builder vs QASM Lowering
// ============================================================================

#[test]
fn test_bell_circuit_lowering_and_builder_equivalence() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Build the identical circuit using CircuitBuilder API
    let mut builder = CircuitBuilder::new("qasm_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();
    let h_id = builder.h(q[0]).unwrap();
    let cx_id = builder.cx(q[0], q[1]).unwrap();
    let m0_id = builder.measure(q[0], c[0]).unwrap();
    let m1_id = builder.measure(q[1], c[1]).unwrap();
    let builder_circuit = builder.finish().expect("Builder circuit must validate");

    // 1. Structural comparison
    assert_eq!(qasm_circuit.num_qubits(), builder_circuit.num_qubits());
    assert_eq!(
        qasm_circuit.num_classical_bits(),
        builder_circuit.num_classical_bits()
    );
    assert_eq!(
        qasm_circuit.graph.node_count(),
        builder_circuit.graph.node_count()
    );
    assert_eq!(
        qasm_circuit.graph.edge_count(),
        builder_circuit.graph.edge_count()
    );

    // 2. Registers
    assert_eq!(qasm_circuit.quantum_registers.len(), 1);
    assert_eq!(qasm_circuit.quantum_registers[0].name, "q");
    assert_eq!(qasm_circuit.quantum_registers[0].size, 2);
    assert_eq!(qasm_circuit.classical_registers.len(), 1);
    assert_eq!(qasm_circuit.classical_registers[0].name, "c");
    assert_eq!(qasm_circuit.classical_registers[0].size, 2);

    // 3. Operation verification
    let qasm_nodes: Vec<_> = qasm_circuit
        .graph
        .deterministic_topological_order()
        .unwrap();
    assert_eq!(qasm_nodes.len(), 4);

    let op0 = qasm_circuit.graph.get_operation(qasm_nodes[0]).unwrap();
    let op1 = qasm_circuit.graph.get_operation(qasm_nodes[1]).unwrap();
    let op2 = qasm_circuit.graph.get_operation(qasm_nodes[2]).unwrap();
    let op3 = qasm_circuit.graph.get_operation(qasm_nodes[3]).unwrap();

    assert_eq!(
        op0,
        &Operation::Gate {
            gate_name: "h".to_string(),
            qubits: vec![QubitId(0)],
            parameters: vec![],
        }
    );
    assert_eq!(
        op1,
        &Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![QubitId(0), QubitId(1)],
            parameters: vec![],
        }
    );
    assert_eq!(
        op2,
        &Operation::Measure {
            qubit: QubitId(0),
            destination: ClassicalBitId(0),
        }
    );
    assert_eq!(
        op3,
        &Operation::Measure {
            qubit: QubitId(1),
            destination: ClassicalBitId(1),
        }
    );

    // 4. Compare with builder operations
    assert_eq!(op0, builder_circuit.graph.get_operation(h_id).unwrap());
    assert_eq!(op1, builder_circuit.graph.get_operation(cx_id).unwrap());
    assert_eq!(op2, builder_circuit.graph.get_operation(m0_id).unwrap());
    assert_eq!(op3, builder_circuit.graph.get_operation(m1_id).unwrap());

    // 4b. Wire dependency comparison
    let qasm_edges = qasm_circuit.graph.all_edges();
    let builder_edges = builder_circuit.graph.all_edges();
    assert_eq!(qasm_edges, builder_edges);
    assert_eq!(qasm_edges.len(), 3);
    assert_eq!(
        qasm_edges[0],
        (
            h_id,
            cx_id,
            WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
        )
    );
    assert_eq!(
        qasm_edges[1],
        (
            cx_id,
            m0_id,
            WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
        )
    );
    assert_eq!(
        qasm_edges[2],
        (
            cx_id,
            m1_id,
            WireDependency::new(WireId::Quantum(QubitId(1)), DependencyKind::QuantumOrder)
        )
    );

    // 4c. Behavioral sampling equivalence with seeded RNG
    let num_shots = 2000;
    let (qasm_res, _) = sample_circuit(&qasm_circuit, num_shots, SplitMix64::new(42))
        .expect("Sampling lowered circuit must succeed");
    let (builder_res, _) = sample_circuit(&builder_circuit, num_shots, SplitMix64::new(42))
        .expect("Sampling builder circuit must succeed");
    assert_eq!(
        qasm_res.counts(ClassicalBitOrder::MsbFirst),
        builder_res.counts(ClassicalBitOrder::MsbFirst)
    );
    let counts = qasm_res.counts(ClassicalBitOrder::MsbFirst);
    assert_eq!(counts.get("01").copied().unwrap_or(0), 0);
    assert_eq!(counts.get("10").copied().unwrap_or(0), 0);
    assert_eq!(
        counts.get("00").copied().unwrap_or(0) + counts.get("11").copied().unwrap_or(0),
        num_shots
    );

    // 4d. Rejection of measurement circuit by unitary equivalence checker
    let equiv_err = check_equivalence(&qasm_circuit, &builder_circuit).unwrap_err();
    match equiv_err {
        VerificationError::NonUnitaryOperation { operation, .. } => {
            assert_eq!(operation, "measure");
        }
        other => panic!("Expected NonUnitaryOperation for measurement, got: {other:?}"),
    }

    // 5. Unitary prefix equivalence check (H then CX)
    let mut u_qasm_builder = CircuitBuilder::new("u_bell_qasm");
    let uq = u_qasm_builder.add_quantum_register("q", 2).unwrap();
    u_qasm_builder.h(uq[0]).unwrap();
    u_qasm_builder.cx(uq[0], uq[1]).unwrap();
    let u_qasm_circuit = u_qasm_builder.finish().unwrap();

    let u_qasm_from_src = lower_success(
        r#"OPENQASM 3.0;
qubit[2] q;
h q[0];
cx q[0], q[1];
"#,
        None,
    );

    let verdict = check_equivalence(&u_qasm_circuit, &u_qasm_from_src)
        .expect("Equivalence check must succeed");
    assert!(
        verdict.is_equivalent,
        "Unitary Bell circuits must be equivalent"
    );
    assert!(
        verdict.max_amplitude_difference < TOLERANCE,
        "Max amplitude difference must be within tolerance"
    );
}

// ============================================================================
// 2. Parameterized Rotation Circuits Equivalence
// ============================================================================

#[test]
fn test_parameterized_rotations_constant_and_pi_expressions() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[1] q;
rx(pi / 4) q[0];
ry(pi / 2) q[0];
rz(3 * pi / 4) q[0];
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Build equivalent circuit with CircuitBuilder
    let mut builder = CircuitBuilder::new("rotations");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], PI / 4.0).unwrap();
    builder.ry(q[0], PI / 2.0).unwrap();
    builder.rz(q[0], 3.0 * PI / 4.0).unwrap();
    let builder_circuit = builder.finish().unwrap();

    // Check unitary equivalence via check_equivalence
    let verdict =
        check_equivalence(&qasm_circuit, &builder_circuit).expect("Equivalence check must succeed");
    assert!(
        verdict.is_equivalent,
        "Parameterized rotation circuits must be unitarily equivalent: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

#[test]
fn test_parameterized_rotations_with_runtime_symbol_bindings() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta) q[0];
ry(phi * 2) q[0];
rz(-theta + phi) q[0];
"#;

    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), 1.2345);
    bindings.insert("phi".to_string(), 0.6789);

    let qasm_circuit = lower_success(qasm_src, Some(bindings.clone()));

    // Build equivalent circuit directly with builder using bound values
    let mut builder = CircuitBuilder::new("bound_rotations");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 1.2345).unwrap();
    builder.ry(q[0], 0.6789 * 2.0).unwrap();
    builder.rz(q[0], -1.2345 + 0.6789).unwrap();
    let builder_circuit = builder.finish().unwrap();

    let verdict =
        check_equivalence(&qasm_circuit, &builder_circuit).expect("Equivalence check must succeed");
    assert!(
        verdict.is_equivalent,
        "Bound symbolic circuits must match builder circuit: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

// ============================================================================
// 3. Measurement Lowering: Single-Bit and Whole-Register
// ============================================================================

#[test]
fn test_single_bit_measurement_assignment_and_arrow_lowering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c[0] = measure q[0];
measure q[1] -> c[1];
"#;

    let circuit = lower_success(qasm_src, None);
    assert_eq!(circuit.graph.node_count(), 2);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(
        circuit.graph.get_operation(nodes[0]).unwrap(),
        &Operation::Measure {
            qubit: QubitId(0),
            destination: ClassicalBitId(0),
        }
    );
    assert_eq!(
        circuit.graph.get_operation(nodes[1]).unwrap(),
        &Operation::Measure {
            qubit: QubitId(1),
            destination: ClassicalBitId(1),
        }
    );
}

#[test]
fn test_whole_register_measurement_lowering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[3] q;
bit[3] c;
c = measure q;
"#;

    let circuit = lower_success(qasm_src, None);
    // Whole register measurement of size 3 must lower to 3 sequential measurements
    assert_eq!(circuit.graph.node_count(), 3);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();
    for (i, &node) in nodes.iter().enumerate().take(3) {
        assert_eq!(
            circuit.graph.get_operation(node).unwrap(),
            &Operation::Measure {
                qubit: QubitId(i),
                destination: ClassicalBitId(i),
            }
        );
    }
}

#[test]
fn test_scalar_measurement_lowering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit q;
bit c;
c = measure q;
"#;

    let circuit = lower_success(qasm_src, None);
    assert_eq!(circuit.num_qubits(), 1);
    assert_eq!(circuit.num_classical_bits(), 1);
    assert_eq!(circuit.graph.node_count(), 1);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(
        circuit.graph.get_operation(nodes[0]).unwrap(),
        &Operation::Measure {
            qubit: QubitId(0),
            destination: ClassicalBitId(0),
        }
    );
}

#[test]
fn test_whole_register_measurement_comparison_and_sampling() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[3] q;
bit[3] c;
c = measure q;
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Build equivalent circuit using builder
    let mut builder = CircuitBuilder::new("whole_meas_builder");
    let q = builder.add_quantum_register("q", 3).unwrap();
    let c = builder.add_classical_register("c", 3).unwrap();
    let m0 = builder.measure(q[0], c[0]).unwrap();
    let m1 = builder.measure(q[1], c[1]).unwrap();
    let m2 = builder.measure(q[2], c[2]).unwrap();
    let builder_circuit = builder.finish().unwrap();

    // Verify operations match
    let qasm_nodes = qasm_circuit
        .graph
        .deterministic_topological_order()
        .unwrap();
    assert_eq!(
        qasm_circuit.graph.get_operation(qasm_nodes[0]).unwrap(),
        builder_circuit.graph.get_operation(m0).unwrap()
    );
    assert_eq!(
        qasm_circuit.graph.get_operation(qasm_nodes[1]).unwrap(),
        builder_circuit.graph.get_operation(m1).unwrap()
    );
    assert_eq!(
        qasm_circuit.graph.get_operation(qasm_nodes[2]).unwrap(),
        builder_circuit.graph.get_operation(m2).unwrap()
    );

    // Verify behavioral sampling on initial |000> state
    let num_shots = 500;
    let (qasm_res, _) = sample_circuit(&qasm_circuit, num_shots, SplitMix64::new(123))
        .expect("Sampling lowered whole-register measurement circuit must succeed");
    let (builder_res, _) = sample_circuit(&builder_circuit, num_shots, SplitMix64::new(123))
        .expect("Sampling builder whole-register measurement circuit must succeed");
    assert_eq!(
        qasm_res.counts(ClassicalBitOrder::MsbFirst),
        builder_res.counts(ClassicalBitOrder::MsbFirst)
    );
    assert_eq!(
        qasm_res
            .counts(ClassicalBitOrder::MsbFirst)
            .get("000")
            .copied()
            .unwrap_or(0),
        num_shots
    );

    // Verify rejection by unitary equivalence checker
    let equiv_err = check_equivalence(&qasm_circuit, &builder_circuit).unwrap_err();
    match equiv_err {
        VerificationError::NonUnitaryOperation { operation, .. } => {
            assert_eq!(operation, "measure");
        }
        other => panic!("Expected NonUnitaryOperation for measurement, got: {other:?}"),
    }
}

// ============================================================================
// 4. Register Allocation, Typed Mapping, and Operand Ordering
// ============================================================================

#[test]
fn test_multi_register_allocation_and_cx_operand_ordering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[2] q;
qubit[3] r;
bit[1] c;
bit[2] d;
cx q[1], r[2];
c[0] = measure q[0];
d[1] = measure r[1];
"#;

    let circuit = lower_success(qasm_src, None);
    assert_eq!(circuit.num_qubits(), 5);
    assert_eq!(circuit.num_classical_bits(), 3);

    // Verify register descriptors
    assert_eq!(circuit.quantum_registers.len(), 2);
    assert_eq!(circuit.quantum_registers[0].name, "q");
    assert_eq!(circuit.quantum_registers[0].start_qubit, 0);
    assert_eq!(circuit.quantum_registers[0].size, 2);
    assert_eq!(circuit.quantum_registers[1].name, "r");
    assert_eq!(circuit.quantum_registers[1].start_qubit, 2);
    assert_eq!(circuit.quantum_registers[1].size, 3);

    assert_eq!(circuit.classical_registers.len(), 2);
    assert_eq!(circuit.classical_registers[0].name, "c");
    assert_eq!(circuit.classical_registers[0].start_bit, 0);
    assert_eq!(circuit.classical_registers[0].size, 1);
    assert_eq!(circuit.classical_registers[1].name, "d");
    assert_eq!(circuit.classical_registers[1].start_bit, 1);
    assert_eq!(circuit.classical_registers[1].size, 2);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(nodes.len(), 3);

    // Verify CX operand order: Operand 0 is control (q[1] -> 1), Operand 1 is target (r[2] -> 4)
    assert_eq!(
        circuit.graph.get_operation(nodes[0]).unwrap(),
        &Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![QubitId(1), QubitId(4)],
            parameters: vec![],
        }
    );

    // Verify c[0] = measure q[0]; (q[0] -> 0, c[0] -> 0)
    assert_eq!(
        circuit.graph.get_operation(nodes[1]).unwrap(),
        &Operation::Measure {
            qubit: QubitId(0),
            destination: ClassicalBitId(0),
        }
    );

    // Verify d[1] = measure r[1]; (r[1] -> 2+1=3, d[1] -> 1+1=2)
    assert_eq!(
        circuit.graph.get_operation(nodes[2]).unwrap(),
        &Operation::Measure {
            qubit: QubitId(3),
            destination: ClassicalBitId(2),
        }
    );
}

// ============================================================================
// 5. Reset and Barrier Lowering
// ============================================================================

#[test]
fn test_reset_statement_lowering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[3] q;
reset q[1];
reset q;
"#;

    let circuit = lower_success(qasm_src, None);
    // 1 single reset + 3 whole-register resets = 4 operations
    assert_eq!(circuit.graph.node_count(), 4);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();
    assert_eq!(
        circuit.graph.get_operation(nodes[0]).unwrap(),
        &Operation::Reset { qubit: QubitId(1) }
    );
    assert_eq!(
        circuit.graph.get_operation(nodes[1]).unwrap(),
        &Operation::Reset { qubit: QubitId(0) }
    );
    assert_eq!(
        circuit.graph.get_operation(nodes[2]).unwrap(),
        &Operation::Reset { qubit: QubitId(1) }
    );
    assert_eq!(
        circuit.graph.get_operation(nodes[3]).unwrap(),
        &Operation::Reset { qubit: QubitId(2) }
    );
}

#[test]
fn test_global_and_selective_barrier_lowering() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[2] q;
qubit[2] r;
barrier;
barrier q[0], r[1];
barrier q;
"#;

    let circuit = lower_success(qasm_src, None);
    assert_eq!(circuit.graph.node_count(), 3);

    let nodes = circuit.graph.deterministic_topological_order().unwrap();

    // 1. Global barrier synchronizes all 4 qubits
    assert_eq!(
        circuit.graph.get_operation(nodes[0]).unwrap(),
        &Operation::Barrier {
            qubits: vec![QubitId(0), QubitId(1), QubitId(2), QubitId(3)],
        }
    );

    // 2. Selective barrier over q[0] and r[1] (q[0] -> 0, r[1] -> 2+1=3)
    assert_eq!(
        circuit.graph.get_operation(nodes[1]).unwrap(),
        &Operation::Barrier {
            qubits: vec![QubitId(0), QubitId(3)],
        }
    );

    // 3. Whole-register barrier over q expands to q[0] and q[1]
    assert_eq!(
        circuit.graph.get_operation(nodes[2]).unwrap(),
        &Operation::Barrier {
            qubits: vec![QubitId(0), QubitId(1)],
        }
    );
}

#[test]
fn test_reset_semantics_independent_validation() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[1] q;
reset q[0];
x q[0];
reset q[0];
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Build equivalent circuit using builder
    let mut builder = CircuitBuilder::new("reset_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let r0 = builder.reset(q[0]).unwrap();
    let x0 = builder.x(q[0]).unwrap();
    let r1 = builder.reset(q[0]).unwrap();
    let builder_circuit = builder.finish().unwrap();

    // 1. Verify DAG wire dependency serialization on quantum wire 0
    let qasm_edges = qasm_circuit.graph.all_edges();
    let builder_edges = builder_circuit.graph.all_edges();
    assert_eq!(qasm_edges, builder_edges);
    assert_eq!(qasm_edges.len(), 2);
    assert_eq!(
        qasm_edges[0],
        (
            r0,
            x0,
            WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
        )
    );
    assert_eq!(
        qasm_edges[1],
        (
            x0,
            r1,
            WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::QuantumOrder)
        )
    );

    // 2. Verify rejection by unitary equivalence checker
    let equiv_err = check_equivalence(&qasm_circuit, &builder_circuit).unwrap_err();
    match equiv_err {
        VerificationError::NonUnitaryOperation { operation, .. } => {
            assert_eq!(operation, "reset");
        }
        other => panic!("Expected NonUnitaryOperation for reset, got: {other:?}"),
    }

    // 3. Verify execution rejection (dynamic simulation of active reset is scheduled for M7)
    let exec_err = execute_circuit(&qasm_circuit).unwrap_err();
    match exec_err {
        ExecutionError::UnsupportedOperation { operation, .. } => {
            assert_eq!(operation, "reset");
        }
        other => panic!("Expected UnsupportedOperation for reset, got: {other:?}"),
    }
}

#[test]
fn test_barrier_semantics_independent_validation() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[2] q;
h q[0];
barrier q[0], q[1];
cx q[0], q[1];
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Build equivalent circuit using builder
    let mut builder = CircuitBuilder::new("barrier_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let h0 = builder.h(q[0]).unwrap();
    let b0 = builder.barrier(&[q[0], q[1]]).unwrap();
    let cx0 = builder.cx(q[0], q[1]).unwrap();
    let builder_circuit = builder.finish().unwrap();

    // 1. Verify operations match
    assert_eq!(qasm_circuit.graph.node_count(), 3);
    assert_eq!(
        qasm_circuit.graph.get_operation(h0).unwrap(),
        builder_circuit.graph.get_operation(h0).unwrap()
    );
    assert_eq!(
        qasm_circuit.graph.get_operation(b0).unwrap(),
        builder_circuit.graph.get_operation(b0).unwrap()
    );
    assert_eq!(
        qasm_circuit.graph.get_operation(cx0).unwrap(),
        builder_circuit.graph.get_operation(cx0).unwrap()
    );

    // 2. Verify barrier dependency edges
    let edges = qasm_circuit.graph.all_edges();
    assert!(edges.contains(&(
        h0,
        b0,
        WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::BarrierBoundary)
    )));
    assert!(edges.contains(&(
        b0,
        cx0,
        WireDependency::new(WireId::Quantum(QubitId(0)), DependencyKind::BarrierBoundary)
    )));
    assert!(edges.contains(&(
        b0,
        cx0,
        WireDependency::new(WireId::Quantum(QubitId(1)), DependencyKind::BarrierBoundary)
    )));

    // 3. Unitary equivalence: a circuit with barrier is unitarily equivalent to barrier-free circuit
    let mut no_barrier_builder = CircuitBuilder::new("no_barrier");
    let nb_q = no_barrier_builder.add_quantum_register("q", 2).unwrap();
    no_barrier_builder.h(nb_q[0]).unwrap();
    no_barrier_builder.cx(nb_q[0], nb_q[1]).unwrap();
    let no_barrier_circuit = no_barrier_builder.finish().unwrap();

    let verdict = check_equivalence(&qasm_circuit, &no_barrier_circuit)
        .expect("Equivalence check must succeed for circuit with barriers");
    assert!(
        verdict.is_equivalent,
        "Circuit with barriers must be unitarily equivalent to barrier-free circuit"
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);

    // 4. Execution equivalence: statevectors must be identical
    let (state_with, _) = execute_circuit(&qasm_circuit).unwrap();
    let (state_without, _) = execute_circuit(&no_barrier_circuit).unwrap();
    assert_eq!(state_with.amplitudes(), state_without.amplitudes());
}

// ============================================================================
// 6. Global Phase Preservation
// ============================================================================

#[test]
fn test_global_phase_accumulation_and_equivalence() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[1] q;
gphase(pi / 3);
h q[0];
gphase(pi / 6);
"#;

    let qasm_circuit = lower_success(qasm_src, None);

    // Expected phase is pi/3 + pi/6 = pi/2
    let expected_phase = ParameterExpr::constant(PI / 3.0) + ParameterExpr::constant(PI / 6.0);
    assert_eq!(qasm_circuit.global_phase(), &expected_phase);

    // Build equivalent circuit with CircuitBuilder
    let mut builder = CircuitBuilder::new("gphase_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.set_global_phase(PI / 2.0);
    builder.h(q[0]).unwrap();
    let builder_circuit = builder.finish().unwrap();

    let verdict =
        check_equivalence(&qasm_circuit, &builder_circuit).expect("Equivalence check must succeed");
    assert!(
        verdict.is_equivalent,
        "Global phase circuits must be equivalent: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

#[test]
fn test_multiple_global_phase_accumulation_and_symbolic_bindings() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[1] q;
gphase(pi / 4);
gphase(-pi / 8);
gphase(phi);
h q[0];
"#;

    let mut bindings = HashMap::new();
    bindings.insert("phi".to_string(), 0.5);

    let qasm_circuit = lower_success(qasm_src, Some(bindings));

    // Build equivalent circuit using builder with same phases
    let mut builder = CircuitBuilder::new("multi_gphase");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.add_global_phase(PI / 4.0);
    builder.add_global_phase(-PI / 8.0);
    builder.add_global_phase(0.5);
    builder.h(q[0]).unwrap();
    let builder_circuit = builder.finish().unwrap();

    // Check equivalence
    let verdict = check_equivalence(&qasm_circuit, &builder_circuit)
        .expect("Multi-gphase equivalence check must succeed");
    assert!(
        verdict.is_equivalent,
        "Lowered multi-gphase circuit must match builder circuit: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

// ============================================================================
// 7. Source Span and Metadata Preservation
// ============================================================================

#[test]
fn test_source_span_preservation_in_metadata() {
    let qasm_src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n";
    let buffer = SourceBuffer::new("test_provenance.qasm", qasm_src);

    let lowerer = CircuitLowerer::new(&buffer);
    let res = lowerer.lower_source_buffer();
    assert!(res.is_ok());
    let circuit = res.output.unwrap();

    // Verify program-level metadata
    assert_eq!(
        circuit.metadata.get("source.file"),
        Some(&"test_provenance.qasm".to_string())
    );
    assert!(circuit.metadata.contains_key("source.program_span"));

    // Verify per-operation source span provenance
    assert!(
        circuit.metadata.contains_key("op.1.span"),
        "H operation must have recorded source span"
    );
    assert!(
        circuit.metadata.contains_key("op.2.span"),
        "CX operation must have recorded source span"
    );

    // Verify span coordinates
    let h_span = circuit.metadata.get("op.1.span").unwrap();
    assert!(
        h_span.starts_with("3:"),
        "H gate is on line 3, span recorded: {h_span}"
    );

    let cx_span = circuit.metadata.get("op.2.span").unwrap();
    assert!(
        cx_span.starts_with("4:"),
        "CX gate is on line 4, span recorded: {cx_span}"
    );
}

#[test]
fn test_operation_source_span_metadata_coverage_and_invariance() {
    let qasm_src = "OPENQASM 3.0;\nqubit[2] q;\nbit[2] c;\ngphase(0.1);\nh q[0];\ncx q[0], q[1];\nbarrier q[0], q[1];\nc = measure q;\n";
    let buffer = SourceBuffer::new("span_audit.qasm", qasm_src);

    let lowerer = CircuitLowerer::new(&buffer);
    let res = lowerer.lower_source_buffer();
    assert!(res.is_ok());
    let circuit = res.output.unwrap();

    assert_eq!(circuit.graph.node_count(), 5);

    let expected_lines = [
        (1, 5), // op 1: line 5 (h)
        (2, 6), // op 2: line 6 (cx)
        (3, 7), // op 3: line 7 (barrier)
        (4, 8), // op 4: line 8 (measure q0)
        (5, 8), // op 5: line 8 (measure q1)
    ];

    for (op_num, expected_line) in expected_lines {
        let key = format!("op.{op_num}.span");
        let span_str = circuit
            .metadata
            .get(&key)
            .unwrap_or_else(|| panic!("Metadata missing key {key}"));
        assert!(
            span_str.starts_with(&format!("{expected_line}:")),
            "Expected op {op_num} to start on line {expected_line}, got span string: {span_str}"
        );
    }

    // Verify metadata remains intact through validation
    let metadata_before = circuit.metadata.clone();
    assert!(circuit.validate().is_ok());
    assert_eq!(circuit.metadata, metadata_before);
}

// ============================================================================
// 8. QccIR Graph Invariant Validation
// ============================================================================

#[test]
fn test_lowered_circuit_satisfies_all_qccir_invariants() {
    let qasm_src = r#"OPENQASM 3.0;
qubit[3] q;
bit[2] c;
x q[0];
y q[1];
z q[2];
h q[0];
s q[1];
sdg q[2];
t q[0];
tdg q[1];
rx(0.5) q[2];
ry(1.0) q[0];
rz(1.5) q[1];
cx q[0], q[1];
cz q[1], q[2];
swap q[0], q[2];
reset q[0];
barrier q[1], q[2];
c[0] = measure q[1];
c[1] = measure q[2];
"#;

    let circuit = lower_success(qasm_src, None);

    // Execute comprehensive QccIR invariant validation
    assert!(
        circuit.validate().is_ok(),
        "Lowered circuit must pass validate_circuit cleanly"
    );
    assert!(!circuit.graph.is_cyclic(), "DAG must be strictly acyclic");
    assert_eq!(circuit.num_qubits(), 3);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert_eq!(circuit.graph.node_count(), 18);

    // Verify deterministic topological order
    let toposort = circuit.graph.deterministic_topological_order();
    assert!(toposort.is_ok(), "Topological order must succeed");
    assert_eq!(toposort.unwrap().len(), 18);
}

// ============================================================================
// 9. Negative Lowering Tests (Fail-Closed Guarantees)
// ============================================================================

#[test]
fn test_negative_unsupported_syntax_rejected_fail_closed() {
    let cases = [
        ("while (c == 0) { h q[0]; }", "E0005"),
        ("for i in [0:2] { h q[i]; }", "E0005"),
        ("def my_func() { return; }", "E0005"),
        ("gate custom q { h q; }", "E0005"),
        ("defcal rx(pi) q {}", "E0005"),
        ("int[32] count = 0;", "E0005"),
        ("ccx q[0], q[1], q[2];", "E0003"),
        ("cswap q[0], q[1], q[2];", "E0003"),
        ("u3(0.1, 0.2, 0.3) q[0];", "E0004"),
        ("qreg q[2];", "E0002"),
    ];

    for (construct, expected_code) in cases {
        let src = format!("OPENQASM 3.0;\nqubit[3] q;\nbit[3] c;\n{construct}\n");
        let res = lower_qasm_str(&src, None, None);
        assert!(
            !res.is_ok(),
            "Expected lowering failure for unsupported construct: {construct}"
        );
        assert!(
            res.output.is_none(),
            "Output must be None on failure for: {construct}"
        );
        assert!(
            res.diagnostics
                .iter()
                .any(|d| d.code.as_deref() == Some(expected_code)),
            "Expected diagnostic code {expected_code} for '{construct}', got: {:?}",
            res.diagnostics
        );
    }
}

#[test]
fn test_negative_semantic_errors_fail_closed() {
    let cases = [
        // 1. Undeclared register
        ("h unk[0];", "E0012"),
        // 2. Index out of bounds
        ("h q[5];", "E0014"),
        // 3. Register type mismatch
        ("h c[0];", "E0013"),
        // 4. Gate param arity mismatch (h takes 0 params)
        ("h(1.0) q[0];", "E0021"),
        // 5. Gate qubit arity mismatch (h takes 1 qubit)
        ("h q[0], q[1];", "E0022"),
        // 6. Duplicate operands in multi-qubit gate
        ("cx q[0], q[0];", "E0023"),
        // 7. Division by zero in parameter
        ("rx(1.0 / 0.0) q[0];", "E0030"),
        // 8. Unresolved symbol in parameter
        ("rx(unknown_sym) q[0];", "E0031"),
        // 9. Whole-register measurement size mismatch
        ("c = measure q;", "E0025"),
    ];

    for (stmt, expected_code) in cases {
        // q has size 2, c has size 3 (for size mismatch test)
        let src = format!("OPENQASM 3.0;\nqubit[2] q;\nbit[3] c;\n{stmt}\n");
        let res = lower_qasm_str(&src, None, None);
        assert!(
            !res.is_ok(),
            "Expected lowering failure for statement: {stmt}"
        );
        assert!(
            res.output.is_none(),
            "Output must be None on failure for: {stmt}"
        );
        assert!(
            res.diagnostics
                .iter()
                .any(|d| d.code.as_deref() == Some(expected_code)),
            "Expected diagnostic code {expected_code} for '{stmt}', got: {:?}",
            res.diagnostics
        );
    }
}

#[test]
fn test_negative_duplicate_register_declaration() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nqubit[2] q;\n";
    let res = lower_qasm_str(src, None, None);
    assert!(!res.is_ok());
    assert!(res
        .diagnostics
        .iter()
        .any(|d| d.code.as_deref() == Some("E0010")));
}

#[test]
fn test_negative_zero_size_register() {
    let src = "OPENQASM 3.0;\nqubit[0] q;\n";
    let res = lower_qasm_str(src, None, None);
    assert!(!res.is_ok());
    assert!(res
        .diagnostics
        .iter()
        .any(|d| d.code.as_deref() == Some("E0011")));
}

#[test]
fn test_openqasm_frontend_adapter_lower_method() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n";
    let buffer = SourceBuffer::new("bell_adapter.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.lower(&buffer, None);
    assert!(res.is_ok());
    let circuit = res.output.unwrap();
    assert_eq!(circuit.name, "bell_adapter");
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.graph.node_count(), 2);

    let res_named = frontend.lower_with_name(&buffer, "custom_bell", None);
    assert!(res_named.is_ok());
    assert_eq!(res_named.output.unwrap().name, "custom_bell");
}

#[test]
fn test_independent_openqasm3_parser_on_qasm_example_artifact() {
    let artifact_path = Path::new("examples/qasm/bell.qasm");
    let content =
        fs::read_to_string(artifact_path).expect("Failed to read examples/qasm/bell.qasm artifact");

    // 1. QCC frontend lowers the artifact cleanly
    let circuit = lower_success(&content, None);
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert_eq!(circuit.graph.node_count(), 4);

    // 2. Pinned independent Python openqasm3 parser validation
    let py_check = Command::new("python3")
        .arg("-c")
        .arg("import sys; sys.path.insert(0, '/tmp/py_packages'); import openqasm3; print(openqasm3.__version__)")
        .output();

    if let Ok(out) = py_check {
        if out.status.success() {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            assert_eq!(version, "1.0.1", "Expected pinned openqasm3 version 1.0.1");

            let py_run = Command::new("python3")
                .arg("-c")
                .arg(
                    r#"
import sys
sys.path.insert(0, '/tmp/py_packages')
import openqasm3

with open('examples/qasm/bell.qasm') as f:
    src = f.read()
ast = openqasm3.parse(src)
assert ast.version == '3.0', f'Expected version 3.0, got {ast.version}'
assert len(ast.statements) == 7, f'Expected 7 statements, got {len(ast.statements)}'
print(f'OPENQASM3_VALIDATION_OK: parsed {len(ast.statements)} statements')
"#,
                )
                .output()
                .expect("Failed to execute python3 parser");

            assert!(
                py_run.status.success(),
                "Independent openqasm3 parser failed on examples/qasm/bell.qasm: {}",
                String::from_utf8_lossy(&py_run.stderr)
            );
        }
    }
}
