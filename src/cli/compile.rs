//! Implementation of the `qcc compile` CLI command.
//!
//! Compiles supported OpenQASM 3 source files into validated QccIR circuits
//! and emits canonical OpenQASM 3 or circuit representations.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Read, Write};

use crate::backend::openqasm::{
    EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
};
use crate::cli::{determine_diagnostic_exit_code, CliExitCode};
use crate::compiler::pass_diagnostic::{
    pass_diagnostics_to_json_pretty, PassDiagnosticConfig, PassDiagnosticReport,
};
use crate::compiler::{
    CompilationManifest, CompilationResult, CompilerConfig, CompilerPipeline, ManifestBuildContext,
    ManifestInputKind, ManifestOutputDestination, ManifestOutputFormat,
};
use crate::frontend::adapter::OpenQasmFrontend;
use crate::frontend::source::SourceBuffer;
use crate::gate_signature::ParameterDomain;
use crate::opt::context::PassContext;
use crate::optimizer::config::{OptimizationLevel, OptimizerConfig};
use crate::optimizer::pipeline::OptimizerPipeline;
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
    /// Optional path to write the versioned JSON compilation manifest.
    pub manifest_file: Option<String>,
    /// Optional path to write the versioned JSON per-pass diagnostic reports array.
    pub pass_report: Option<String>,
    /// Optional pass ID to inspect in human-readable diagnostic format.
    pub inspect_pass: Option<String>,
    /// Whether to print concise per-pass diagnostic execution summary table to stderr.
    pub pass_summary: bool,
    /// Optional diagnostic collection mode preset ("basic", "metrics", "fingerprints", "standard", "full").
    pub pass_diagnostics_mode: Option<String>,
    /// Whether to explicitly enable before/after circuit snapshot metrics.
    pub pass_metrics: bool,
    /// Whether to explicitly enable SHA-256 circuit representation fingerprints.
    pub pass_fingerprints: bool,
    /// Whether to explicitly enable per-pass equivalence verification hooks.
    pub pass_verify: bool,
    /// Optimization aggressiveness level (default: OptimizationLevel::O2).
    pub opt_level: OptimizationLevel,
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
            manifest_file: None,
            pass_report: None,
            inspect_pass: None,
            pass_summary: false,
            pass_diagnostics_mode: None,
            pass_metrics: false,
            pass_fingerprints: false,
            pass_verify: false,
            opt_level: OptimizationLevel::O2,
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
    println!("  -O, --opt-level <LEVEL>      Optimization level: '0', '1', '2' (default), '3'");
    println!("      --target <TARGET>        Compile for target topology: 'linear', 'ring'");
    println!(
        "      --stats                  Print compilation statistics (gate count, depth, SWAPs)"
    );
    println!(
        "      --emit-manifest <FILE>   Write machine-readable JSON compilation manifest to <FILE>"
    );
    println!(
        "      --pass-report <FILE>     Write Phase 71 per-pass diagnostic JSON report array to <FILE>"
    );
    println!(
        "      --inspect-pass <ID>      Display detailed human-readable diagnostic report for pass <ID>"
    );
    println!(
        "      --pass-summary           Print concise per-pass execution summary table to stderr"
    );
    println!(
        "      --pass-diagnostics <MOD> Diagnostic mode: 'basic', 'metrics', 'fingerprints', 'standard', 'full'"
    );
    println!(
        "      --pass-metrics           Collect before/after circuit snapshot metrics in pass diagnostics"
    );
    println!(
        "      --pass-fingerprints      Compute SHA-256 circuit representation fingerprints in pass diagnostics"
    );
    println!(
        "      --pass-verify            Run equivalence verification hooks between pass boundaries"
    );
    println!("  -h, --help                   Print help information and exit");
}

fn parse_opt_level(s: &str) -> Result<OptimizationLevel, (String, CliExitCode)> {
    match s.to_lowercase().trim() {
        "0" | "o0" => Ok(OptimizationLevel::O0),
        "1" | "o1" => Ok(OptimizationLevel::O1),
        "2" | "o2" => Ok(OptimizationLevel::O2),
        "3" | "o3" => Ok(OptimizationLevel::O3),
        other => Err((
            format!(
                "error: unrecognized optimization level '{other}'; expected '0', '1', '2', or '3'"
            ),
            CliExitCode::CliError,
        )),
    }
}

fn resolve_diagnostic_config(opts: &CompileOptions) -> PassDiagnosticConfig {
    let diagnostics_requested = opts.pass_report.is_some()
        || opts.inspect_pass.is_some()
        || opts.pass_summary
        || opts.pass_diagnostics_mode.is_some()
        || opts.pass_metrics
        || opts.pass_fingerprints
        || opts.pass_verify;

    if !diagnostics_requested {
        return PassDiagnosticConfig::default();
    }

    let mut cfg = match opts.pass_diagnostics_mode.as_deref() {
        Some("basic") => PassDiagnosticConfig::basic(),
        Some("metrics") => PassDiagnosticConfig::basic().with_metrics(true),
        Some("fingerprints") => PassDiagnosticConfig::basic().with_fingerprints(true),
        Some("standard") => PassDiagnosticConfig::standard(),
        Some("full") => PassDiagnosticConfig::full(),
        None => PassDiagnosticConfig::basic(),
        _ => unreachable!(),
    };

    if opts.pass_metrics {
        cfg = cfg.with_metrics(true);
    }
    if opts.pass_fingerprints {
        cfg = cfg.with_fingerprints(true);
    }
    if opts.pass_verify {
        cfg = cfg.with_verification(true);
    }

    cfg
}

fn render_pass_summary(diagnostics: &[PassDiagnosticReport]) -> String {
    let mut out = String::new();
    out.push_str("Pass Execution Summary:\n");
    out.push_str("+------+-----------------------+---------+----------+------------------+----------------+--------------+\n");
    out.push_str("| Step | Pass ID               | Status  | Time(ms) | Gates (In -> Out)| Delta (+/-/~ ) | Verification |\n");
    out.push_str("+------+-----------------------+---------+----------+------------------+----------------+--------------+\n");

    let mut ran_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;

    for rep in diagnostics {
        match rep.status {
            crate::compiler::pass_diagnostic::PassStatus::Ran => ran_count += 1,
            crate::compiler::pass_diagnostic::PassStatus::Skipped => skipped_count += 1,
            crate::compiler::pass_diagnostic::PassStatus::Failed => failed_count += 1,
        }

        let time_str = if let Some(dur) = rep.measurements.duration_ms {
            format!("{dur:>8.2}")
        } else {
            "       -".to_string()
        };

        let gates_str = match (&rep.measurements.before, &rep.measurements.after) {
            (Some(b), Some(a)) => {
                let bin = b
                    .total_gate_count
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let aout = a
                    .total_gate_count
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "-".to_string());
                format!("{bin} -> {aout}")
            }
            (Some(b), None) => {
                let bin = b
                    .total_gate_count
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "-".to_string());
                format!("{bin} -> -")
            }
            _ => "-".to_string(),
        };

        let delta_str = format!(
            "+{} / -{} / ~{}",
            rep.transformation.operations_added,
            rep.transformation.operations_removed,
            rep.transformation.operations_rewritten
        );

        let ver_str = rep.verification.status.to_string();

        out.push_str(&format!(
            "| {:>4} | {:<21} | {:<7} | {} | {:<16} | {:<14} | {:<12} |\n",
            rep.execution_order, rep.pass_id, rep.status, time_str, gates_str, delta_str, ver_str
        ));
    }

    out.push_str("+------+-----------------------+---------+----------+------------------+----------------+--------------+\n");
    out.push_str(&format!(
        "Total passes: {} (ran: {}, skipped: {}, failed: {})\n",
        diagnostics.len(),
        ran_count,
        skipped_count,
        failed_count
    ));
    out
}

fn render_pass_inspection(
    diagnostics: &[PassDiagnosticReport],
    target_id: &str,
) -> Result<String, String> {
    let matching: Vec<&PassDiagnosticReport> = diagnostics
        .iter()
        .filter(|d| d.pass_id == target_id)
        .collect();

    if matching.is_empty() {
        let mut available: Vec<&str> = diagnostics.iter().map(|d| d.pass_id.as_str()).collect();
        available.sort_unstable();
        available.dedup();
        let avail_str = if available.is_empty() {
            "none".to_string()
        } else {
            available.join(", ")
        };
        return Err(format!(
            "error: pass ID '{target_id}' was not found in pass diagnostics.\nAvailable pass IDs: {avail_str}"
        ));
    }

    let mut out = String::new();
    let total = matching.len();
    for (i, rep) in matching.iter().enumerate() {
        if total > 1 {
            out.push_str(&format!(
                "=== Execution {} of {} (Order: {}) ===\n",
                i + 1,
                total,
                rep.execution_order
            ));
        } else {
            out.push_str(&format!(
                "=== Pass Inspection: {} (Order: {}) ===\n",
                rep.pass_id, rep.execution_order
            ));
        }
        out.push_str(&format!("Pass ID:        {}\n", rep.pass_id));
        out.push_str(&format!("Pass Name:      {}\n", rep.pass_name));
        out.push_str(&format!("Execution Order:{}\n", rep.execution_order));
        out.push_str(&format!("Status:         {}\n", rep.status));

        out.push_str("Configuration:\n");
        if rep.pass_config.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for (k, v) in &rep.pass_config {
                out.push_str(&format!("  {k}: {v}\n"));
            }
        }

        out.push_str("Measurements:\n");
        if let Some(dur) = rep.measurements.duration_ms {
            out.push_str(&format!("  Duration:     {:.3} ms\n", dur));
        } else {
            let avail = rep.measurements.get_availability("duration_ms");
            out.push_str(&format!("  Duration:     {}\n", avail));
        }

        out.push_str("  Before Snapshot:\n");
        if let Some(b) = &rep.measurements.before {
            if let Some(gates) = b.total_gate_count {
                out.push_str(&format!("    Total Gates:   {}\n", gates));
            }
            if let Some(cx) = b.cx_count {
                out.push_str(&format!("    CX Gates:      {}\n", cx));
            }
            if let Some(depth) = b.circuit_depth {
                out.push_str(&format!("    Circuit Depth: {}\n", depth));
            }
            if let Some(hist) = &b.per_gate_counts {
                let h_str: Vec<String> = hist.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                out.push_str(&format!("    Histogram:     {}\n", h_str.join(", ")));
            }
        } else {
            let avail = rep.measurements.get_availability("before");
            out.push_str(&format!("    (availability: {avail})\n"));
        }

        out.push_str("  After Snapshot:\n");
        if let Some(a) = &rep.measurements.after {
            if let Some(gates) = a.total_gate_count {
                out.push_str(&format!("    Total Gates:   {}\n", gates));
            }
            if let Some(cx) = a.cx_count {
                out.push_str(&format!("    CX Gates:      {}\n", cx));
            }
            if let Some(depth) = a.circuit_depth {
                out.push_str(&format!("    Circuit Depth: {}\n", depth));
            }
            if let Some(hist) = &a.per_gate_counts {
                let h_str: Vec<String> = hist.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                out.push_str(&format!("    Histogram:     {}\n", h_str.join(", ")));
            }
        } else {
            let avail = rep.measurements.get_availability("after");
            out.push_str(&format!("    (availability: {avail})\n"));
        }

        out.push_str("Transformation Accounting:\n");
        out.push_str(&format!(
            "  Operations Added:     {}\n",
            rep.transformation.operations_added
        ));
        out.push_str(&format!(
            "  Operations Removed:   {}\n",
            rep.transformation.operations_removed
        ));
        out.push_str(&format!(
            "  Operations Rewritten: {}\n",
            rep.transformation.operations_rewritten
        ));
        if let Some(phase) = &rep.transformation.global_phase_adjustment {
            out.push_str(&format!(
                "  Phase Adjustment:     {:.6} rad ({})\n",
                phase.radians, phase.expression
            ));
        } else {
            out.push_str("  Phase Adjustment:     none\n");
        }
        if rep.transformation.analysis_invalidations.is_empty() {
            out.push_str("  Invalidations:        none\n");
        } else {
            out.push_str(&format!(
                "  Invalidations:        {}\n",
                rep.transformation.analysis_invalidations.join(", ")
            ));
        }

        out.push_str("Circuit Fingerprints:\n");
        out.push_str(&format!(
            "  Algorithm:            {}\n",
            rep.fingerprints.algorithm
        ));
        out.push_str(&format!(
            "  Input Fingerprint:    {}\n",
            rep.fingerprints
                .input_fingerprint
                .as_deref()
                .unwrap_or("uncomputed")
        ));
        out.push_str(&format!(
            "  Output Fingerprint:   {}\n",
            rep.fingerprints
                .output_fingerprint
                .as_deref()
                .unwrap_or("uncomputed")
        ));
        out.push_str(&format!(
            "  Semantic Equivalence Guaranteed: {} (representation fingerprint only)\n",
            rep.fingerprints.semantic_equivalence_guaranteed
        ));

        out.push_str("Verification:\n");
        out.push_str(&format!("  Status:     {}\n", rep.verification.status));
        if let Some(v_name) = &rep.verification.verifier_name {
            let ver_str = rep
                .verification
                .verifier_version
                .as_deref()
                .unwrap_or("unknown");
            let mode_str = rep
                .verification
                .verifier_mode
                .as_deref()
                .unwrap_or("unknown");
            out.push_str(&format!(
                "  Verifier:   {v_name} (v{ver_str}, mode: {mode_str})\n"
            ));
        }
        if let Some(prov) = &rep.verification.provenance {
            out.push_str(&format!("  Provenance: {}\n", prov));
        }
        if let Some(detail) = &rep.verification.detail {
            out.push_str(&format!("  Detail:     {}\n", detail));
        }

        if i + 1 < total {
            out.push('\n');
        }
    }

    Ok(out)
}

fn handle_compilation_error_diagnostics(
    opts: &CompileOptions,
    diagnostics: &[PassDiagnosticReport],
    error_msg: &str,
) -> CliExitCode {
    eprintln!("{error_msg}");
    if opts.pass_summary {
        eprint!("{}", render_pass_summary(diagnostics));
    }
    if let Some(pass_id) = &opts.inspect_pass {
        if let Ok(inspection) = render_pass_inspection(diagnostics, pass_id) {
            eprint!("{inspection}");
        }
    }
    if let Some(report_file) = &opts.pass_report {
        if let Ok(mut json) = pass_diagnostics_to_json_pretty(diagnostics) {
            json.push('\n');
            let _ = fs::write(report_file, json);
        }
    }
    CliExitCode::CompileError
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
            "--emit-manifest" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--emit-manifest'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.manifest_file = Some(args[i].clone());
            }
            "--pass-report" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--pass-report'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.pass_report = Some(args[i].clone());
            }
            "--inspect-pass" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--inspect-pass'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.inspect_pass = Some(args[i].clone());
            }
            "--pass-summary" => {
                opts.pass_summary = true;
            }
            "--pass-diagnostics" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for '--pass-diagnostics'".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                let mode_lower = args[i].to_lowercase();
                match mode_lower.as_str() {
                    "basic" | "metrics" | "fingerprints" | "standard" | "full" => {
                        opts.pass_diagnostics_mode = Some(mode_lower);
                    }
                    other => {
                        return Err((
                            format!("error: unrecognized pass-diagnostics mode '{other}'; expected 'basic', 'metrics', 'fingerprints', 'standard', or 'full'"),
                            CliExitCode::CliError,
                        ));
                    }
                }
            }
            "--pass-metrics" => {
                opts.pass_metrics = true;
            }
            "--pass-fingerprints" => {
                opts.pass_fingerprints = true;
            }
            "--pass-verify" => {
                opts.pass_verify = true;
            }
            "-O" | "--opt-level" => {
                i += 1;
                if i >= args.len() {
                    return Err((
                        "error: missing argument for optimization level".to_string(),
                        CliExitCode::CliError,
                    ));
                }
                opts.opt_level = parse_opt_level(&args[i])?;
            }
            opt if opt.starts_with("-O") && opt.len() > 2 => {
                opts.opt_level = parse_opt_level(&opt[2..])?;
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

    let diag_cfg = resolve_diagnostic_config(opts);
    let mut pass_diagnostics: Vec<PassDiagnosticReport> = Vec::new();

    // 5. If a target is specified, run the integrated compilation pipeline
    let (frontend_circuit, compilation_result): (Option<_>, Option<CompilationResult>) =
        if let Some(target_name) = &opts.target {
            let num_qubits = circuit.num_qubits().max(2);
            let target = match target_name.to_lowercase().as_str() {
                "linear" | "line" => create_standard_target("linear_target", num_qubits, false),
                "ring" => create_standard_target("ring_target", num_qubits, true),
                "linear-generic" => {
                    create_generic_target("linear_generic_target", num_qubits, false)
                }
                "ring-generic" => create_generic_target("ring_generic_target", num_qubits, true),
                other => {
                    eprintln!(
                        "error: unsupported target topology '{other}'; supported: 'linear', 'ring', 'linear-generic', 'ring-generic'"
                    );
                    return CliExitCode::CliError;
                }
            };

            let compiler_config =
                CompilerConfig::for_level(opts.opt_level).with_diagnostics(diag_cfg.clone());
            let pipeline = CompilerPipeline::new(compiler_config);
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
                        eprintln!(
                            "| Original SWAPs | {} | {} |",
                            m.original_swaps, m.final_swaps
                        );
                        eprintln!(
                            "| Router SWAPs inserted | 0 | {} |",
                            m.router_swaps_inserted
                        );
                    }
                    pass_diagnostics = result.pass_diagnostics.clone();
                    (None, Some(result))
                }
                Err(e) => {
                    pass_diagnostics = e.pass_diagnostics().to_vec();
                    return handle_compilation_error_diagnostics(
                        opts,
                        &pass_diagnostics,
                        &format!("error: compilation pipeline failed: {e}"),
                    );
                }
            }
        } else {
            let needs_optimizer = diag_cfg.enabled
                || opts.pass_report.is_some()
                || opts.inspect_pass.is_some()
                || opts.pass_summary
                || opts.opt_level != OptimizationLevel::O2;

            if needs_optimizer {
                let mut opt_circuit = circuit.clone();
                let opt_config =
                    OptimizerConfig::new(opts.opt_level).with_diagnostics(diag_cfg.clone());
                let optimizer = OptimizerPipeline::new(opt_config);
                let mut opt_context = PassContext::new();
                match optimizer.optimize_with_context(&mut opt_circuit, &mut opt_context) {
                    Ok(rep) => {
                        pass_diagnostics = rep.pass_diagnostics;
                        if opts.stats {
                            eprintln!("| | Before | After |");
                            eprintln!("|---|---|---|");
                            eprintln!(
                                "| Gates | {} | {} |",
                                rep.initial_metrics.total_operations,
                                rep.final_metrics.total_operations
                            );
                            eprintln!(
                                "| Depth | {} | {} |",
                                rep.initial_metrics.circuit_depth, rep.final_metrics.circuit_depth
                            );
                            eprintln!("| Original SWAPs | 0 | 0 |");
                            eprintln!("| Router SWAPs inserted | 0 | 0 |");
                        }
                        (Some(opt_circuit), None)
                    }
                    Err(e) => {
                        pass_diagnostics = opt_context.diagnostic_reports().to_vec();
                        return handle_compilation_error_diagnostics(
                            opts,
                            &pass_diagnostics,
                            &format!("error: optimization pipeline failed: {e}"),
                        );
                    }
                }
            } else {
                if opts.stats {
                    let initial_gates = circuit.num_operations();
                    let initial_depth = CircuitMetrics::compute(&circuit).circuit_depth;
                    let orig_swaps = circuit
                        .graph
                        .ordered_nodes()
                        .map(|nodes| {
                            nodes
                                .iter()
                                .filter(|n| n.operation.name().eq_ignore_ascii_case("swap"))
                                .count()
                        })
                        .unwrap_or(0);
                    eprintln!("| | Before | After |");
                    eprintln!("|---|---|---|");
                    eprintln!("| Gates | {initial_gates} | {initial_gates} |");
                    eprintln!("| Depth | {initial_depth} | {initial_depth} |");
                    eprintln!("| Original SWAPs | {orig_swaps} | {orig_swaps} |");
                    eprintln!("| Router SWAPs inserted | 0 | 0 |");
                }
                (Some(circuit), None)
            }
        };

    // 6. Handle terminal diagnostics (summary and inspection)
    if opts.pass_summary {
        eprint!("{}", render_pass_summary(&pass_diagnostics));
    }

    if let Some(pass_id) = &opts.inspect_pass {
        match render_pass_inspection(&pass_diagnostics, pass_id) {
            Ok(inspection) => {
                eprint!("{inspection}");
            }
            Err(err_msg) => {
                eprintln!("{err_msg}");
                return CliExitCode::CliError;
            }
        }
    }

    // 7. Write pass report file if requested
    if let Some(report_path) = &opts.pass_report {
        match pass_diagnostics_to_json_pretty(&pass_diagnostics) {
            Ok(mut json) => {
                json.push('\n');
                if let Err(e) = fs::write(report_path, json) {
                    eprintln!(
                        "error: failed to write pass diagnostic report file '{report_path}': {e}"
                    );
                    return CliExitCode::CliError;
                }
            }
            Err(e) => {
                eprintln!("error: failed to serialize pass diagnostic report: {e}");
                return CliExitCode::CliError;
            }
        }
    }

    let final_circuit = compilation_result
        .as_ref()
        .map(CompilationResult::circuit)
        .or(frontend_circuit.as_ref())
        .expect("Compiled or lowered circuit must be present");

    // 8. Format output
    let output_text = match opts.format {
        CompileFormat::OpenQasm => {
            let emitter_opts = EmitterOptions {
                global_phase_policy: opts.global_phase_policy,
                measurement_syntax: opts.measurement_syntax,
                ..Default::default()
            };
            match OpenQasmEmitter::with_options(emitter_opts).emit(final_circuit) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("error: failed to emit OpenQASM: {e}");
                    return CliExitCode::CliError;
                }
            }
        }
        CompileFormat::QccIr => {
            format!("{final_circuit:#?}\n")
        }
    };

    // 9. Build optional manifest JSON directly from compiler result data
    let manifest_json = if opts.manifest_file.is_some() {
        let (input_kind, input_path) = if opts.input_file == "-" {
            (ManifestInputKind::Stdin, None)
        } else {
            (ManifestInputKind::File, Some(opts.input_file.clone()))
        };
        let (output_destination, output_path) = match &opts.output_file {
            Some(path) => (ManifestOutputDestination::File, Some(path.clone())),
            None => (ManifestOutputDestination::Stdout, None),
        };
        let format = match opts.format {
            CompileFormat::OpenQasm => ManifestOutputFormat::OpenQasm,
            CompileFormat::QccIr => ManifestOutputFormat::QccIr,
        };
        let parameter_bindings: BTreeMap<String, f64> =
            opts.bindings.iter().map(|(k, &v)| (k.clone(), v)).collect();

        let ctx = ManifestBuildContext {
            input_kind,
            input_path,
            cli_target_spec: opts.target.clone(),
            format,
            output_destination,
            output_path,
            emitted_text: &output_text,
            global_phase_policy: opts.global_phase_policy,
            measurement_syntax: opts.measurement_syntax,
            parameter_bindings,
        };

        let manifest = if let Some(result) = compilation_result.as_ref() {
            CompilationManifest::from_compilation_result(result, &ctx)
        } else {
            CompilationManifest::from_frontend_circuit(
                frontend_circuit
                    .as_ref()
                    .expect("Frontend circuit must be present"),
                &ctx,
            )
        };

        match manifest.to_json_pretty() {
            Ok(mut json) => {
                json.push('\n');
                Some(json)
            }
            Err(e) => {
                eprintln!("error: failed to serialize compilation manifest: {e}");
                return CliExitCode::CliError;
            }
        }
    } else {
        None
    };

    // 10. Write circuit output and optional manifest
    if let Some(out_path) = &opts.output_file {
        if let Err(e) = fs::write(out_path, &output_text) {
            eprintln!("error: failed to write output file '{out_path}': {e}");
            return CliExitCode::CliError;
        }
    }

    if let (Some(manifest_path), Some(json)) = (&opts.manifest_file, &manifest_json) {
        if let Err(e) = fs::write(manifest_path, json) {
            eprintln!("error: failed to write manifest file '{manifest_path}': {e}");
            return CliExitCode::CliError;
        }
    }

    if opts.output_file.is_none() {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(output_text.as_bytes()) {
            eprintln!("error: failed to write to standard output: {e}");
            return CliExitCode::CliError;
        }
    }

    CliExitCode::Success
}

/// Helper building a standard target topology with strict hardware-native basis {RZ, SX, CX}.
fn create_standard_target(name: &str, num_qubits: usize, is_ring: bool) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
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
        .add_operation(sx_desc)
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
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(150.0)
            .build();

        builder = builder.add_operation(cx_desc);
    }

    builder.build().expect("Failed to build target model")
}

/// Helper building a generic target topology supporting canonical gates and native SWAP.
fn create_generic_target(name: &str, num_qubits: usize, is_ring: bool) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    for op_name in &["h", "x", "y", "z", "s", "sdg", "t", "tdg", "sx", "sxdg"] {
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
