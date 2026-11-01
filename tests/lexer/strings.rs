//! Tests for string literal scanning, include paths, and unterminated string diagnostics.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_valid_strings() {
    let src = r#"include "stdgates.inc"; include "custom/gates.qasm"; """#;
    let buffer = SourceBuffer::new("strings.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let strings: Vec<String> = tokens
        .into_iter()
        .filter_map(|t| match t.kind {
            TokenKind::StringLiteral(s) => Some(s),
            _ => None,
        })
        .collect();

    assert_eq!(
        strings,
        vec![
            "stdgates.inc".to_string(),
            "custom/gates.qasm".to_string(),
            "".to_string()
        ]
    );
}

#[test]
fn test_unterminated_string_at_newline() {
    let src = "include \"unclosed\nqubit q;";
    let buffer = SourceBuffer::new("bad_str1.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 1);
    assert!(diags[0]
        .message
        .contains("unterminated string literal before newline"));
}

#[test]
fn test_unterminated_string_at_eof() {
    let src = "include \"unclosed";
    let buffer = SourceBuffer::new("bad_str2.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 1);
    assert!(diags[0]
        .message
        .contains("unterminated string literal at end of input"));
}
