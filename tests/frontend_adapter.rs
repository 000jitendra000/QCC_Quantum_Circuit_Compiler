//! Integration tests for the OpenQASM 3 frontend adapter boundary.
//!
//! Validates:
//! 1. `SourceBuffer` coordinates, slicing, and 1-indexed `SourceSpan` accuracy.
//! 2. Parsing of the reconciled OpenQASM 3 subset (Bell state, standard gates, parameterized gates, measurement, reset, barrier).
//! 3. Precise source span preservation on AST nodes.
//! 4. Early rejection and diagnostics for OpenQASM 2 reserved keywords (`qreg`, `creg`).
//! 5. Early rejection and diagnostics for unsupported constructs (`ccx`, `cswap`, `while`, `defcal`, `extern`, `delay`).
//! 6. Error recovery and multi-diagnostic emission.
//! 7. Parser-swappability via `MockFrontend` (Exit Gate proof).

use qcc::diagnostics::DiagnosticLevel;
use qcc::frontend::{
    AstProgram, AstRegisterKind, AstStatement, Frontend, FrontendError, FrontendResult,
    OpenQasmFrontend, SourceBuffer,
};

#[test]
fn test_source_buffer_coordinates_and_slicing() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\n";
    let buffer = SourceBuffer::new("test.qasm", src);

    assert_eq!(buffer.line_count(), 4);
    assert_eq!(buffer.line_text(1), Some("OPENQASM 3.0;"));
    assert_eq!(buffer.line_text(2), Some("qubit[2] q;"));
    assert_eq!(buffer.line_text(3), Some("h q[0];"));

    // "qubit" on line 2, cols 1..6
    let span = buffer.span(14, 19).unwrap();
    assert_eq!(span.start_line(), 2);
    assert_eq!(span.start_col(), 1);
    assert_eq!(span.end_line(), 2);
    assert_eq!(span.end_col(), 6);
    assert_eq!(buffer.slice(span), Some("qubit"));
}

#[test]
fn test_parse_bell_state_corpus() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/bell.qasm")
        .expect("bell.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Expected successful parse of bell.qasm, got errors: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    assert_eq!(
        program.version.as_ref().map(|v| v.specifier.as_str()),
        Some("3.0")
    );
    assert_eq!(program.includes.len(), 1);
    assert_eq!(program.includes[0].path, "stdgates.inc");

    // Statements: qubit[2] q; bit[2] c; h q[0]; cx q[0], q[1]; c[0] = measure q[0]; c[1] = measure q[1];
    assert_eq!(program.statements.len(), 6);

    match &program.statements[0] {
        AstStatement::RegisterDecl(reg) => {
            assert_eq!(reg.kind, AstRegisterKind::Qubit);
            assert_eq!(reg.name, "q");
            assert_eq!(reg.size, Some(2));
        }
        other => panic!("expected RegisterDecl, got {:?}", other),
    }

    match &program.statements[1] {
        AstStatement::RegisterDecl(reg) => {
            assert_eq!(reg.kind, AstRegisterKind::Bit);
            assert_eq!(reg.name, "c");
            assert_eq!(reg.size, Some(2));
        }
        other => panic!("expected RegisterDecl, got {:?}", other),
    }

    match &program.statements[2] {
        AstStatement::GateCall(gate) => {
            assert_eq!(gate.name, "h");
            assert_eq!(gate.qubits.len(), 1);
            assert_eq!(gate.qubits[0].name, "q");
            assert_eq!(gate.qubits[0].index, Some(0));
        }
        other => panic!("expected GateCall(h), got {:?}", other),
    }

    match &program.statements[3] {
        AstStatement::GateCall(gate) => {
            assert_eq!(gate.name, "cx");
            assert_eq!(gate.qubits.len(), 2);
            assert_eq!(gate.qubits[0].to_string(), "q[0]");
            assert_eq!(gate.qubits[1].to_string(), "q[1]");
        }
        other => panic!("expected GateCall(cx), got {:?}", other),
    }

    match &program.statements[4] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.to_string(), "q[0]");
            assert_eq!(m.bit.to_string(), "c[0]");
        }
        other => panic!("expected Measure, got {:?}", other),
    }
}

#[test]
fn test_parse_registers_corpus() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/registers.qasm")
        .expect("registers.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Expected successful parse of registers.qasm: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    // Declarations: qubit[5] q_arr; qubit single_q; bit[5] c_arr; bit single_c;
    assert_eq!(program.statements.len(), 8);

    match &program.statements[0] {
        AstStatement::RegisterDecl(r) => {
            assert_eq!(r.kind, AstRegisterKind::Qubit);
            assert_eq!(r.name, "q_arr");
            assert_eq!(r.size, Some(5));
        }
        _ => panic!("unexpected statement 0"),
    }

    match &program.statements[1] {
        AstStatement::RegisterDecl(r) => {
            assert_eq!(r.kind, AstRegisterKind::Qubit);
            assert_eq!(r.name, "single_q");
            assert_eq!(r.size, None); // Scalar register
        }
        _ => panic!("unexpected statement 1"),
    }
}

#[test]
fn test_parse_standard_gates_corpus() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/standard_gates.qasm")
        .expect("standard_gates.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Expected successful parse of standard_gates.qasm: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    // qubit[3] q; then x, y, z, h, s, sdg, t, tdg, cx, cz, swap (12 statements)
    assert_eq!(program.statements.len(), 12);

    let gate_names: Vec<&str> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            AstStatement::GateCall(g) => Some(g.name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(
        gate_names,
        vec!["x", "y", "z", "h", "s", "sdg", "t", "tdg", "cx", "cz", "swap"]
    );
}

#[test]
fn test_parse_measurement_assignment_and_arrow_syntax() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/measurement.qasm")
        .expect("measurement.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Expected successful parse of measurement.qasm: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    // Statements: qubit[2] q; bit[2] c; c[0] = measure q[0]; measure q[1] -> c[1];
    assert_eq!(program.statements.len(), 4);

    // Assignment syntax: c[0] = measure q[0];
    match &program.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.to_string(), "q[0]");
            assert_eq!(m.bit.to_string(), "c[0]");
        }
        other => panic!("expected Measure, got {:?}", other),
    }

    // Arrow syntax: measure q[1] -> c[1];
    match &program.statements[3] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.to_string(), "q[1]");
            assert_eq!(m.bit.to_string(), "c[1]");
        }
        other => panic!("expected Measure, got {:?}", other),
    }
}

#[test]
fn test_parse_barrier_and_reset_corpus() {
    let barrier_buf = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/barrier.qasm")
        .expect("barrier.qasm must exist");
    let reset_buf = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/reset.qasm")
        .expect("reset.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let b_res = frontend.parse(&barrier_buf);
    assert!(
        b_res.is_ok(),
        "barrier.qasm failed: {:?}",
        b_res.diagnostics
    );
    let b_prog = b_res.into_result().unwrap();

    // Check barrier statements
    let barriers: Vec<&AstStatement> = b_prog
        .statements
        .iter()
        .filter(|s| matches!(s, AstStatement::Barrier(_)))
        .collect();
    assert_eq!(barriers.len(), 2);
    match barriers[0] {
        AstStatement::Barrier(b) => assert!(b.qubits.is_empty(), "Global barrier has no qubits"),
        _ => unreachable!(),
    }
    match barriers[1] {
        AstStatement::Barrier(b) => {
            assert_eq!(b.qubits.len(), 2);
            assert_eq!(b.qubits[0].to_string(), "q[0]");
            assert_eq!(b.qubits[1].to_string(), "q[1]");
        }
        _ => unreachable!(),
    }

    let r_res = frontend.parse(&reset_buf);
    assert!(r_res.is_ok(), "reset.qasm failed: {:?}", r_res.diagnostics);
    let r_prog = r_res.into_result().unwrap();
    let resets: Vec<&AstStatement> = r_prog
        .statements
        .iter()
        .filter(|s| matches!(s, AstStatement::Reset(_)))
        .collect();
    assert_eq!(resets.len(), 2);
}

#[test]
fn test_parse_parameterized_gates_and_expressions() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";
qubit[2] q;

rx(pi / 2) q[0];
ry(pi) q[0];
rz(2 * pi - 0.5) q[0];
gphase(pi / 4);
"#;
    let buffer = SourceBuffer::new("param.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Parameterized gates parse failed: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    assert_eq!(program.statements.len(), 5);

    // Check rx(pi / 2)
    match &program.statements[1] {
        AstStatement::GateCall(g) => {
            assert_eq!(g.name, "rx");
            assert_eq!(g.parameters.len(), 1);
            let val = g.parameters[0]
                .eval(&std::collections::HashMap::new())
                .unwrap();
            assert!((val - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        }
        _ => panic!("expected GateCall rx"),
    }

    // Check gphase(pi / 4)
    match &program.statements[4] {
        AstStatement::GlobalPhase(gp) => {
            let val = gp.angle.eval(&std::collections::HashMap::new()).unwrap();
            assert!((val - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        }
        _ => panic!("expected GlobalPhase"),
    }
}

#[test]
fn test_source_span_preservation() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\n";
    let buffer = SourceBuffer::new("span_test.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let prog = frontend.parse(&buffer).into_result().unwrap();

    // Verify version span
    let v_span = prog.version.unwrap().span;
    assert_eq!(buffer.slice(v_span), Some("OPENQASM 3.0;"));

    // Verify register span
    let r_span = prog.statements[0].span();
    assert_eq!(buffer.slice(r_span), Some("qubit[2] q;"));

    // Verify gate span
    let g_span = prog.statements[1].span();
    assert_eq!(buffer.slice(g_span), Some("h q[0];"));
}

#[test]
fn test_invalid_syntax_diagnostics() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/invalid_syntax.qasm")
        .expect("invalid_syntax.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        !result.is_ok(),
        "Expected invalid_syntax.qasm to fail parsing"
    );
    assert!(result.has_errors());

    // Check diagnostic properties
    for diag in result.errors() {
        assert_eq!(diag.level, DiagnosticLevel::Error);
        assert!(diag.code.is_some());
        assert!(diag.span.is_some());
        assert!(diag.file_path.is_some());
    }
}

#[test]
fn test_reserved_keywords_rejection() {
    let buffer = SourceBuffer::from_file(
        "experiments/oq3-evaluation/corpus/registers_reserved_keywords.qasm",
    )
    .expect("registers_reserved_keywords.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        !result.is_ok(),
        "Expected reserved keywords file to fail parsing"
    );
    assert!(result.has_errors());

    // Must find diagnostic with code E0002 referencing reserved keywords
    let has_e0002 = result.errors().any(|d| d.code.as_deref() == Some("E0002"));
    assert!(
        has_e0002,
        "Expected E0002 diagnostic for reserved keywords 'qreg'/'creg'"
    );

    // Also test OpenQASM 2 style declaration: qreg q[5];
    let q2_src = "qreg q[5];\ncreg c[5];";
    let q2_buf = SourceBuffer::new("q2.qasm", q2_src);
    let q2_res = frontend.parse(&q2_buf);
    assert!(!q2_res.is_ok());
    let msgs: Vec<&str> = q2_res.errors().map(|d| d.message.as_str()).collect();
    assert!(msgs.iter().any(|m| m.contains("OpenQASM 2 keyword 'qreg'")));
    assert!(msgs.iter().any(|m| m.contains("OpenQASM 2 keyword 'creg'")));
}

#[test]
fn test_unsupported_features_early_rejection() {
    let buffer =
        SourceBuffer::from_file("experiments/oq3-evaluation/corpus/unsupported_features.qasm")
            .expect("unsupported_features.qasm must exist");
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(
        !result.is_ok(),
        "Expected unsupported_features.qasm to be rejected"
    );
    assert!(result.has_errors());

    let error_messages: Vec<String> = result.errors().map(|d| d.message.clone()).collect();
    assert!(
        error_messages
            .iter()
            .any(|m| m.contains("classical control flow loops ('while'/'for')")),
        "Expected diagnostic rejecting while loop"
    );
    assert!(
        error_messages
            .iter()
            .any(|m| m.contains("pulse calibration definitions ('defcal')")),
        "Expected diagnostic rejecting defcal"
    );
    assert!(
        error_messages
            .iter()
            .any(|m| m.contains("extern symbol declarations")),
        "Expected diagnostic rejecting extern"
    );
}

#[test]
fn test_unsupported_three_qubit_gates_rejection() {
    let src = "OPENQASM 3.0;\nqubit[3] q;\nccx q[0], q[1], q[2];\ncswap q[0], q[1], q[2];\n";
    let buffer = SourceBuffer::new("three_qubit.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(!result.is_ok(), "Expected ccx and cswap to be rejected");

    let codes: Vec<Option<&str>> = result.errors().map(|d| d.code.as_deref()).collect();
    assert!(
        codes.contains(&Some("E0003")),
        "Expected E0003 code for three-qubit gates"
    );
}

#[test]
fn test_unsupported_derived_gates_rejection() {
    let src = "OPENQASM 3.0;\nqubit[1] q;\np(0.5) q[0];\nu3(0.1, 0.2, 0.3) q[0];\n";
    let buffer = SourceBuffer::new("derived.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let result = frontend.parse(&buffer);
    assert!(!result.is_ok(), "Expected p and u3 to be rejected");

    let codes: Vec<Option<&str>> = result.errors().map(|d| d.code.as_deref()).collect();
    assert!(
        codes.contains(&Some("E0004")),
        "Expected E0004 code for unsupported derived gates"
    );
}

// ============================================================================
// Phase 16 Exit Gate Proof: Parser Swappability via MockFrontend
// ============================================================================

/// A mock frontend demonstrating that compiler consumers depend strictly on the
/// [`Frontend`] interface and AST data structures, without any coupling to concrete
/// parsing technology or third-party crates.
struct MockFrontend {
    tag: &'static str,
}

impl MockFrontend {
    fn new(tag: &'static str) -> Self {
        Self { tag }
    }
}

impl Frontend for MockFrontend {
    fn name(&self) -> &'static str {
        self.tag
    }

    fn parse(&self, buffer: &SourceBuffer) -> FrontendResult<AstProgram> {
        // Synthesizes a mock AST program with 2 qubits and an H gate
        let span = buffer
            .full_span()
            .unwrap_or_else(|_| qcc::SourceSpan::point(0, 1, 1).unwrap());
        let reg = AstStatement::RegisterDecl(qcc::frontend::AstRegisterDecl {
            kind: AstRegisterKind::Qubit,
            name: "mock_q".to_string(),
            size: Some(2),
            span,
        });
        let gate = AstStatement::GateCall(qcc::frontend::AstGateCall {
            name: "h".to_string(),
            parameters: Vec::new(),
            qubits: vec![qcc::frontend::AstQubitRef {
                name: "mock_q".to_string(),
                index: Some(0),
                span,
            }],
            span,
        });

        FrontendResult::success(AstProgram::new(None, Vec::new(), vec![reg, gate], span))
    }
}

/// Generic consumer illustrating that frontend implementations are 100% interchangeable
/// and isolated from compiler core.
fn count_statements(
    frontend: &dyn Frontend,
    buffer: &SourceBuffer,
) -> Result<usize, FrontendError> {
    let result = frontend.parse(buffer);
    let program = result.into_result()?;
    Ok(program.statements.len())
}

#[test]
fn test_frontend_swappability_exit_gate() {
    let buffer = SourceBuffer::new("test.qasm", "qubit[2] q;\nh q[0];\n");

    let oq3_frontend = OpenQasmFrontend::new();
    let mock_frontend = MockFrontend::new("mock-adapter");

    // Both frontends can be passed to the exact same consumer function
    let count_oq3 = count_statements(&oq3_frontend, &buffer).expect("oq3 parse should succeed");
    let count_mock = count_statements(&mock_frontend, &buffer).expect("mock parse should succeed");

    assert_eq!(count_oq3, 2);
    assert_eq!(count_mock, 2);

    // Verify names
    assert_eq!(oq3_frontend.name(), "openqasm3");
    assert_eq!(mock_frontend.name(), "mock-adapter");
}
