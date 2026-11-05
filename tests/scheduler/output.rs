//! Comprehensive integration tests for Phase 58: Schedule Serialization,
//! Output Representation, Unknown Timing Preservation, and Timeline Visualization Data.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::node::OperationId;
use qcc::ir::wire::ClassicalBitId;
use qcc::scheduler::alap::AlapScheduler;
use qcc::scheduler::asap::AsapScheduler;
use qcc::scheduler::output::{
    OperationTiming, ScheduleOutput, ScheduleSummary, ScheduledOperationOutput, TimingKind,
};
use qcc::scheduler::resource_constraints::{
    ResourceConstrainedScheduler, ResourceConstraintManager,
};
use qcc::scheduler::timeline::{DependencyLinkKind, TimelineData, TimelineLaneKind};
use qcc::scheduler::validator::ScheduleValidator;
use qcc::scheduler::ScheduleError;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::resources::{ResourceId, ResourceModel, ResourceRequirement};
use qcc::target::timing::{DurationUnit, OperationDuration, TimingModel};
use qcc::target::OperationConnectivity;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper building a standard 6-qubit test target with calibrated durations and base resource model.
fn make_test_target_6q() -> Target {
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0); // 1 GHz clock (1 cycle = 1 ns)
    timing.set_default_duration("x", OperationDuration::known_ns(30.0));
    timing.set_default_duration("h", OperationDuration::known_ns(30.0));
    timing.set_default_duration("sx", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0)); // virtual Z
    timing.set_default_duration("cx", OperationDuration::known_ns(100.0));
    timing.set_default_duration("measure", OperationDuration::known_ns(500.0));
    timing.set_default_duration("reset", OperationDuration::known_ns(300.0));

    let mut resources = ResourceModel::new();
    resources.set_default_requirements("x", vec![]);
    resources.set_default_requirements("h", vec![]);
    resources.set_default_requirements("sx", vec![]);
    resources.set_default_requirements("rz", vec![]);
    resources.set_default_requirements("cx", vec![]);
    resources.set_default_requirements("measure", vec![]);
    resources.set_default_requirements("reset", vec![]);

    let x_desc = TargetOperationBuilder::new("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(30.0)
        .build();

    let h_desc = TargetOperationBuilder::new("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(30.0)
        .build();

    let sx_desc = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(30.0)
        .build();

    let rz_desc = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(0.0)
        .build();

    let cx_desc = TargetOperationBuilder::new("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit([
            (q(0), q(1)),
            (q(1), q(2)),
            (q(2), q(3)),
            (q(3), q(4)),
            (q(4), q(5)),
            (q(0), q(2)),
            (q(1), q(3)),
            (q(2), q(4)),
        ]))
        .duration(100.0)
        .build();

    let meas_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(500.0)
        .build();

    let reset_desc = TargetOperationBuilder::new("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(6))
        .duration(300.0)
        .build();

    TargetBuilder::new("generic_6q", 6)
        .timing(timing)
        .resources(resources)
        .add_operation(x_desc)
        .add_operation(h_desc)
        .add_operation(sx_desc)
        .add_operation(rz_desc)
        .add_operation(cx_desc)
        .add_operation(meas_desc)
        .add_operation(reset_desc)
        .build()
        .expect("target build failed")
}

#[test]
fn test_schedule_output_round_trip_json_fidelity() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("round_trip_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    builder.h(qreg[0]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.cx(qreg[2], qreg[3]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();
    builder.measure(qreg[3], creg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    // 1. Export schedule to ScheduleOutput
    let output = schedule.to_output(Some(&target));
    assert_eq!(output.circuit_name, "round_trip_circuit");
    assert_eq!(output.target_name, "generic_6q");
    assert_eq!(output.operations.len(), 8);
    assert_eq!(output.summary.total_operations, 8);
    assert_eq!(output.summary.scheduled_operations, 8);
    assert_eq!(output.summary.unknown_timing_operations, 0);

    // 2. Serialize to JSON string
    let json_str = output.to_json_pretty().unwrap();
    assert!(json_str.contains("qcc.schedule.v1"));
    assert!(json_str.contains("round_trip_circuit"));

    // 3. Deserialize back from JSON string
    let deserialized = ScheduleOutput::from_json(&json_str).unwrap();
    assert_eq!(output, deserialized);

    // 4. Reconstruct Schedule from deserialized output
    let reconstructed = deserialized.to_schedule().unwrap();
    assert_eq!(reconstructed.circuit_name, schedule.circuit_name);
    assert_eq!(reconstructed.target_name, schedule.target_name);
    assert_eq!(reconstructed.makespan(), schedule.makespan());
    assert_eq!(reconstructed.len(), schedule.len());

    for (orig_op, recon_op) in schedule.operations().iter().zip(reconstructed.operations()) {
        assert_eq!(orig_op.id, recon_op.id);
        assert_eq!(orig_op.name, recon_op.name);
        assert_eq!(orig_op.operands, recon_op.operands);
        assert_eq!(orig_op.start_time, recon_op.start_time);
        assert_eq!(orig_op.duration, recon_op.duration);
        assert_eq!(orig_op.end_time, recon_op.end_time);
        assert_eq!(orig_op.resources, recon_op.resources);
        assert_eq!(orig_op.classical_reads, recon_op.classical_reads);
        assert_eq!(orig_op.classical_writes, recon_op.classical_writes);
    }
}

#[test]
fn test_exit_gate_reconstructed_schedule_passes_independent_validator() {
    // MANDATORY ROADMAP EXIT GATE: Schedule output round-trips and matches validator.
    let mut target = make_test_target_6q();

    // Add shared drive line and feedline constraints
    let drive0 = ResourceId::DriveLine(0);
    target.resources.as_mut().unwrap().set_tuple_requirements(
        "x",
        [q(0)],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(0))),
            ResourceRequirement::exclusive(drive0.clone()),
        ],
    );
    target.resources.as_mut().unwrap().set_tuple_requirements(
        "x",
        [q(2)],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(2))),
            ResourceRequirement::exclusive(drive0.clone()),
        ],
    );

    let feedline = ResourceId::ReadoutLine(1);
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(feedline.clone(), 2);
    for idx in 0..4 {
        target
            .resources
            .as_mut()
            .unwrap()
            .map_qubit_readout_line(q(idx), 1);
    }

    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    let mut builder = CircuitBuilder::new("exit_gate_schedule");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();
    builder.measure(qreg[3], creg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let sched_res = scheduler.schedule(&circuit, &target).unwrap();

    // Export to JSON string and deserialize
    let output = sched_res.schedule.to_output(Some(&target));
    let json = output.to_json_pretty().unwrap();
    let round_tripped_output = ScheduleOutput::from_json(&json).unwrap();

    // Reconstruct Schedule
    let reconstructed_schedule = round_tripped_output.to_schedule().unwrap();

    // Validate using independent ScheduleValidator
    let validator = ScheduleValidator::new();
    let val_report = validator.validate(&reconstructed_schedule, &circuit, &target);
    assert!(
        val_report.is_valid(),
        "independent validator failed on reconstructed schedule: {:?}",
        val_report.violations
    );
}

#[test]
fn test_deterministic_ordering_and_reproducible_output() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("det_order");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    // 4 independent gates at t = 0 on disjoint qubits
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.h(qreg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let output1 = schedule.to_output(Some(&target));
    let output2 = schedule.to_output(Some(&target));

    let json1 = output1.to_json().unwrap();
    let json2 = output2.to_json().unwrap();

    assert_eq!(
        json1, json2,
        "output JSON must be 100% bit-for-bit deterministic"
    );

    // Check operation order is sorted by (start_time, end_time, sequence_number, id)
    for w in output1.operations.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        assert!(a.start_time.unwrap() <= b.start_time.unwrap());
        if a.start_time == b.start_time {
            assert!(a.sequence_number <= b.sequence_number);
        }
    }
}

#[test]
fn test_explicit_unknown_timing_preservation() {
    // Non-negotiable invariant: Unknown duration must NEVER be defaulted to 0.0 or fabricated number!
    let op_out = ScheduledOperationOutput {
        id: OperationId(42),
        sequence_number: 1,
        name: "uncalibrated_gate".to_string(),
        operands: vec![q(3)],
        classical_reads: vec![],
        classical_writes: vec![],
        start_time: Some(0.0),
        timing: OperationTiming::Unknown,
        end_time: None,
        resources: vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(3)))],
        predecessors: vec![],
        slack: None,
    };

    assert!(op_out.timing.is_unknown());
    assert_eq!(op_out.timing.duration_ns(), None); // Never Some(0.0)!
    assert_eq!(op_out.timing.kind(), TimingKind::Unknown);

    // Serialization preserves Unknown explicitly
    let json = serde_json::to_string(&op_out).unwrap();
    assert!(json.contains("\"timing\":\"Unknown\""));
    assert!(!json.contains("duration_ns"));

    // Deserialization round-trip
    let deserialized: ScheduledOperationOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(op_out, deserialized);
    assert!(deserialized.timing.is_unknown());
    assert_eq!(deserialized.timing.duration_ns(), None);

    // Attempting to reconstruct a physical ScheduledOperation with unknown duration fails fail-closed
    let res = deserialized.to_scheduled();
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ScheduleError::UnknownDuration {
            node_id,
            ref op_name,
            ref operands
        } if node_id == OperationId(42) && op_name == "uncalibrated_gate" && operands == &[q(3)]
    ));
}

#[test]
fn test_estimated_duration_preservation() {
    let timing = OperationTiming::Estimated {
        duration_ns: 45.0,
        unit: DurationUnit::Nanoseconds,
        original_value: 45.0,
    };

    assert!(timing.is_estimated());
    assert!(!timing.is_known());
    assert!(!timing.is_unknown());
    assert_eq!(timing.duration_ns(), Some(45.0));

    let json = serde_json::to_string(&timing).unwrap();
    assert!(json.contains("Estimated"));
    let deserialized: OperationTiming = serde_json::from_str(&json).unwrap();
    assert_eq!(timing, deserialized);
}

#[test]
fn test_physical_operand_identity_preservation() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("operand_circuit");
    let qreg = builder.add_quantum_register("q", 6).unwrap();
    // Operations on specific physical qubits: q0, q3, q5
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[3]).unwrap();
    builder.x(qreg[5]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let output = schedule.to_output(Some(&target));
    assert_eq!(output.summary.physical_qubits_used, vec![q(0), q(3), q(5)]);

    let json = output.to_json().unwrap();
    let des = ScheduleOutput::from_json(&json).unwrap();

    assert_eq!(des.summary.physical_qubits_used, vec![q(0), q(3), q(5)]);
    assert_eq!(des.operations[0].operands, vec![q(0)]);
    assert_eq!(des.operations[1].operands, vec![q(3)]);
    assert_eq!(des.operations[2].operands, vec![q(5)]);
}

#[test]
fn test_resource_information_preservation() {
    let op = ScheduledOperationOutput {
        id: OperationId(7),
        sequence_number: 2,
        name: "cx".to_string(),
        operands: vec![q(1), q(2)],
        classical_reads: vec![],
        classical_writes: vec![],
        start_time: Some(10.0),
        timing: OperationTiming::known_ns(100.0),
        end_time: Some(110.0),
        resources: vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::exclusive(ResourceId::Qubit(q(2))),
            ResourceRequirement::exclusive(ResourceId::coupler(q(1), q(2))),
            ResourceRequirement::shared(ResourceId::DriveLine(3), 1),
            ResourceRequirement::shared(ResourceId::Custom("cryo_channel".to_string()), 2),
        ],
        predecessors: vec![OperationId(1)],
        slack: Some(0.0),
    };

    let json = serde_json::to_string_pretty(&op).unwrap();
    let des: ScheduledOperationOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(op, des);
    assert_eq!(des.resources.len(), 5);
    assert_eq!(des.resources[2].resource, ResourceId::coupler(q(1), q(2)));
    assert_eq!(des.resources[3].resource, ResourceId::DriveLine(3));
}

#[test]
fn test_timeline_data_generation_and_consistency() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("timeline_circuit");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    let creg = builder.add_classical_register("c", 3).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.rz(qreg[2], 0.0).unwrap(); // virtual Z (0 ns)
    builder.measure(qreg[0], creg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let timeline = schedule.generate_timeline();
    assert_eq!(timeline.circuit_name, "timeline_circuit");
    assert_eq!(timeline.target_name, "generic_6q");
    assert_eq!(timeline.makespan_ns, 630.0); // X(30) + CX(100) + Measure(500) = 630ns

    // Lanes check: Qubit 0, Qubit 1, Qubit 2, Coupler (0, 1)
    let q0_lane = timeline.lane(&TimelineLaneKind::Qubit(q(0))).unwrap();
    assert_eq!(q0_lane.blocks.len(), 3); // X, CX, Measure
    assert_eq!(q0_lane.blocks[0].op_name, "x");
    assert_eq!(q0_lane.blocks[0].start_ns, 0.0);
    assert_eq!(q0_lane.blocks[0].end_ns, 30.0);

    let coupler_lane = timeline
        .lane(&TimelineLaneKind::coupler(q(0), q(1)))
        .unwrap();
    assert_eq!(coupler_lane.blocks.len(), 1);
    assert_eq!(coupler_lane.blocks[0].op_name, "cx");
    assert_eq!(coupler_lane.blocks[0].start_ns, 30.0);
    assert_eq!(coupler_lane.blocks[0].end_ns, 130.0);

    // Qubit 2 lane has virtual Rz (duration 0.0, instantaneous)
    let q2_lane = timeline.lane(&TimelineLaneKind::Qubit(q(2))).unwrap();
    assert_eq!(q2_lane.blocks.len(), 1);
    assert!(q2_lane.blocks[0].is_instantaneous);
    assert_eq!(q2_lane.blocks[0].duration_ns, 0.0);

    // Dependency links
    assert!(!timeline.dependency_links.is_empty());
    let cx_link = timeline
        .dependency_links
        .iter()
        .find(|l| l.link_kind == DependencyLinkKind::DagPredecessor)
        .unwrap();
    assert!(cx_link.source_end_ns <= cx_link.target_start_ns);

    // JSON serialization of TimelineData
    let json = timeline.to_json().unwrap();
    let des = TimelineData::from_json(&json).unwrap();
    assert_eq!(timeline, des);
}

#[test]
fn test_timeline_ascii_visualization_summary() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("ascii_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let timeline = schedule.generate_timeline();
    let ascii = timeline.to_ascii_timeline();

    assert!(ascii.contains("Timeline: ascii_circuit on generic_6q"));
    assert!(ascii.contains("Qubit 0"));
    assert!(ascii.contains("Qubit 1"));
    assert!(ascii.contains("Coupler (0, 1)"));
    assert!(ascii.contains("Makespan: 130.00 ns"));
}

#[test]
fn test_invalid_or_incomplete_serialized_data_rejected() {
    // 1. Missing start_time in ScheduledOperationOutput
    let incomplete_op = ScheduledOperationOutput {
        id: OperationId(1),
        sequence_number: 0,
        name: "x".to_string(),
        operands: vec![q(0)],
        classical_reads: vec![],
        classical_writes: vec![],
        start_time: None,
        timing: OperationTiming::known_ns(30.0),
        end_time: None,
        resources: vec![],
        predecessors: vec![],
        slack: None,
    };
    assert!(matches!(
        incomplete_op.to_scheduled(),
        Err(ScheduleError::IncompleteScheduleData(_))
    ));

    // 2. ScheduleOutput with inconsistent makespan
    let output = ScheduleOutput {
        schema_version: ScheduleOutput::CURRENT_SCHEMA_VERSION.to_string(),
        circuit_name: "test".to_string(),
        target_name: "target".to_string(),
        makespan_ns: Some(999.0), // Inconsistent makespan: actual is 30.0
        requested_makespan_ns: None,
        operations: vec![ScheduledOperationOutput {
            id: OperationId(1),
            sequence_number: 0,
            name: "x".to_string(),
            operands: vec![q(0)],
            classical_reads: vec![],
            classical_writes: vec![],
            start_time: Some(0.0),
            timing: OperationTiming::known_ns(30.0),
            end_time: Some(30.0),
            resources: vec![],
            predecessors: vec![],
            slack: None,
        }],
        resources_used: vec![],
        summary: ScheduleSummary {
            total_operations: 1,
            scheduled_operations: 1,
            unknown_timing_operations: 0,
            physical_qubits_used: vec![q(0)],
            critical_path_operations: vec![],
            peak_concurrency: 1,
        },
    };

    assert!(matches!(
        output.to_schedule(),
        Err(ScheduleError::IncompleteScheduleData(_))
    ));

    // 3. Malformed JSON
    assert!(ScheduleOutput::from_json("{ not valid json }").is_err());
}

#[test]
fn test_timeline_blocks_at_timestamp_queries() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("query_circuit");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    builder.x(qreg[0]).unwrap(); // [0, 30)
    builder.h(qreg[1]).unwrap(); // [0, 30)
    builder.cx(qreg[0], qreg[1]).unwrap(); // [30, 130)
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let timeline = schedule.generate_timeline();

    // At t = 15.0: X(q0) and H(q1) active
    let active_15 = timeline.blocks_at(15.0);
    assert_eq!(active_15.len(), 2);

    // At t = 50.0: CX(q0, q1) active
    let active_50 = timeline.blocks_at(50.0);
    assert_eq!(active_50.len(), 1);
    assert_eq!(active_50[0].op_name, "cx");

    // At t = 130.0: circuit complete, 0 active
    let active_130 = timeline.blocks_at(130.0);
    assert_eq!(active_130.len(), 0);
}

#[test]
fn test_alap_slack_preservation_in_export() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("alap_slack_circuit");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    // Path 1 (long): X(0) -> CX(0, 1) -> X(1) -> total 30 + 100 + 30 = 160ns
    builder.x(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.x(qreg[1]).unwrap();
    // Path 2 (short, independent): X(2) -> total 30ns
    builder.x(qreg[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AlapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let output = schedule.to_output(Some(&target));
    assert_eq!(output.makespan_ns, Some(160.0));

    // X on q2 should have positive slack (160 - 30 = 130ns)
    let op_q2 = output
        .operations
        .iter()
        .find(|o| o.operands == vec![q(2)])
        .unwrap();
    assert_eq!(op_q2.slack, Some(130.0));

    // Operations on path 1 are critical path (slack 0.0)
    let op_cx = output.operations.iter().find(|o| o.name == "cx").unwrap();
    assert_eq!(op_cx.slack, Some(0.0));
    assert!(output.summary.critical_path_operations.contains(&op_cx.id));

    // Round-trip preserves slacks
    let json = output.to_json().unwrap();
    let des = ScheduleOutput::from_json(&json).unwrap();
    assert_eq!(
        des.operations
            .iter()
            .find(|o| o.operands == vec![q(2)])
            .unwrap()
            .slack,
        Some(130.0)
    );
}

#[test]
fn test_classical_wire_hazards_reads_writes_preservation() {
    let target = make_test_target_6q();

    let mut builder = CircuitBuilder::new("classical_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let output = schedule.to_output(Some(&target));
    let meas_op = output
        .operations
        .iter()
        .find(|o| o.name == "measure" && o.operands == vec![q(0)])
        .unwrap();
    assert_eq!(meas_op.classical_writes, vec![ClassicalBitId(0)]);

    // Round-trip preserves classical writes
    let json = output.to_json().unwrap();
    let des = ScheduleOutput::from_json(&json).unwrap();
    let des_meas = des
        .operations
        .iter()
        .find(|o| o.name == "measure" && o.operands == vec![q(0)])
        .unwrap();
    assert_eq!(des_meas.classical_writes, vec![ClassicalBitId(0)]);
}
