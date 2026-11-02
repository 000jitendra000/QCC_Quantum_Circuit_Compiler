//! Tests for detection and rejection of unsupported classical scalar types, modifiers, timing, and legacy keywords.

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_classical_scalar_types() {
    let cases = [
        (
            "int_type.qasm",
            "OPENQASM 3.0;\nint[32] count = 0;\n",
            "int",
        ),
        ("uint_type.qasm", "OPENQASM 3.0;\nuint[16] u = 1;\n", "uint"),
        (
            "float_type.qasm",
            "OPENQASM 3.0;\nfloat[64] f = 1.0;\n",
            "float",
        ),
        ("angle_type.qasm", "OPENQASM 3.0;\nangle[32] a;\n", "angle"),
        (
            "bool_type.qasm",
            "OPENQASM 3.0;\nbool flag = true;\n",
            "bool",
        ),
        (
            "complex_type.qasm",
            "OPENQASM 3.0;\ncomplex[float[64]] c;\n",
            "complex",
        ),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, kw) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {kw} to be rejected");

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single diagnostic for {kw}");
        assert_eq!(diags[0].code.as_deref(), Some("E0005"));
        assert!(diags[0].message.contains("classical scalar memory types"));

        let (program, _) = frontend.parse_recovering(&buffer);
        assert_eq!(program.statements().len(), 1);
        match &program.statements()[0] {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "classical_type");
                assert!(unsupp.raw.contains(kw));
            }
            other => panic!("expected Unsupported statement for {kw}, got {other:?}"),
        }
    }
}

#[test]
fn test_reject_modifiers_and_directives() {
    let cases = [
        (
            "const.qasm",
            "OPENQASM 3.0;\nconst int[32] MAX = 100;\n",
            "const",
        ),
        (
            "readonly.qasm",
            "OPENQASM 3.0;\nreadonly bit[2] ro;\n",
            "readonly",
        ),
        ("pragma.qasm", "OPENQASM 3.0;\npragma unroll;\n", "pragma"),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, kw) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {kw} to be rejected");

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single diagnostic for {kw}");
        assert_eq!(diags[0].code.as_deref(), Some("E0005"));

        let (program, _) = frontend.parse_recovering(&buffer);
        assert_eq!(program.statements().len(), 1);
        match &program.statements()[0] {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "modifier_or_directive");
                assert!(unsupp.raw.contains(kw));
            }
            other => panic!("expected Unsupported statement for {kw}, got {other:?}"),
        }
    }
}

#[test]
fn test_reject_timing_constructs() {
    let cases = [
        (
            "delay.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\ndelay[100ns] q[0];\n",
            "delay",
        ),
        (
            "duration.qasm",
            "OPENQASM 3.0;\nduration d = 100ns;\n",
            "duration",
        ),
        ("stretch.qasm", "OPENQASM 3.0;\nstretch s;\n", "stretch"),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, kw) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {kw} to be rejected");

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single diagnostic for {kw}");
        assert_eq!(diags[0].code.as_deref(), Some("E0005"));

        let (program, _) = frontend.parse_recovering(&buffer);
        let last_stmt = program.statements().last().unwrap();
        match last_stmt {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "timing");
                assert!(unsupp.raw.contains(kw));
            }
            other => panic!("expected Unsupported statement for {kw}, got {other:?}"),
        }
    }
}

#[test]
fn test_reject_legacy_qasm2_keywords() {
    let cases = [
        ("qreg.qasm", "OPENQASM 3.0;\nqreg q[5];\n", "qreg"),
        ("creg.qasm", "OPENQASM 3.0;\ncreg c[5];\n", "creg"),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, kw) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {kw} to be rejected");

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single diagnostic for {kw}");
        assert_eq!(diags[0].code.as_deref(), Some("E0002"));
        assert!(diags[0].message.contains("OpenQASM 2 keyword"));

        let (program, _) = frontend.parse_recovering(&buffer);
        assert_eq!(program.statements().len(), 1);
        match &program.statements()[0] {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "legacy_keyword");
                assert!(unsupp.raw.contains(kw));
            }
            other => panic!("expected Unsupported statement for {kw}, got {other:?}"),
        }
    }
}
