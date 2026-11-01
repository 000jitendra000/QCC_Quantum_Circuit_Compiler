//! Property-based testing and fuzz-smoke harness for the OpenQASM lexer.

use proptest::prelude::*;
use qcc::frontend::{Lexer, LexerLimits, SourceBuffer, TokenKind};

proptest! {
    /// Asserts that the lexer never panics on arbitrary Unicode inputs.
    #[test]
    fn prop_lexer_never_panics_on_arbitrary_unicode(s in "\\PC*") {
        let buffer = SourceBuffer::new("fuzz_unicode.qasm", s);
        let limits = LexerLimits {
            max_source_bytes: 65_536,
            max_tokens: 1_000,
            max_token_bytes: 256,
        };
        let res = Lexer::tokenize_with_limits(&buffer, limits);
        // Must always terminate with Eof and never panic
        assert!(!res.tokens.is_empty());
        assert_eq!(res.tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
    }

    /// Asserts that every emitted token has a valid source span and that
    /// the token's byte offsets slice the exact matching text from the buffer.
    #[test]
    fn prop_token_spans_are_valid_and_consistent(
        s in "[a-zA-Z0-9_ \\t\\n;,\\[\\]()=+\\-*/\"]{0,200}"
    ) {
        let buffer = SourceBuffer::new("span_prop.qasm", &s);
        let limits = LexerLimits {
            max_source_bytes: 10_000,
            max_tokens: 500,
            max_token_bytes: 128,
        };
        let res = Lexer::tokenize_with_limits(&buffer, limits);

        for tok in &res.tokens {
            // Check span validity
            assert!(tok.span.start_line() >= 1);
            assert!(tok.span.end_line() >= 1);
            assert!(tok.span.start_col() >= 1);
            assert!(tok.span.end_col() >= 1);
            assert!(tok.byte_start <= tok.byte_end);
            assert!(tok.byte_end <= buffer.len());

            if !tok.is_eof() {
                // Buffer slice matches token text
                let slice = buffer.byte_slice(tok.byte_start, tok.byte_end);
                assert_eq!(slice, Some(tok.text.as_str()));
            }
        }
    }

    /// Asserts that numeric scanning correctly recognizes valid integer representations.
    #[test]
    fn prop_integers_round_trip(n in 0u64..1_000_000_000) {
        let src = format!("{n}");
        let buffer = SourceBuffer::new("num_prop.qasm", src);
        let res = Lexer::tokenize_with_limits(&buffer, LexerLimits::default());
        assert!(res.is_ok());
        assert_eq!(res.tokens.len(), 2); // integer + Eof
        assert_eq!(res.tokens[0].kind, TokenKind::IntLiteral(n));
    }
}

#[test]
fn test_fuzz_smoke_malformed_corpus() {
    let malformed_cases = [
        "/* unclosed comment without end",
        "\"unterminated string at eof",
        "\"unterminated\nstring with newline\"",
        "1.2.3.4.5",
        "1e+",
        "1e-",
        "1E",
        ".",
        "..",
        "...",
        "qubit[999999999999999999999999999999] q;",
        "@#$%^&~`",
        "// comment followed by nothing",
        "/* empty block */",
        "/**/",
        "/*/* nested */*/",
        "\0\r\n\t ",
        "OPENQASM 3.0; // valid line\n!@#$ invalid line\nqubit[2] q;",
    ];

    for (i, case) in malformed_cases.iter().enumerate() {
        let buffer = SourceBuffer::new(format!("malformed_{i}.qasm"), *case);
        let res = Lexer::tokenize_with_limits(&buffer, LexerLimits::default());
        // Must never panic and must produce Eof
        assert!(!res.tokens.is_empty(), "Failed on case {i}: {case}");
        assert_eq!(res.tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
    }
}
