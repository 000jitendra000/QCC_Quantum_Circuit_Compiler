//! Comprehensive integration tests for OpenQASM 3 semantic analysis and diagnostics.
//!
//! Verifies:
//! - Undefined register references (`E0012`).
//! - Out-of-bounds register indices (`E0014`).
//! - Parameter arity (`E0021`) and qubit arity (`E0022`) mismatches.
//! - Invalid measurement destinations and sources (`E0013`).
//! - Whole-register measurement size mismatches (`E0025`).
//! - Unresolved parameter symbols (`E0031`) vs mathematical constants (`pi`, `π`).
//! - Division by zero (`E0030`) and non-finite values (`E0032`).
//! - Duplicate qubit operands in multi-qubit gates (`E0023`).
//! - Reset and barrier operand domain checks (`E0013`).
//! - Duplicate declarations (`E0010`) and zero sizes (`E0011`).
//! - Direct AST semantic validation bypassing parser checks.
//! - Source span precision and positive end-to-end semantic validation.

use std::collections::HashMap;

use qcc::diagnostics::SourceSpan;
use qcc::frontend::{
    AstExpression, AstGateCall, AstMeasure, AstProgram, AstQubitRef, AstRegisterDecl,
    AstRegisterKind, AstReset, AstStatement, OpenQasmFrontend, SemanticAnalyzer, SourceBuffer,
    SymbolTable,
};

// ============================================================================
// Helper Utilities
// ============================================================================

fn analyze_src(
    name: &str,
    src: &str,
    bindings: Option<HashMap<String, f64>>,
) -> Result<SymbolTable, Vec<qcc::diagnostics::Diagnostic>> {
    let buffer = SourceBuffer::new(name, src);
    let frontend = OpenQasmFrontend::new();
    let res = frontend.parse_and_analyze(&buffer, bindings);
    if res.has_errors() {
        Err(res.diagnostics)
    } else {
        let (_, symbols) = res.into_result().unwrap();
        Ok(symbols)
    }
}

// ============================================================================
// 1. Undefined Register Tests (E0012)
// ============================================================================

#[test]
fn test_undefined_register_in_gate() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
x undeclared[0];
"#;
    let errs = analyze_src("undef_gate.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undeclared'")),
        "Expected E0012 for undeclared register, got: {:?}",
        errs
    );
}

#[test]
fn test_undefined_register_in_multi_qubit_gate() {
    let src1 = r#"OPENQASM 3.0;
qubit[2] q;
cx undeclared[0], q[1];
"#;
    let errs1 = analyze_src("undef_cx1.qasm", src1, None).unwrap_err();
    assert!(
        errs1.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undeclared'")),
        "Expected E0012 for undeclared control in cx, got: {:?}",
        errs1
    );

    let src2 = r#"OPENQASM 3.0;
qubit[2] q;
cx q[0], undeclared[1];
"#;
    let errs2 = analyze_src("undef_cx2.qasm", src2, None).unwrap_err();
    assert!(
        errs2.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undeclared'")),
        "Expected E0012 for undeclared target in cx, got: {:?}",
        errs2
    );
}

#[test]
fn test_undefined_register_in_measurement() {
    let src_dest = r#"OPENQASM 3.0;
qubit[2] q;
undef_c[0] = measure q[0];
"#;
    let errs = analyze_src("undef_meas_dest.qasm", src_dest, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undef_c'")),
        "Expected E0012 for undeclared measurement destination, got: {:?}",
        errs
    );

    let src_src = r#"OPENQASM 3.0;
bit[2] c;
c[0] = measure undef_q[0];
"#;
    let errs = analyze_src("undef_meas_src.qasm", src_src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undef_q'")),
        "Expected E0012 for undeclared measurement source, got: {:?}",
        errs
    );
}

#[test]
fn test_undefined_register_in_reset_and_barrier() {
    let src_reset = r#"OPENQASM 3.0;
reset undef_q[0];
"#;
    let errs = analyze_src("undef_reset.qasm", src_reset, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undef_q'")),
        "Expected E0012 for undeclared operand in reset, got: {:?}",
        errs
    );

    let src_barrier = r#"OPENQASM 3.0;
barrier undef_q[0];
"#;
    let errs = analyze_src("undef_barrier.qasm", src_barrier, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0012")
            && d.message.contains("undeclared register 'undef_q'")),
        "Expected E0012 for undeclared operand in barrier, got: {:?}",
        errs
    );
}

// ============================================================================
// 2. Out-of-Range Bit and Qubit Index Tests (E0014)
// ============================================================================

#[test]
fn test_out_of_bounds_gate_indices() {
    // Exact boundary k = n
    let src_exact = r#"OPENQASM 3.0;
qubit[2] q;
x q[2];
"#;
    let errs = analyze_src("oob_exact.qasm", src_exact, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 2 out of bounds for register 'q' of size 2")),
        "Expected E0014 for index 2 out of bounds, got: {:?}",
        errs
    );

    // Far boundary k > n
    let src_far = r#"OPENQASM 3.0;
qubit[2] q;
h q[10];
"#;
    let errs = analyze_src("oob_far.qasm", src_far, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 10 out of bounds for register 'q' of size 2")),
        "Expected E0014 for index 10 out of bounds, got: {:?}",
        errs
    );
}

#[test]
fn test_out_of_bounds_measurement_indices() {
    // Classical destination out of bounds
    let src_c = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c[3] = measure q[0];
"#;
    let errs = analyze_src("oob_meas_c.qasm", src_c, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 3 out of bounds for register 'c' of size 2")),
        "Expected E0014 for bit index 3 out of bounds, got: {:?}",
        errs
    );

    // Quantum source out of bounds
    let src_q = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c[0] = measure q[5];
"#;
    let errs = analyze_src("oob_meas_q.qasm", src_q, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 5 out of bounds for register 'q' of size 2")),
        "Expected E0014 for qubit index 5 out of bounds, got: {:?}",
        errs
    );
}

#[test]
fn test_out_of_bounds_reset_and_barrier() {
    let src_reset = r#"OPENQASM 3.0;
qubit[2] q;
reset q[2];
"#;
    let errs = analyze_src("oob_reset.qasm", src_reset, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 2 out of bounds for register 'q' of size 2")),
        "Expected E0014 for reset index 2 out of bounds, got: {:?}",
        errs
    );

    let src_barrier = r#"OPENQASM 3.0;
qubit[2] q;
barrier q[0], q[4];
"#;
    let errs = analyze_src("oob_barrier.qasm", src_barrier, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0014")
            && d.message
                .contains("index 4 out of bounds for register 'q' of size 2")),
        "Expected E0014 for barrier index 4 out of bounds, got: {:?}",
        errs
    );
}

// ============================================================================
// 3. Wrong Gate Arity (Parameter Count E0021 & Qubit Count E0022)
// ============================================================================

#[test]
fn test_fixed_gate_with_parameters_rejected() {
    let src1 = r#"OPENQASM 3.0;
qubit[1] q;
h(0.5) q[0];
"#;
    let errs = analyze_src("h_with_param.qasm", src1, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0021")
            && d.message.contains("gate 'h' takes 0 parameter")),
        "Expected E0021 for h with parameter, got: {:?}",
        errs
    );

    let src2 = r#"OPENQASM 3.0;
qubit[2] q;
cx(1.0, 2.0) q[0], q[1];
"#;
    let errs = analyze_src("cx_with_param.qasm", src2, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0021")
            && d.message.contains("gate 'cx' takes 0 parameter")),
        "Expected E0021 for cx with parameters, got: {:?}",
        errs
    );
}

#[test]
fn test_parameterized_gate_parameter_count_mismatches() {
    let src_too_many = r#"OPENQASM 3.0;
qubit[1] q;
rx(0.1, 0.2) q[0];
"#;
    let errs = analyze_src("rx_too_many.qasm", src_too_many, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0021")
            && d.message
                .contains("gate 'rx' takes 1 parameter(s), but 2 provided")),
        "Expected E0021 for rx with 2 parameters, got: {:?}",
        errs
    );
}

#[test]
fn test_wrong_qubit_operand_arity() {
    // Single-qubit gate with 2 operands
    let src_h2 = r#"OPENQASM 3.0;
qubit[2] q;
h q[0], q[1];
"#;
    let errs = analyze_src("h_two_qubits.qasm", src_h2, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0022")
            && d.message
                .contains("gate 'h' takes 1 qubit operand(s), but 2 provided")),
        "Expected E0022 for h with 2 qubits, got: {:?}",
        errs
    );

    // Two-qubit gate with 1 operand
    let src_cx1 = r#"OPENQASM 3.0;
qubit[2] q;
cx q[0];
"#;
    let errs = analyze_src("cx_one_qubit.qasm", src_cx1, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0022")
            && d.message
                .contains("gate 'cx' takes 2 qubit operand(s), but 1 provided")),
        "Expected E0022 for cx with 1 qubit, got: {:?}",
        errs
    );

    // Two-qubit gate with 3 operands
    let src_cx3 = r#"OPENQASM 3.0;
qubit[3] q;
cx q[0], q[1], q[2];
"#;
    let errs = analyze_src("cx_three_qubits.qasm", src_cx3, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0022")
            && d.message
                .contains("gate 'cx' takes 2 qubit operand(s), but 3 provided")),
        "Expected E0022 for cx with 3 qubits, got: {:?}",
        errs
    );

    // Single-qubit gate with 0 operands
    let src_h0 = r#"OPENQASM 3.0;
qubit[1] q;
h;
"#;
    let errs_h0 = analyze_src("h_zero_qubits.qasm", src_h0, None).unwrap_err();
    assert!(
        errs_h0.iter().any(|d| d.code.as_deref() == Some("E0022")
            && d.message
                .contains("gate 'h' takes 1 qubit operand(s), but 0 provided")),
        "Expected E0022 for h with 0 qubits, got: {:?}",
        errs_h0
    );

    // Two-qubit gate with 0 operands
    let src_cx0 = r#"OPENQASM 3.0;
qubit[2] q;
cx;
"#;
    let errs_cx0 = analyze_src("cx_zero_qubits.qasm", src_cx0, None).unwrap_err();
    assert!(
        errs_cx0.iter().any(|d| d.code.as_deref() == Some("E0022")
            && d.message
                .contains("gate 'cx' takes 2 qubit operand(s), but 0 provided")),
        "Expected E0022 for cx with 0 qubits, got: {:?}",
        errs_cx0
    );
}

// ============================================================================
// 4. Invalid Measurement Destination and Source (E0013 & E0025)
// ============================================================================

#[test]
fn test_invalid_measurement_destination_is_qubit() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
qubit[1] r;
r[0] = measure q[0];
"#;
    let errs = analyze_src("meas_dest_qubit.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("qubit register")
            && d.message.contains("bit")),
        "Expected E0013 for qubit destination in measurement, got: {:?}",
        errs
    );
}

#[test]
fn test_invalid_measurement_source_is_classical() {
    let src = r#"OPENQASM 3.0;
bit[1] c;
bit[1] d;
d[0] = measure c[0];
"#;
    let errs = analyze_src("meas_src_bit.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("bit register")
            && d.message.contains("qubit")),
        "Expected E0013 for bit source in measurement, got: {:?}",
        errs
    );
}

#[test]
fn test_arrow_measurement_type_mismatch() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
qubit[1] r;
measure q[0] -> r[0];
"#;
    let errs = analyze_src("arrow_dest_qubit.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("qubit register")
            && d.message.contains("bit")),
        "Expected E0013 for arrow measurement destination qubit, got: {:?}",
        errs
    );
}

#[test]
fn test_whole_register_measurement_size_mismatch() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[3] c;
c = measure q;
"#;
    let errs = analyze_src("meas_size_mismatch.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0025")
            && d.message.contains("measurement register size mismatch: quantum register 'q' has size 2, but classical register 'c' has size 3")),
        "Expected E0025 for measurement size mismatch, got: {:?}",
        errs
    );
}

// ============================================================================
// 5. Unresolved Parameter Symbols (E0031) vs Mathematical Constants
// ============================================================================

#[test]
fn test_unresolved_parameter_in_gate_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta) q[0];
"#;
    let errs = analyze_src("unresolved_param.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0031")
            && d.message
                .contains("unresolved parameter 'theta' in gate 'rx'")),
        "Expected E0031 for unresolved theta, got: {:?}",
        errs
    );
}

#[test]
fn test_unresolved_parameter_in_composite_expression_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
ry(theta * 2 + pi / 4) q[0];
"#;
    let errs = analyze_src("unresolved_composite.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0031")
            && d.message
                .contains("unresolved parameter 'theta' in gate 'ry'")),
        "Expected E0031 for unresolved theta in composite expression, got: {:?}",
        errs
    );
}

#[test]
fn test_unresolved_parameter_in_global_phase_rejected() {
    let src = r#"OPENQASM 3.0;
gphase(phi);
"#;
    let errs = analyze_src("unresolved_gphase.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0031")
            && d.message
                .contains("unresolved parameter 'phi' in global phase directive 'gphase'")),
        "Expected E0031 for unresolved phi in gphase, got: {:?}",
        errs
    );
}

#[test]
fn test_mathematical_constants_pi_accepted_without_binding() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
rx(pi / 2) q[0];
ry(π / 4) q[1];
gphase(pi);
"#;
    let res = analyze_src("pi_constants.qasm", src, None);
    assert!(
        res.is_ok(),
        "Expected success for pi and π: {:?}",
        res.err()
    );
}

#[test]
fn test_bound_parameters_succeed() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta) q[0];
ry(theta + pi / 2) q[0];
"#;
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), std::f64::consts::FRAC_PI_2);

    let res = analyze_src("bound_param.qasm", src, Some(bindings));
    assert!(
        res.is_ok(),
        "Expected success with bound parameter: {:?}",
        res.err()
    );
}

// ============================================================================
// 6. Invalid Parameter Domains and Arithmetic Failures (E0030, E0032)
// ============================================================================

#[test]
fn test_division_by_zero_after_symbol_binding() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(pi / theta) q[0];
"#;
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), 0.0);

    let errs = analyze_src("div_zero_binding.qasm", src, Some(bindings)).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0030")
            && d.message
                .contains("division by zero in parameter expression")),
        "Expected E0030 for division by zero after binding, got: {:?}",
        errs
    );
}

#[test]
fn test_non_finite_parameter_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta) q[0];
"#;
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), f64::NAN);

    let errs = analyze_src("nan_param.qasm", src, Some(bindings)).unwrap_err();
    assert!(
        errs.iter()
            .any(|d| d.code.as_deref() == Some("E0032") && d.message.contains("non-finite")),
        "Expected E0032 for NaN parameter evaluation, got: {:?}",
        errs
    );

    let mut bindings_inf = HashMap::new();
    bindings_inf.insert("theta".to_string(), f64::INFINITY);

    let errs_inf = analyze_src("inf_param.qasm", src, Some(bindings_inf)).unwrap_err();
    assert!(
        errs_inf
            .iter()
            .any(|d| d.code.as_deref() == Some("E0032") && d.message.contains("non-finite")),
        "Expected E0032 for infinity parameter evaluation, got: {:?}",
        errs_inf
    );

    let mut bindings_neg_inf = HashMap::new();
    bindings_neg_inf.insert("theta".to_string(), f64::NEG_INFINITY);

    let errs_neg_inf = analyze_src("neg_inf_param.qasm", src, Some(bindings_neg_inf)).unwrap_err();
    assert!(
        errs_neg_inf
            .iter()
            .any(|d| d.code.as_deref() == Some("E0032") && d.message.contains("non-finite")),
        "Expected E0032 for negative infinity parameter evaluation, got: {:?}",
        errs_neg_inf
    );
}

#[test]
fn test_small_nonzero_denominator_accepted() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(1.0 / 1e-16) q[0];
ry(1e-20 / 1e-16) q[0];
"#;
    let res = analyze_src("small_denom.qasm", src, None);
    assert!(
        res.is_ok(),
        "Expected small nonzero denominator to evaluate cleanly without E0030: {:?}",
        res.err()
    );
}

#[test]
fn test_division_overflow_to_infinity_produces_e0032() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta / 1e-200) q[0];
"#;
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), 1e200);

    let errs = analyze_src("div_overflow.qasm", src, Some(bindings)).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0032")),
        "Expected E0032 for division overflow to infinity, got: {:?}",
        errs
    );
}

#[test]
fn test_invalid_parameter_domain_diagnostic_helper() {
    let span = SourceSpan::point(10, 2, 5).unwrap();
    let diag = qcc::diagnostics::qasm::invalid_parameter_domain(
        "gate 'custom' parameter 42 out of domain [0, 10]",
        span,
        "custom_domain.qasm",
    );
    assert_eq!(diag.code.as_deref(), Some("E0024"));
    assert_eq!(diag.span, Some(span));
    assert_eq!(diag.file_path.as_deref(), Some("custom_domain.qasm"));
    assert!(diag.message.contains("parameter 42 out of domain"));
}

#[test]
fn test_rotation_gates_accept_all_finite_real_parameters() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;

// Zero parameters
rx(0.0) q[0];
ry(0) q[1];
rz(0.0) q[2];
gphase(0.0);

// Negative parameters
rx(-pi) q[0];
ry(-1.5707963) q[1];
rz(-100.0) q[2];
gphase(-pi / 2);

// Large positive parameters
rx(100.0) q[0];
ry(1e5) q[1];
rz(360.0) q[2];
gphase(10.0);

// Bound parameters with arbitrary finite real values
rx(theta) q[0];
ry(phi) q[1];
rz(gamma) q[2];
"#;
    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), -42.5);
    bindings.insert("phi".to_string(), 1000000.0);
    bindings.insert("gamma".to_string(), 0.0);

    let res = analyze_src("all_real_params.qasm", src, Some(bindings));
    assert!(
        res.is_ok(),
        "Expected all finite real parameters to be accepted without error: {:?}",
        res.err()
    );
}

// ============================================================================
// 7. Reset and Barrier Operand Errors (E0013, E0015)
// ============================================================================

#[test]
fn test_reset_on_classical_bit_rejected() {
    let src = r#"OPENQASM 3.0;
bit[2] c;
reset c[0];
"#;
    let errs = analyze_src("reset_bit.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("bit register")
            && d.message.contains("qubit")),
        "Expected E0013 for reset on bit, got: {:?}",
        errs
    );
}

#[test]
fn test_barrier_on_classical_bit_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
barrier q[0], c[1];
"#;
    let errs = analyze_src("barrier_bit.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("bit register")
            && d.message.contains("qubit")),
        "Expected E0013 for barrier on bit, got: {:?}",
        errs
    );
}

// ============================================================================
// 8. Duplicate Qubit Operands in Multi-Qubit Gates (E0023)
// ============================================================================

#[test]
fn test_duplicate_qubits_in_cx_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
cx q[0], q[0];
"#;
    let errs = analyze_src("cx_dup.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0023")
            && d.message.contains("duplicate qubit operand in 'cx' gate")
            && d.message.contains("q[0]")),
        "Expected E0023 for duplicate operand in cx, got: {:?}",
        errs
    );
}

#[test]
fn test_duplicate_qubits_in_cz_and_swap_rejected() {
    let src_cz = r#"OPENQASM 3.0;
qubit[2] q;
cz q[1], q[1];
"#;
    let errs = analyze_src("cz_dup.qasm", src_cz, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0023")
            && d.message.contains("duplicate qubit operand in 'cz' gate")),
        "Expected E0023 for duplicate operand in cz, got: {:?}",
        errs
    );

    let src_swap = r#"OPENQASM 3.0;
qubit[2] q;
swap q[0], q[0];
"#;
    let errs = analyze_src("swap_dup.qasm", src_swap, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0023")
            && d.message.contains("duplicate qubit operand in 'swap' gate")),
        "Expected E0023 for duplicate operand in swap, got: {:?}",
        errs
    );
}

// ============================================================================
// 9. Unknown Gate Names & Case-Sensitivity Suggestions (E0020)
// ============================================================================

#[test]
fn test_unknown_gate_name_rejected_with_suggestion() {
    let src_h = r#"OPENQASM 3.0;
qubit[1] q;
H q[0];
"#;
    let errs = analyze_src("cap_h.qasm", src_h, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0020"))
        .unwrap();
    assert!(
        diag.message.contains("unknown gate 'H'")
            || diag.message.contains("unknown or unsupported gate 'H'")
    );
    assert!(diag.notes.iter().any(|n| n.contains("did you mean 'h'?")));

    let src_foo = r#"OPENQASM 3.0;
qubit[1] q;
foo q[0];
"#;
    let errs_foo = analyze_src("foo_gate.qasm", src_foo, None).unwrap_err();
    assert!(errs_foo
        .iter()
        .any(|d| d.code.as_deref() == Some("E0020") && d.message.contains("foo")));
}

// ============================================================================
// 10. Duplicate Register Declarations (E0010) & Zero Size (E0011)
// ============================================================================

#[test]
fn test_duplicate_register_declaration_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] q;
"#;
    let errs = analyze_src("dup_reg.qasm", src, None).unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0010")
            && d.message.contains("duplicate register declaration 'q'")),
        "Expected E0010 for duplicate register, got: {:?}",
        errs
    );
}

#[test]
fn test_zero_sized_register_declaration_rejected() {
    let src = r#"OPENQASM 3.0;
qubit[0] q;
"#;
    let errs = analyze_src("zero_size.qasm", src, None).unwrap_err();
    assert!(
        errs.iter()
            .any(|d| d.code.as_deref() == Some("E0011") && d.message.contains("invalid size 0")),
        "Expected E0011 for zero-size register, got: {:?}",
        errs
    );
}

// ============================================================================
// 11. Direct AST Semantic Validation (Bypassing Parser)
// ============================================================================

#[test]
fn test_direct_ast_semantic_validation_rejects_malformed_constructs() {
    let dummy_span = SourceSpan::point(0, 1, 1).unwrap();
    let buffer = SourceBuffer::new("direct_ast.qasm", "// Direct AST construction\n");

    // Case A: Gate call referencing undeclared register
    let prog_undef = AstProgram::new(
        None,
        Vec::new(),
        vec![AstStatement::GateCall(AstGateCall::new(
            "x".to_string(),
            Vec::new(),
            vec![AstQubitRef::indexed("ghost", 0, dummy_span)],
            dummy_span,
        ))],
        dummy_span,
    );
    let diags_undef = SemanticAnalyzer::new(&buffer)
        .analyze(&prog_undef)
        .unwrap_err();
    assert!(diags_undef
        .iter()
        .any(|d| d.code.as_deref() == Some("E0012")));

    // Case B: Gate call with duplicate operands
    let prog_dup = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::RegisterDecl(AstRegisterDecl::new(
                AstRegisterKind::Qubit,
                "q".to_string(),
                Some(2),
                dummy_span,
            )),
            AstStatement::GateCall(AstGateCall::new(
                "cx".to_string(),
                Vec::new(),
                vec![
                    AstQubitRef::indexed("q", 1, dummy_span),
                    AstQubitRef::indexed("q", 1, dummy_span),
                ],
                dummy_span,
            )),
        ],
        dummy_span,
    );
    let diags_dup = SemanticAnalyzer::new(&buffer)
        .analyze(&prog_dup)
        .unwrap_err();
    assert!(diags_dup.iter().any(|d| d.code.as_deref() == Some("E0023")));

    // Case C: Reset on classical bit
    let prog_reset_bit = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::RegisterDecl(AstRegisterDecl::new(
                AstRegisterKind::Bit,
                "c".to_string(),
                Some(2),
                dummy_span,
            )),
            AstStatement::Reset(AstReset::new(
                AstQubitRef::indexed("c", 0, dummy_span),
                dummy_span,
            )),
        ],
        dummy_span,
    );
    let diags_reset = SemanticAnalyzer::new(&buffer)
        .analyze(&prog_reset_bit)
        .unwrap_err();
    assert!(diags_reset
        .iter()
        .any(|d| d.code.as_deref() == Some("E0013")));

    // Case D: Measurement into qubit
    let prog_meas = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::RegisterDecl(AstRegisterDecl::new(
                AstRegisterKind::Qubit,
                "q".to_string(),
                Some(2),
                dummy_span,
            )),
            AstStatement::Measure(AstMeasure::new(
                AstQubitRef::indexed("q", 0, dummy_span),
                qcc::frontend::AstBitRef::indexed("q", 1, dummy_span),
                dummy_span,
            )),
        ],
        dummy_span,
    );
    let diags_meas = SemanticAnalyzer::new(&buffer)
        .analyze(&prog_meas)
        .unwrap_err();
    assert!(diags_meas
        .iter()
        .any(|d| d.code.as_deref() == Some("E0013")));

    // Case E: Unresolved parameter
    let prog_unresolved = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::RegisterDecl(AstRegisterDecl::new(
                AstRegisterKind::Qubit,
                "q".to_string(),
                Some(2),
                dummy_span,
            )),
            AstStatement::GateCall(AstGateCall::new(
                "rx".to_string(),
                vec![AstExpression::identifier("unbound_var", dummy_span)],
                vec![AstQubitRef::indexed("q", 0, dummy_span)],
                dummy_span,
            )),
        ],
        dummy_span,
    );
    let diags_param = SemanticAnalyzer::new(&buffer)
        .analyze(&prog_unresolved)
        .unwrap_err();
    assert!(diags_param
        .iter()
        .any(|d| d.code.as_deref() == Some("E0031")));
}

// ============================================================================
// 12. Source Span Accuracy
// ============================================================================

#[test]
fn test_semantic_source_spans_accuracy() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nx q[5];\n";
    let errs = analyze_src("spans.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0014"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 3);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 7);
    assert_eq!(diag.file_path.as_deref(), Some("spans.qasm"));
}

#[test]
fn test_undefined_register_source_span_precision() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nx undef_reg[0];\n";
    let errs = analyze_src("undef_span.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0012"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    // Line 3: "x undef_reg[0];" -> "undef_reg[0]" begins at column 3 and ends at column 15
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 3);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 15);
}

#[test]
fn test_parameter_error_source_span_precision() {
    let src = "OPENQASM 3.0;\nqubit[1] q;\nrx(theta) q[0];\n";
    let errs = analyze_src("param_span.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0031"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    // Line 3: "rx(theta) q[0];" -> "theta" is at columns 4..9
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 4);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 9);
}

#[test]
fn test_invalid_measurement_dest_source_span_precision() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nq[0] = measure q[1];\n";
    let errs = analyze_src("meas_span.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0013"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    // Line 3: "q[0] = measure q[1];" -> destination operand "q[0]" is at columns 1..5
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 1);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 5);
}

#[test]
fn test_reset_operand_error_source_span_precision() {
    let src = "OPENQASM 3.0;\nbit[2] c;\nreset c[0];\n";
    let errs = analyze_src("reset_span.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0013"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    // Line 3: "reset c[0];" -> operand "c[0]" is at columns 7..11
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 7);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 11);
}

#[test]
fn test_duplicate_qubit_operand_source_span_precision() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], q[0];\n";
    let errs = analyze_src("dup_span.qasm", src, None).unwrap_err();
    let diag = errs
        .iter()
        .find(|d| d.code.as_deref() == Some("E0023"))
        .unwrap();

    let span = diag.span.expect("Diagnostic must have a source span");
    // Line 3: "cx q[0], q[0];" -> duplicate second operand "q[0]" is at columns 10..14
    assert_eq!(span.start_line(), 3);
    assert_eq!(span.start_col(), 10);
    assert_eq!(span.end_line(), 3);
    assert_eq!(span.end_col(), 14);
}

#[test]
fn test_missing_operand_diagnostic_span() {
    // Case 1: 0 operands for single-qubit gate "h;" -> points to gate identifier "h"
    let src_h = "OPENQASM 3.0;\nqubit[1] q;\nh;\n";
    let errs_h = analyze_src("missing_h.qasm", src_h, None).unwrap_err();
    let diag_h = errs_h
        .iter()
        .find(|d| d.code.as_deref() == Some("E0022"))
        .unwrap();
    let span_h = diag_h.span.expect("Must have span");
    assert_eq!(span_h.start_line(), 3);
    assert_eq!(span_h.start_col(), 1);
    assert_eq!(span_h.end_line(), 3);
    assert_eq!(span_h.end_col(), 2);

    // Case 2: 0 operands for two-qubit gate "cx;" -> points to gate identifier "cx"
    let src_cx0 = "OPENQASM 3.0;\nqubit[2] q;\ncx;\n";
    let errs_cx0 = analyze_src("missing_cx0.qasm", src_cx0, None).unwrap_err();
    let diag_cx0 = errs_cx0
        .iter()
        .find(|d| d.code.as_deref() == Some("E0022"))
        .unwrap();
    let span_cx0 = diag_cx0.span.expect("Must have span");
    assert_eq!(span_cx0.start_line(), 3);
    assert_eq!(span_cx0.start_col(), 1);
    assert_eq!(span_cx0.end_line(), 3);
    assert_eq!(span_cx0.end_col(), 3);

    // Case 3: 1 operand for two-qubit gate "cx q[0];" -> points to "cx q[0]"
    let src_cx1 = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0];\n";
    let errs_cx1 = analyze_src("missing_cx1.qasm", src_cx1, None).unwrap_err();
    let diag_cx1 = errs_cx1
        .iter()
        .find(|d| d.code.as_deref() == Some("E0022"))
        .unwrap();
    let span_cx1 = diag_cx1.span.expect("Must have span");
    assert_eq!(span_cx1.start_line(), 3);
    assert_eq!(span_cx1.start_col(), 1);
    assert_eq!(span_cx1.end_line(), 3);
    assert_eq!(span_cx1.end_col(), 8);

    // Case 4: Extra operand retains coverage: "h q[0], q[1];" -> points to "h q[0], q[1]"
    let src_extra = "OPENQASM 3.0;\nqubit[2] q;\nh q[0], q[1];\n";
    let errs_extra = analyze_src("extra_h.qasm", src_extra, None).unwrap_err();
    let diag_extra = errs_extra
        .iter()
        .find(|d| d.code.as_deref() == Some("E0022"))
        .unwrap();
    let span_extra = diag_extra.span.expect("Must have span");
    assert_eq!(span_extra.start_line(), 3);
    assert_eq!(span_extra.start_col(), 1);
    assert_eq!(span_extra.end_line(), 3);
    assert_eq!(span_extra.end_col(), 13);
}

// ============================================================================
// 13. Full Positive End-to-End Semantic Validation
// ============================================================================

#[test]
fn test_valid_program_semantic_analysis_success() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;
bit[3] c;

h q[0];
cx q[0], q[1];
rx(pi / 4) q[2];
barrier q[0], q[1], q[2];
c[0] = measure q[0];
c[1] = measure q[1];
reset q[2];
barrier;
"#;
    let res = analyze_src("valid_full.qasm", src, None);
    assert!(
        res.is_ok(),
        "Expected valid program to pass semantic analysis: {:?}",
        res.err()
    );

    let symbols = res.unwrap();
    assert_eq!(symbols.total_qubits(), 3);
    assert_eq!(symbols.total_bits(), 3);
    assert_eq!(symbols.quantum_registers().len(), 1);
    assert_eq!(symbols.classical_registers().len(), 1);
}

#[test]
fn test_frontend_parse_and_analyze_success() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
"#;
    let buffer = SourceBuffer::new("bell_front.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let res = frontend.parse_and_analyze(&buffer, None);

    assert!(res.is_ok(), "Failed: {:?}", res.diagnostics);
    let (program, symbols) = res.into_result().unwrap();
    assert_eq!(program.statements.len(), 6);
    assert_eq!(symbols.total_qubits(), 2);
    assert_eq!(symbols.total_bits(), 2);
}

#[test]
fn test_frontend_parse_and_analyze_with_bound_parameters() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(theta) q[0];
ry(theta + pi / 4) q[0];
"#;
    let buffer = SourceBuffer::new("param_front.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let mut bindings = HashMap::new();
    bindings.insert("theta".to_string(), std::f64::consts::FRAC_PI_4);

    let res = frontend.parse_and_analyze(&buffer, Some(bindings));
    assert!(
        res.is_ok(),
        "Failed with bound params: {:?}",
        res.diagnostics
    );
    let (program, symbols) = res.into_result().unwrap();
    assert_eq!(program.statements.len(), 3);
    assert_eq!(
        symbols.parameter_value("theta"),
        Some(std::f64::consts::FRAC_PI_4)
    );
}
