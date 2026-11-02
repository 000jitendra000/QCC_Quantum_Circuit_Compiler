//! Tests for detection and rejection of multi-qubit and standard decomposition-required gates.

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_three_qubit_gates() {
    let cases = [
        (
            "ccx.qasm",
            "OPENQASM 3.0;\nqubit[3] q;\nccx q[0], q[1], q[2];\n",
            "ccx",
        ),
        (
            "cswap.qasm",
            "OPENQASM 3.0;\nqubit[3] q;\ncswap q[0], q[1], q[2];\n",
            "cswap",
        ),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, gate_name) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {gate_name} to be rejected");
        assert!(res.output.is_none());

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single error for {gate_name}");
        assert_eq!(diags[0].code.as_deref(), Some("E0003"));
        assert!(diags[0].message.contains("three-qubit gate"));

        let (program, recovering_diags) = frontend.parse_recovering(&buffer);
        assert_eq!(recovering_diags.len(), 1);
        let stmts = program.statements();
        assert_eq!(stmts.len(), 2);
        match &stmts[1] {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "three_qubit_gate");
                assert!(unsupp.raw.contains(gate_name));
            }
            other => panic!("expected Unsupported statement, got {other:?}"),
        }
    }
}

#[test]
fn test_reject_decomposition_required_standard_gates() {
    let cases = [
        (
            "u1.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nu1(0.5) q[0];\n",
            "u1",
        ),
        (
            "u2.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nu2(0.1, 0.2) q[0];\n",
            "u2",
        ),
        (
            "u3.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nu3(0.1, 0.2, 0.3) q[0];\n",
            "u3",
        ),
        (
            "u.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nu(0.1, 0.2, 0.3) q[0];\n",
            "u",
        ),
        ("p.qasm", "OPENQASM 3.0;\nqubit[1] q;\np(0.5) q[0];\n", "p"),
        ("sx.qasm", "OPENQASM 3.0;\nqubit[1] q;\nsx q[0];\n", "sx"),
        (
            "sxdg.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nsxdg q[0];\n",
            "sxdg",
        ),
        (
            "cy.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncy q[0], q[1];\n",
            "cy",
        ),
        (
            "ch.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\nch q[0], q[1];\n",
            "ch",
        ),
        (
            "crx.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncrx(0.5) q[0], q[1];\n",
            "crx",
        ),
        (
            "cry.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncry(0.5) q[0], q[1];\n",
            "cry",
        ),
        (
            "crz.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncrz(0.5) q[0], q[1];\n",
            "crz",
        ),
        (
            "cu.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncu(0.1, 0.2, 0.3, 0.4) q[0], q[1];\n",
            "cu",
        ),
        (
            "rxx.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\nrxx(0.5) q[0], q[1];\n",
            "rxx",
        ),
        (
            "ryy.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\nryy(0.5) q[0], q[1];\n",
            "ryy",
        ),
        (
            "rzz.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\nrzz(0.5) q[0], q[1];\n",
            "rzz",
        ),
    ];

    let frontend = OpenQasmFrontend::new();

    for (name, src, gate_name) in cases {
        let buffer = SourceBuffer::new(name, src);
        let res = frontend.parse(&buffer);
        assert!(!res.is_ok(), "Expected {gate_name} to be rejected");
        assert!(res.output.is_none());

        let diags: Vec<_> = res.errors().collect();
        assert_eq!(diags.len(), 1, "Expected single error for {gate_name}");
        assert_eq!(
            diags[0].code.as_deref(),
            Some("E0004"),
            "Expected E0004 for {gate_name}"
        );
        assert!(diags[0].message.contains("without decomposition"));

        let (program, recovering_diags) = frontend.parse_recovering(&buffer);
        assert_eq!(recovering_diags.len(), 1);
        let stmts = program.statements();
        assert_eq!(stmts.len(), 2);
        match &stmts[1] {
            AstStatement::Unsupported(unsupp) => {
                assert_eq!(unsupp.kind, "unsupported_gate");
                assert!(unsupp.raw.contains(gate_name));
            }
            other => panic!("expected Unsupported statement, got {other:?}"),
        }
    }
}

#[test]
fn test_distinguish_unknown_gates_from_unsupported_gates() {
    let src = "OPENQASM 3.0;\nqubit[1] q;\nfoobar q[0];\n";
    let buffer = SourceBuffer::new("unknown_gate.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].code.as_deref(),
        Some("E0020"),
        "Unknown gates must produce E0020, not E0004 or E0005"
    );
}
