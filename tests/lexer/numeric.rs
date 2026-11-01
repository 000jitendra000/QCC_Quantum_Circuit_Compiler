//! Tests for numeric literal edge cases: integers, floats, scientific notation, and malformed inputs.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_valid_integers() {
    let src = "0 1 42 1000000 18446744073709551615";
    let buffer = SourceBuffer::new("integers.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let expected = vec![
        TokenKind::IntLiteral(0),
        TokenKind::IntLiteral(1),
        TokenKind::IntLiteral(42),
        TokenKind::IntLiteral(1_000_000),
        TokenKind::IntLiteral(u64::MAX),
        TokenKind::Eof,
    ];
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
    assert_eq!(kinds, expected);
}

#[test]
fn test_valid_floats_and_dots() {
    let src = "123.456 0.0 0.5 42. .5";
    let buffer = SourceBuffer::new("floats.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    match &tokens[0].kind {
        TokenKind::FloatLiteral(f) => assert!((f - 123.456).abs() < 1e-10),
        other => panic!("expected FloatLiteral, got {other:?}"),
    }
    match &tokens[1].kind {
        TokenKind::FloatLiteral(f) => assert_eq!(*f, 0.0),
        other => panic!("expected FloatLiteral, got {other:?}"),
    }
    match &tokens[2].kind {
        TokenKind::FloatLiteral(f) => assert_eq!(*f, 0.5),
        other => panic!("expected FloatLiteral, got {other:?}"),
    }
    match &tokens[3].kind {
        TokenKind::FloatLiteral(f) => assert_eq!(*f, 42.0),
        other => panic!("expected FloatLiteral 42., got {other:?}"),
    }
    match &tokens[4].kind {
        TokenKind::FloatLiteral(f) => assert_eq!(*f, 0.5),
        other => panic!("expected FloatLiteral .5, got {other:?}"),
    }
}

#[test]
fn test_scientific_notation() {
    let src = "1e5 1e-3 2.5E+4 1e+0 .5e2 5.e-1";
    let buffer = SourceBuffer::new("sci.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let values: Vec<f64> = tokens
        .into_iter()
        .filter_map(|t| match t.kind {
            TokenKind::FloatLiteral(f) => Some(f),
            _ => None,
        })
        .collect();

    assert_eq!(values.len(), 6);
    assert!((values[0] - 100_000.0).abs() < 1e-10);
    assert!((values[1] - 0.001).abs() < 1e-10);
    assert!((values[2] - 25_000.0).abs() < 1e-10);
    assert!((values[3] - 1.0).abs() < 1e-10);
    assert!((values[4] - 50.0).abs() < 1e-10);
    assert!((values[5] - 0.5).abs() < 1e-10);
}

#[test]
fn test_malformed_scientific_notation() {
    let src = "1e 1e+ 2.5E-";
    let buffer = SourceBuffer::new("bad_sci.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(
        diags.len(),
        3,
        "Expected 3 diagnostics for malformed exponents"
    );
    for diag in &diags {
        assert!(diag
            .message
            .contains("exponent requires at least one digit"));
    }
    // Should still finish with Eof and not crash or loop
    assert_eq!(tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
}

#[test]
fn test_malformed_multiple_decimal_points() {
    let src = "1.2.3";
    let buffer = SourceBuffer::new("multi_dot.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("multiple decimal points"));
}

#[test]
fn test_integer_overflow_diagnostic() {
    // Number larger than u64::MAX (18446744073709551615)
    let src = "999999999999999999999999999999";
    let buffer = SourceBuffer::new("overflow.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 1);
    assert!(diags[0]
        .message
        .contains("integer literal exceeds 64-bit unsigned limit"));
}
