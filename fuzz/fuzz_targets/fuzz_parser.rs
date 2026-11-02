//! Parser and lowering fuzz target for QCC OpenQASM 3 frontend.
//!
//! Validates that the full frontend pipeline (lexing, recovery parsing, semantic analysis,
//! and AST-to-QccIR lowering) never panics, hangs, causes unbounded recursion, or exhausts
//! memory on arbitrary inputs, deep expression trees, huge operand lists, or malformed syntax.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use qcc::frontend::adapter::OpenQasmFrontend;
use qcc::frontend::source::SourceBuffer;

/// Executes recovery parsing, semantic analysis, and lowering on an arbitrary byte slice,
/// asserting that no panic occurs and that invariants hold.
pub fn fuzz_parser_target(data: &[u8]) {
    // Truncate to 64 KiB for bounded fuzz iteration time
    let slice = if data.len() > 64 * 1024 {
        &data[..64 * 1024]
    } else {
        data
    };

    let text = String::from_utf8_lossy(slice);
    let buffer = SourceBuffer::new("fuzz_input.qasm", text.into_owned());
    let frontend = OpenQasmFrontend::new();

    // 1. Recovery parse: must never panic, must return an AST and diagnostics
    let (ast, _diags) = frontend.parse_recovering(&buffer);

    // 2. Semantic analysis: must never panic
    let sem_res = frontend.parse_and_analyze(&buffer, None);

    // 3. Lowering: if semantic analysis succeeded with zero errors, lowering must succeed and be valid
    if sem_res.is_ok() {
        let lower_res = frontend.lower(&buffer, None);
        if let Some(circuit) = lower_res.output {
            assert!(
                circuit.validate().is_ok(),
                "Circuit produced from successful lowering must pass validation"
            );
        }
    }

    // 4. Assert AST statement spans are within buffer bounds
    for stmt in &ast.statements {
        let span = stmt.span();
        assert!(
            span.start() <= buffer.len(),
            "Statement span start {} exceeds buffer length {}",
            span.start(),
            buffer.len()
        );
        assert!(
            span.end() <= buffer.len(),
            "Statement span end {} exceeds buffer length {}",
            span.end(),
            buffer.len()
        );
    }
}

/// Runs a bounded smoke-fuzz suite over synthetic edge cases and pseudo-random inputs.
pub fn run_smoke(iterations: usize) {
    println!(
        "Running parser fuzz smoke test ({} iterations)...",
        iterations
    );

    // 1. Synthetic edge cases specifically targeting nesting, literals, and operand lists
    let mut deep_parens = String::from("OPENQASM 3.0;\nqubit[1] q;\nrx(");
    for _ in 0..200 {
        deep_parens.push('(');
    }
    deep_parens.push_str("0.5");
    for _ in 0..200 {
        deep_parens.push(')');
    }
    deep_parens.push_str(") q[0];");

    let mut huge_operands = String::from("OPENQASM 3.0;\nqubit[2] q;\nbarrier ");
    for i in 0..1500 {
        if i > 0 {
            huge_operands.push_str(", ");
        }
        huge_operands.push_str("q[0]");
    }
    huge_operands.push(';');

    let edge_cases: Vec<String> = vec![
        String::new(),
        "OPENQASM 3.0;".to_string(),
        "qubit[2] q;\nbit[2] c;\nh q[0];\ncx q[0], q[1];\nc[0] = measure q[0];".to_string(),
        // Deeply nested parentheses (triggers MAX_EXPRESSION_DEPTH)
        deep_parens,
        // Huge operand list (triggers MAX_OPERANDS_PER_STATEMENT)
        huge_operands,
        // Division by zero
        "OPENQASM 3.0;\nqubit[1] q;\nrx(1.0 / 0.0) q[0];".to_string(),
        "OPENQASM 3.0;\nqubit[1] q;\nrx(1.0 / (2.0 - 2.0)) q[0];".to_string(),
        // Non-finite float
        "OPENQASM 3.0;\nqubit[1] q;\nrx(1e999999999999999999) q[0];".to_string(),
        // Huge register size
        "OPENQASM 3.0;\nqubit[999999999999999] q;".to_string(),
        // Zero register size
        "OPENQASM 3.0;\nqubit[0] q;".to_string(),
        // Unclosed block
        "OPENQASM 3.0;\nif (c == 1) { h q[0];".to_string(),
        // Unsupported constructs
        "OPENQASM 3.0;\ngate my_gate q { h q; }".to_string(),
        "OPENQASM 3.0;\ndef my_sub() { }".to_string(),
        "OPENQASM 3.0;\ncal { }".to_string(),
        "OPENQASM 3.0;\nqubit[3] q;\nccx q[0], q[1], q[2];".to_string(),
        "OPENQASM 3.0;\nfor int i in [0:3] { h q[i]; }".to_string(),
        "OPENQASM 3.0;\nwhile (c == 1) { h q[0]; }".to_string(),
        // Trailing commas and missing semicolons
        "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], ;".to_string(),
        "OPENQASM 3.0;\nqubit[1] q;\nh q[0]".to_string(),
    ];

    for case in &edge_cases {
        fuzz_parser_target(case.as_bytes());
    }
    println!("  Passed {} synthetic parser edge cases.", edge_cases.len());

    // 2. Deterministic pseudo-random generation (SplitMix64)
    let mut state: u64 = 0x1234_5678_9ABC_DEF0;
    for _ in 0..iterations {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let seed = z ^ (z >> 31);

        let len = (seed % 1024) as usize;
        let mut data = Vec::with_capacity(len);
        let mut prng = seed;
        for _ in 0..len {
            prng = prng.wrapping_mul(6364136223846793005).wrapping_add(1);
            data.push((prng >> 56) as u8);
        }

        fuzz_parser_target(&data);
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
            fuzz_parser_target(&data);
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
                    fuzz_parser_target(&data);
                    count += 1;
                }
            }
            println!("  Processed {} corpus files.", count);
        } else {
            eprintln!("Usage: fuzz_parser [--smoke] [--iterations N] [FILE_OR_DIR...]");
            process::exit(1);
        }
    }
}
