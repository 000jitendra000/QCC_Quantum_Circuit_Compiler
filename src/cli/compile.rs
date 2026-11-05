//! Implementation of the `qcc compile` CLI command.
//!
//! Compiles supported OpenQASM 3 source files into validated QccIR circuits
//! and emits canonical OpenQASM 3 or circuit representations.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};

use crate::backend::openqasm::{
    EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
};
use crate::cli::{determine_diagnostic_exit_code, CliExitCode};
use crate::compiler::CompilerPipeline;
use crate::frontend::adapter::OpenQasmFrontend;
use crate::frontend::source::SourceBuffer;
use crate::gate_signature::ParameterDomain;
use crate::optimizer::report::CircuitMetrics;
use crate::target::model::{Target, TargetBuilder};
use crate::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use crate::target::OperationConnectivity;

/// Target output format for `qcc compile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileFormat {
    /// Canonical OpenQASM 3.0 source text.
    OpenQasm,
    /// Human-readable QccIR graph dump.
    QccIr,
}

/// Options configured for the `qcc compile` command.
#[derive(Clone, Debug)]
pub struct CompileOptions {
    /// Input file path (or `"-"` for standard input).
    pub input_file: String,
    /// Optional output file path (defaults to standard output).
    pub output_file: Option<String>,
    /// Target emission format.
    pub format: CompileFormat,
    /// Global phase emission policy.
    pub global_phase_policy: GlobalPhasePolicy,
    /// Measurement statement syntax.
    pub measurement_syntax: MeasurementSyntax,
    /// Runtime parameter symbol bindings.
    pub bindings: HashMap<String, f64>,
    /// Optional target topology for end-to-end compilation (e.g. "linear", "ring").
    pub target: Option<String>,
    /// Whether to print compilation statistics summary.
    pub stats: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            input_file: String::new(),
            output_file: None,
            format: CompileFormat::OpenQasm,
            global_phase_policy: GlobalPhasePolicy::EmitGPhase,
            measurement_syntax: MeasurementSyntax::Assignment,
            bindings: HashMap::new(),
            target: None,
            stats: false,
        }
    }
}

/// Prints command-line help for `qcc compile`.
pub fn print_compile_help() {
    println!("Usage: qcc compile [OPTIONS] <FILE>");
    println!();
    println!("Compiles an OpenQASM 3 source file into QccIR and emits the compiled output.");
    println!();
    println!("Arguments:");
    println!("  <FILE>                       Source file to compile (use '-' for stdin)");
    println!();
    println!("Options:");
    println!("  -o, --output <FILE>          Write output to <FILE> instead of stdout");
    println!("      --format <FMT>           Output format: 'openqasm' (default) or 'qccir'");
    println!("      --global-phase <POLICY>  Global phase policy: 'auto' (default), 'always', or 'reject-nonzero'");
    println!("      --measure-syntax <SYN>   Measurement syntax: 'assign' (default) or 'arrow'");
    println!("  -b, --bind <NAME=VAL>        Bind parameter symbol to a numerical value");
    println!("      --target <TARGET>        Compile for target topology: 'linear', 'ring'");
    println!(
        "      --stats                  Print compilation statistics (gate count, depth, SWAPs)"
    );
    println!("  -h, --help                   Print help information and exit");
}

/// Parses command-line arguments for `qcc compile`.
pub fn parse_compile_args(args: &[String]) -> Result<CompileOptions, (String, CliExitCode)> {
    let mut opts = CompileOptions::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_compile_help();
                return Err((String::new(), CliExitCode::Success));
            }
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--output'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.output_file = Some(args[i].clone());
            }
            "--format" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--format'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                match args[i].to_lowercase().as_str() {
                    "openqasm" | "qasm" => opts.format = CompileFormat::OpenQasm,
                    "qccir" | "ir" => opts.format = CompileFormat::QccIr,
                    other => {
                        return Err((
                            format!("error: unrecognized format '{other}'; expected 'openqasm' or 'qccir'"),
                            CliExitCode::CliError,
                        ));
                    }
                }
            }
            "--global-phase" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--global-phase'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                match args[i].to_lowercase().as_str() {
                    "auto" | "emit" => opts.global_phase_policy = GlobalPhasePolicy::EmitGPhase,
                    "always" => opts.global_phase_policy = GlobalPhasePolicy::AlwaysEmit,
                    "reject-nonzero" => opts.global_phase_policy = GlobalPhasePolicy::RejectNonZero,
                    other => {
                        return Err((
                            format!("error: unrecognized global-phase policy '{other}'; expected 'auto', 'always', or 'reject-nonzero'"),
                            CliExitCode::CliError,
                        ));
                    }
                }
            }
            "--measure-syntax" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--measure-syntax'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                match args[i].to_lowercase().as_str() {
                    "assign" | "assignment" => {
                        opts.measurement_syntax = MeasurementSyntax::Assignment
                    }
                    "arrow" => opts.measurement_syntax = MeasurementSyntax::Arrow,
                    other => {
                        return Err((
                            format!("error: unrecognized measurement syntax '{other}'; expected 'assign' or 'arrow'"),
                            CliExitCode::CliError,
                        ));
                    }
                }
            }
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
            "--target" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--target'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.target = Some(args[i].clone());
            }
            "--stats" => {
                opts.stats = true;
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
            "error: missing required input file argument <FILE>\nTry 'qcc compile --help' for usage."
                .to_string(),
            CliExitCode::CliError,
        ));
    }

    Ok(opts)
}

/// Executes the `qcc compile` command with the provided options.
pub fn execute_compile(opts: &CompileOptions) -> CliExitCode {
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

    // 2. Compile and lower source to QccIR
    let bindings = if opts.bindings.is_empty() {
        None
    } else {
        Some(opts.bindings.clone())
    };

    let lower_res = frontend.lower(&buffer, bindings);

    // 3. Print any diagnostics (warnings and errors) to stderr with exact line and column
    for diag in &lower_res.diagnostics {
        eprintln!("{diag}");
    }

    // 4. If compilation failed, determine deterministic exit code
    if !lower_res.is_ok() {
        return determine_diagnostic_exit_code(&lower_res.diagnostics);
    }

    let circuit = lower_res
        .output
        .expect("Circuit must be present on success");

    // 5. If a target is specified, run the integrated compilation pipeline
    let circuit = if let Some(target_name) = &opts.target {
        let num_qubits = circuit.num_qubits().max(2);
        let target = match target_name.to_lowercase().as_str() {
            "linear" | "line" => create_standard_target("linear_target", num_qubits, false),
            "ring" => create_standard_target("ring_target", num_qubits, true),
            other => {
                eprintln!(
                    "error: unsupported target topology '{other}'; supported: 'linear', 'ring'"
                );
                return CliExitCode::CliError;
            }
        };

        let pipeline = CompilerPipeline::default();
        match pipeline.compile(&circuit, &target) {
            Ok(result) => {
                if opts.stats {
                    let m = &result.report.metrics;
                    eprintln!("| | Before | After |");
                    eprintln!("|---|---|---|");
                    eprintln!(
                        "| Gates | {} | {} |",
                        m.initial_gate_count, m.final_gate_count
                    );
                    eprintln!("| Depth | {} | {} |", m.initial_depth, m.final_depth);
                    eprintln!("| SWAPs inserted | 0 | {} |", m.swaps_inserted);
                }
                result.physical_circuit
            }
            Err(e) => {
                eprintln!("error: compilation pipeline failed: {e}");
                return CliExitCode::CompileError;
            }
        }
    } else {
        if opts.stats {
            let initial_gates = circuit.num_operations();
            let initial_depth = CircuitMetrics::compute(&circuit).circuit_depth;
            eprintln!("| | Before | After |");
            eprintln!("|---|---|---|");
            eprintln!("| Gates | {initial_gates} | {initial_gates} |");
            eprintln!("| Depth | {initial_depth} | {initial_depth} |");
            eprintln!("| SWAPs inserted | 0 | 0 |");
        }
        circuit
    };

    // 6. Format output
    let output_text = match opts.format {
        CompileFormat::OpenQasm => {
            let emitter_opts = EmitterOptions {
                global_phase_policy: opts.global_phase_policy,
                measurement_syntax: opts.measurement_syntax,
                ..Default::default()
            };
            match OpenQasmEmitter::with_options(emitter_opts).emit(&circuit) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("error: failed to emit OpenQASM: {e}");
                    return CliExitCode::CliError;
                }
            }
        }
        CompileFormat::QccIr => {
            format!("{circuit:#?}\n")
        }
    };

    // 6. Write output
    if let Some(out_path) = &opts.output_file {
        if let Err(e) = fs::write(out_path, output_text) {
            eprintln!("error: failed to write output file '{out_path}': {e}");
            return CliExitCode::CliError;
        }
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(output_text.as_bytes()) {
            eprintln!("error: failed to write to standard output: {e}");
            return CliExitCode::CliError;
        }
    }

    CliExitCode::Success
}

/// Helper building a standard target topology for CLI compilation.
fn create_standard_target(name: &str, num_qubits: usize, is_ring: bool) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    for op_name in &["h", "x", "y", "z", "s", "sdg", "t", "tdg", "sx"] {
        let desc = TargetOperationBuilder::new(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(30.0)
            .build();
        builder = builder.add_operation(desc);
    }

    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let meas_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(500.0)
        .build();
    let reset_desc = TargetOperationBuilder::new("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(1000.0)
        .build();

    builder = builder
        .add_operation(rz_desc)
        .add_operation(meas_desc)
        .add_operation(reset_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((PhysicalQubitId(i as u32), PhysicalQubitId((i + 1) as u32)));
    }
    if is_ring && num_qubits > 2 {
        edges.push((PhysicalQubitId((num_qubits - 1) as u32), PhysicalQubitId(0)));
    }

    if !edges.is_empty() {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
            .duration(150.0)
            .build();
        let cz_desc = TargetOperationBuilder::new("cz", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
            .duration(150.0)
            .build();
        let swap_desc = TargetOperationBuilder::new("swap", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(300.0)
            .build();

        builder = builder
            .add_operation(cx_desc)
            .add_operation(cz_desc)
            .add_operation(swap_desc);
    }

    builder.build().expect("Failed to build target model")
}
