//! Implementation of the `qcc inspect` CLI command.
//!
//! Provides inspection and structural analysis views of parsed OpenQASM 3 programs
//! and lowered QccIR circuits without bypassing semantic validation.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};

use crate::cli::{determine_diagnostic_exit_code, CliExitCode};
use crate::frontend::adapter::OpenQasmFrontend;
use crate::frontend::source::SourceBuffer;
use crate::ir::operation::Operation;

/// View mode for `qcc inspect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectMode {
    /// Comprehensive circuit summary (default).
    Summary,
    /// Parsed AST structure.
    Ast,
    /// Lowered QccIR DAG and operation list.
    Ir,
    /// Declared register tables.
    Registers,
}

/// Options configured for the `qcc inspect` command.
#[derive(Clone, Debug)]
pub struct InspectOptions {
    /// Input file path (or `"-"` for standard input).
    pub input_file: String,
    /// Inspection view mode.
    pub mode: InspectMode,
    /// Runtime parameter symbol bindings.
    pub bindings: HashMap<String, f64>,
}

impl Default for InspectOptions {
    fn default() -> Self {
        Self {
            input_file: String::new(),
            mode: InspectMode::Summary,
            bindings: HashMap::new(),
        }
    }
}

/// Prints command-line help for `qcc inspect`.
pub fn print_inspect_help() {
    println!("Usage: qcc inspect [OPTIONS] <FILE>");
    println!();
    println!("Inspects an OpenQASM 3 file and displays its structural and semantic details.");
    println!();
    println!("Arguments:");
    println!("  <FILE>                 Source file to inspect (use '-' for stdin)");
    println!();
    println!("Options:");
    println!("      --summary          Display circuit summary and topological trace (default)");
    println!("      --ast              Display parsed Abstract Syntax Tree (AST)");
    println!("      --ir               Display lowered QccIR dependency graph");
    println!("      --registers        Display declared quantum and classical registers");
    println!("  -b, --bind <NAME=VAL>  Bind parameter symbol to a numerical value");
    println!("  -h, --help             Print help information and exit");
}

/// Parses command-line arguments for `qcc inspect`.
pub fn parse_inspect_args(args: &[String]) -> Result<InspectOptions, (String, CliExitCode)> {
    let mut opts = InspectOptions::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_inspect_help();
                return Err((String::new(), CliExitCode::Success));
            }
            "--summary" => opts.mode = InspectMode::Summary,
            "--ast" => opts.mode = InspectMode::Ast,
            "--ir" => opts.mode = InspectMode::Ir,
            "--registers" => opts.mode = InspectMode::Registers,
            "-b" | "--bind" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--bind'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                let pair = &args[i];
                if let Some((name, val_str)) = pair.split_once('=') {
                    if let Ok(val) = val_str.parse::<f64>() {
                        opts.bindings.insert(name.trim().to_string(), val);
                    } else {
                        return Err((
                            format!("error: invalid numerical value in binding '{pair}'"),
                            CliExitCode::CliError,
                        ));
                    }
                } else {
                    return Err((
                        format!("error: invalid binding format '{pair}'; expected NAME=VALUE"),
                        CliExitCode::CliError,
                    ));
                }
            }
            flag if flag.starts_with('-') && flag != "-" => {
                return Err((
                    format!("error: unrecognized option '{flag}'"),
                    CliExitCode::CliError,
                ));
            }
            file => {
                if !opts.input_file.is_empty() {
                    return Err((
                        format!("error: unexpected second input file '{file}'"),
                        CliExitCode::CliError,
                    ));
                }
                opts.input_file = file.to_string();
            }
        }
        i += 1;
    }

    if opts.input_file.is_empty() {
        return Err((
            "error: missing required input file argument <FILE>\nTry 'qcc inspect --help' for usage."
                .to_string(),
            CliExitCode::CliError,
        ));
    }

    Ok(opts)
}

/// Executes the `qcc inspect` command with the provided options.
pub fn execute_inspect(opts: &InspectOptions) -> CliExitCode {
    // 1. Read input source
    let (source, file_name) = if opts.input_file == "-" {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("error: failed to read from standard input: {e}");
            return CliExitCode::CliError;
        }
        (buf, "<stdin>".to_string())
    } else {
        match fs::read_to_string(&opts.input_file) {
            Ok(src) => (src, opts.input_file.clone()),
            Err(e) => {
                eprintln!(
                    "error: failed to read input file '{}': {e}",
                    opts.input_file
                );
                return CliExitCode::CliError;
            }
        }
    };

    let buffer = SourceBuffer::new(&file_name, source);
    let frontend = OpenQasmFrontend::new();
    let bindings = if opts.bindings.is_empty() {
        None
    } else {
        Some(opts.bindings.clone())
    };

    // Mode-specific execution
    match opts.mode {
        InspectMode::Ast => {
            let parse_res = frontend.parse_and_analyze(&buffer, bindings);
            for diag in &parse_res.diagnostics {
                eprintln!("{diag}");
            }
            if !parse_res.is_ok() {
                return determine_diagnostic_exit_code(&parse_res.diagnostics);
            }
            let (program, _symbols) = parse_res.output.unwrap();
            println!("{program:#?}");
            CliExitCode::Success
        }
        InspectMode::Registers => {
            let parse_res = frontend.parse_and_analyze(&buffer, bindings);
            for diag in &parse_res.diagnostics {
                eprintln!("{diag}");
            }
            if !parse_res.is_ok() {
                return determine_diagnostic_exit_code(&parse_res.diagnostics);
            }
            let (_program, symbols) = parse_res.output.unwrap();
            println!("=== Declared Registers ({}) ===", file_name);
            println!("Quantum Registers:");
            for q_reg in symbols.quantum_registers() {
                let kind = if q_reg.is_scalar { "scalar" } else { "array" };
                println!(
                    "  {} (size: {}, {}, base qubit: {})",
                    q_reg.name, q_reg.size, kind, q_reg.base_index
                );
            }
            println!("Classical Registers:");
            for c_reg in symbols.classical_registers() {
                let kind = if c_reg.is_scalar { "scalar" } else { "array" };
                println!(
                    "  {} (size: {}, {}, base bit: {})",
                    c_reg.name, c_reg.size, kind, c_reg.base_index
                );
            }
            CliExitCode::Success
        }
        InspectMode::Ir => {
            let lower_res = frontend.lower(&buffer, bindings);
            for diag in &lower_res.diagnostics {
                eprintln!("{diag}");
            }
            if !lower_res.is_ok() {
                return determine_diagnostic_exit_code(&lower_res.diagnostics);
            }
            let circuit = lower_res.output.unwrap();
            println!("{circuit:#?}");
            CliExitCode::Success
        }
        InspectMode::Summary => {
            let lower_res = frontend.lower(&buffer, bindings);
            for diag in &lower_res.diagnostics {
                eprintln!("{diag}");
            }
            if !lower_res.is_ok() {
                return determine_diagnostic_exit_code(&lower_res.diagnostics);
            }
            let circuit = lower_res.output.unwrap();

            println!("============================================================");
            println!(" QCC Circuit Inspection: {}", circuit.name);
            println!("============================================================");
            println!("Source File:          {}", file_name);
            println!("Total Qubits:         {}", circuit.num_qubits());
            println!("Total Classical Bits: {}", circuit.num_classical_bits());
            println!("Global Phase:         {:?}", circuit.global_phase);
            println!("Dependency Edges:     {}", circuit.graph.edge_count());
            println!();

            println!("Declared Quantum Registers:");
            for r in &circuit.quantum_registers {
                println!(
                    "  '{}' [size: {}, qubits: {}..{}]",
                    r.name,
                    r.size,
                    r.start_qubit,
                    r.start_qubit + r.size
                );
            }

            println!("Declared Classical Registers:");
            for r in &circuit.classical_registers {
                println!(
                    "  '{}' [size: {}, bits: {}..{}]",
                    r.name,
                    r.size,
                    r.start_bit,
                    r.start_bit + r.size
                );
            }
            println!();

            println!("Topological Operation Sequence:");
            if let Ok(order) = circuit.graph.deterministic_topological_order() {
                for (step, &node_idx) in order.iter().enumerate() {
                    let op = circuit.graph.get_operation(node_idx).unwrap();
                    let op_str = match op {
                        Operation::Gate {
                            gate_name,
                            qubits,
                            parameters,
                        } => {
                            let params_str = if parameters.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "({})",
                                    parameters
                                        .iter()
                                        .map(|p| format!("{p:?}"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            };
                            let q_str = qubits
                                .iter()
                                .map(|q| format!("q[{}]", q.0))
                                .collect::<Vec<_>>()
                                .join(" ");
                            format!("{gate_name}{params_str} {q_str}")
                        }
                        Operation::Measure { qubit, destination } => {
                            format!("measure q[{}] -> c[{}]", qubit.0, destination.0)
                        }
                        Operation::Reset { qubit } => format!("reset q[{}]", qubit.0),
                        Operation::Barrier { qubits } => {
                            let q_str = qubits
                                .iter()
                                .map(|q| format!("q[{}]", q.0))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("barrier {q_str}")
                        }
                        Operation::Classical {
                            name,
                            reads,
                            writes,
                        } => {
                            let r_str = reads
                                .iter()
                                .map(|b| format!("c[{}]", b.0))
                                .collect::<Vec<_>>()
                                .join(", ");
                            let w_str = writes
                                .iter()
                                .map(|b| format!("c[{}]", b.0))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("classical {name} (reads: [{r_str}], writes: [{w_str}])")
                        }
                    };
                    println!("  Step {:2}: [node #{:?}] {}", step + 1, node_idx, op_str);
                }
            }
            println!("============================================================");

            CliExitCode::Success
        }
    }
}
