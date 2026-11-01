//! Rust OpenQASM 3 Parser Evaluation Experiment (QCC Phase 15).
//!
//! Evaluates the Rust `oq3_syntax` and `oq3_semantics` crates against the representative
//! corpus of OpenQASM 3 programs.

use std::fs;
use std::path::Path;

use oq3_syntax::ast::AstNode;
use oq3_syntax::SourceFile;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct RustParserResult {
    file: String,
    parse_errors_count: usize,
    parse_errors: Vec<String>,
    statement_count: usize,
    statement_kinds: Vec<String>,
    syntax_valid: bool,
}

fn evaluate_rust_corpus() -> Vec<RustParserResult> {
    let corpus_dir = if Path::new("experiments/oq3-evaluation/corpus").exists() {
        Path::new("experiments/oq3-evaluation/corpus")
    } else {
        Path::new("corpus")
    };

    let entries = [
        "barrier.qasm",
        "bell.qasm",
        "invalid_syntax.qasm",
        "malformed_operands.qasm",
        "measurement.qasm",
        "parameterized_gates.qasm",
        "registers.qasm",
        "registers_reserved_keywords.qasm",
        "reset.qasm",
        "standard_gates.qasm",
        "unsupported_features.qasm",
    ];

    let mut results = Vec::new();

    println!("================================================================================");
    println!("     QCC Phase 15: Rust oq3_syntax Parser Evaluation Harness                    ");
    println!("================================================================================");

    for filename in &entries {
        let path = corpus_dir.join(filename);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", path));

        let parsed = SourceFile::parse(&source);
        let errors: Vec<String> = parsed.errors().iter().map(|e| e.to_string()).collect();
        let tree = parsed.tree();
        let statements: Vec<String> = tree
            .statements()
            .map(|s| format!("{:?}", s.syntax().kind()))
            .collect();

        let is_valid = errors.is_empty();
        let status = if is_valid { "PASS" } else { "SYNTAX ERR" };

        println!(
            "[{:10}] {:<34} | Stmts: {:<2} | Errors: {:<2}",
            status, filename, statements.len(), errors.len()
        );
        for err in &errors {
            println!("   -> Error: {}", err);
        }

        results.push(RustParserResult {
            file: filename.to_string(),
            parse_errors_count: errors.len(),
            parse_errors: errors,
            statement_count: statements.len(),
            statement_kinds: statements,
            syntax_valid: is_valid,
        });
    }

    results
}

fn main() {
    let results = evaluate_rust_corpus();
    let json = serde_json::to_string_pretty(&results).unwrap();
    let out_path = if Path::new("experiments/oq3-evaluation").exists() {
        "experiments/oq3-evaluation/rust_parser_results.json"
    } else {
        "rust_parser_results.json"
    };
    fs::write(out_path, json).unwrap();
    println!("--------------------------------------------------------------------------------");
    println!("Saved Rust oq3_syntax evaluation results to {}", out_path);
}
