//! Comprehensive integration tests for Phase 55: ASAP Scheduler & Independent Validator.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::scheduler::asap::AsapScheduler;
use qcc::scheduler::schedule::Schedule;
use qcc::scheduler::validator::{ScheduleValidationError, ScheduleValidator};
use qcc::scheduler::ScheduleError;
use qcc::target::calibration::Provenance;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOpId, TargetOperationBuilder};
use qcc::target::resources::{MutualExclusionRule, ResourceId, ResourceModel, ResourceRequirement};
use qcc::target::timing::{OperationDuration, TimingModel};
use qcc::target::OperationConnectivity;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper building a standard 4-qubit test target with calibrated durations and full resource model.
fn make_test_target_4q() -> Target {
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0); // 1 GHz clock (1 cycle = 1 ns)
    timing.set_default_duration("x", OperationDuration::known_ns(30.0));
    timing.set_default_duration("h", OperationDuration::known_ns(30.0));
    timing.set_default_duration("sx", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0)); // virtual Z
    timing.set_default_duration("cx", OperationDuration::known_ns(200.0));
    timing.set_default_duration("measure", OperationDuration::known_ns(1000.0));
    timing.set_default_duration("reset", OperationDuration::known_ns(500.0));

    let mut resources = ResourceModel::new();
    // Default requirements: exclusive qubit locks (and coupler for 2Q)
    resources.set_default_requirements("x", vec![]);
    resources.set_default_requirements("h", vec![]);
    resources.set_default_requirements("sx", vec![]);
    resources.set_default_requirements("rz", vec![]);
    resources.set_default_requirements("cx", vec![]);
    resources.set_default_requirements("measure", vec![]);
    resources.set_default_requirements("reset", vec![]);

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

    let cx_desc = TargetOperationBuilder::new("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit([
            (q(0), q(1)),
            (q(0), q(2)),
            (q(0), q(3)),
            (q(1), q(2)),
            (q(1), q(3)),
            (q(2), q(3)),
        ]))
        .duration(200.0)
        .build();

    let meas_desc = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(1000.0)
        .build();

    let reset_desc = TargetOperationBuilder::new("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(500.0)
        .build();

    TargetBuilder::new("test_target_4q", 4)
        .add_operation(x_desc)
        .add_operation(h_desc)
        .add_operation(sx_desc)
        .add_operation(rz_desc)
        .add_operation(cx_desc)
        .add_operation(meas_desc)
        .add_operation(reset_desc)
        .timing(timing)
        .resources(resources)
        .build()
        .expect("target construction must succeed")
}

#[test]
fn test_sequential_single_qubit_pipeline_schedule() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("seq_pipeline");
    let qreg = builder.add_quantum_register("q", 1).unwrap();

    builder.x(qreg[0]).unwrap();
    builder.h(qreg[0]).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    assert_eq!(schedule.len(), 3);
    assert_eq!(schedule.makespan(), 90.0);

    let ops = schedule.operations();
    assert_eq!(ops[0].name, "x");
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].duration, 30.0);
    assert_eq!(ops[0].end_time, 30.0);

    assert_eq!(ops[1].name, "h");
    assert_eq!(ops[1].start_time, 30.0);
    assert_eq!(ops[1].duration, 30.0);
    assert_eq!(ops[1].end_time, 60.0);

    assert_eq!(ops[2].name, "x");
    assert_eq!(ops[2].start_time, 60.0);
    assert_eq!(ops[2].duration, 30.0);
    assert_eq!(ops[2].end_time, 90.0);

    // Independent validator verification
    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_parallel_independent_disjoint_gates() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("parallel_gates");
    let qreg = builder.add_quantum_register("q", 3).unwrap();

    // 3 independent gates on disjoint qubits q0, q1, q2
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    assert_eq!(schedule.len(), 3);
    // All 3 gates must execute in parallel starting at t = 0.0
    for op in schedule.operations() {
        assert_eq!(op.start_time, 0.0);
        assert_eq!(op.duration, 30.0);
        assert_eq!(op.end_time, 30.0);
    }
    assert_eq!(schedule.makespan(), 30.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_unequal_durations_and_critical_path() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("unequal_durations");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 1).unwrap();

    // X on q0 (30 ns)
    builder.x(qreg[0]).unwrap();
    // CX on (q0, q1) (200 ns) -> depends on X(q0)
    builder.cx(qreg[0], qreg[1]).unwrap();
    // Measure q0 (1000 ns) -> depends on CX
    builder.measure(qreg[0], creg[0]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    assert_eq!(schedule.len(), 3);

    let ops = schedule.operations();
    // Op 0: X on q0 at [0, 30]
    assert_eq!(ops[0].name, "x");
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 30.0);

    // Op 1: CX on (q0, q1) at [30, 230]
    assert_eq!(ops[1].name, "cx");
    assert_eq!(ops[1].start_time, 30.0);
    assert_eq!(ops[1].end_time, 230.0);

    // Op 2: Measure q0 at [230, 1230]
    assert_eq!(ops[2].name, "measure");
    assert_eq!(ops[2].start_time, 230.0);
    assert_eq!(ops[2].end_time, 1230.0);

    assert_eq!(schedule.makespan(), 1230.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_diamond_dag_with_multiple_predecessors() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("diamond_dag");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Branch 1 on q0: X(0) [0..30] -> H(0) [30..60] (total 60 ns)
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[0]).unwrap();

    // Branch 2 on q1: X(1) [0..30] -> X(1) [30..60] -> H(1) [60..90] (total 90 ns)
    builder.x(qreg[1]).unwrap();
    builder.x(qreg[1]).unwrap();
    builder.h(qreg[1]).unwrap();

    // Recombination: CX(0, 1) has predecessors H(0) [ends 60] and H(1) [ends 90]
    builder.cx(qreg[0], qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    assert_eq!(schedule.len(), 6);

    let cx_op = schedule
        .operations()
        .iter()
        .find(|o| o.name == "cx")
        .unwrap();
    // CX must start at max(60.0, 90.0) = 90.0 ns
    assert_eq!(cx_op.start_time, 90.0);
    assert_eq!(cx_op.end_time, 290.0);
    assert_eq!(schedule.makespan(), 290.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_shared_drive_line_conflict_serializes_disjoint_gates() {
    // Target with q0 and q2 sharing DriveLine(0)
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // Set shared drive line for X on q0 and X on q2
    let drive_line_0 = ResourceId::DriveLine(0);
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::exclusive(drive_line_0.clone())],
    );
    resources.set_tuple_requirements(
        "x",
        [q(2)],
        vec![ResourceRequirement::exclusive(drive_line_0)],
    );
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("drive_line_conflict");
    let qreg = builder.add_quantum_register("q", 3).unwrap();

    // Independent gates on q0 and q2
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[2]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    // Despite disjoint qubits, shared DriveLine(0) forces serialization!
    let ops = schedule.operations();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 30.0);

    assert_eq!(ops[1].start_time, 30.0);
    assert_eq!(ops[1].end_time, 60.0);
    assert_eq!(schedule.makespan(), 60.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_multiplexed_readout_line_capacity_constraints() {
    // 3 qubits q0, q1, q2 mapped to ReadoutLine(1) with capacity = 2
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    resources.map_qubit_readout_line(q(0), 1);
    resources.map_qubit_readout_line(q(1), 1);
    resources.map_qubit_readout_line(q(2), 1);
    resources.set_capacity(ResourceId::ReadoutLine(1), 2);
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("readout_capacity");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    let creg = builder.add_classical_register("c", 3).unwrap();

    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    // Capacity is 2: first 2 measurements start at 0.0, 3rd measurement must be delayed to 1000.0
    let ops = schedule.operations();
    assert_eq!(ops.len(), 3);

    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 1000.0);

    assert_eq!(ops[1].start_time, 0.0);
    assert_eq!(ops[1].end_time, 1000.0);

    assert_eq!(ops[2].start_time, 1000.0);
    assert_eq!(ops[2].end_time, 2000.0);
    assert_eq!(schedule.makespan(), 2000.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_mutual_exclusion_rule_cross_talk_avoidance() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // Cross-talk rule: CX(0, 1) and CX(2, 3) cannot run concurrently
    resources.add_mutual_exclusion(MutualExclusionRule::operation_pair(
        "cx_cross_talk",
        "cx",
        Some(vec![q(0), q(1)]),
        "cx",
        Some(vec![q(2), q(3)]),
    ));
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("mutual_exclusion_pair");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.cx(qreg[2], qreg[3]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    let ops = schedule.operations();
    assert_eq!(ops.len(), 2);
    // They must be serialized due to the mutual exclusion rule
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 200.0);

    assert_eq!(ops[1].start_time, 200.0);
    assert_eq!(ops[1].end_time, 400.0);
    assert_eq!(schedule.makespan(), 400.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_global_exclusive_operation_delays_concurrent_gates() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // Reset requires global device exclusivity
    resources.add_mutual_exclusion(MutualExclusionRule::global_exclusive(
        "global_reset",
        "reset",
    ));
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("global_exclusive");
    let qreg = builder.add_quantum_register("q", 3).unwrap();

    // Reset on q0 (500 ns) and X on q1 (30 ns)
    builder.reset(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    let ops = schedule.operations();
    assert_eq!(ops.len(), 2);
    // Global exclusive reset must not overlap with X
    assert_eq!(ops[0].name, "reset");
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 500.0);

    assert_eq!(ops[1].name, "x");
    assert_eq!(ops[1].start_time, 500.0);
    assert_eq!(ops[1].end_time, 530.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_compiler_barrier_synchronization() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("barrier_sync");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Gate 1: X(0) takes 30 ns
    builder.x(qreg[0]).unwrap();
    // Barrier across both q0 and q1
    builder.barrier(&[qreg[0], qreg[1]]).unwrap();
    // Gate 2: X(1) on q1 is ready, but barrier depends on X(0)
    builder.x(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let schedule = scheduler
        .schedule(&circuit, &target)
        .expect("scheduling must succeed");

    let x0 = schedule
        .operations()
        .iter()
        .find(|o| o.name == "x" && o.touches_qubit(q(0)))
        .unwrap();
    let x1 = schedule
        .operations()
        .iter()
        .find(|o| o.name == "x" && o.touches_qubit(q(1)))
        .unwrap();

    assert_eq!(x0.start_time, 0.0);
    assert_eq!(x0.end_time, 30.0);

    // X(1) cannot start until barrier finishes, which waits for X(0) at 30.0 ns!
    assert!(x1.start_time >= 30.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_unknown_duration_rejects_scheduling_with_explicit_error() {
    let mut target = make_test_target_4q();
    // Clear duration for "sx" gate
    let mut timing = target.timing.clone().unwrap();
    timing.default_durations.remove(&TargetOpId::new("sx"));
    target.timing = Some(timing);

    // Clear descriptor duration as well so it returns OperationDuration::Unknown
    if let Some(op) = target.operations.get_mut(&TargetOpId::new("sx")) {
        op.duration = None;
    }

    let mut builder = CircuitBuilder::new("unknown_dur");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.custom_gate("sx", &[qreg[0]], vec![]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let result = scheduler.schedule(&circuit, &target);

    match result {
        Err(ScheduleError::UnknownDuration { op_name, .. }) => {
            assert_eq!(op_name, "sx");
        }
        other => panic!("expected ScheduleError::UnknownDuration, got {other:?}"),
    }
}

#[test]
fn test_missing_resource_model_rejects_scheduling() {
    let mut target = make_test_target_4q();
    target.resources = None; // completely missing resource model

    let mut builder = CircuitBuilder::new("missing_res");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let result = scheduler.schedule(&circuit, &target);

    match result {
        Err(ScheduleError::MissingResourceModel { target: name }) => {
            assert_eq!(name, "test_target_4q");
        }
        other => panic!("expected ScheduleError::MissingResourceModel, got {other:?}"),
    }
}

#[test]
fn test_missing_operation_resources_rejects_scheduling() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();
    resources.default_requirements.remove(&TargetOpId::new("x"));
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("missing_op_res");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();

    let scheduler = AsapScheduler::new();
    let result = scheduler.schedule(&circuit, &target);

    match result {
        Err(ScheduleError::MissingOperationResources { op_name, .. }) => {
            assert_eq!(op_name, "x");
        }
        other => panic!("expected ScheduleError::MissingOperationResources, got {other:?}"),
    }
}

#[test]
fn test_validator_rejects_predecessor_violation() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("pred_violation");
    let qreg = builder.add_quantum_register("q", 1).unwrap();

    builder.x(qreg[0]).unwrap();
    builder.h(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper with the schedule: make second gate start at 10.0 ns while first gate runs until 30.0 ns!
    schedule.operations[1].start_time = 10.0;
    schedule.operations[1].end_time = 40.0;

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::PredecessorViolation { .. })));
}

#[test]
fn test_validator_rejects_qubit_collision() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("qubit_collision");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Two independent gates on disjoint qubits in circuit
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper with second gate: change its operand to q0 so it collides with op 0!
    schedule.operations[1].operands = vec![q(0)];

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::QubitCollision { .. })));
}

#[test]
fn test_validator_rejects_duration_mismatch() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("dur_mismatch");
    let qreg = builder.add_quantum_register("q", 1).unwrap();

    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper with duration: change duration from 30.0 to 50.0 without updating target
    schedule.operations[0].duration = 50.0;
    schedule.operations[0].end_time = 50.0;

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::DurationMismatch { .. })));
}

#[test]
fn test_validator_rejects_capacity_exceeded() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();
    resources.map_qubit_readout_line(q(0), 1);
    resources.map_qubit_readout_line(q(1), 1);
    resources.set_capacity(ResourceId::ReadoutLine(1), 1); // capacity = 1
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("capacity_violation");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();

    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper: force second measurement to run simultaneously with the first (both at 0..1000)
    schedule.operations[1].start_time = 0.0;
    schedule.operations[1].end_time = 1000.0;

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::CapacityExceeded { .. })));
}

#[test]
fn test_deterministic_repeatability_across_runs() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("repeatable_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    builder.h(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.cx(qreg[1], qreg[2]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();

    let baseline = scheduler.schedule(&circuit, &target).unwrap();

    for _ in 0..50 {
        let candidate = scheduler.schedule(&circuit, &target).unwrap();
        assert_eq!(baseline, candidate);
    }
}

#[test]
fn test_schedule_json_serialization_round_trip() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("json_roundtrip");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    builder.x(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let json_str = schedule.to_json().expect("JSON serialization must succeed");
    let deserialized = Schedule::from_json(&json_str).expect("JSON deserialization must succeed");

    assert_eq!(schedule.circuit_name, deserialized.circuit_name);
    assert_eq!(schedule.target_name, deserialized.target_name);
    assert_eq!(schedule.makespan, deserialized.makespan);
    assert_eq!(schedule.operations, deserialized.operations);

    // Fast lookup index check
    assert!(deserialized.get(schedule.operations[0].id).is_some());
    assert!(deserialized.get(schedule.operations[1].id).is_some());
}

#[test]
fn test_virtual_zero_duration_gates_scheduling() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("virtual_gates");
    let qreg = builder.add_quantum_register("q", 1).unwrap();

    // X (30 ns) -> Rz (0 ns virtual) -> X (30 ns)
    builder.x(qreg[0]).unwrap();
    builder.rz(qreg[0], std::f64::consts::PI / 2.0).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    assert_eq!(schedule.len(), 3);
    assert_eq!(schedule.makespan(), 60.0);

    let ops = schedule.operations();
    // Op 0: X at [0..30]
    assert_eq!(ops[0].name, "x");
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 30.0);

    // Op 1: Rz at [30..30]
    assert_eq!(ops[1].name, "rz");
    assert_eq!(ops[1].start_time, 30.0);
    assert_eq!(ops[1].duration, 0.0);
    assert_eq!(ops[1].end_time, 30.0);

    // Op 2: X at [30..60]
    assert_eq!(ops[2].name, "x");
    assert_eq!(ops[2].start_time, 30.0);
    assert_eq!(ops[2].end_time, 60.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_classical_data_hazards_raw_war_waw() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("classical_hazards");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 1).unwrap();

    // 1. Measure q0 writing to c[0] (WAW writer 1, RAW producer) at [0..1000]
    builder.measure(qreg[0], creg[0]).unwrap();

    // 2. Measure q1 writing to c[0] (WAW writer 2) -> depends on first measure!
    builder.measure(qreg[1], creg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    assert_eq!(schedule.len(), 2);
    // Despite being on disjoint qubits q0 and q1, WAW hazard on c[0] forces second measure to start at 1000.0 ns!
    let ops = schedule.operations();
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 1000.0);

    assert_eq!(ops[1].start_time, 1000.0);
    assert_eq!(ops[1].end_time, 2000.0);
    assert_eq!(schedule.makespan(), 2000.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_validator_rejects_exclusive_resource_collision() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();
    let drive_0 = ResourceId::DriveLine(0);
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::exclusive(drive_0.clone())],
    );
    resources.set_tuple_requirements("x", [q(1)], vec![ResourceRequirement::exclusive(drive_0)]);
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("exclusive_res_collision");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper: force second gate to start at 0.0 ns while first gate runs at 0..30 ns
    schedule.operations[1].start_time = 0.0;
    schedule.operations[1].end_time = 30.0;

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::ExclusiveResourceCollision { .. }
    )));
}

#[test]
fn test_validator_rejects_mutual_exclusion_violation() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();
    resources.add_mutual_exclusion(MutualExclusionRule::operation_pair(
        "cx_pair_rule",
        "cx",
        Some(vec![q(0), q(1)]),
        "cx",
        Some(vec![q(2), q(3)]),
    ));
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("mutex_violation");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.cx(qreg[2], qreg[3]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper: force second CX to run at 0..200 simultaneously
    schedule.operations[1].start_time = 0.0;
    schedule.operations[1].end_time = 200.0;

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::MutualExclusionViolation { .. })));
}

#[test]
fn test_validator_rejects_missing_and_unexpected_operations() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("missing_op");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper: remove first operation
    let removed_op = schedule.operations.remove(0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::MissingOperation { .. })));

    // Tamper: add back an unexpected operation with foreign ID
    let mut unexpected = removed_op;
    unexpected.id = qcc::ir::OperationId(9999);
    schedule.operations.push(unexpected);

    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::UnexpectedOperation { .. })));
}

// -----------------------------------------------------------------------------
// Audit Area 1: Resource-calendar correctness & Earliest Feasible Time Proof
// -----------------------------------------------------------------------------

#[test]
fn test_earliest_feasible_start_time_multiple_resources_different_release_times() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    let res_a = ResourceId::Custom("res_a".to_string());
    let res_b = ResourceId::Custom("res_b".to_string());

    // Gate 1 on q0: requires res_a exclusively (duration 30 ns) -> released at 30.0 ns
    // Gate 2 on q1: requires res_b exclusively (duration 70 ns) -> released at 70.0 ns
    // Gate 3 on q2: requires BOTH res_a and res_b exclusively (duration 20 ns)
    // Predecessors for Gate 3: None (independent on q2).
    // Earliest feasible start time for Gate 3 MUST be max(release(res_a), release(res_b)) = 70.0 ns.
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::exclusive(res_a.clone())],
    );
    resources.set_tuple_requirements(
        "h",
        [q(1)],
        vec![ResourceRequirement::exclusive(res_b.clone())],
    );
    resources.set_tuple_requirements(
        "x",
        [q(2)],
        vec![
            ResourceRequirement::exclusive(res_a),
            ResourceRequirement::exclusive(res_b),
        ],
    );
    // Custom duration for h on q1: 70 ns, x on q2: 20 ns
    let mut timing = target.timing.clone().unwrap();
    timing.set_tuple_duration("h", [q(1)], OperationDuration::known_ns(70.0));
    timing.set_tuple_duration("x", [q(2)], OperationDuration::known_ns(20.0));
    target.resources = Some(resources);
    target.timing = Some(timing);

    let mut builder = CircuitBuilder::new("multi_resource_earliest_time");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    builder.x(qreg[0]).unwrap(); // [0..30]
    builder.h(qreg[1]).unwrap(); // [0..70]
    builder.x(qreg[2]).unwrap(); // Must start at exactly 70.0 ns!

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let op3 = schedule
        .operations()
        .iter()
        .find(|o| o.touches_qubit(q(2)))
        .unwrap();
    assert_eq!(
        op3.start_time, 70.0,
        "Gate 3 must wait until both resources are released"
    );
    assert_eq!(op3.end_time, 90.0);
    assert_eq!(schedule.makespan(), 90.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_shared_capacity_overlapping_unequal_durations() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    let pool = ResourceId::Custom("feedline_pool".to_string());
    resources.set_capacity(pool.clone(), 2); // capacity = 2

    // Each gate takes 1 unit of feedline_pool
    // Op 0 on q0: 40 ns
    // Op 1 on q1: 20 ns
    // Op 2 on q2: 30 ns
    // Op 0 and Op 1 start at 0.0 ns (filling capacity = 2).
    // Op 1 finishes at 20.0 ns, freeing 1 unit.
    // Op 2 should start immediately at 20.0 ns!
    // Op 2 runs from 20.0 to 50.0 ns.
    // Between 20 and 40 ns, Op 0 and Op 2 both run (total usage = 2 <= 2).
    // At 40 ns, Op 0 finishes.
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::shared(pool.clone(), 1)],
    );
    resources.set_tuple_requirements(
        "x",
        [q(1)],
        vec![ResourceRequirement::shared(pool.clone(), 1)],
    );
    resources.set_tuple_requirements("x", [q(2)], vec![ResourceRequirement::shared(pool, 1)]);

    let mut timing = target.timing.clone().unwrap();
    timing.set_tuple_duration("x", [q(0)], OperationDuration::known_ns(40.0));
    timing.set_tuple_duration("x", [q(1)], OperationDuration::known_ns(20.0));
    timing.set_tuple_duration("x", [q(2)], OperationDuration::known_ns(30.0));
    target.resources = Some(resources);
    target.timing = Some(timing);

    let mut builder = CircuitBuilder::new("shared_capacity_unequal");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let ops = schedule.operations();
    let op_q0 = ops.iter().find(|o| o.touches_qubit(q(0))).unwrap();
    let op_q1 = ops.iter().find(|o| o.touches_qubit(q(1))).unwrap();
    let op_q2 = ops.iter().find(|o| o.touches_qubit(q(2))).unwrap();

    assert_eq!(op_q0.start_time, 0.0);
    assert_eq!(op_q0.end_time, 40.0);

    assert_eq!(op_q1.start_time, 0.0);
    assert_eq!(op_q1.end_time, 20.0);

    // op_q2 starts as soon as op_q1 releases capacity at 20.0 ns!
    assert_eq!(op_q2.start_time, 20.0);
    assert_eq!(op_q2.end_time, 50.0);
    assert_eq!(schedule.makespan(), 50.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_asymmetrical_exclusive_vs_shared_resource_conflict() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    let line = ResourceId::Custom("line_x".to_string());
    resources.set_capacity(line.clone(), 5); // capacity = 5

    // Op 0 requests Shared(1)
    // Op 1 requests Exclusive
    // Even though capacity is 5, Op 1 is Exclusive, so they cannot overlap!
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::shared(line.clone(), 1)],
    );
    resources.set_tuple_requirements("h", [q(1)], vec![ResourceRequirement::exclusive(line)]);
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("asymm_conflict");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let ops = schedule.operations();
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 30.0);
    assert_eq!(
        ops[1].start_time, 30.0,
        "Exclusive op must wait for shared op to finish"
    );
    assert_eq!(ops[1].end_time, 60.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

// -----------------------------------------------------------------------------
// Audit Area 2: Validator Independence & Adversarial Tampering Checks
// -----------------------------------------------------------------------------

#[test]
fn test_validator_rejects_stripped_resources_metadata() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();
    let drive = ResourceId::DriveLine(0);
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::exclusive(drive.clone())],
    );
    resources.set_tuple_requirements("h", [q(1)], vec![ResourceRequirement::exclusive(drive)]);
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("stripped_res");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Adversarial tampering: strip resources from schedule operation metadata!
    schedule.operations[0].resources.clear();

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);

    // Validator MUST detect that schedule resources do not match target expected requirements!
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::ResourceMetadataMismatch { .. })));
}

#[test]
fn test_validator_rejects_operation_name_mismatch() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("name_mismatch");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Adversarial tampering: rename operation in schedule
    schedule.operations[0].name = "h".to_string();

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::OperationMismatch { .. })));
}

#[test]
fn test_validator_rejects_operand_mismatch() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("operand_mismatch");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Adversarial tampering: change operand to qubit 1
    schedule.operations[0].operands = vec![q(1)];

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::OperandMismatch { .. })));
}

#[test]
fn test_validator_rejects_forged_predecessors_metadata() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("forged_preds");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[0]).unwrap(); // depends on x(q0)

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Adversarial tampering: forge predecessor list to empty
    schedule.operations[1].predecessors.clear();

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::PredecessorMetadataMismatch { .. }
    )));
}

// -----------------------------------------------------------------------------
// Audit Area 3: Zero-Duration Operations & Uncalibrated Boundary Checks
// -----------------------------------------------------------------------------

#[test]
fn test_zero_duration_operations_sequence_on_same_qubit() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("zero_duration_seq");
    let qreg = builder.add_quantum_register("q", 1).unwrap();

    // Sequence of 3 virtual Rz gates followed by physical X gate on same qubit
    builder.rz(qreg[0], 0.1).unwrap();
    builder.rz(qreg[0], 0.2).unwrap();
    builder.rz(qreg[0], 0.3).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    assert_eq!(schedule.len(), 4);
    let ops = schedule.operations();
    // All 3 virtual gates execute at t = 0.0 ns
    assert_eq!(ops[0].start_time, 0.0);
    assert_eq!(ops[0].end_time, 0.0);
    assert_eq!(ops[1].start_time, 0.0);
    assert_eq!(ops[1].end_time, 0.0);
    assert_eq!(ops[2].start_time, 0.0);
    assert_eq!(ops[2].end_time, 0.0);

    // Physical X starts at 0.0 ns and finishes at 30.0 ns
    assert_eq!(ops[3].start_time, 0.0);
    assert_eq!(ops[3].end_time, 30.0);
    assert_eq!(schedule.makespan(), 30.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

// -----------------------------------------------------------------------------
// Audit Area 4: Floating-Point & Interval Boundaries
// -----------------------------------------------------------------------------

#[test]
fn test_interval_boundary_touching_vs_overlapping() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("boundary_touching");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap(); // [0..30]
    builder.h(qreg[0]).unwrap(); // [30..60]

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Baseline: exact touching at 30.0 ns is valid
    let validator = ScheduleValidator::new().with_epsilon(1e-9);
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid());

    // Tamper: overlap by 1e-6 ns (exceeds 1e-9 epsilon)
    schedule.operations[1].start_time = 30.0 - 1e-6;
    schedule.operations[1].end_time = 60.0 - 1e-6;
    schedule.makespan = 60.0 - 1e-6;

    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::QubitCollision { .. }
            | ScheduleValidationError::PredecessorViolation { .. }
    )));
}

#[test]
fn test_validator_rejects_non_finite_timestamps_and_makespan() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("nan_timing");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    // Tamper with NaN start_time
    let mut schedule_nan = schedule.clone();
    schedule_nan.operations[0].start_time = f64::NAN;
    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule_nan, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::InvalidTiming { .. })));

    // Tamper with corrupted makespan
    schedule.makespan = 999.0;
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::InvalidTiming { .. })));
}

// -----------------------------------------------------------------------------
// Audit Area 5: Physical Timing, Calibration Provenance, & Tuple Specificity
// -----------------------------------------------------------------------------

#[test]
fn test_tuple_specific_calibrated_durations() {
    let mut target = make_test_target_4q();
    let mut timing = target.timing.clone().unwrap();

    // Default x duration is 30 ns, but q1 has custom calibrated duration 45 ns
    timing.set_tuple_duration("x", [q(1)], OperationDuration::known_ns(45.0));
    target.timing = Some(timing);

    let mut builder = CircuitBuilder::new("tuple_duration");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let scheduler = AsapScheduler::new();
    let mut schedule = scheduler.schedule(&circuit, &target).unwrap();

    let target_id = schedule.operations()[1].id;
    assert_eq!(schedule.operations()[0].duration, 30.0);
    assert_eq!(schedule.operations()[1].duration, 45.0);

    let validator = ScheduleValidator::new();
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    // Tamper: change q1's duration back to default 30.0 ns
    schedule.operations[1].duration = 30.0;
    schedule.operations[1].end_time = 30.0;
    schedule.makespan = 30.0;

    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::DurationMismatch { op_id, expected_ns, actual_ns }
        if *op_id == target_id && (*expected_ns - 45.0).abs() < 1e-9 && (*actual_ns - 30.0).abs() < 1e-9
    )));
}

#[test]
fn test_stale_calibration_rejected_by_validator_and_scheduler() {
    let mut target = make_test_target_4q();
    let prov = Provenance::new("rigetti_api").with_timestamp("2026-11-01T00:00:00Z");
    target.provenance = Some(prov);

    let mut builder = CircuitBuilder::new("stale_cal");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    // 1. Scheduler with reference timestamp 10 days later (> 86,400s max age)
    let scheduler = AsapScheduler::new().with_freshness_check("2026-11-11T00:00:00Z", Some(86_400));
    let sched_err = scheduler.schedule(&circuit, &target).unwrap_err();
    assert!(matches!(sched_err, ScheduleError::Target(_)));

    // 2. Normal schedule without freshness check validates cleanly if validator does not check freshness
    let normal_scheduler = AsapScheduler::new();
    let schedule = normal_scheduler.schedule(&circuit, &target).unwrap();

    // 3. Validator configured with stale reference timestamp rejects schedule
    let validator =
        ScheduleValidator::new().with_freshness_check("2026-11-11T00:00:00Z", Some(86_400));
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::StaleCalibration { .. })));
}
