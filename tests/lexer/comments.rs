//! Tests for line and block comments, line endings (LF/CRLF), and span integrity.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_line_comments() {
    let src =
        "// Header comment\nqubit[2] q; // Inline comment\n// Trailing comment without newline";
    let buffer = SourceBuffer::new("comments.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
    let expected = vec![
        TokenKind::Qubit,
        TokenKind::LBracket,
        TokenKind::IntLiteral(2),
        TokenKind::RBracket,
        TokenKind::Ident("q".to_string()),
        TokenKind::Semicolon,
        TokenKind::Eof,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn test_block_comments() {
    let src = "/* Block comment before */ qubit[2] q; /* Inline block */ h q[0]; /* Trailing */";
    let buffer = SourceBuffer::new("block.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
    let expected = vec![
        TokenKind::Qubit,
        TokenKind::LBracket,
        TokenKind::IntLiteral(2),
        TokenKind::RBracket,
        TokenKind::Ident("q".to_string()),
        TokenKind::Semicolon,
        TokenKind::Ident("h".to_string()),
        TokenKind::Ident("q".to_string()),
        TokenKind::LBracket,
        TokenKind::IntLiteral(0),
        TokenKind::RBracket,
        TokenKind::Semicolon,
        TokenKind::Eof,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn test_multiline_block_comment_preserves_line_counting() {
    let src = "/*\n * Multiline\n * Comment\n */\nqubit q;\n";
    let buffer = SourceBuffer::new("multi_block.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    // qubit is on line 5
    let qubit_tok = &tokens[0];
    assert_eq!(qubit_tok.kind, TokenKind::Qubit);
    assert_eq!(qubit_tok.span.start_line(), 5);
}

#[test]
fn test_unterminated_block_comment() {
    let src = "qubit q; /* unclosed comment";
    let buffer = SourceBuffer::new("unclosed.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("unterminated block comment"));
}

#[test]
fn test_crlf_line_endings() {
    let src = "OPENQASM 3.0;\r\nqubit[2] q;\r\nh q[0];\r\n";
    let buffer = SourceBuffer::new("crlf.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    // Line 1: OPENQASM 3.0; (tokens: OPENQASM, 3.0, ;)
    assert_eq!(tokens[0].span.start_line(), 1);
    // Line 2: qubit[2] q; (tokens: qubit, [, 2, ], q, ;)
    assert_eq!(tokens[3].span.start_line(), 2);
    assert_eq!(tokens[8].span.start_line(), 2); // ';' on line 2
                                                // Line 3: h q[0]; (tokens: h, q, [, 0, ], ;)
    assert_eq!(tokens[9].span.start_line(), 3); // 'h' on line 3
}
