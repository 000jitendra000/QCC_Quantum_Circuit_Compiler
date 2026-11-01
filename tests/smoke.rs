use std::process::Command;

use proptest::prelude::*;
use qcc::diagnostics::{Diagnostic, SourceSpan};

#[test]
fn test_cli_version_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_qcc"))
        .arg("--version")
        .output()
        .expect("failed to execute qcc binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("qcc {}", env!("CARGO_PKG_VERSION"))),
        "Expected version output to contain `qcc {}`, got: `{}`",
        env!("CARGO_PKG_VERSION"),
        stdout
    );
}

#[test]
fn test_cli_help_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_qcc"))
        .arg("--help")
        .output()
        .expect("failed to execute qcc binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: qcc [OPTIONS]"));
}

#[test]
fn test_cli_invalid_arg_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_qcc"))
        .arg("--nonexistent-flag")
        .output()
        .expect("failed to execute qcc binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unrecognized argument"));
}

#[test]
fn test_diagnostic_formatting_golden() {
    let span = SourceSpan::single_line(12, 25, 3, 5, 18).expect("valid span");
    let diag = Diagnostic::error("cannot find gate `foo` in scope")
        .with_code("E0001")
        .with_file_path("circuits/bell.qasm")
        .with_span(span)
        .with_note("supported standard gates are defined in stdgates.inc")
        .with_note("did you mean `h` or `cx`?");

    let expected_golden = "\
circuits/bell.qasm:3:5-18: error[E0001]: cannot find gate `foo` in scope
  note: supported standard gates are defined in stdgates.inc
  note: did you mean `h` or `cx`?";

    assert_eq!(diag.to_string(), expected_golden);
}

// Property-based test suite verifying that diagnostic span formatting
// never panics on any valid source span generated across arbitrary offsets and dimensions.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_valid_span_formatting_never_panics(
        start in 0usize..50_000,
        len in 0usize..5_000,
        start_line in 1usize..5_000,
        line_delta in 0usize..50,
        start_col in 1usize..200,
        col_delta in 0usize..200,
    ) {
        let end = start + len;
        let end_line = start_line + line_delta;
        let end_col = if start_line == end_line {
            start_col + col_delta
        } else {
            1 + (col_delta % 150)
        };

        // Assert generator respects validity invariants
        prop_assert!(SourceSpan::is_valid(start, end, start_line, start_col, end_line, end_col));

        let span = SourceSpan::new(start, end, start_line, start_col, end_line, end_col)
            .expect("generated span must be valid");

        // 1. Formatting span must never panic and must produce non-empty text
        let formatted_span = span.to_string();
        prop_assert!(!formatted_span.is_empty());

        // 2. Formatting diagnostic with valid span must never panic
        let diag = Diagnostic::error("synthetic property test error")
            .with_span(span)
            .with_code("E9999")
            .with_file_path("source.qasm")
            .with_note("synthetic diagnostic note");

        let formatted_diag = diag.to_string();
        prop_assert!(!formatted_diag.is_empty());
        prop_assert!(formatted_diag.contains(&formatted_span));
    }
}
