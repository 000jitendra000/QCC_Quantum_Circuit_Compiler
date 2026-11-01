//! Tests for Unicode identifier handling, 'π' character support, and column accuracy.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_unicode_pi_identifier() {
    let src = "gphase(π / 2);";
    let buffer = SourceBuffer::new("unicode.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    // gphase ( π / 2 ) ;
    assert_eq!(tokens[0].kind, TokenKind::Gphase);
    assert_eq!(tokens[1].kind, TokenKind::LParen);
    assert_eq!(tokens[2].kind, TokenKind::Ident("π".to_string()));
    assert_eq!(tokens[3].kind, TokenKind::Slash);
    assert_eq!(tokens[4].kind, TokenKind::IntLiteral(2));
    assert_eq!(tokens[5].kind, TokenKind::RParen);
    assert_eq!(tokens[6].kind, TokenKind::Semicolon);

    // Verify 'π' span: 'π' starts at col 8, end col 9
    let pi_tok = &tokens[2];
    assert_eq!(pi_tok.span.start_col(), 8);
    assert_eq!(pi_tok.span.end_col(), 9);
}

#[test]
fn test_invalid_characters_diagnostic() {
    let src = "qubit q; @ $ ` ~";
    let buffer = SourceBuffer::new("invalid_chars.qasm", src);
    let (_tokens, diags) = Lexer::tokenize(&buffer);

    assert_eq!(diags.len(), 4, "Expected 4 invalid character diagnostics");
    assert!(diags[0].message.contains("unexpected character '@'"));
    assert!(diags[1].message.contains("unexpected character '$'"));
    assert!(diags[2].message.contains("unexpected character '`'"));
    assert!(diags[3].message.contains("unexpected character '~'"));
}
