//! Versioned, machine-readable compilation manifest for QCC.
//!
//! Defines [`CompilationManifest`] (schema `qcc.compile_manifest` v1.0.0) generated
//! directly from [`CompilationResult`] (or frontend [`Circuit`] when compiled without
//! a hardware target). Provides the structured interface between QCC and the
//! independent verification oracle without stdout parsing or guessed defaults.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::backend::openqasm::format::{format_parameter_expr, is_zero_parameter};
use crate::backend::openqasm::{GlobalPhasePolicy, MeasurementSyntax};
use crate::compiler::result::{CompilationResult, VerificationStatus};
use crate::ir::circuit::Circuit;
use crate::ir::parameter::ParameterExpr;
use crate::optimizer::report::CircuitMetrics;
use crate::routing::layout::CircuitLayout;
use crate::target::connectivity::OperationConnectivity;
use crate::target::model::Target;

/// Canonical schema identifier for QCC compilation manifests.
pub const MANIFEST_SCHEMA_ID: &str = "qcc.compile_manifest";

/// Current semantic schema version for [`CompilationManifest`].
pub const MANIFEST_SCHEMA_VERSION: &str = "1.0.0";

/// Explicit compilation mode recorded in the manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestCompilationMode {
    /// Full target compilation pipeline executed via [`crate::compiler::CompilerPipeline`].
    TargetCompiled,
    /// Frontend lowering and emission without a physical target.
    FrontendOnly,
}

/// Source input kind for compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestInputKind {
    /// Input read from a file path on disk.
    File,
    /// Input read from standard input.
    Stdin,
}

/// Format of the emitted circuit artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestOutputFormat {
    /// Canonical OpenQASM 3.0 output.
    OpenQasm,
    /// Human-readable QccIR debug dump.
    QccIr,
}

/// Output destination of the emitted circuit artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestOutputDestination {
    /// Written to a file on disk.
    File,
    /// Streamed to standard output.
    Stdout,
}

/// Compiler and build identity recorded in the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerIdentityManifest {
    /// Package name (`"qcc"`).
    pub name: String,
    /// Package semantic version (e.g. `"0.1.0"`).
    pub version: String,
    /// Formatted version string (e.g. `"qcc 0.1.0"`).
    pub version_string: String,
    /// Cargo build profile (`"debug"` or `"release"`).
    pub profile: String,
    /// Target operating system (`std::env::consts::OS`).
    pub target_os: String,
    /// Target CPU architecture (`std::env::consts::ARCH`).
    pub target_arch: String,
    /// Optional build-time Git commit SHA (populated only when `QCC_GIT_SHA` is supplied at compile time).
    pub git_commit: Option<String>,
}

impl CompilerIdentityManifest {
    /// Captures QCC package and compile-time build identity without fabricating unavailable fields.
    pub fn current() -> Self {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let git_commit = option_env!("QCC_GIT_SHA")
            .filter(|s| !s.trim().is_empty())
            .map(ToString::to_string);

        Self {
            name: crate::PKG_NAME.to_string(),
            version: crate::PKG_VERSION.to_string(),
            version_string: crate::version_string(),
            profile: profile.to_string(),
            target_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
            git_commit,
        }
    }
}

/// Input source provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSourceManifest {
    /// Whether the input came from a file or stdin.
    pub kind: ManifestInputKind,
    /// Input file path when `kind == File`; `None` for stdin.
    pub path: Option<String>,
}

/// Connectivity specification of a native target operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetConnectivityManifest {
    /// Connectivity model variant.
    pub kind: String,
    /// Allowed physical qubits for single-qubit constraints, when explicitly enumerated.
    pub qubits: Option<Vec<u32>>,
    /// Allowed physical qubit pairs `[source, target]` for two-qubit constraints, when explicitly enumerated.
    pub edges: Option<Vec<[u32; 2]>>,
}

/// Descriptor of one native operation supported by the target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetOperationManifest {
    /// Target-native mnemonic (e.g. `"rz"`, `"sx"`, `"cx"`).
    pub name: String,
    /// Quantum operand arity.
    pub quantum_arity: usize,
    /// Directionality constraint (`"single_qubit"`, `"directed"`, `"undirected"`, or `"none"`).
    pub directionality: String,
    /// Physical connectivity details.
    pub connectivity: TargetConnectivityManifest,
}

/// Target identity and configuration summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetManifest {
    /// Target device identifier (e.g. `"linear_target"`).
    pub name: String,
    /// CLI `--target` selector string, if invoked via CLI.
    pub cli_target_spec: Option<String>,
    /// Physical qubit capacity of the target.
    pub num_qubits: usize,
    /// Deterministically ordered list of native target operations.
    pub native_operations: Vec<TargetOperationManifest>,
}

impl TargetManifest {
    /// Builds a [`TargetManifest`] directly from a [`Target`].
    pub fn from_target(target: &Target, cli_target_spec: Option<String>) -> Self {
        let native_operations = target
            .operations()
            .iter()
            .map(|(op_id, desc)| {
                let (directionality, connectivity) = match &desc.connectivity {
                    OperationConnectivity::SingleQubit { qubits } => (
                        "single_qubit".to_string(),
                        TargetConnectivityManifest {
                            kind: "single_qubit".to_string(),
                            qubits: Some(qubits.iter().map(|p| p.0).collect()),
                            edges: None,
                        },
                    ),
                    OperationConnectivity::DirectedTwoQubit { edges } => (
                        "directed".to_string(),
                        TargetConnectivityManifest {
                            kind: "directed_two_qubit".to_string(),
                            qubits: None,
                            edges: Some(edges.iter().map(|(u, v)| [u.0, v.0]).collect()),
                        },
                    ),
                    OperationConnectivity::UndirectedTwoQubit { edges } => (
                        "undirected".to_string(),
                        TargetConnectivityManifest {
                            kind: "undirected_two_qubit".to_string(),
                            qubits: None,
                            edges: Some(edges.iter().map(|(u, v)| [u.0, v.0]).collect()),
                        },
                    ),
                    OperationConnectivity::AllToAll => {
                        let dir = if desc.quantum_arity <= 1 {
                            "single_qubit"
                        } else {
                            "undirected"
                        };
                        (
                            dir.to_string(),
                            TargetConnectivityManifest {
                                kind: "all_to_all".to_string(),
                                qubits: None,
                                edges: None,
                            },
                        )
                    }
                    OperationConnectivity::General { tuples } => {
                        let dir = if desc.quantum_arity == 0 {
                            "none"
                        } else if desc.quantum_arity == 1 {
                            "single_qubit"
                        } else {
                            "directed"
                        };
                        let edges = if desc.quantum_arity == 2
                            && tuples.iter().all(|tuple| tuple.len() == 2)
                        {
                            Some(
                                tuples
                                    .iter()
                                    .map(|tuple| [tuple[0].0, tuple[1].0])
                                    .collect(),
                            )
                        } else {
                            None
                        };
                        (
                            dir.to_string(),
                            TargetConnectivityManifest {
                                kind: "general".to_string(),
                                qubits: None,
                                edges,
                            },
                        )
                    }
                };

                TargetOperationManifest {
                    name: op_id.as_str().to_string(),
                    quantum_arity: desc.quantum_arity,
                    directionality,
                    connectivity,
                }
            })
            .collect();

        Self {
            name: target.name().to_string(),
            cli_target_spec,
            num_qubits: target.num_qubits(),
            native_operations,
        }
    }
}

/// Explicit initial and final logical-to-physical qubit mappings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutManifest {
    /// Explicit mapping direction contract: `"logical_to_physical"`.
    pub mapping_direction: String,
    /// Qubit bit-ordering convention: `"little_endian_q0_lsb"`.
    pub qubit_indexing_convention: String,
    /// Logical qubit count capacity.
    pub num_logical: usize,
    /// Physical device qubit count capacity.
    pub num_physical: usize,
    /// Initial layout mapping: logical qubit index -> physical qubit index ($L_{\text{initial}}$).
    pub initial_logical_to_physical: BTreeMap<usize, u32>,
    /// Final layout mapping: logical qubit index -> physical qubit index ($L_{\text{final}}$).
    pub final_logical_to_physical: BTreeMap<usize, u32>,
    /// Sorted list of physical qubit indices outside $L_{\text{initial}}$ image.
    pub unused_physical_qubits_initial: Vec<u32>,
    /// Sorted list of physical qubit indices outside $L_{\text{final}}$ image (ADR-0001 clean ancillas).
    pub unused_physical_qubits_final: Vec<u32>,
}

impl LayoutManifest {
    /// Builds a [`LayoutManifest`] directly from [`CircuitLayout`].
    pub fn from_circuit_layout(layout: &CircuitLayout) -> Self {
        let initial = layout.initial();
        let final_map = layout.final_layout();

        let initial_logical_to_physical = extract_logical_to_physical(initial);
        let final_logical_to_physical = extract_logical_to_physical(final_map);

        let unused_physical_qubits_initial = initial
            .unused_physical_qubits()
            .into_iter()
            .map(|p| p.0)
            .collect();
        let unused_physical_qubits_final = final_map
            .unused_physical_qubits()
            .into_iter()
            .map(|p| p.0)
            .collect();

        Self {
            mapping_direction: "logical_to_physical".to_string(),
            qubit_indexing_convention: "little_endian_q0_lsb".to_string(),
            num_logical: initial.num_logical(),
            num_physical: initial.num_physical(),
            initial_logical_to_physical,
            final_logical_to_physical,
            unused_physical_qubits_initial,
            unused_physical_qubits_final,
        }
    }
}

/// Quantum register metadata on the final circuit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantumRegisterManifest {
    /// Register identifier (e.g. `"p"` for routed physical circuits).
    pub name: String,
    /// Register size in qubits.
    pub size: usize,
    /// Base flat qubit index.
    pub start_qubit: usize,
}

/// Classical register metadata on the final circuit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassicalRegisterManifest {
    /// Classical register identifier.
    pub name: String,
    /// Register size in classical bits.
    pub size: usize,
    /// Base flat classical bit index.
    pub start_bit: usize,
}

/// Numeric/symbolic global phase value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalPhaseValueManifest {
    /// Evaluated global phase in radians, when numerically evaluable without unbound symbols.
    pub radians: Option<f64>,
    /// Canonical expression text for the global phase.
    pub expression: String,
    /// True if the phase evaluated to a finite numeric value.
    pub is_numeric: bool,
}

/// Final circuit global phase along with OpenQASM emission metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmittedGlobalPhaseManifest {
    /// Evaluated global phase in radians, when numerically evaluable without unbound symbols.
    pub radians: Option<f64>,
    /// Canonical expression text for the global phase.
    pub expression: String,
    /// True if the phase evaluated to a finite numeric value.
    pub is_numeric: bool,
    /// CLI/emitter global phase policy (`"auto"`, `"always"`, or `"reject-nonzero"`).
    pub policy: String,
    /// True if a `gphase(...);` statement was emitted in the OpenQASM artifact.
    pub emitted_in_openqasm: bool,
}

/// Identity and provenance of the emitted physical circuit artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmittedCircuitManifest {
    /// Artifact format (`"openqasm"` or `"qccir"`).
    pub format: ManifestOutputFormat,
    /// Destination (`"file"` or `"stdout"`).
    pub output_destination: ManifestOutputDestination,
    /// File path when written to disk; `None` when emitted to stdout.
    pub path: Option<String>,
    /// Lowercase hex SHA-256 digest of the exact emitted artifact bytes.
    pub sha256: String,
    /// Exact byte length of the emitted artifact.
    pub byte_length: usize,
}

/// Gate, SWAP, and depth metrics for the compiled circuit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitMetricsManifest {
    /// Total unitary quantum gates (`Operation::Gate`) in the final circuit; equals sum of `per_gate_counts`.
    pub total_gate_count: usize,
    /// Total operations in the final circuit DAG (gates + barriers + non-unitary ops).
    pub total_operations: usize,
    /// Deterministic sorted map of lowercase gate mnemonic to count in the final circuit.
    pub per_gate_counts: BTreeMap<String, usize>,
    /// Total 1-qubit unitary gates in the final circuit.
    pub single_qubit_gates: usize,
    /// Total 2-qubit unitary gates in the final circuit.
    pub two_qubit_gates: usize,
    /// Total multi-qubit (>= 2 qubits) unitary gates in the final circuit.
    pub multi_qubit_gates: usize,
    /// Total `cx` gates in the final circuit.
    pub cx_count: usize,
    /// Total barrier operations in the final circuit.
    pub barrier_operations: usize,
    /// Total non-unitary operations (`measure`, `reset`, `classical`) in the final circuit.
    pub non_unitary_operations: usize,
    /// `swap` operations present in the input logical circuit prior to compilation.
    pub logical_swaps: usize,
    /// Routing SWAP operations inserted during physical routing.
    pub router_swaps_inserted: usize,
    /// `swap` operations remaining in the final emitted circuit.
    pub final_emitted_swaps: usize,
    /// Critical-path DAG depth of the final emitted circuit.
    pub circuit_depth: usize,
    /// Critical-path DAG depth of the input logical circuit.
    pub initial_circuit_depth: Option<usize>,
    /// True if the final circuit contains zero non-unitary operations.
    pub is_unitary: bool,
}

/// Reproducible compilation and emission configuration metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilationConfigMetadataManifest {
    /// Optimization level when target-compiled; `None` in frontend-only mode.
    pub optimization_level: Option<String>,
    /// Router name when target-compiled; `None` in frontend-only mode.
    pub router: Option<String>,
    /// Initial placement strategy when target-compiled; `None` in frontend-only mode.
    pub placement_strategy: Option<String>,
    /// Post-routing scheduler choice when target-compiled; `None` in frontend-only mode.
    pub scheduler: Option<String>,
    /// Whether internal verification was enabled when target-compiled; `None` in frontend-only mode.
    pub verification_enabled: Option<bool>,
    /// Whether target legality validation was enforced; `None` in frontend-only mode.
    pub validate_target_legality: Option<bool>,
    /// Active global phase emission policy (`"auto"`, `"always"`, `"reject-nonzero"`).
    pub global_phase_policy: String,
    /// Active measurement statement syntax (`"assign"`, `"arrow"`).
    pub measurement_syntax: String,
    /// Deterministic sorted map of parameter bindings supplied at compile time.
    pub parameter_bindings: BTreeMap<String, f64>,
}

/// Routing and post-routing translation telemetry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingMetadataManifest {
    /// Name of the router invoked.
    pub router_name: String,
    /// Initial placement strategy used.
    pub placement_strategy: String,
    /// Operation count immediately prior to routing.
    pub original_operations: usize,
    /// Operation count immediately after routing.
    pub routed_operations: usize,
    /// Number of physical SWAP gates inserted by the router.
    pub swaps_inserted: usize,
    /// Entangling operations examined for target directionality.
    pub directions_examined: usize,
    /// Entangling operations corrected for directionality.
    pub directions_corrected: usize,
    /// Operations lowered during post-routing translation.
    pub lowered_operations: usize,
}

/// Final target legality validation metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalityMetadataManifest {
    /// True if the final circuit satisfied all target legality constraints.
    pub is_legal: bool,
    /// Total operations inspected by the legality validator.
    pub operations_examined: usize,
    /// Number of violations detected.
    pub violation_count: usize,
}

/// Post-routing scheduling metadata (present only when scheduling ran).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchedulingMetadataManifest {
    /// Scheduler algorithm invoked.
    pub scheduler_name: String,
    /// Total schedule makespan in nanoseconds.
    pub makespan_ns: f64,
    /// Total scheduled operations.
    pub total_scheduled_operations: usize,
    /// Operations on the critical path.
    pub critical_path_operations: usize,
    /// Peak concurrent scheduled operations.
    pub peak_concurrency: usize,
}

/// QCC's internal equivalence verifier metadata (distinct from any external oracle verdict).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InternalVerificationMetadataManifest {
    /// Verification status string (`"verified_equivalent"`, `"skipped_non_unitary"`, etc.).
    pub status: String,
    /// Internal equivalence verdict boolean.
    pub is_equivalent: bool,
    /// Estimated common global phase difference in radians, if computed.
    pub phase_difference_radians: Option<f64>,
    /// Maximum pointwise amplitude discrepancy, if computed.
    pub max_amplitude_diff: Option<f64>,
}

/// Additional context available at the CLI / emission boundary.
#[derive(Clone, Debug)]
pub struct ManifestBuildContext<'a> {
    /// Whether the input source came from a file or stdin.
    pub input_kind: ManifestInputKind,
    /// Input file path when read from a file.
    pub input_path: Option<String>,
    /// CLI `--target` argument string, if provided.
    pub cli_target_spec: Option<String>,
    /// Emitted output format (`OpenQasm` or `QccIr`).
    pub format: ManifestOutputFormat,
    /// Destination (`File` or `Stdout`).
    pub output_destination: ManifestOutputDestination,
    /// Output file path when written to disk.
    pub output_path: Option<String>,
    /// Exact serialized text of the emitted circuit artifact.
    pub emitted_text: &'a str,
    /// Active global phase emission policy.
    pub global_phase_policy: GlobalPhasePolicy,
    /// Active measurement syntax.
    pub measurement_syntax: MeasurementSyntax,
    /// Runtime parameter bindings supplied at compile time.
    pub parameter_bindings: BTreeMap<String, f64>,
}

/// Versioned, deterministic compilation manifest (`qcc.compile_manifest` v1.0.0).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilationManifest {
    /// Canonical schema identifier (`"qcc.compile_manifest"`).
    pub schema_id: String,
    /// Schema version (`"1.0.0"`).
    pub schema_version: String,
    /// QCC package and build identity.
    pub compiler: CompilerIdentityManifest,
    /// Compilation mode (`"target_compiled"` or `"frontend_only"`).
    pub compilation_mode: ManifestCompilationMode,
    /// Circuit name.
    pub circuit_name: String,
    /// Source input provenance.
    pub input_source: InputSourceManifest,
    /// Logical qubit count in the input circuit.
    pub logical_qubits: usize,
    /// Physical qubit count on the target (`None` in frontend-only mode).
    pub physical_qubits: Option<usize>,
    /// Hardware target identity and configuration (`None` in frontend-only mode).
    pub target: Option<TargetManifest>,
    /// Explicit logical-to-physical initial and final layouts (`None` in frontend-only mode).
    pub layout: Option<LayoutManifest>,
    /// Quantum registers on the emitted circuit.
    pub quantum_registers: Vec<QuantumRegisterManifest>,
    /// Classical registers on the emitted circuit.
    pub classical_registers: Vec<ClassicalRegisterManifest>,
    /// Input logical circuit global phase.
    pub input_global_phase: GlobalPhaseValueManifest,
    /// Emitted circuit global phase and policy metadata.
    pub global_phase: EmittedGlobalPhaseManifest,
    /// Identity, digest, and path of the emitted physical circuit artifact.
    pub emitted_circuit: EmittedCircuitManifest,
    /// Unambiguous gate, SWAP, and depth metrics of the emitted circuit.
    pub metrics: CircuitMetricsManifest,
    /// Compiler and emitter configuration metadata.
    pub config: CompilationConfigMetadataManifest,
    /// Routing and post-routing translation telemetry (`None` in frontend-only mode).
    pub routing: Option<RoutingMetadataManifest>,
    /// Final target legality report summary (`None` in frontend-only mode).
    pub legality: Option<LegalityMetadataManifest>,
    /// Post-routing scheduling summary (`None` when disabled or frontend-only).
    pub scheduling: Option<SchedulingMetadataManifest>,
    /// Internal verifier status (`None` in frontend-only mode; never replaces external oracle).
    pub internal_verification: Option<InternalVerificationMetadataManifest>,
}

impl CompilationManifest {
    /// Constructs a [`CompilationManifest`] directly from a [`CompilationResult`].
    pub fn from_compilation_result(
        result: &CompilationResult,
        ctx: &ManifestBuildContext<'_>,
    ) -> Self {
        let report = &result.report;
        let final_circuit = result.circuit();
        let circuit_metrics = CircuitMetrics::compute(final_circuit);

        let per_gate_counts = circuit_metrics.gate_histogram.clone();
        let cx_count = per_gate_counts.get("cx").copied().unwrap_or(0);

        let metrics = CircuitMetricsManifest {
            total_gate_count: circuit_metrics.gate_count,
            total_operations: circuit_metrics.total_operations,
            per_gate_counts,
            single_qubit_gates: circuit_metrics.single_qubit_gates,
            two_qubit_gates: circuit_metrics.two_qubit_gates,
            multi_qubit_gates: circuit_metrics.multi_qubit_gates,
            cx_count,
            barrier_operations: circuit_metrics.barrier_operations,
            non_unitary_operations: circuit_metrics.non_unitary_operations,
            logical_swaps: report.metrics.original_swaps,
            router_swaps_inserted: report.metrics.router_swaps_inserted,
            final_emitted_swaps: report.metrics.final_swaps,
            circuit_depth: circuit_metrics.circuit_depth,
            initial_circuit_depth: Some(report.metrics.initial_depth),
            is_unitary: circuit_metrics.is_unitary(),
        };

        let post_routing = report.post_routing_translation.as_ref();
        let routing = Some(RoutingMetadataManifest {
            router_name: report.routing.router_name.clone(),
            placement_strategy: report.routing.placement_strategy.clone(),
            original_operations: report.routing.original_operations,
            routed_operations: report.routing.routed_operations,
            swaps_inserted: report.routing.swaps_inserted,
            directions_examined: post_routing.map(|p| p.directions_examined).unwrap_or(0),
            directions_corrected: post_routing.map(|p| p.directions_corrected).unwrap_or(0),
            lowered_operations: post_routing.map(|p| p.lowered_operations).unwrap_or(0),
        });

        let legality = Some(LegalityMetadataManifest {
            is_legal: report.legality.is_legal,
            operations_examined: report.legality.operations_examined,
            violation_count: report.legality.violation_count,
        });

        let scheduling = report
            .scheduling
            .as_ref()
            .map(|s| SchedulingMetadataManifest {
                scheduler_name: s.scheduler_name.clone(),
                makespan_ns: normalize_float(s.makespan_ns),
                total_scheduled_operations: s.total_scheduled_operations,
                critical_path_operations: s.critical_path_operations,
                peak_concurrency: s.peak_concurrency,
            });

        let internal_verification = Some(InternalVerificationMetadataManifest {
            status: verification_status_label(&report.verification.status).to_string(),
            is_equivalent: report.verification.is_equivalent,
            phase_difference_radians: report.verification.phase_difference.map(normalize_float),
            max_amplitude_diff: report.verification.max_amplitude_diff.map(normalize_float),
        });

        let config = CompilationConfigMetadataManifest {
            optimization_level: Some(report.config.optimization_level.clone()),
            router: Some(report.config.router.clone()),
            placement_strategy: Some(report.config.placement_strategy.clone()),
            scheduler: Some(report.config.scheduler.clone()),
            verification_enabled: Some(report.config.verification_enabled),
            validate_target_legality: Some(report.config.validate_target_legality),
            global_phase_policy: global_phase_policy_label(ctx.global_phase_policy).to_string(),
            measurement_syntax: measurement_syntax_label(ctx.measurement_syntax).to_string(),
            parameter_bindings: normalize_bindings(&ctx.parameter_bindings),
        };

        Self {
            schema_id: MANIFEST_SCHEMA_ID.to_string(),
            schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            compiler: CompilerIdentityManifest::current(),
            compilation_mode: ManifestCompilationMode::TargetCompiled,
            circuit_name: report.circuit_name.clone(),
            input_source: InputSourceManifest {
                kind: ctx.input_kind,
                path: ctx.input_path.clone(),
            },
            logical_qubits: report.logical_qubits,
            physical_qubits: Some(report.physical_qubits),
            target: Some(TargetManifest::from_target(
                result.target(),
                ctx.cli_target_spec.clone(),
            )),
            layout: Some(LayoutManifest::from_circuit_layout(result.layout())),
            quantum_registers: extract_quantum_registers(final_circuit),
            classical_registers: extract_classical_registers(final_circuit),
            input_global_phase: evaluate_global_phase(result.input_global_phase()),
            global_phase: build_emitted_global_phase(
                final_circuit.global_phase(),
                ctx.global_phase_policy,
                ctx.format,
            ),
            emitted_circuit: build_emitted_circuit_manifest(ctx),
            metrics,
            config,
            routing,
            legality,
            scheduling,
            internal_verification,
        }
    }

    /// Constructs a [`CompilationManifest`] for frontend-only compilation (`--target` omitted).
    ///
    /// Target-dependent fields (`physical_qubits`, `target`, `layout`, `routing`, `legality`,
    /// `scheduling`, `internal_verification`) remain explicitly `None` (`null` in JSON).
    pub fn from_frontend_circuit(circuit: &Circuit, ctx: &ManifestBuildContext<'_>) -> Self {
        let circuit_metrics = CircuitMetrics::compute(circuit);
        let per_gate_counts = circuit_metrics.gate_histogram.clone();
        let cx_count = per_gate_counts.get("cx").copied().unwrap_or(0);
        let swap_count = per_gate_counts.get("swap").copied().unwrap_or(0);

        let metrics = CircuitMetricsManifest {
            total_gate_count: circuit_metrics.gate_count,
            total_operations: circuit_metrics.total_operations,
            per_gate_counts,
            single_qubit_gates: circuit_metrics.single_qubit_gates,
            two_qubit_gates: circuit_metrics.two_qubit_gates,
            multi_qubit_gates: circuit_metrics.multi_qubit_gates,
            cx_count,
            barrier_operations: circuit_metrics.barrier_operations,
            non_unitary_operations: circuit_metrics.non_unitary_operations,
            logical_swaps: swap_count,
            router_swaps_inserted: 0,
            final_emitted_swaps: swap_count,
            circuit_depth: circuit_metrics.circuit_depth,
            initial_circuit_depth: Some(circuit_metrics.circuit_depth),
            is_unitary: circuit_metrics.is_unitary(),
        };

        let config = CompilationConfigMetadataManifest {
            optimization_level: None,
            router: None,
            placement_strategy: None,
            scheduler: None,
            verification_enabled: None,
            validate_target_legality: None,
            global_phase_policy: global_phase_policy_label(ctx.global_phase_policy).to_string(),
            measurement_syntax: measurement_syntax_label(ctx.measurement_syntax).to_string(),
            parameter_bindings: normalize_bindings(&ctx.parameter_bindings),
        };

        let phase_val = evaluate_global_phase(circuit.global_phase());

        Self {
            schema_id: MANIFEST_SCHEMA_ID.to_string(),
            schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            compiler: CompilerIdentityManifest::current(),
            compilation_mode: ManifestCompilationMode::FrontendOnly,
            circuit_name: circuit.name.clone(),
            input_source: InputSourceManifest {
                kind: ctx.input_kind,
                path: ctx.input_path.clone(),
            },
            logical_qubits: circuit.num_qubits(),
            physical_qubits: None,
            target: None,
            layout: None,
            quantum_registers: extract_quantum_registers(circuit),
            classical_registers: extract_classical_registers(circuit),
            input_global_phase: phase_val,
            global_phase: build_emitted_global_phase(
                circuit.global_phase(),
                ctx.global_phase_policy,
                ctx.format,
            ),
            emitted_circuit: build_emitted_circuit_manifest(ctx),
            metrics,
            config,
            routing: None,
            legality: None,
            scheduling: None,
            internal_verification: None,
        }
    }

    /// Serializes this manifest to compact deterministic JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this manifest to pretty-printed deterministic JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a manifest from JSON.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

fn extract_logical_to_physical(
    layout: &crate::routing::layout_map::LayoutMap,
) -> BTreeMap<usize, u32> {
    layout
        .assigned_logical_qubits()
        .into_iter()
        .filter_map(|logical| {
            layout
                .get_physical(logical)
                .map(|physical| (logical.0, physical.0))
        })
        .collect()
}

fn extract_quantum_registers(circuit: &Circuit) -> Vec<QuantumRegisterManifest> {
    circuit
        .quantum_registers
        .iter()
        .map(|reg| QuantumRegisterManifest {
            name: reg.name.clone(),
            size: reg.size,
            start_qubit: reg.start_qubit,
        })
        .collect()
}

fn extract_classical_registers(circuit: &Circuit) -> Vec<ClassicalRegisterManifest> {
    circuit
        .classical_registers
        .iter()
        .map(|reg| ClassicalRegisterManifest {
            name: reg.name.clone(),
            size: reg.size,
            start_bit: reg.start_bit,
        })
        .collect()
}

fn evaluate_global_phase(expr: &ParameterExpr) -> GlobalPhaseValueManifest {
    let expression = format_parameter_expr(expr).unwrap_or_else(|_| expr.to_string());
    match expr.eval(&HashMap::new()) {
        Ok(val) if val.is_finite() => GlobalPhaseValueManifest {
            radians: Some(normalize_float(val)),
            expression,
            is_numeric: true,
        },
        _ => GlobalPhaseValueManifest {
            radians: None,
            expression,
            is_numeric: false,
        },
    }
}

fn build_emitted_global_phase(
    expr: &ParameterExpr,
    policy: GlobalPhasePolicy,
    format: ManifestOutputFormat,
) -> EmittedGlobalPhaseManifest {
    let val = evaluate_global_phase(expr);
    let emitted_in_openqasm = match format {
        ManifestOutputFormat::QccIr => false,
        ManifestOutputFormat::OpenQasm => match policy {
            GlobalPhasePolicy::AlwaysEmit => true,
            GlobalPhasePolicy::EmitGPhase => !is_zero_parameter(expr),
            GlobalPhasePolicy::RejectNonZero => false,
        },
    };
    EmittedGlobalPhaseManifest {
        radians: val.radians,
        expression: val.expression,
        is_numeric: val.is_numeric,
        policy: global_phase_policy_label(policy).to_string(),
        emitted_in_openqasm,
    }
}

fn build_emitted_circuit_manifest(ctx: &ManifestBuildContext<'_>) -> EmittedCircuitManifest {
    let bytes = ctx.emitted_text.as_bytes();
    EmittedCircuitManifest {
        format: ctx.format,
        output_destination: ctx.output_destination,
        path: ctx.output_path.clone(),
        sha256: sha256_hex(bytes),
        byte_length: bytes.len(),
    }
}

fn global_phase_policy_label(policy: GlobalPhasePolicy) -> &'static str {
    match policy {
        GlobalPhasePolicy::EmitGPhase => "auto",
        GlobalPhasePolicy::AlwaysEmit => "always",
        GlobalPhasePolicy::RejectNonZero => "reject-nonzero",
    }
}

fn measurement_syntax_label(syntax: MeasurementSyntax) -> &'static str {
    match syntax {
        MeasurementSyntax::Assignment => "assign",
        MeasurementSyntax::Arrow => "arrow",
    }
}

fn verification_status_label(status: &VerificationStatus) -> &'static str {
    match status {
        VerificationStatus::VerifiedEquivalent => "verified_equivalent",
        VerificationStatus::VerifiedNotEquivalent { .. } => "verified_not_equivalent",
        VerificationStatus::SkippedNonUnitary => "skipped_non_unitary",
        VerificationStatus::SkippedExceedsQubitLimit { .. } => "skipped_exceeds_qubit_limit",
        VerificationStatus::Disabled => "disabled",
    }
}

fn normalize_float(val: f64) -> f64 {
    if val == 0.0 {
        0.0
    } else {
        val
    }
}

fn normalize_bindings(bindings: &BTreeMap<String, f64>) -> BTreeMap<String, f64> {
    bindings
        .iter()
        .map(|(k, &v)| (k.clone(), normalize_float(v)))
        .collect()
}

/// Computes the lowercase hexadecimal SHA-256 digest (FIPS 180-4) of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((data.len() + 9 + 63) & !63);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let offset = i * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        use std::fmt::Write;
        let _ = write!(hex, "{word:08x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_fips_180_4_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn test_symbolic_global_phase_reports_none_radians() {
        let expr = ParameterExpr::symbol("theta");
        let val = evaluate_global_phase(&expr);
        assert_eq!(val.radians, None);
        assert!(!val.is_numeric);
        assert_eq!(val.expression, "theta");
    }

    #[test]
    fn test_numeric_global_phase_normalizes_negative_zero() {
        let expr = ParameterExpr::Constant(-0.0);
        let val = evaluate_global_phase(&expr);
        assert_eq!(val.radians, Some(0.0));
        assert!(val.radians.unwrap().is_sign_positive());
        assert!(val.is_numeric);
    }

    #[test]
    fn test_emitted_global_phase_policy_flags() {
        let zero = ParameterExpr::zero();
        let auto_zero = build_emitted_global_phase(
            &zero,
            GlobalPhasePolicy::EmitGPhase,
            ManifestOutputFormat::OpenQasm,
        );
        assert_eq!(auto_zero.policy, "auto");
        assert!(!auto_zero.emitted_in_openqasm);

        let always_zero = build_emitted_global_phase(
            &zero,
            GlobalPhasePolicy::AlwaysEmit,
            ManifestOutputFormat::OpenQasm,
        );
        assert_eq!(always_zero.policy, "always");
        assert!(always_zero.emitted_in_openqasm);

        let qccir_nonzero = build_emitted_global_phase(
            &ParameterExpr::Constant(1.25),
            GlobalPhasePolicy::AlwaysEmit,
            ManifestOutputFormat::QccIr,
        );
        assert!(!qccir_nonzero.emitted_in_openqasm);
    }

    #[test]
    fn test_layout_manifest_explicit_direction_and_ancillas() {
        use crate::ir::wire::QubitId;
        use crate::routing::layout_map::LayoutMap;
        use crate::target::operation::PhysicalQubitId;

        let initial = LayoutMap::from_mappings(
            2,
            4,
            [
                (QubitId(0), PhysicalQubitId(2)),
                (QubitId(1), PhysicalQubitId(0)),
            ],
        )
        .unwrap();
        let final_map = LayoutMap::from_mappings(
            2,
            4,
            [
                (QubitId(0), PhysicalQubitId(3)),
                (QubitId(1), PhysicalQubitId(1)),
            ],
        )
        .unwrap();
        let layout =
            CircuitLayout::with_layouts(initial.clone(), final_map.clone(), final_map).unwrap();

        let lm = LayoutManifest::from_circuit_layout(&layout);
        assert_eq!(lm.mapping_direction, "logical_to_physical");
        assert_eq!(lm.qubit_indexing_convention, "little_endian_q0_lsb");
        assert_eq!(lm.num_logical, 2);
        assert_eq!(lm.num_physical, 4);
        assert_eq!(lm.initial_logical_to_physical.get(&0), Some(&2));
        assert_eq!(lm.initial_logical_to_physical.get(&1), Some(&0));
        assert_eq!(lm.final_logical_to_physical.get(&0), Some(&3));
        assert_eq!(lm.final_logical_to_physical.get(&1), Some(&1));
        assert_eq!(lm.unused_physical_qubits_initial, vec![1, 3]);
        assert_eq!(lm.unused_physical_qubits_final, vec![0, 2]);
    }
}
