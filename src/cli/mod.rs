//! Command-line interface subcommands, argument dispatch, diagnostic rendering,
//! and exit-code semantics for QCC.
//!
//! # Stable Exit Codes
//!
//! | Exit Code | Identifier | Description |
//! |:---:|---|---|
//! | `0` | [`CliExitCode::Success`] | Successful compilation, inspection, or informational request |
//! | `1` | [`CliExitCode::CliError`] | Operational error (CLI argument syntax, unreadable file, I/O error) |
//! | `2` | [`CliExitCode::CompileError`] | Source syntax or semantic error in user OpenQASM program |
//! | `3` | [`CliExitCode::UnsupportedError`] | Unsupported OpenQASM construct detected (fail-closed subset policy) |

pub mod compile;
pub mod inspect;

use crate::diagnostics::Diagnostic;

/// Stable exit codes returned by the QCC command-line interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliExitCode {
    /// Successful execution (code 0).
    Success = 0,
    /// Operational or CLI argument error (code 1).
    CliError = 1,
    /// Compilation or source code syntax/semantic error (code 2).
    CompileError = 2,
    /// Unsupported OpenQASM construct detected (code 3).
    UnsupportedError = 3,
}

impl CliExitCode {
    /// Returns the integer exit code suitable for `std::process::exit`.
    #[inline]
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

/// Categorizes a set of compiler diagnostics into either a standard compilation error (`2`)
/// or an unsupported construct error (`3`).
pub fn determine_diagnostic_exit_code(diagnostics: &[Diagnostic]) -> CliExitCode {
    let has_unsupported = diagnostics.iter().any(|d| {
        if let Some(code) = &d.code {
            matches!(code.as_str(), "E0002" | "E0003" | "E0004" | "E0005")
        } else {
            d.message.to_lowercase().contains("unsupported")
        }
    });

    if has_unsupported {
        CliExitCode::UnsupportedError
    } else {
        CliExitCode::CompileError
    }
}

/// Prints top-level CLI help text.
pub fn print_main_help() {
    println!("QCC — Quantum Circuit Compiler");
    println!();
    println!("Usage: qcc [OPTIONS] [COMMAND]");
    println!();
    println!("Commands:");
    println!("  compile  Compile an OpenQASM 3 file to QccIR and emit compiled output");
    println!("  inspect  Inspect an OpenQASM 3 file and display structural/semantic details");
    println!();
    println!("Options:");
    println!("  -V, --version  Print version information and exit");
    println!("  -h, --help     Print help information and exit");
    println!();
    println!("Run 'qcc <COMMAND> --help' for details on a specific command.");
}

/// Main entry point for CLI argument dispatch.
pub fn run(args: &[String]) -> CliExitCode {
    if args.is_empty() {
        print_main_help();
        return CliExitCode::Success;
    }

    match args[0].as_str() {
        "-V" | "--version" => {
            println!("{}", crate::version_string());
            CliExitCode::Success
        }
        "-h" | "--help" => {
            print_main_help();
            CliExitCode::Success
        }
        "compile" => match compile::parse_compile_args(&args[1..]) {
            Ok(opts) => compile::execute_compile(&opts),
            Err((msg, code)) => {
                if !msg.is_empty() {
                    eprintln!("{msg}");
                }
                code
            }
        },
        "inspect" => match inspect::parse_inspect_args(&args[1..]) {
            Ok(opts) => inspect::execute_inspect(&opts),
            Err((msg, code)) => {
                if !msg.is_empty() {
                    eprintln!("{msg}");
                }
                code
            }
        },
        unknown => {
            eprintln!("error: unrecognized argument '{unknown}'");
            eprintln!("Try 'qcc --help' for available commands.");
            CliExitCode::CliError
        }
    }
}
