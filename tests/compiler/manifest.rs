//! Integration tests for machine-readable compilation manifest generation (`CompilationManifest`).

use std::collections::{BTreeMap, BTreeSet};

use qcc::backend::openqasm::{GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter};
use qcc::builder::CircuitBuilder;
use qcc::compiler::{
    CompilationManifest, CompilerConfig, CompilerPipeline, ManifestBuildContext,
    ManifestCompilationMode, ManifestInputKind, ManifestOutputDestination, ManifestOutputFormat,
    MANIFEST_SCHEMA_ID, MANIFEST_SCHEMA_VERSION,
};
use qcc::gate_signature::ParameterDomain;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::routing::layout_map::LayoutMap;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::resources::ResourceModel;
use qcc::target::timing::{OperationDuration, TimingModel};

fn make_linear_native_target(
    name: &str,
    num_qubits: usize,
    directed: bool,
    with_timing: bool,
) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let sx = if with_timing {
        TargetOperationBuilder::new("sx", 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(30.0)
            .build()
    } else {
        TargetOperationBuilder::new("sx", 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build()
    };

    let rz = if with_timing {
        TargetOperationBuilder::new("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(0.0)
            .build()
    } else {
        TargetOperationBuilder::new("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build()
    };

    builder = builder.add_operation(sx).add_operation(rz);

    let edges: Vec<(PhysicalQubitId, PhysicalQubitId)> = (0..num_qubits.saturating_sub(1))
        .map(|i| (PhysicalQubitId(i as u32), PhysicalQubitId((i + 1) as u32)))
        .collect();

    if !edges.is_empty() {
        let conn = if directed {
            OperationConnectivity::directed_two_qubit(edges)
        } else {
            OperationConnectivity::undirected_two_qubit(edges)
        };
        let cx = if with_timing {
            TargetOperationBuilder::new("cx", 2)
                .connectivity(conn)
                .duration(150.0)
                .build()
        } else {
            TargetOperationBuilder::new("cx", 2)
                .connectivity(conn)
                .build()
        };
        builder = builder.add_operation(cx);
    }

    if with_timing {
        let mut timing = TimingModel::new();
        timing.clock_period_ns = Some(1.0);
        timing.set_default_duration("sx", OperationDuration::known_ns(30.0));
        timing.set_default_duration("rz", OperationDuration::known_ns(0.0));
        timing.set_default_duration("cx", OperationDuration::known_ns(150.0));

        let mut resources = ResourceModel::new();
        resources.set_default_requirements("sx", vec![]);
        resources.set_default_requirements("rz", vec![]);
        resources.set_default_requirements("cx", vec![]);

        builder = builder.timing(timing).resources(resources);
    }

    builder.build().expect("valid target")
}

#[test]
fn test_manifest_nontrivial_layout_permutation_and_direction() {
    // Build a 3-qubit circuit whose CX pairs force routing under an explicit permutation.
    let mut builder = CircuitBuilder::new("layout_perm_test");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_native_target("linear_3q", 3, true, false);
    // Asymmetric permutation: q0 -> p2, q1 -> p0, q2 -> p1.
    let initial_layout = LayoutMap::from_mappings(
        3,
        3,
        [
            (QubitId(0), PhysicalQubitId(2)),
            (QubitId(1), PhysicalQubitId(0)),
            (QubitId(2), PhysicalQubitId(1)),
        ],
    )
    .unwrap();

    let config = CompilerConfig::default().with_initial_layout(initial_layout.clone());
    let pipeline = CompilerPipeline::new(config);
    let result = pipeline.compile(&circuit, &target).unwrap();

    let qasm_text = OpenQasmEmitter::new()
        .emit(result.circuit())
        .expect("emit compiled OpenQASM");

    let ctx = ManifestBuildContext {
        input_kind: ManifestInputKind::File,
        input_path: Some("synthetic_perm.qasm".to_string()),
        cli_target_spec: Some("linear".to_string()),
        format: ManifestOutputFormat::OpenQasm,
        output_destination: ManifestOutputDestination::Stdout,
        output_path: None,
        emitted_text: &qasm_text,
        global_phase_policy: GlobalPhasePolicy::EmitGPhase,
        measurement_syntax: MeasurementSyntax::Assignment,
        parameter_bindings: BTreeMap::new(),
    };

    let manifest = CompilationManifest::from_compilation_result(&result, &ctx);

    assert_eq!(manifest.schema_id, MANIFEST_SCHEMA_ID);
    assert_eq!(manifest.schema_version, MANIFEST_SCHEMA_VERSION);
    assert_eq!(
        manifest.compilation_mode,
        ManifestCompilationMode::TargetCompiled
    );
    assert_eq!(manifest.logical_qubits, 3);
    assert_eq!(manifest.physical_qubits, Some(3));

    let layout = manifest.layout.as_ref().expect("layout must be present");
    assert_eq!(layout.mapping_direction, "logical_to_physical");
    assert_eq!(layout.qubit_indexing_convention, "little_endian_q0_lsb");
    assert_eq!(layout.initial_logical_to_physical.get(&0), Some(&2));
    assert_eq!(layout.initial_logical_to_physical.get(&1), Some(&0));
    assert_eq!(layout.initial_logical_to_physical.get(&2), Some(&1));

    // Final layout matches CompilationResult.layout().final_layout() exactly.
    for l in 0..3 {
        let expected_p = result
            .layout()
            .final_layout()
            .get_physical(QubitId(l))
            .unwrap()
            .0;
        assert_eq!(layout.final_logical_to_physical.get(&l), Some(&expected_p));
    }

    // Target directionality is faithfully reported from Target.
    let target_info = manifest.target.as_ref().expect("target must be present");
    assert_eq!(target_info.name, "linear_3q");
    assert_eq!(target_info.num_qubits, 3);
    let cx_op = target_info
        .native_operations
        .iter()
        .find(|op| op.name == "cx")
        .expect("cx must be listed in native operations");
    assert_eq!(cx_op.directionality, "directed");
    assert_eq!(cx_op.connectivity.kind, "directed_two_qubit");
    assert_eq!(cx_op.connectivity.edges, Some(vec![[0, 1], [1, 2]]));

    // Gate histogram reconciles with total gate count.
    let hist_sum: usize = manifest.metrics.per_gate_counts.values().sum();
    assert_eq!(hist_sum, manifest.metrics.total_gate_count);
    assert_eq!(
        manifest.metrics.cx_count,
        manifest
            .metrics
            .per_gate_counts
            .get("cx")
            .copied()
            .unwrap_or(0)
    );
    assert_eq!(
        manifest.metrics.final_emitted_swaps,
        manifest
            .metrics
            .per_gate_counts
            .get("swap")
            .copied()
            .unwrap_or(0)
    );
    assert_eq!(manifest.scheduling, None);

    // JSON round-trip preserves all manifest values identically.
    let json = manifest.to_json_pretty().unwrap();
    let decoded = CompilationManifest::from_json(&json).unwrap();
    assert_eq!(manifest, decoded);
}

#[test]
fn test_manifest_spare_physical_qubits_and_scheduling() {
    // 2 logical qubits compiled onto a 5-qubit linear target with ASAP timing.
    let mut builder = CircuitBuilder::new("spare_ancilla_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.set_global_phase(ParameterExpr::Constant(0.5));
    let circuit = builder.finish().unwrap();

    let target = make_linear_native_target("linear_5q_timed", 5, false, true);
    let config = CompilerConfig::default().with_asap_scheduling();
    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    let qasm_text = OpenQasmEmitter::new().emit(result.circuit()).unwrap();
    let ctx = ManifestBuildContext {
        input_kind: ManifestInputKind::Stdin,
        input_path: None,
        cli_target_spec: Some("linear".to_string()),
        format: ManifestOutputFormat::OpenQasm,
        output_destination: ManifestOutputDestination::File,
        output_path: Some("target/test_spare.qasm".to_string()),
        emitted_text: &qasm_text,
        global_phase_policy: GlobalPhasePolicy::EmitGPhase,
        measurement_syntax: MeasurementSyntax::Assignment,
        parameter_bindings: BTreeMap::new(),
    };

    let manifest = CompilationManifest::from_compilation_result(&result, &ctx);

    assert_eq!(manifest.logical_qubits, 2);
    assert_eq!(manifest.physical_qubits, Some(5));

    let layout = manifest.layout.as_ref().unwrap();
    assert_eq!(layout.initial_logical_to_physical.len(), 2);
    assert_eq!(layout.final_logical_to_physical.len(), 2);
    assert_eq!(layout.unused_physical_qubits_initial.len(), 3);
    assert_eq!(layout.unused_physical_qubits_final.len(), 3);

    // Partitioning property: assigned physical qubits and unused physical qubits cover 0..5 cleanly.
    let assigned_final: BTreeSet<u32> =
        layout.final_logical_to_physical.values().copied().collect();
    let unused_final: BTreeSet<u32> = layout
        .unused_physical_qubits_final
        .iter()
        .copied()
        .collect();
    assert!(assigned_final.is_disjoint(&unused_final));
    assert_eq!(assigned_final.union(&unused_final).count(), 5);

    // Input and output global phase both tracked accurately.
    assert_eq!(manifest.input_global_phase.radians, Some(0.5));
    assert!(manifest.input_global_phase.is_numeric);
    assert!(manifest.global_phase.is_numeric);
    assert!(manifest.global_phase.emitted_in_openqasm);

    // Scheduling metadata present when enabled.
    let sched = manifest
        .scheduling
        .as_ref()
        .expect("scheduling metadata must be populated");
    assert_eq!(sched.scheduler_name, "AsapScheduler");
    assert!(sched.makespan_ns > 0.0);
    assert_eq!(
        sched.total_scheduled_operations,
        manifest.metrics.total_operations
    );
}

#[test]
fn test_manifest_deterministic_across_repeated_compilations() {
    let mut builder = CircuitBuilder::new("determinism_test");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[3]).unwrap();
    builder.swap(q[1], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_native_target("linear_4q", 4, false, false);
    let pipeline = CompilerPipeline::default();

    let res1 = pipeline.compile(&circuit, &target).unwrap();
    let res2 = pipeline.compile(&circuit, &target).unwrap();

    let qasm1 = OpenQasmEmitter::new().emit(res1.circuit()).unwrap();
    let qasm2 = OpenQasmEmitter::new().emit(res2.circuit()).unwrap();
    assert_eq!(qasm1, qasm2);

    let ctx1 = ManifestBuildContext {
        input_kind: ManifestInputKind::File,
        input_path: Some("det.qasm".to_string()),
        cli_target_spec: Some("linear".to_string()),
        format: ManifestOutputFormat::OpenQasm,
        output_destination: ManifestOutputDestination::File,
        output_path: Some("det_out.qasm".to_string()),
        emitted_text: &qasm1,
        global_phase_policy: GlobalPhasePolicy::EmitGPhase,
        measurement_syntax: MeasurementSyntax::Assignment,
        parameter_bindings: BTreeMap::new(),
    };
    let ctx2 = ManifestBuildContext {
        emitted_text: &qasm2,
        ..ctx1.clone()
    };

    let m1 = CompilationManifest::from_compilation_result(&res1, &ctx1);
    let m2 = CompilationManifest::from_compilation_result(&res2, &ctx2);

    assert_eq!(m1.to_json_pretty().unwrap(), m2.to_json_pretty().unwrap());
    assert_eq!(m1.metrics.logical_swaps, 1);
    assert_eq!(m1.metrics.final_emitted_swaps, 0);
}

#[test]
fn test_manifest_frontend_only_symbolic_global_phase() {
    let mut builder = CircuitBuilder::new("symbolic_phase_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.set_global_phase(ParameterExpr::symbol("theta"));
    let circuit = builder.finish().unwrap();

    let qasm_text = OpenQasmEmitter::new().emit(&circuit).unwrap();
    let ctx = ManifestBuildContext {
        input_kind: ManifestInputKind::File,
        input_path: Some("sym.qasm".to_string()),
        cli_target_spec: None,
        format: ManifestOutputFormat::OpenQasm,
        output_destination: ManifestOutputDestination::Stdout,
        output_path: None,
        emitted_text: &qasm_text,
        global_phase_policy: GlobalPhasePolicy::EmitGPhase,
        measurement_syntax: MeasurementSyntax::Assignment,
        parameter_bindings: BTreeMap::new(),
    };

    let manifest = CompilationManifest::from_frontend_circuit(&circuit, &ctx);
    assert_eq!(
        manifest.compilation_mode,
        ManifestCompilationMode::FrontendOnly
    );
    assert_eq!(manifest.target, None);
    assert_eq!(manifest.layout, None);
    assert_eq!(manifest.physical_qubits, None);
    assert_eq!(manifest.input_global_phase.radians, None);
    assert!(!manifest.input_global_phase.is_numeric);
    assert_eq!(manifest.input_global_phase.expression, "theta");
    assert_eq!(manifest.global_phase.radians, None);
    assert!(!manifest.global_phase.is_numeric);
    assert_eq!(manifest.global_phase.expression, "theta");
    assert!(manifest.global_phase.emitted_in_openqasm);
}
