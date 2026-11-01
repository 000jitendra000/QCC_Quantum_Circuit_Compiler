//! Golden token sequence verification tests for standard OpenQASM 3 circuits.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_golden_bell_qasm_tokens() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
"#;
    let buffer = SourceBuffer::new("bell.qasm", src);
    let (tokens, diagnostics) = Lexer::tokenize(&buffer);

    assert!(
        diagnostics.is_empty(),
        "Unexpected diagnostics: {diagnostics:?}"
    );

    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();

    let expected = vec![
        TokenKind::OpenQasm,
        TokenKind::FloatLiteral(3.0),
        TokenKind::Semicolon,
        TokenKind::Include,
        TokenKind::StringLiteral("stdgates.inc".to_string()),
        TokenKind::Semicolon,
        TokenKind::Qubit,
        TokenKind::LBracket,
        TokenKind::IntLiteral(2),
        TokenKind::RBracket,
        TokenKind::Ident("q".to_string()),
        TokenKind::Semicolon,
        TokenKind::Bit,
        TokenKind::LBracket,
        TokenKind::IntLiteral(2),
        TokenKind::RBracket,
        TokenKind::Ident("c".to_string()),
        TokenKind::Semicolon,
        TokenKind::Ident("h".to_string()),
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(0),
        TokenKind::RBracket,
        TokenKind::Semicolon,
        TokenKind::Ident("cx".to_string()),
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(0),
        TokenKind::RBracket,
        TokenKind::Comma,
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(1),
        TokenKind::RBracket,
        TokenKind::Semicolon,
        TokenKind::Ident("c".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(0),
        TokenKind::RBracket,
        TokenKind::Equals,
        TokenKind::Measure,
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(0),
        TokenKind::RBracket,
        TokenKind::Semicolon,
        TokenKind::Ident("c".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(1),
        TokenKind::RBracket,
        TokenKind::Equals,
        TokenKind::Measure,
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(1),
        TokenKind::RBracket,
        TokenKind::Semicolon,
        TokenKind::Eof,
    ];

    assert_eq!(kinds, expected);
}

#[test]
fn test_golden_file_corpus_bell() {
    let buffer = SourceBuffer::from_file("experiments/oq3-evaluation/corpus/bell.qasm")
        .expect("bell.qasm must exist");
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");
    assert_eq!(tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
}
