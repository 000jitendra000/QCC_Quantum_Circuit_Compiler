//! Lexer fuzz target for QCC OpenQASM 3 frontend.
//!
//! Validates that the lexical scanner never panics, hangs, or consumes unbounded
//! memory when presented with arbitrary byte streams, deeply repeated characters,
//! oversized tokens, non-ASCII Unicode sequences, or malformed numeric literals.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use qcc::frontend::lexer::{Lexer, LexerLimits};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::token::TokenKind;

/// Executes lexical analysis on an arbitrary byte slice, asserting safety invariants.
pub fn fuzz_lexer_target(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    let buffer = SourceBuffer::new("fuzz_input.qasm", text.into_owned());

    // Fuzz with tight limits to exercise boundary checks
    let limits = LexerLimits {
        max_source_bytes: 64 * 1024, // 64 KiB
        max_tokens: 5_000,
        max_token_bytes: 256,
    };

    let lex_res = Lexer::tokenize_with_limits(&buffer, limits);

    // INVARIANT 1: Token sequence must never be empty (always ends with EOF)
    assert!(
        !lex_res.tokens.is_empty(),
        "Token sequence must never be empty"
    );

    // INVARIANT 2: Last token must always be EOF
    assert_eq!(
        lex_res.tokens.last().map(|t| &t.kind),
        Some(&TokenKind::Eof),
        "Last token must be EOF"
    );

    // INVARIANT 3: Total token count must respect the configured limit (plus EOF)
    assert!(
        lex_res.tokens.len() <= limits.max_tokens + 1,
        "Token count {} exceeded limit {}",
        lex_res.tokens.len(),
        limits.max_tokens
    );

    // INVARIANT 4: Every emitted token must have a valid source span
    for tok in &lex_res.tokens {
        assert!(
            tok.span.start() <= buffer.len(),
            "Token start {} exceeds buffer length {}",
            tok.span.start(),
            buffer.len()
        );
        assert!(
            tok.span.end() <= buffer.len(),
            "Token end {} exceeds buffer length {}",
            tok.span.end(),
            buffer.len()
        );
        assert!(
            tok.span.start() <= tok.span.end(),
            "Token start {} > end {}",
            tok.span.start(),
            tok.span.end()
        );
    }
}

/// Runs a bounded smoke-fuzz suite over synthetic edge cases and pseudo-random inputs.
pub fn run_smoke(iterations: usize) {
    println!(
        "Running lexer fuzz smoke test ({} iterations)...",
        iterations
    );

    // 1. Synthetic edge cases
    let edge_cases: Vec<&[u8]> = vec![
        b"",
        b" ",
        b"\t\n\r",
        b"// unclosed comment",
        b"// comment\n",
        b"/* unclosed block comment",
        b"/* nested /* comment */ */",
        b"1234567890123456789012345678901234567890", // Long int literal
        b"1e99999999999999999999999999999999999999", // Overflow float literal
        b"0.00000000000000000000000000000000000001",
        b"\"unclosed string literal",
        b"\"valid string\"",
        b"OPENQASM 3.0;\nqubit[2] q;\nh q[0];\ncx q[0], q[1];",
        &[0xFF, 0xFE, 0xFD, 0x00, 0x80, 0xC0], // Invalid UTF-8 bytes
    ];

    for case in &edge_cases {
        fuzz_lexer_target(case);
    }
    println!("  Passed {} synthetic edge cases.", edge_cases.len());

    // 2. Deterministic pseudo-random generation (SplitMix64)
    let mut state: u64 = 0xCAFE_BABE_DEAD_BEEF;
    for _iter in 0..iterations {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let seed = z ^ (z >> 31);

        let len = (seed % 512) as usize;
        let mut data = Vec::with_capacity(len);
        let mut prng = seed;
        for _ in 0..len {
            prng = prng.wrapping_mul(6364136223846793005).wrapping_add(1);
            data.push((prng >> 56) as u8);
        }

        fuzz_lexer_target(&data);
    }

    println!("  Passed {} pseudo-random iterations cleanly.", iterations);
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--smoke" || a == "-s") {
        let iters = args
            .iter()
            .position(|a| a == "--iterations" || a == "-n")
            .and_then(|idx| args.get(idx + 1))
            .and_then(|val| val.parse::<usize>().ok())
            .unwrap_or(1000);
        run_smoke(iters);
        return;
    }

    for arg in &args {
        let path = Path::new(arg);
        if path.is_file() {
            println!("Fuzzing file: {:?}", path);
            let data = fs::read(path).unwrap_or_else(|e| {
                eprintln!("Failed to read file {:?}: {}", path, e);
                process::exit(1);
            });
            fuzz_lexer_target(&data);
        } else if path.is_dir() {
            println!("Fuzzing corpus directory: {:?}", path);
            let entries = fs::read_dir(path).unwrap_or_else(|e| {
                eprintln!("Failed to read directory {:?}: {}", path, e);
                process::exit(1);
            });
            let mut count = 0;
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let data = fs::read(entry.path()).unwrap_or_default();
                    fuzz_lexer_target(&data);
                    count += 1;
                }
            }
            println!("  Processed {} corpus files.", count);
        } else {
            eprintln!("Usage: fuzz_lexer [--smoke] [--iterations N] [FILE_OR_DIR...]");
            process::exit(1);
        }
    }
}
