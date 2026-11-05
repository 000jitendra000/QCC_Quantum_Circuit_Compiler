//! Comprehensive end-to-end integration tests for the QCC compilation pipeline (Phase 59).

use qcc::builder::CircuitBuilder;
use qcc::compiler::{
    CompilerConfig, CompilerError, CompilerPipeline, RouterChoice, SchedulerChoice,
};
use qcc::gate_signature::ParameterDomain;
use qcc::ir::operation::Operation;
use qcc::ir::wire::QubitId;
use qcc::routing::greedy::BaselineRouterConfig;
use qcc::routing::pathfinding::RoutingError;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::resources::ResourceModel;
use qcc::target::timing::{OperationDuration, TimingModel};
use qcc::target::OperationConnectivity;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper constructing a standard linear coupling target (0 - 1 - ... - n-1).
fn make_linear_target(name: &str, num_qubits: usize) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let z_desc = TargetOperationBuilder::new("z", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let measure_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(500.0)
        .build();

    builder = builder
        .add_operation(h_desc)
        .add_operation(x_desc)
        .add_operation(z_desc)
        .add_operation(rz_desc)
        .add_operation(sx_desc)
        .add_operation(measure_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((q(i as u32), q((i + 1) as u32)));
    }

    if !edges.is_empty() {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx_desc);

        let swap_desc = TargetOperationBuilder::new("swap", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(300.0)
            .build();
        builder = builder.add_operation(swap_desc);
    }

    builder.build().unwrap()
}

/// Helper constructing a ring coupling target (0 - 1 - ... - n-1 - 0).
fn make_ring_target(name: &str, num_qubits: usize) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();

    builder = builder
        .add_operation(h_desc)
        .add_operation(x_desc)
        .add_operation(rz_desc)
        .add_operation(sx_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits {
        edges.push((q(i as u32), q(((i + 1) % num_qubits) as u32)));
    }

    let cx_desc = TargetOperationBuilder::new("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edges))
        .duration(150.0)
        .build();
    builder = builder.add_operation(cx_desc);

    builder.build().unwrap()
}

/// Helper constructing a 2-qubit target where CX is strictly directed from 1 to 0 (1 -> 0 only, NOT 0 -> 1).
fn make_directed_target_2q() -> Target {
    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(30.0)
        .build();
    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(30.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(0.0)
        .build();
    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(30.0)
        .build();

    // CX directed strictly 1 -> 0
    let cx_desc = TargetOperationBuilder::new("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([(q(1), q(0))]))
        .duration(150.0)
        .build();

    TargetBuilder::new("directed_1_to_0_target", 2)
        .add_operation(h_desc)
        .add_operation(x_desc)
        .add_operation(rz_desc)
        .add_operation(sx_desc)
        .add_operation(cx_desc)
        .build()
        .unwrap()
}

/// Helper constructing a disconnected target with isolated qubits.
fn make_disconnected_target(name: &str, num_qubits: usize) -> Target {
    let mut builder = TargetBuilder::new(name, num_qubits);

    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(30.0)
        .build();
    builder = builder.add_operation(h_desc).add_operation(x_desc);

    // Only edge is between (0, 1); any qubits >= 2 are completely disconnected
    if num_qubits >= 2 {
        let cx_desc = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit([(q(0), q(1))]))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx_desc);
    }

    builder.build().unwrap()
}

/// Helper constructing a standard 4-qubit test target with calibrated durations and resource model.
fn make_timing_aware_target_4q() -> Target {
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("x", OperationDuration::known_ns(30.0));
    timing.set_default_duration("h", OperationDuration::known_ns(30.0));
    timing.set_default_duration("sx", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0));
    timing.set_default_duration("cx", OperationDuration::known_ns(150.0));
    timing.set_default_duration("swap", OperationDuration::known_ns(300.0));
    timing.set_default_duration("measure", OperationDuration::known_ns(500.0));

    let mut resources = ResourceModel::new();
    resources.set_default_requirements("x", vec![]);
    resources.set_default_requirements("h", vec![]);
    resources.set_default_requirements("sx", vec![]);
    resources.set_default_requirements("rz", vec![]);
    resources.set_default_requirements("cx", vec![]);
    resources.set_default_requirements("swap", vec![]);
    resources.set_default_requirements("measure", vec![]);

    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .build();
    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .build();
    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(30.0)
        .build();
    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0)
        .build();

    let edges = vec![(q(0), q(1)), (q(1), q(2)), (q(2), q(3))];
    let cx_desc = TargetOperationBuilder::new("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
        .duration(150.0)
        .build();
    let swap_desc = TargetOperationBuilder::new("swap", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edges))
        .duration(300.0)
        .build();
    let meas_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(500.0)
        .build();

    TargetBuilder::new("timing_4q_linear", 4)
        .timing(timing)
        .resources(resources)
        .add_operation(x_desc)
        .add_operation(h_desc)
        .add_operation(sx_desc)
        .add_operation(rz_desc)
        .add_operation(cx_desc)
        .add_operation(swap_desc)
        .add_operation(meas_desc)
        .build()
        .unwrap()
}

// ============================================================================
// 1. Core Roadmap Integration Tests
// ============================================================================

#[test]
fn test_bell_circuit_end_to_end() {
    let mut builder = CircuitBuilder::new("bell_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_2q", 2);
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.is_successful());
    assert!(result.report.legality.is_legal);
    assert!(result.report.verification.is_equivalent);
    assert_eq!(result.physical_circuit.num_qubits(), 2);
    assert!(!result.is_scheduled());

    // Check JSON serialization of report
    let report_json = result.report.to_json().unwrap();
    assert!(report_json.contains("bell_circuit"));
    assert!(report_json.contains("target_2q"));
}

#[test]
fn test_line_topology_routing_and_swaps() {
    // 3-qubit line: 0 - 1 - 2.
    // Triangle interactions CX(0, 1), CX(1, 2), CX(0, 2) cannot be embedded
    // on a line without inserting physical SWAPs.
    let mut builder = CircuitBuilder::new("line_test_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_3q_line", 3);
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.is_successful());
    assert!(result.report.routing.swaps_inserted > 0);
    assert!(result.report.legality.is_legal);
    assert!(result.report.verification.is_equivalent);

    // Initial and final layouts must be recorded
    assert_eq!(result.layout.initial().num_logical(), 3);
    assert_eq!(result.layout.final_layout().num_physical(), 3);
}

#[test]
fn test_ring_topology_routing_shortcuts() {
    // 4-qubit ring: 0 - 1 - 2 - 3 - 0.
    // Interaction between (0, 3) is a direct coupling edge on the ring,
    // so no SWAPs are required when placed adjacently.
    let mut builder = CircuitBuilder::new("ring_test_circuit");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_ring_target("target_4q_ring", 4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.is_successful());
    assert_eq!(result.report.routing.swaps_inserted, 0);
    assert!(result.report.legality.is_legal);
    assert!(result.report.verification.is_equivalent);
}

#[test]
fn test_directed_target_topology_with_hadamard_conjugation() {
    // Target supports CX strictly 1 -> 0, but input requires CX(0, 1).
    // With initial layout q0 -> p0, q1 -> p1, CX(0, 1) maps to physical (p0, p1).
    // Pipeline must apply post-routing direction correction via Hadamard conjugation:
    // CX(0, 1) = (H_0 (x) H_1) . CX(1, 0) . (H_0 (x) H_1).
    let mut builder = CircuitBuilder::new("directed_input_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.x(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_directed_target_2q();
    let initial_layout = qcc::routing::layout_map::LayoutMap::from_slice(
        2,
        &[PhysicalQubitId(0), PhysicalQubitId(1)],
    )
    .unwrap();
    let config = CompilerConfig::default()
        .with_initial_layout(initial_layout)
        .with_router(RouterChoice::Baseline(BaselineRouterConfig::default()));
    let pipeline = CompilerPipeline::new(config);

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.is_successful());
    assert!(
        result
            .report
            .post_routing_translation
            .as_ref()
            .unwrap()
            .directions_corrected
            > 0
    );
    assert!(result.report.legality.is_legal);
    assert!(result.report.verification.is_equivalent);

    // Verify all emitted CX gates in physical circuit have control=p1, target=p0
    for node in result.physical_circuit.graph.nodes() {
        if let Operation::Gate { gate_name, .. } = &node.operation {
            if gate_name == "cx" {
                let qubits = node.operation.qubits();
                assert_eq!(qubits[0], QubitId(1)); // control is p1
                assert_eq!(qubits[1], QubitId(0)); // target is p0
            }
        }
    }
}

#[test]
fn test_impossible_topologies_fail_fast() {
    // 1. Target with insufficient capacity (3 logical qubits on 2-qubit target)
    let mut builder = CircuitBuilder::new("too_large_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let target_2q = make_linear_target("target_2q", 2);
    let pipeline = CompilerPipeline::default();

    let err = pipeline.compile(&circuit, &target_2q).unwrap_err();
    assert!(matches!(
        err,
        CompilerError::Routing(RoutingError::InsufficientCapacity {
            required: 3,
            capacity: 2
        })
    ));

    // 2. Disconnected target where interacting qubits cannot communicate
    let mut builder = CircuitBuilder::new("disconnected_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap(); // Requires 2 edges, but target only has 1 edge (p0, p1)
    let circuit = builder.finish().unwrap();

    let target_disconn = make_disconnected_target("target_disconn", 3);
    let err = pipeline.compile(&circuit, &target_disconn).unwrap_err();
    assert!(matches!(
        err,
        CompilerError::Routing(RoutingError::DisconnectedTopology { .. })
    ));
}

#[test]
fn test_timing_aware_target_with_post_routing_scheduling() {
    let mut builder = CircuitBuilder::new("timing_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_timing_aware_target_4q();

    // 1. Test ASAP post-routing scheduling
    let config_asap = CompilerConfig::default().with_asap_scheduling();
    let pipeline_asap = CompilerPipeline::new(config_asap);
    let result_asap = pipeline_asap.compile(&circuit, &target).unwrap();

    assert!(result_asap.is_scheduled());
    assert!(result_asap.schedule.is_some());
    assert!(result_asap.schedule_output.is_some());
    assert!(result_asap.timeline.is_some());
    let sched_rep_asap = result_asap.report.scheduling.as_ref().unwrap();
    assert_eq!(sched_rep_asap.scheduler_name, "AsapScheduler");
    assert!(sched_rep_asap.makespan_ns > 0.0);

    // 2. Test ALAP post-routing scheduling
    let config_alap = CompilerConfig::default().with_alap_scheduling();
    let pipeline_alap = CompilerPipeline::new(config_alap);
    let result_alap = pipeline_alap.compile(&circuit, &target).unwrap();

    assert!(result_alap.is_scheduled());
    let sched_rep_alap = result_alap.report.scheduling.as_ref().unwrap();
    assert_eq!(sched_rep_alap.scheduler_name, "AlapScheduler");
    assert!(sched_rep_alap.makespan_ns > 0.0);

    // 3. Test Resource-Constrained post-routing scheduling
    let config_res = CompilerConfig::default().with_resource_constrained_scheduling();
    let pipeline_res = CompilerPipeline::new(config_res);
    let result_res = pipeline_res.compile(&circuit, &target).unwrap();

    assert!(result_res.is_scheduled());
    let sched_rep_res = result_res.report.scheduling.as_ref().unwrap();
    assert_eq!(sched_rep_res.scheduler_name, "ResourceConstrainedScheduler");
    assert!(sched_rep_res.makespan_ns > 0.0);
}

// ============================================================================
// 2. Architectural Invariant & Contract Tests
// ============================================================================

#[test]
fn test_deterministic_compilation_under_identical_inputs() {
    let mut builder = CircuitBuilder::new("det_circuit");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[1], q[3]).unwrap();
    builder.cx(q[2], q[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_4q_line", 4);
    let pipeline = CompilerPipeline::default();

    let result1 = pipeline.compile(&circuit, &target).unwrap();
    let result2 = pipeline.compile(&circuit, &target).unwrap();

    // Outputs must be bit-for-bit identical
    assert_eq!(
        result1.physical_circuit.num_operations(),
        result2.physical_circuit.num_operations()
    );
    assert_eq!(result1.layout, result2.layout);
    assert_eq!(
        result1.report.routing.swaps_inserted,
        result2.report.routing.swaps_inserted
    );
    assert_eq!(
        result1.report.metrics.final_gate_count,
        result2.report.metrics.final_gate_count
    );
}

#[test]
fn test_scheduling_disabled_vs_enabled() {
    let mut builder = CircuitBuilder::new("sched_toggle_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_timing_aware_target_4q();

    // Disabled
    let config_disabled = CompilerConfig::default();
    assert_eq!(config_disabled.scheduler, SchedulerChoice::Disabled);
    let res_disabled = CompilerPipeline::new(config_disabled)
        .compile(&circuit, &target)
        .unwrap();
    assert!(!res_disabled.is_scheduled());
    assert!(res_disabled.schedule.is_none());
    assert!(res_disabled.schedule_output.is_none());
    assert!(res_disabled.timeline.is_none());
    assert!(res_disabled.report.scheduling.is_none());

    // Enabled
    let config_enabled = CompilerConfig::default().with_asap_scheduling();
    let res_enabled = CompilerPipeline::new(config_enabled)
        .compile(&circuit, &target)
        .unwrap();
    assert!(res_enabled.is_scheduled());
    assert!(res_enabled.schedule.is_some());
    assert!(res_enabled.schedule_output.is_some());
    assert!(res_enabled.timeline.is_some());
    assert!(res_enabled.report.scheduling.is_some());
}

#[test]
fn test_final_legality_validation_after_direction_correction() {
    // Input requires CX(0, 1) on directed 1 -> 0 target.
    // Post-routing direction correction inserts Hadamards and reverses CX to (1, 0).
    // Final target legality validation must confirm all gates (including inserted Hadamards) are legal.
    let mut builder = CircuitBuilder::new("legality_dir_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_directed_target_2q();
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.legality.is_legal);
    assert_eq!(result.report.legality.violation_count, 0);
}

#[test]
fn test_layout_aware_verification_with_nontrivial_layout_permutation() {
    // 3-qubit line: 0 - 1 - 2.
    // CX(0, 2) causes a SWAP between (0, 1) or (1, 2), altering final layout.
    let mut builder = CircuitBuilder::new("perm_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_3q_line", 3);
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    assert!(result.report.is_successful());
    assert!(result.report.verification.is_equivalent);
    // Nontrivial layout permutation verified
    assert_ne!(result.layout.initial(), result.layout.final_layout());
}

#[test]
fn test_adr0001_strict_ancilla_zero_state_enforcement() {
    // 2 logical qubits placed on a 4-qubit target: physical qubits 2 and 3 are unused ancillae.
    // Under ADR-0001, unused ancillae start in |0> and MUST return to |0>.
    let mut builder = CircuitBuilder::new("ancilla_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_4q", 4);
    let pipeline = CompilerPipeline::default();

    let result = pipeline.compile(&circuit, &target).unwrap();

    // Standard compilation succeeds and verifies on the full 16-dimensional physical statevector
    assert!(result.report.is_successful());
    assert!(result.report.verification.is_equivalent);

    // Tamper test: If the physical circuit flips an unused ancilla to |1>, verification MUST fail!
    let mut tampered_circuit = result.physical_circuit.clone();
    tampered_circuit
        .add_operation(Operation::custom_gate("x", vec![QubitId(3)], vec![]))
        .unwrap();

    let checker = qcc::LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &tampered_circuit, &result.layout)
        .unwrap();

    assert!(
        !verdict.is_equivalent,
        "ADR-0001 invariant violation: flipping unused ancilla must fail equivalence check!"
    );
}

#[test]
fn test_failure_propagation_with_useful_structured_diagnostics() {
    // 1. Unsupported gate without decomposition rule
    let mut builder = CircuitBuilder::new("unsupported_gate_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder
        .custom_gate("nonexistent_exotic_gate", &[q[0]], vec![])
        .unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_2q", 2);
    let pipeline = CompilerPipeline::default();

    let err = pipeline.compile(&circuit, &target).unwrap_err();
    assert!(matches!(err, CompilerError::BasisTranslation(_)));

    // 2. Baseline router with impossible placement
    let mut builder = CircuitBuilder::new("baseline_impossible_circuit");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target_disconn = make_disconnected_target("target_disconn", 3);
    let config = CompilerConfig::default()
        .with_router(RouterChoice::Baseline(BaselineRouterConfig::default()));
    let err = CompilerPipeline::new(config)
        .compile(&circuit, &target_disconn)
        .unwrap_err();
    assert!(matches!(err, CompilerError::Routing(_)));
}

#[test]
fn test_scheduled_artifact_matches_final_routed_circuit() {
    let mut builder = CircuitBuilder::new("match_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_timing_aware_target_4q();
    let config = CompilerConfig::default().with_asap_scheduling();
    let result = CompilerPipeline::new(config)
        .compile(&circuit, &target)
        .unwrap();

    assert!(result.is_scheduled());
    let schedule = result.schedule.as_ref().unwrap();

    // Verify scheduled operations count matches physical circuit operations count
    assert_eq!(
        schedule.operations.len(),
        result.physical_circuit.num_operations()
    );

    // Verify each scheduled op corresponds to a physical node
    let phys_ids: Vec<u64> = result
        .physical_circuit
        .graph
        .nodes()
        .map(|n| n.id.0)
        .collect();
    for scheduled_op in &schedule.operations {
        assert!(
            phys_ids.contains(&scheduled_op.id.0),
            "Scheduled operation ID {} must exist in physical circuit",
            scheduled_op.id.0
        );
    }
}

#[test]
fn test_different_optimization_levels() {
    let mut builder = CircuitBuilder::new("opt_level_circuit");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // Identity pair: X followed by X cancels out under O1/O2
    builder.x(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.h(q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_linear_target("target_2q", 2);

    // O0: Optimization disabled, X gates not cancelled
    let config_o0 = CompilerConfig::o0();
    let res_o0 = CompilerPipeline::new(config_o0)
        .compile(&circuit, &target)
        .unwrap();
    assert!(res_o0.report.metrics.optimized_gate_count >= 3);

    // O2: Standard optimization, X pair cancelled
    let config_o2 = CompilerConfig::o2();
    let res_o2 = CompilerPipeline::new(config_o2)
        .compile(&circuit, &target)
        .unwrap();
    assert!(
        res_o2.report.metrics.optimized_gate_count < res_o0.report.metrics.optimized_gate_count
    );
}
