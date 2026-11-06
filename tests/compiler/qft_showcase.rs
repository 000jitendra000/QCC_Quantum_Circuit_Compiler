//! Comprehensive regression test suite for QFT compilation, target legality,
//! SWAP accounting, and specialized Euler synthesis.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::fs;

use qcc::backend::openqasm::format::format_parameter_expr;
use qcc::compiler::pipeline::CompilerPipeline;
use qcc::frontend::{OpenQasmFrontend, SourceBuffer};
use qcc::gate_signature::ParameterDomain;
use qcc::ir::parameter::ParameterExpr;
use qcc::quantum::gate_matrix::matrix_h;
use qcc::simulator::execute_circuit;
use qcc::synthesis::single_qubit::SingleQubitSynthesizer;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::synthesis_basis::SingleQubitBasis;
use qcc::target::OperationConnectivity;
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Constructs a linear hardware target strictly supporting native {RZ, SX, CX}.
fn create_strict_linear_target(num_qubits: usize) -> Target {
    let mut builder = TargetBuilder::new("strict_linear", num_qubits);

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

    if !edges.is_empty() {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx_desc);
    }

    builder.build().expect("valid target")
}

/// Loads and lowers `examples/showcase.qasm` to QccIR.
fn load_showcase_circuit() -> qcc::ir::circuit::Circuit {
    let qasm_text =
        fs::read_to_string("examples/showcase.qasm").expect("Read examples/showcase.qasm");
    let buffer = SourceBuffer::new("examples/showcase.qasm", qasm_text);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.lower(&buffer, None);
    assert!(
        result.is_ok(),
        "Frontend lowering diagnostics: {:?}",
        result.diagnostics
    );
    result.output.expect("Circuit must be parsed")
}

#[test]
fn test_strict_native_basis_legality() {
    let circuit = load_showcase_circuit();
    let target = create_strict_linear_target(4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline
        .compile(&circuit, &target)
        .expect("QFT compilation must succeed");

    assert!(
        result.legality_report.is_valid(),
        "Output circuit must pass legality validation"
    );
    assert_eq!(result.legality_report.violations.len(), 0);

    // Verify all physical operations belong strictly to {rz, sx, cx}
    for node in result.physical_circuit.graph.ordered_nodes().unwrap() {
        let name = node.operation.name().to_ascii_lowercase();
        assert!(
            matches!(name.as_str(), "rz" | "sx" | "cx"),
            "Unexpected non-native operation '{name}' in final circuit"
        );
    }
}

#[test]
fn test_no_remaining_h_or_swap_when_excluded() {
    let circuit = load_showcase_circuit();
    let target = create_strict_linear_target(4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline
        .compile(&circuit, &target)
        .expect("QFT compilation must succeed");

    for node in result.physical_circuit.graph.ordered_nodes().unwrap() {
        let name = node.operation.name().to_ascii_lowercase();
        assert_ne!(name, "h", "Hadamard must be decomposed into native basis");
        assert_ne!(name, "swap", "SWAP must be decomposed into native basis");
    }
}

#[test]
fn test_qft_computational_basis_behavior() {
    let circuit = load_showcase_circuit();
    let target = create_strict_linear_target(4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline
        .compile(&circuit, &target)
        .expect("QFT compilation must succeed");

    // Simulate input |0000> on physical circuit:
    // QFT(|0000>) = 1/4 * sum_{k=0}^{15} |k>
    let (state, _) = execute_circuit(&result.physical_circuit).expect("Simulation must succeed");

    let probs = state
        .probabilities()
        .expect("Probabilities computation must succeed");
    let expected_prob = 1.0 / 16.0;

    for (k, prob) in probs.probabilities().iter().enumerate() {
        assert!(
            (prob - expected_prob).abs() < 1e-6,
            "State |{k}> probability discrepancy: got {prob}, expected {expected_prob}"
        );
    }
}

#[test]
fn test_semantic_equivalence_and_global_phase() {
    let circuit = load_showcase_circuit();
    let target = create_strict_linear_target(4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline
        .compile(&circuit, &target)
        .expect("Compilation must succeed");

    assert!(
        result.report.metrics.is_verified,
        "Layout equivalence check must succeed"
    );

    let checker = LayoutEquivalenceChecker::new();
    let verdict = checker
        .check_layout_equivalence(&circuit, &result.physical_circuit, &result.layout)
        .expect("Checker must complete without error");

    assert!(
        verdict.is_equivalent,
        "Circuit must be unitarily equivalent up to global phase"
    );
    assert!(
        verdict.max_amplitude_difference < 1e-6,
        "Max amplitude diff {:.2e} exceeds tolerance",
        verdict.max_amplitude_difference
    );
}

#[test]
fn test_swap_accounting_accuracy() {
    let circuit = load_showcase_circuit();
    let target = create_strict_linear_target(4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline
        .compile(&circuit, &target)
        .expect("Compilation must succeed");

    let m = &result.report.metrics;

    // Original QFT circuit has 2 bit-reversal SWAPs: swap(q0, q3) and swap(q1, q2)
    assert_eq!(m.original_swaps, 2, "Original QFT has exactly 2 SWAP gates");

    // SABRE inserts 3 routing SWAPs
    assert_eq!(
        m.router_swaps_inserted, 3,
        "Router inserted exactly 3 SWAPs"
    );
    assert_eq!(
        m.swaps_inserted, 3,
        "swaps_inserted alias must match router_swaps_inserted"
    );

    // Final physical circuit has 0 SWAPs since SWAP is not native on {rz, sx, cx}
    assert_eq!(m.final_swaps, 0, "Final physical circuit has 0 SWAPs");
}

#[test]
fn test_specialized_hadamard_synthesis() {
    let target = create_strict_linear_target(2);
    let synth = SingleQubitSynthesizer::new();
    let p0 = PhysicalQubitId::new(0);

    let seq = synth
        .synthesize_with_basis(&matrix_h(), p0, &target, SingleQubitBasis::RzSx)
        .expect("Hadamard synthesis must succeed");

    // Must be exactly 3 operations: Rz(pi/2) -> SX -> Rz(pi/2)
    assert_eq!(
        seq.operations().len(),
        3,
        "Specialized H must be exactly 3 gates"
    );
    assert_eq!(seq.operations()[0].name(), "rz");
    assert_eq!(seq.operations()[1].name(), "sx");
    assert_eq!(seq.operations()[2].name(), "rz");

    // Check angle values: pi / 2
    let theta0 = seq.operations()[0].parameters()[0].as_constant().unwrap();
    let theta2 = seq.operations()[2].parameters()[0].as_constant().unwrap();
    assert!((theta0 - FRAC_PI_2).abs() < 1e-12);
    assert!((theta2 - FRAC_PI_2).abs() < 1e-12);

    // Global phase must be pi / 4
    let phase = seq.global_phase().angle().radians();
    assert!((phase - FRAC_PI_4).abs() < 1e-12);

    // Reconstructed matrix must match H exactly
    let rec = seq.reconstruct_matrix();
    assert!(rec.max_difference(&matrix_h()) < 1e-10);
}

#[test]
fn test_parameter_expression_constant_folding() {
    let expr = ParameterExpr::Add(
        Box::new(ParameterExpr::Constant(FRAC_PI_2)),
        Box::new(ParameterExpr::Constant(PI)),
    );
    let formatted = format_parameter_expr(&expr).expect("Formatting must succeed");

    // Must be folded into a single numeric float (approx 4.71238898038469), NOT "1.57... + 3.14..."
    assert!(
        !formatted.contains('+'),
        "Constant additions must be folded: got '{formatted}'"
    );
    let val: f64 = formatted
        .parse()
        .expect("Formatted expression must be a valid float");
    assert!((val - (FRAC_PI_2 + PI)).abs() < 1e-10);
}
