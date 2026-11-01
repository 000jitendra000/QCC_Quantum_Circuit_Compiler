//! Integration and unit tests for OpenQASM 3 register declaration parsing and operand resolution.

use qcc::frontend::ast::{AstBitRef, AstQubitRef, AstRegisterKind};
use qcc::frontend::parser::declarations::DeclarationParser;
use qcc::frontend::parser::registers::{
    BitId, ClassicalRegisterId, QuantumRegisterId, QubitId, RegisterKind, RegisterTable,
};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::{Frontend, OpenQasmFrontend};

#[test]
fn test_multiple_quantum_and_classical_registers() {
    let src = "\
OPENQASM 3.0;
include \"stdgates.inc\";
qubit[2] q1;
qubit[3] q2;
bit[4] c1;
bit[2] c2;
";
    let buffer = SourceBuffer::new("multi_reg.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);
    let prog = result.into_result().unwrap();
    assert_eq!(prog.statements.len(), 4);

    assert_eq!(table.quantum_registers().len(), 2);
    assert_eq!(table.classical_registers().len(), 2);
    assert_eq!(table.total_qubits(), 5);
    assert_eq!(table.total_bits(), 6);

    let q1 = table.get_quantum("q1").unwrap();
    assert_eq!(q1.id, QuantumRegisterId(0));
    assert_eq!(q1.size, 2);
    assert_eq!(q1.base_index, 0);

    let q2 = table.get_quantum("q2").unwrap();
    assert_eq!(q2.id, QuantumRegisterId(1));
    assert_eq!(q2.size, 3);
    assert_eq!(q2.base_index, 2);

    let c1 = table.get_classical("c1").unwrap();
    assert_eq!(c1.id, ClassicalRegisterId(0));
    assert_eq!(c1.size, 4);
    assert_eq!(c1.base_index, 0);

    let c2 = table.get_classical("c2").unwrap();
    assert_eq!(c2.id, ClassicalRegisterId(1));
    assert_eq!(c2.size, 2);
    assert_eq!(c2.base_index, 4);
}

#[test]
fn test_scalar_and_array_declarations() {
    let src = "\
qubit[4] q_arr;
qubit q_scalar;
bit[3] c_arr;
bit c_scalar;
";
    let buffer = SourceBuffer::new("scalar_array.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);

    let q_arr = table.get_quantum("q_arr").unwrap();
    assert!(!q_arr.is_scalar);
    assert_eq!(q_arr.size, 4);
    assert_eq!(q_arr.base_index, 0);

    let q_scalar = table.get_quantum("q_scalar").unwrap();
    assert!(q_scalar.is_scalar);
    assert_eq!(q_scalar.size, 1);
    assert_eq!(q_scalar.base_index, 4);

    let c_arr = table.get_classical("c_arr").unwrap();
    assert!(!c_arr.is_scalar);
    assert_eq!(c_arr.size, 3);
    assert_eq!(c_arr.base_index, 0);

    let c_scalar = table.get_classical("c_scalar").unwrap();
    assert!(c_scalar.is_scalar);
    assert_eq!(c_scalar.size, 1);
    assert_eq!(c_scalar.base_index, 3);

    assert_eq!(table.total_qubits(), 5);
    assert_eq!(table.total_bits(), 4);
}

#[test]
fn test_correct_typed_id_assignment_and_stable_identity() {
    let mut table = RegisterTable::new();
    let dummy_span = qcc::diagnostics::SourceSpan::point(0, 1, 1).unwrap();

    let q0_id = table.declare_quantum("q0", Some(2), dummy_span).unwrap();
    let q1_id = table.declare_quantum("q1", None, dummy_span).unwrap();
    let c0_id = table.declare_classical("c0", Some(3), dummy_span).unwrap();
    let c1_id = table.declare_classical("c1", None, dummy_span).unwrap();

    assert_eq!(q0_id, QuantumRegisterId(0));
    assert_eq!(q1_id, QuantumRegisterId(1));
    assert_eq!(c0_id, ClassicalRegisterId(0));
    assert_eq!(c1_id, ClassicalRegisterId(1));

    // Stable retrieval by ID
    assert_eq!(table.get_quantum_by_id(q0_id).unwrap().name, "q0");
    assert_eq!(table.get_quantum_by_id(q1_id).unwrap().name, "q1");
    assert_eq!(table.get_classical_by_id(c0_id).unwrap().name, "c0");
    assert_eq!(table.get_classical_by_id(c1_id).unwrap().name, "c1");
}

#[test]
fn test_declaration_order_preservation() {
    let src = "\
qubit q_first;
bit c_second;
qubit[3] q_third;
bit[2] c_fourth;
";
    let buffer = SourceBuffer::new("order.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);

    let order = table.declaration_order();
    assert_eq!(order.len(), 4);
    assert_eq!(order[0].name(), "q_first");
    assert_eq!(order[0].kind(), RegisterKind::Quantum);

    assert_eq!(order[1].name(), "c_second");
    assert_eq!(order[1].kind(), RegisterKind::Classical);

    assert_eq!(order[2].name(), "q_third");
    assert_eq!(order[2].kind(), RegisterKind::Quantum);

    assert_eq!(order[3].name(), "c_fourth");
    assert_eq!(order[3].kind(), RegisterKind::Classical);
}

#[test]
fn test_version_and_include_parsing() {
    let src = "\
OPENQASM 3.0;
include \"stdgates.inc\";
include \"custom.inc\";
qubit q;
";
    let buffer = SourceBuffer::new("version_inc.qasm", src);
    let (tokens, diags) = qcc::frontend::Lexer::tokenize(&buffer);
    assert!(diags.is_empty());

    let mut parser = DeclarationParser::new(&buffer, &tokens);

    let version = parser
        .parse_version()
        .unwrap()
        .expect("version must be present");
    assert_eq!(version.specifier, "3.0");

    let (includes, inc_diags) = parser.parse_includes();
    assert!(inc_diags.is_empty());
    assert_eq!(includes.len(), 2);
    assert_eq!(includes[0].path, "stdgates.inc");
    assert_eq!(includes[1].path, "custom.inc");

    let mut table = RegisterTable::new();
    let decl = parser.parse_register_decl(&mut table).unwrap();
    assert_eq!(decl.name, "q");
    assert_eq!(decl.kind, AstRegisterKind::Qubit);
    assert!(decl.size.is_none());
}

#[test]
fn test_zero_sized_declarations_rejected() {
    let src_quantum = "qubit[0] q_zero;";
    let buf_q = SourceBuffer::new("zero_q.qasm", src_quantum);
    let frontend = OpenQasmFrontend::new();
    let res_q = frontend.parse(&buf_q);
    assert!(!res_q.is_ok(), "Expected qubit[0] to be rejected");
    assert!(res_q.errors().any(|d| d.code.as_deref() == Some("E0011")));

    let src_classical = "bit[0] c_zero;";
    let buf_c = SourceBuffer::new("zero_c.qasm", src_classical);
    let res_c = frontend.parse(&buf_c);
    assert!(!res_c.is_ok(), "Expected bit[0] to be rejected");
    assert!(res_c.errors().any(|d| d.code.as_deref() == Some("E0011")));
}

#[test]
fn test_duplicate_register_names_rejected() {
    // Duplicate quantum register
    let src_dup_q = "qubit[2] q;\nqubit[3] q;";
    let buf_dup_q = SourceBuffer::new("dup_q.qasm", src_dup_q);
    let frontend = OpenQasmFrontend::new();
    let res_dup_q = frontend.parse(&buf_dup_q);
    assert!(!res_dup_q.is_ok());
    assert!(res_dup_q
        .errors()
        .any(|d| d.code.as_deref() == Some("E0010")));

    // Cross-kind duplicate collision (qubit then bit with same name)
    let src_collision = "qubit[2] shared;\nbit[2] shared;";
    let buf_coll = SourceBuffer::new("collision.qasm", src_collision);
    let res_coll = frontend.parse(&buf_coll);
    assert!(!res_coll.is_ok());
    assert!(res_coll
        .errors()
        .any(|d| d.code.as_deref() == Some("E0010")));
}

#[test]
fn test_undeclared_register_references_rejected() {
    let src = "qubit[2] q;\nh ghost[0];";
    let buffer = SourceBuffer::new("ghost.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(!result.is_ok());
    assert!(result.errors().any(|d| d.code.as_deref() == Some("E0012")));
}

#[test]
fn test_valid_first_and_last_indices() {
    let src = "\
qubit[5] q;
bit[5] c;
h q[0];
h q[4];
c[0] = measure q[0];
c[4] = measure q[4];
";
    let buffer = SourceBuffer::new("boundary_indices.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(
        result.is_ok(),
        "Expected boundary indices 0 and 4 to succeed: {:?}",
        result.diagnostics
    );

    // Verify resolving first and last indices
    let span = qcc::diagnostics::SourceSpan::point(0, 1, 1).unwrap();
    let q0 = table
        .resolve_qubit(&AstQubitRef::indexed("q", 0, span))
        .unwrap();
    assert_eq!(q0.flat_qubit_id, QubitId(0));
    assert_eq!(q0.index_in_register, 0);

    let q4 = table
        .resolve_qubit(&AstQubitRef::indexed("q", 4, span))
        .unwrap();
    assert_eq!(q4.flat_qubit_id, QubitId(4));
    assert_eq!(q4.index_in_register, 4);

    let c0 = table
        .resolve_bit(&AstBitRef::indexed("c", 0, span))
        .unwrap();
    assert_eq!(c0.flat_bit_id, BitId(0));

    let c4 = table
        .resolve_bit(&AstBitRef::indexed("c", 4, span))
        .unwrap();
    assert_eq!(c4.flat_bit_id, BitId(4));
}

#[test]
fn test_out_of_bounds_indices_rejected() {
    // Index equal to size (off-by-one check: size 3, index 3 is out of bounds)
    let src_eq = "qubit[3] q;\nh q[3];";
    let buf_eq = SourceBuffer::new("eq_bound.qasm", src_eq);
    let frontend = OpenQasmFrontend::new();
    let res_eq = frontend.parse(&buf_eq);
    assert!(!res_eq.is_ok());
    assert!(res_eq.errors().any(|d| d.code.as_deref() == Some("E0014")));

    // Index strictly greater than size
    let src_gt = "qubit[3] q;\nh q[10];";
    let buf_gt = SourceBuffer::new("gt_bound.qasm", src_gt);
    let res_gt = frontend.parse(&buf_gt);
    assert!(!res_gt.is_ok());
    assert!(res_gt.errors().any(|d| d.code.as_deref() == Some("E0014")));

    // Classical bit index equal to size
    let src_c_eq = "qubit[2] q;\nbit[2] c;\nc[2] = measure q[0];";
    let buf_c_eq = SourceBuffer::new("c_eq_bound.qasm", src_c_eq);
    let res_c_eq = frontend.parse(&buf_c_eq);
    assert!(!res_c_eq.is_ok());
    assert!(res_c_eq
        .errors()
        .any(|d| d.code.as_deref() == Some("E0014")));
}

#[test]
fn test_quantum_classical_type_mismatches_rejected() {
    // Classical register used as qubit operand
    let src_mismatch = "bit[2] c;\nh c[0];";
    let buffer = SourceBuffer::new("mismatch.qasm", src_mismatch);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(!result.is_ok());
    assert!(result.errors().any(|d| d.code.as_deref() == Some("E0013")));
}

#[test]
fn test_scalar_vs_array_indexing_mismatches_rejected() {
    // Scalar qubit indexed with [0]
    let src_scalar_idx = "qubit q;\nh q[0];";
    let buf_sc = SourceBuffer::new("scalar_idx.qasm", src_scalar_idx);
    let frontend = OpenQasmFrontend::new();
    let res_sc = frontend.parse(&buf_sc);
    assert!(!res_sc.is_ok());
    assert!(res_sc.errors().any(|d| d.code.as_deref() == Some("E0015")));

    // Array qubit referenced without index
    let src_arr_unidx = "qubit[2] q;\nh q;";
    let buf_arr = SourceBuffer::new("arr_unidx.qasm", src_arr_unidx);
    let res_arr = frontend.parse(&buf_arr);
    assert!(!res_arr.is_ok());
    assert!(res_arr.errors().any(|d| d.code.as_deref() == Some("E0016")));
}

#[test]
fn test_source_span_correctness() {
    let src = "qubit[4] target_q;\nbit[2] target_c;\n";
    let buffer = SourceBuffer::new("spans.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok());
    let q = table.get_quantum("target_q").unwrap();
    assert_eq!(q.span.start_line(), 1);
    assert_eq!(q.span.start_col(), 1);
    assert_eq!(q.span.end_line(), 1);
    assert_eq!(q.span.end_col(), 19);

    let c = table.get_classical("target_c").unwrap();
    assert_eq!(c.span.start_line(), 2);
    assert_eq!(c.span.start_col(), 1);
    assert_eq!(c.span.end_line(), 2);
    assert_eq!(c.span.end_col(), 17);
}

#[test]
fn test_malformed_declarations_diagnostics() {
    // Missing semicolon
    let src_missing_semi = "qubit[2] q\nh q[0];";
    let buf_semi = SourceBuffer::new("no_semi.qasm", src_missing_semi);
    let frontend = OpenQasmFrontend::new();
    let res_semi = frontend.parse(&buf_semi);
    assert!(!res_semi.is_ok());
    assert!(res_semi.has_errors());

    // Missing closing bracket
    let src_no_rbracket = "qubit[2 q;";
    let buf_rb = SourceBuffer::new("no_rb.qasm", src_no_rbracket);
    let res_rb = frontend.parse(&buf_rb);
    assert!(!res_rb.is_ok());
    assert!(res_rb.has_errors());

    // Non-integer size in brackets
    let src_non_int = "qubit[two] q;";
    let buf_ni = SourceBuffer::new("non_int.qasm", src_non_int);
    let res_ni = frontend.parse(&buf_ni);
    assert!(!res_ni.is_ok());
    assert!(res_ni.has_errors());
}
