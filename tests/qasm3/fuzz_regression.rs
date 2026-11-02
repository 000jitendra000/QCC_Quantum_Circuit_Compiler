//! Fuzz regression tests for QCC OpenQASM 3 frontend.
//!
//! Validates:
//! - All corpus files under `fuzz/corpus/` process without panicking.
//! - Deeply nested expressions, large operand lists, long numeric literals,
//!   and unclosed tokens are cleanly bounded without recursion stack overflow or OOM.

use std::fs;
use std::path::Path;

use qcc::frontend::adapter::OpenQasmFrontend;
use qcc::frontend::lexer::{Lexer, LexerLimits};
use qcc::frontend::source::SourceBuffer;

#[test]
fn test_fuzz_regression_lexer_corpus() {
    let corpus_dir = Path::new("fuzz/corpus/lexer");
    if !corpus_dir.exists() {
        return;
    }

    for entry in fs::read_dir(corpus_dir)
        .expect("Read lexer corpus dir")
        .flatten()
    {
        if entry.path().is_file() {
            let data = fs::read_to_string(entry.path()).expect("Read corpus file");
            let buffer = SourceBuffer::new(entry.path().to_str().unwrap(), data);
            let (tokens, diags) = Lexer::tokenize(&buffer);
            assert!(
                !tokens.is_empty(),
                "Tokens must not be empty for {:?}",
                entry.path()
            );
            // Diagnostics may or may not be empty depending on corpus file, but no panic
            let _ = diags;
        }
    }
}

#[test]
fn test_fuzz_regression_parser_corpus() {
    let corpus_dir = Path::new("fuzz/corpus/parser");
    if !corpus_dir.exists() {
        return;
    }

    let frontend = OpenQasmFrontend::new();

    for entry in fs::read_dir(corpus_dir)
        .expect("Read parser corpus dir")
        .flatten()
    {
        if entry.path().is_file() {
            let data = fs::read_to_string(entry.path()).expect("Read corpus file");
            let buffer = SourceBuffer::new(entry.path().to_str().unwrap(), data);
            let (ast, _diags) = frontend.parse_recovering(&buffer);
            let _ = ast;
            let _ = frontend.parse_and_analyze(&buffer, None);
        }
    }
}

#[test]
fn test_fuzz_deep_nesting_expression_bounded() {
    // 300 levels of parenthesized nesting: must exceed MAX_EXPRESSION_DEPTH (128)
    // and fail cleanly with E0001 without stack overflow.
    let mut src = String::from("OPENQASM 3.0;\nqubit[1] q;\nrx(");
    for _ in 0..300 {
        src.push('(');
    }
    src.push_str("0.5");
    for _ in 0..300 {
        src.push(')');
    }
    src.push_str(") q[0];\n");

    let buffer = SourceBuffer::new("deep_nesting_test.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let res = frontend.lower(&buffer, None);
    assert!(!res.is_ok(), "Deep nesting beyond limit must be rejected");
    assert!(
        res.diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("E0001")),
        "Expected E0001 diagnostic for deep nesting: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_fuzz_huge_operand_list_bounded() {
    // 10,001 operands: must exceed MAX_OPERANDS_PER_STATEMENT (10,000)
    // and fail cleanly with E0001 without hanging or unbounded allocation.
    let mut src = String::from("OPENQASM 3.0;\nqubit[2] q;\nbarrier ");
    for i in 0..10_005 {
        if i > 0 {
            src.push_str(", ");
        }
        src.push_str("q[0]");
    }
    src.push_str(";\n");

    let buffer = SourceBuffer::new("huge_operands_test.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let res = frontend.lower(&buffer, None);
    assert!(!res.is_ok(), "Operand count beyond limit must be rejected");
    assert!(
        res.diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("E0001")),
        "Expected E0001 diagnostic for huge operand list: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_fuzz_long_numeric_literals_bounded() {
    // Long integer literal exceeding u64 and long float with huge exponent
    let src = r#"OPENQASM 3.0;
qubit[1] q;
rx(12345678901234567890123456789012345678901234567890) q[0];
ry(1e999999999999999999999999999999999999999999999999) q[0];
"#;

    let buffer = SourceBuffer::new("long_literals_test.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let res = frontend.lower(&buffer, None);
    assert!(!res.is_ok(), "Oversized numeric literals must be rejected");
    assert!(
        res.diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("E0001")),
        "Expected E0001 diagnostic for oversized numeric literals: {:?}",
        res.diagnostics
    );
}

#[test]
fn test_fuzz_lexer_limits_enforced() {
    // Small limits to verify strict enforcement
    let src = "h q[0];\n".repeat(50);
    let buffer = SourceBuffer::new("limits_test.qasm", src);
    let tight_limits = LexerLimits {
        max_source_bytes: 10_000,
        max_tokens: 20,
        max_token_bytes: 50,
    };

    let lex_res = Lexer::tokenize_with_limits(&buffer, tight_limits);
    assert!(lex_res.tokens.len() <= 21); // 20 tokens + EOF
    assert!(lex_res.has_errors());
}
