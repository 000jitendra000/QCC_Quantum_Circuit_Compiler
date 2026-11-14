//! Integration and property-based test suite for Logical SWAP Elision (ADR-0006).
//!
//! Validates:
//! 1. Unit invariants: wire tracking, inverse maps, swap chains, self-swaps, and arity bounds.
//! 2. Edge-case preservation: measurement classical destinations, barriers, resets, global phase.
//! 3. Pipeline CX gate reduction:
//!    - `04_logical_swap`: 3 CX -> 0 CX (-3 CX).
//!    - `07_qft4_showcase_linear`: 27 CX -> 21 CX (-6 CX).
//!    - `08_qft4_showcase_ring`: 24 CX -> 18 CX (-6 CX).
//! 4. ADR-0001 clean ancilla preservation on partially occupied physical targets.
//! 5. Pass diagnostic reporting: `logical_swap_elision` with `PassStatus::Ran` and `Skipped`.
//! 6. Mandatory negative fault-injection: omitting or corrupting layout permutation fails closed.
//! 7. Property-based testing with `proptest`: 100% equivalence on randomized circuits with dense SWAPs.

use std::f64::consts::PI;
use std::fs;

use proptest::prelude::*;

use qcc::builder::CircuitBuilder;
use qcc::compiler::config::CompilerConfig;
use qcc::compiler::pass_diagnostic::{PassDiagnosticConfig, PassStatus};
use qcc::compiler::pipeline::CompilerPipeline;
use qcc::frontend::{OpenQasmFrontend, SourceBuffer};
use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::ir::error::IrError;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::optimizer::passes::{elide_logical_swaps, ElidedSwapPermutation};
use qcc::routing::layout::CircuitLayout;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

fn parse_qasm_helper(name: &str, qasm: &str) -> Circuit {
    let frontend = OpenQasmFrontend::new();
    let buffer = SourceBuffer::new(name, qasm);
    frontend.lower(&buffer, None).output.unwrap()
}

/// Helper constructing a standard hardware target with strict `{rz, sx, cx}` native basis.
fn make_test_target(name: &str, num_qubits: usize, is_ring: bool) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let meas_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(500.0)
        .build();
    let reset_desc = TargetOperationBuilder::new("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(1000.0)
        .build();

    builder = builder
        .add_operation(sx_desc)
        .add_operation(rz_desc)
        .add_operation(meas_desc)
        .add_operation(reset_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((PhysicalQubitId(i as u32), PhysicalQubitId((i + 1) as u32)));
    }
    if is_ring && num_qubits > 2 {
        edges.push((PhysicalQubitId((num_qubits - 1) as u32), PhysicalQubitId(0)));
    }

    if !edges.is_empty() {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx_desc);
    }

    builder.build().expect("Failed to build target model")
}

// ============================================================================
// 1. Unit Invariant Tests
// ============================================================================

#[test]
fn test_default_config_elide_swaps_enabled() {
    let cfg = CompilerConfig::default();
    assert!(
        cfg.elide_swaps,
        "elide_swaps must be true by default following WP3 benchmark evidence"
    );
    assert!(
        cfg.optimization.elide_swaps,
        "optimizer elide_swaps must be true by default"
    );

    let disabled_cfg = CompilerConfig::default().with_elide_swaps(false);
    assert!(!disabled_cfg.elide_swaps);
    assert!(!disabled_cfg.optimization.elide_swaps);
}

#[test]
fn test_permutation_identity_properties() {
    let perm = ElidedSwapPermutation::identity(4);
    assert_eq!(perm.num_qubits(), 4);
    assert_eq!(perm.swaps_elided(), 0);
    assert!(perm.is_identity());
    for i in 0..4 {
        assert_eq!(perm.wire_of(i), Some(i));
        assert_eq!(perm.logical_of(i), Some(i));
    }
    assert_eq!(perm.wire_of(4), None);
    assert_eq!(perm.logical_of(4), None);
}

#[test]
fn test_permutation_swap_and_inverses() {
    let mut perm = ElidedSwapPermutation::identity(3);
    perm.apply_swap(0, 1).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert!(!perm.is_identity());
    assert_eq!(perm.logical_to_wire(), &[1, 0, 2]);
    assert_eq!(perm.wire_to_logical(), &[1, 0, 2]);

    perm.apply_swap(1, 2).unwrap();
    assert_eq!(perm.swaps_elided(), 2);
    assert_eq!(perm.logical_to_wire(), &[1, 2, 0]);
    assert_eq!(perm.wire_to_logical(), &[2, 0, 1]);

    // Check bidirectional lookups
    for (l, &w) in perm.logical_to_wire().iter().enumerate() {
        assert_eq!(perm.wire_of(l), Some(w));
        assert_eq!(perm.logical_of(w), Some(l));
    }
}

#[test]
fn test_permutation_out_of_bounds_rejection() {
    let mut perm = ElidedSwapPermutation::identity(2);
    assert!(matches!(
        perm.apply_swap(0, 2),
        Err(IrError::InvalidQubit { qubit: 2, total: 2 })
    ));
    assert!(matches!(
        perm.apply_swap(3, 1),
        Err(IrError::InvalidQubit { qubit: 3, total: 2 })
    ));
}

#[test]
fn test_self_swap_is_noop() {
    let mut perm = ElidedSwapPermutation::identity(3);
    perm.apply_swap(1, 1).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert!(perm.is_identity());
    assert_eq!(perm.logical_to_wire(), &[0, 1, 2]);
}

// ============================================================================
// 2. Circuit Elision Unit Tests
// ============================================================================

#[test]
fn test_terminal_swap_elision() {
    let mut b = CircuitBuilder::new("term_swap");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.t(q[0]).unwrap();
    b.x(q[1]).unwrap();
    b.rz(q[1], ParameterExpr::from(PI / 5.0)).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let c = b.finish().unwrap();

    let (elided, perm) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert_eq!(perm.logical_to_wire(), &[1, 0]);
    assert_eq!(elided.num_operations(), 4);

    // Verify no swap operation remains
    let remaining_swaps = elided
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name().eq_ignore_ascii_case("swap"))
        .count();
    assert_eq!(remaining_swaps, 0);
}

#[test]
fn test_start_swap_elision() {
    let mut b = CircuitBuilder::new("start_swap");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.swap(q[0], q[1]).unwrap();
    b.h(q[0]).unwrap();
    b.x(q[1]).unwrap();
    let c = b.finish().unwrap();

    let (elided, perm) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert_eq!(perm.logical_to_wire(), &[1, 0]);
    assert_eq!(elided.num_operations(), 2);

    let nodes = elided.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "h");
    assert_eq!(nodes[0].operation.qubits(), &[QubitId(1)]);
    assert_eq!(nodes[1].operation.name(), "x");
    assert_eq!(nodes[1].operation.qubits(), &[QubitId(0)]);
}

#[test]
fn test_middle_swap_elision() {
    let mut b = CircuitBuilder::new("middle_swap");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    b.x(q[0]).unwrap();
    let c = b.finish().unwrap();

    let (elided, perm) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert_eq!(perm.logical_to_wire(), &[1, 0]);
    assert_eq!(elided.num_operations(), 2);

    let nodes = elided.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "h");
    assert_eq!(nodes[0].operation.qubits(), &[QubitId(0)]);
    assert_eq!(nodes[1].operation.name(), "x");
    assert_eq!(nodes[1].operation.qubits(), &[QubitId(1)]);
}

#[test]
fn test_swap_chain_elision() {
    let mut b = CircuitBuilder::new("swap_chain");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.swap(q[0], q[1]).unwrap();
    b.swap(q[1], q[2]).unwrap();
    b.swap(q[2], q[3]).unwrap();
    let c = b.finish().unwrap();

    let (elided, perm) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm.swaps_elided(), 3);
    assert_eq!(perm.logical_to_wire(), &[1, 2, 3, 0]);
    assert_eq!(elided.num_operations(), 0);
}

#[test]
fn test_interleaved_barriers_resets_and_measurements() {
    let mut b = CircuitBuilder::new("non_unitary");
    let q = b.add_quantum_register("q", 2).unwrap();
    let cr = b.add_classical_register("c", 2).unwrap();

    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    b.barrier(&[q[0], q[1]]).unwrap();
    b.reset(q[0]).unwrap();
    b.measure(q[0], cr[0]).unwrap();
    b.measure(q[1], cr[1]).unwrap();
    let c = b.finish().unwrap();

    let (elided, perm) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm.swaps_elided(), 1);
    assert_eq!(perm.logical_to_wire(), &[1, 0]);

    let nodes = elided.graph.ordered_nodes().unwrap();
    assert_eq!(nodes.len(), 5);

    // H on wire 0
    assert_eq!(nodes[0].operation.qubits(), &[QubitId(0)]);
    // Barrier rewritten to active wires [1, 0]
    assert_eq!(nodes[1].operation.qubits(), &[QubitId(1), QubitId(0)]);
    // Reset rewritten to wire 1
    assert_eq!(nodes[2].operation.qubits(), &[QubitId(1)]);
    // Measurement of logical 0 reads wire 1 and writes classical bit c[0]
    if let qcc::ir::operation::Operation::Measure { qubit, destination } = &nodes[3].operation {
        assert_eq!(*qubit, QubitId(1));
        assert_eq!(*destination, ClassicalBitId(0));
    } else {
        panic!("expected measurement");
    }
    // Measurement of logical 1 reads wire 0 and writes classical bit c[1]
    if let qcc::ir::operation::Operation::Measure { qubit, destination } = &nodes[4].operation {
        assert_eq!(*qubit, QubitId(0));
        assert_eq!(*destination, ClassicalBitId(1));
    } else {
        panic!("expected measurement");
    }
}

#[test]
fn test_global_phase_preservation() {
    let mut b = CircuitBuilder::new("gp_circuit");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let mut c = b.finish().unwrap();
    c.set_global_phase(ParameterExpr::from(0.5));

    let (elided, _) = elide_logical_swaps(&c).unwrap();
    assert_eq!(elided.global_phase(), c.global_phase());
}

#[test]
fn test_elision_idempotence() {
    let mut b = CircuitBuilder::new("idempotence");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let c = b.finish().unwrap();

    let (elided1, perm1) = elide_logical_swaps(&c).unwrap();
    assert_eq!(perm1.swaps_elided(), 1);

    let (elided2, perm2) = elide_logical_swaps(&elided1).unwrap();
    assert_eq!(perm2.swaps_elided(), 0);
    assert!(perm2.is_identity());
    assert_eq!(elided1.num_operations(), elided2.num_operations());
}

// ============================================================================
// 3. Pipeline Integration & CX Reduction Tests
// ============================================================================

#[test]
fn test_compile_04_logical_swap_reduces_cx_to_zero() {
    let qasm = r#"
OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
h q[0];
t q[0];
x q[1];
rz(pi / 5) q[1];
swap q[0], q[1];
"#;
    let circuit = parse_qasm_helper("04_logical_swap.qasm", qasm);
    let target = make_test_target("test_linear", 2, false);

    // 1. Without elide-swaps (baseline): emits 3 CX
    // 1. Explicit opt-out (elide_swaps = false): emits 3 CX
    let baseline_cfg = CompilerConfig::default().with_elide_swaps(false);
    let baseline_res = CompilerPipeline::new(baseline_cfg)
        .compile(&circuit, &target)
        .unwrap();
    let baseline_cx = baseline_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        baseline_cx, 3,
        "explicit opt-out without elide-swaps must emit 3 CX for logical swap"
    );

    // 2. Default (with elide-swaps): emits 0 CX
    let elide_cfg = CompilerConfig::default();
    let elide_res = CompilerPipeline::new(elide_cfg)
        .compile(&circuit, &target)
        .unwrap();
    let elide_cx = elide_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        elide_cx, 0,
        "with --elide-swaps, emitted CX count must drop to 0 (-3 CX)"
    );

    // 3. Verify final layout absorbs the permutation: q0 -> p1, q1 -> p0
    assert_eq!(
        elide_res
            .layout
            .final_layout()
            .get_physical(QubitId(0))
            .unwrap(),
        PhysicalQubitId(1)
    );
    assert_eq!(
        elide_res
            .layout
            .final_layout()
            .get_physical(QubitId(1))
            .unwrap(),
        PhysicalQubitId(0)
    );

    // 4. Verify semantic equivalence passed
    assert!(elide_res.report.verification.is_equivalent);
}

#[test]
fn test_compile_07_qft4_showcase_linear_reduces_cx() {
    let qasm = fs::read_to_string("examples/showcase.qasm").expect("Read examples/showcase.qasm");
    let circuit = parse_qasm_helper("showcase.qasm", &qasm);
    let target = make_test_target("test_linear", 4, false);

    // Opt-out baseline: 27 CX
    let baseline_res = CompilerPipeline::new(CompilerConfig::default().with_elide_swaps(false))
        .compile(&circuit, &target)
        .unwrap();
    let baseline_cx = baseline_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(baseline_cx, 27);

    // Default (elide-swaps): 21 CX (-6 CX)
    let elide_cfg = CompilerConfig::default();
    let elide_res = CompilerPipeline::new(elide_cfg)
        .compile(&circuit, &target)
        .unwrap();
    let elide_cx = elide_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        elide_cx, 21,
        "QFT4 linear should drop by exactly 6 CX (from 27 to 21)"
    );
    assert!(elide_res.report.verification.is_equivalent);
}

#[test]
fn test_compile_08_qft4_showcase_ring_reduces_cx() {
    let qasm = fs::read_to_string("examples/showcase.qasm").expect("Read examples/showcase.qasm");
    let circuit = parse_qasm_helper("showcase.qasm", &qasm);
    let target = make_test_target("test_ring", 4, true);

    // Opt-out baseline: 24 CX
    let baseline_res = CompilerPipeline::new(CompilerConfig::default().with_elide_swaps(false))
        .compile(&circuit, &target)
        .unwrap();
    let baseline_cx = baseline_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(baseline_cx, 24);

    // Default (elide-swaps): 18 CX (-6 CX)
    let elide_cfg = CompilerConfig::default();
    let elide_res = CompilerPipeline::new(elide_cfg)
        .compile(&circuit, &target)
        .unwrap();
    let elide_cx = elide_res
        .physical_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        elide_cx, 18,
        "QFT4 ring should drop by exactly 6 CX (from 24 to 18)"
    );
    assert!(elide_res.report.verification.is_equivalent);
}

#[test]
fn test_adr0001_clean_ancilla_preservation_with_elide_swaps() {
    let mut b = CircuitBuilder::new("ancilla_test");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // 2 logical qubits on 4-qubit linear target (qubits 2 and 3 are idle ancillas)
    let target = make_test_target("test_linear", 4, false);
    let config = CompilerConfig::default().with_elide_swaps(true);
    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    assert!(result.report.verification.is_equivalent);
    let unused_phys = result.layout.final_layout().unused_physical_qubits();
    assert_eq!(unused_phys.len(), 2);

    let checker = LayoutEquivalenceChecker::new();
    let detailed = checker
        .check_layout_equivalence_detailed(&circuit, &result.physical_circuit, &result.layout)
        .unwrap();
    assert!(detailed.logical_equivalence_passed);
    assert!(detailed.ancilla_zero_state_passed);
    assert!(
        detailed.max_ancilla_leakage < 1e-12,
        "Ancilla leakage must be strictly zero"
    );
}

// ============================================================================
// 4. Pass Diagnostic Integration
// ============================================================================

#[test]
fn test_pass_diagnostic_generated_when_enabled() {
    let mut b = CircuitBuilder::new("diag_circ");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();
    let target = make_test_target("test_linear", 2, false);

    let diag_cfg = PassDiagnosticConfig::standard();
    let config = CompilerConfig::default()
        .with_elide_swaps(true)
        .with_diagnostics(diag_cfg);

    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    let elision_diag = result
        .pass_diagnostics
        .iter()
        .find(|d| d.pass_id == "logical_swap_elision");
    assert!(
        elision_diag.is_some(),
        "logical_swap_elision diagnostic report must be present"
    );
    let rep = elision_diag.unwrap();
    assert_eq!(rep.status, PassStatus::Ran);
    assert_eq!(rep.pass_name, "Logical SWAP Elision");
    assert_eq!(
        rep.pass_config.get("swaps_elided").unwrap(),
        &serde_json::json!(1)
    );
}

#[test]
fn test_pass_diagnostic_skipped_when_zero_swaps() {
    let mut b = CircuitBuilder::new("no_swaps");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    let circuit = b.finish().unwrap();
    let target = make_test_target("test_linear", 2, false);

    let diag_cfg = PassDiagnosticConfig::standard();
    let config = CompilerConfig::default()
        .with_elide_swaps(true)
        .with_diagnostics(diag_cfg);

    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    let elision_diag = result
        .pass_diagnostics
        .iter()
        .find(|d| d.pass_id == "logical_swap_elision");
    assert!(
        elision_diag.is_some(),
        "logical_swap_elision diagnostic report must be present"
    );
    let rep = elision_diag.unwrap();
    assert_eq!(rep.status, PassStatus::Skipped);
}

// ============================================================================
// 5. Mandatory Negative Fault-Injection Test
// ============================================================================

#[test]
fn test_fault_injection_tampered_final_layout_fails_verification() {
    let mut b = CircuitBuilder::new("tampered_layout");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.t(q[0]).unwrap();
    b.swap(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();
    let target = make_test_target("test_linear", 2, false);

    let config = CompilerConfig::default().with_elide_swaps(true);
    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    // The genuine compiled layout passes equivalence
    let checker = LayoutEquivalenceChecker::new();
    let verdict = checker
        .check_layout_equivalence(&circuit, &result.physical_circuit, &result.layout)
        .unwrap();
    assert!(verdict.is_equivalent);

    // FAULT INJECTION: Tamper with final layout by omitting the elided permutation
    // (set final_layout equal to initial_layout)
    let tampered_layout = CircuitLayout::with_layouts(
        result.layout.initial().clone(),
        result.layout.initial().clone(),
        result.layout.initial().clone(),
    )
    .unwrap();

    let tampered_verdict = checker
        .check_layout_equivalence(&circuit, &result.physical_circuit, &tampered_layout)
        .unwrap();
    assert!(
        !tampered_verdict.is_equivalent,
        "Tampered final layout (omitting permutation) MUST fail closed"
    );
    assert!(
        tampered_verdict.max_amplitude_difference > 0.5,
        "Mismatch error must be significant (> 0.5), got {}",
        tampered_verdict.max_amplitude_difference
    );
}

// ============================================================================
// 6. Property-Based Testing (proptest)
// ============================================================================

#[derive(Clone, Debug)]
enum RandomOp {
    H(usize),
    X(usize),
    T(usize),
    Rz(usize, f64),
    Cx(usize, usize),
    Swap(usize, usize),
}

fn random_op_strategy(num_qubits: usize) -> impl Strategy<Value = RandomOp> {
    prop_oneof![
        (0..num_qubits).prop_map(RandomOp::H),
        (0..num_qubits).prop_map(RandomOp::X),
        (0..num_qubits).prop_map(RandomOp::T),
        ((0..num_qubits), (-PI..PI)).prop_map(|(q, angle)| RandomOp::Rz(q, angle)),
        (0..num_qubits, 0..num_qubits)
            .prop_filter("distinct qubits", |(c, t)| c != t)
            .prop_map(|(c, t)| RandomOp::Cx(c, t)),
        (0..num_qubits, 0..num_qubits)
            .prop_filter("distinct qubits", |(a, b)| a != b)
            .prop_map(|(a, b)| RandomOp::Swap(a, b)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_property_random_circuit_swap_elision_equivalence(
        num_qubits in 2usize..=4,
        ops in prop::collection::vec(random_op_strategy(4), 5..=15),
    ) {
        let mut builder = CircuitBuilder::new("prop_circuit");
        let q = builder.add_quantum_register("q", num_qubits).unwrap();

        for op in &ops {
            match *op {
                RandomOp::H(i) if i < num_qubits => { builder.h(q[i]).unwrap(); }
                RandomOp::X(i) if i < num_qubits => { builder.x(q[i]).unwrap(); }
                RandomOp::T(i) if i < num_qubits => { builder.t(q[i]).unwrap(); }
                RandomOp::Rz(i, angle) if i < num_qubits => {
                    builder.rz(q[i], ParameterExpr::from(angle)).unwrap();
                }
                RandomOp::Cx(c, t) if c < num_qubits && t < num_qubits && c != t => {
                    builder.cx(q[c], q[t]).unwrap();
                }
                RandomOp::Swap(a, b) if a < num_qubits && b < num_qubits && a != b => {
                    builder.swap(q[a], q[b]).unwrap();
                }
                _ => {}
            }
        }

        let orig = builder.finish().unwrap();

        // 1. Direct elision check
        let (elided, _perm) = elide_logical_swaps(&orig).unwrap();

        // Ensure 0 remaining swaps
        let remaining_swaps = elided
            .graph
            .ordered_nodes()
            .unwrap()
            .iter()
            .filter(|n| n.operation.name().eq_ignore_ascii_case("swap"))
            .count();
        prop_assert_eq!(remaining_swaps, 0);

        // 2. Target compilation check
        let target = make_test_target("test_linear", num_qubits, false);
        let config = CompilerConfig::default().with_elide_swaps(true);
        let result = CompilerPipeline::new(config).compile(&orig, &target).unwrap();

        prop_assert!(
            result.report.verification.is_equivalent,
            "Verification failed: max_amp_diff={:?}",
            result.report.verification.max_amplitude_diff
        );
    }
}
