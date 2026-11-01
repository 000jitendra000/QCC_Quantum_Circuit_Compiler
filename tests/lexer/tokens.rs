//! Lexical recognition tests for all supported keywords, operators, delimiters, and punctuation.

use qcc::frontend::{Lexer, SourceBuffer, TokenKind};

#[test]
fn test_all_keywords() {
    let src = "OPENQASM include qubit bit measure reset barrier gphase \
               qreg creg defcal defcalgrammar delay while for in if else \
               return break continue end extern gate int uint float angle \
               bool duration stretch const readonly pragma";
    let buffer = SourceBuffer::new("keywords.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
    let expected = vec![
        TokenKind::OpenQasm,
        TokenKind::Include,
        TokenKind::Qubit,
        TokenKind::Bit,
        TokenKind::Measure,
        TokenKind::Reset,
        TokenKind::Barrier,
        TokenKind::Gphase,
        TokenKind::Qreg,
        TokenKind::Creg,
        TokenKind::Defcal,
        TokenKind::Defcalgrammar,
        TokenKind::Delay,
        TokenKind::While,
        TokenKind::For,
        TokenKind::In,
        TokenKind::If,
        TokenKind::Else,
        TokenKind::Return,
        TokenKind::Break,
        TokenKind::Continue,
        TokenKind::End,
        TokenKind::Extern,
        TokenKind::Gate,
        TokenKind::Int,
        TokenKind::Uint,
        TokenKind::Float,
        TokenKind::Angle,
        TokenKind::Bool,
        TokenKind::Duration,
        TokenKind::Stretch,
        TokenKind::Const,
        TokenKind::Readonly,
        TokenKind::Pragma,
        TokenKind::Eof,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn test_all_punctuation_and_operators() {
    let src = "; , : [ ] ( ) { } = -> + - * / % ^ == != < <= > >= += -= *= /=";
    let buffer = SourceBuffer::new("punct.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
    let expected = vec![
        TokenKind::Semicolon,
        TokenKind::Comma,
        TokenKind::Colon,
        TokenKind::LBracket,
        TokenKind::RBracket,
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::Equals,
        TokenKind::Arrow,
        TokenKind::Plus,
        TokenKind::Minus,
        TokenKind::Star,
        TokenKind::Slash,
        TokenKind::Percent,
        TokenKind::Caret,
        TokenKind::DoubleEquals,
        TokenKind::NotEquals,
        TokenKind::Less,
        TokenKind::LessEquals,
        TokenKind::Greater,
        TokenKind::GreaterEquals,
        TokenKind::PlusEquals,
        TokenKind::MinusEquals,
        TokenKind::StarEquals,
        TokenKind::SlashEquals,
        TokenKind::Eof,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn test_identifiers_not_prematurely_classified_as_gates() {
    let src = "h cx my_gate custom_identifier _ancilla q0";
    let buffer = SourceBuffer::new("ident.qasm", src);
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "diags: {diags:?}");

    for tok in &tokens[..tokens.len() - 1] {
        assert!(
            matches!(tok.kind, TokenKind::Ident(_)),
            "Token should be generic Ident, got {:?}",
            tok.kind
        );
    }
}

#[test]
fn test_empty_buffer_eof() {
    let buffer = SourceBuffer::new("empty.qasm", "");
    let (tokens, diags) = Lexer::tokenize(&buffer);
    assert!(diags.is_empty());
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
    assert_eq!(tokens[0].byte_start, 0);
    assert_eq!(tokens[0].byte_end, 0);
    assert_eq!(tokens[0].span.start_line(), 1);
    assert_eq!(tokens[0].span.start_col(), 1);
}
