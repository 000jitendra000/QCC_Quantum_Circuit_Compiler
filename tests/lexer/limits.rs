//! Tests for lexer resource limits: source buffer size, token count, and token byte length limits.

use qcc::frontend::{Lexer, LexerLimits, SourceBuffer, TokenKind};

#[test]
fn test_source_size_limit_enforced() {
    let src = "qubit[2] q;";
    let buffer = SourceBuffer::new("size_limit.qasm", src);

    let limits = LexerLimits {
        max_source_bytes: 5, // smaller than src.len() (11 bytes)
        max_tokens: 100,
        max_token_bytes: 100,
    };

    let res = Lexer::tokenize_with_limits(&buffer, limits);
    assert!(!res.is_ok());
    assert_eq!(res.diagnostics.len(), 1);
    assert_eq!(res.diagnostics[0].code.as_deref(), Some("E0101"));
    assert!(res.diagnostics[0]
        .message
        .contains("source buffer size (11 bytes) exceeds limit of 5 bytes"));
    assert_eq!(res.tokens.len(), 1);
    assert_eq!(res.tokens[0].kind, TokenKind::Eof);
}

#[test]
fn test_token_count_limit_enforced() {
    let src = "x q[0]; y q[0]; z q[0]; h q[0]; s q[0];";
    let buffer = SourceBuffer::new("count_limit.qasm", src);

    let limits = LexerLimits {
        max_source_bytes: 1024,
        max_tokens: 5, // Allow only 5 tokens before aborting
        max_token_bytes: 100,
    };

    let res = Lexer::tokenize_with_limits(&buffer, limits);
    assert!(!res.is_ok());
    assert!(res
        .diagnostics
        .iter()
        .any(|d| d.code.as_deref() == Some("E0102")));
    // Emitted tokens up to limit + Eof
    assert_eq!(res.tokens.len(), 6);
    assert_eq!(res.tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
}

#[test]
fn test_token_byte_length_limit_enforced() {
    let long_ident = "a".repeat(200);
    let src = format!("qubit {long_ident};");
    let buffer = SourceBuffer::new("token_len_limit.qasm", src);

    let limits = LexerLimits {
        max_source_bytes: 1024,
        max_tokens: 100,
        max_token_bytes: 50, // limit identifier to 50 bytes
    };

    let res = Lexer::tokenize_with_limits(&buffer, limits);
    assert!(!res.is_ok());
    assert!(res
        .diagnostics
        .iter()
        .any(|d| d.code.as_deref() == Some("E0103")));
}
