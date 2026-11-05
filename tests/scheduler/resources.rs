//! Comprehensive integration tests for Phase 57: Resource-Constrained Scheduling,
//! ResourceCalendar, Readout Groups, Shared Control Channels, and Explainability.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::node::OperationId;
use qcc::scheduler::resource_calendar::{ResourceCalendar, ResourceConflictReason};
use qcc::scheduler::resource_constraints::{
    InhibitingConstraint, PlacementBlocker, ReadoutGroup, ReadoutSynchronizationPolicy,
    ResourceConstrainedScheduler, ResourceConstraintManager,
};
use qcc::scheduler::schedule::{Schedule, ScheduledOperation};
use qcc::scheduler::validator::{ScheduleValidationError, ScheduleValidator};
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::resources::{MutualExclusionRule, ResourceId, ResourceModel, ResourceRequirement};
use qcc::target::timing::{OperationDuration, TimingModel};
use qcc::target::OperationConnectivity;

fn q(idx: u32) -> PhysicalQubitId {
    PhysicalQubitId(idx)
}

/// Helper building a standard 6-qubit test target with calibrated durations and base resource model.
fn make_test_target_6q() -> Target {
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0); // 1 GHz clock
    timing.set_default_duration("x", OperationDuration::known_ns(30.0));
    timing.set_default_duration("h", OperationDuration::known_ns(30.0));
    timing.set_default_duration("sx", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0));
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
fn test_resource_calendar_basic_reservations_and_queries() {
    let target = make_test_target_6q();
    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Reserve 1Q operation on q0 in [0.0, 30.0)
    let op0 = ScheduledOperation::new(
        OperationId(0),
        0,
        "x",
        vec![q(0)],
        0.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op0).is_ok());

    // Reserve 2Q operation on (q1, q2) in [0.0, 100.0)
    let op1 = ScheduledOperation::new(
        OperationId(1),
        1,
        "cx",
        vec![q(1), q(2)],
        0.0,
        100.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::exclusive(ResourceId::Qubit(q(2))),
            ResourceRequirement::exclusive(ResourceId::coupler(q(1), q(2))),
        ],
        vec![],
    );
    assert!(calendar.reserve(&op1).is_ok());

    assert_eq!(calendar.len(), 2);
    assert_eq!(calendar.makespan(), 100.0);

    // Check active usage at t = 15.0
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(0)), 15.0), 1);
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(1)), 15.0), 1);
    assert_eq!(
        calendar.active_usage_at(&ResourceId::coupler(q(1), q(2)), 15.0),
        1
    );
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(3)), 15.0), 0);

    // At t = 30.0, op0 has completed, so q0 active usage is 0
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(0)), 30.0), 0);
    // op1 is still active
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(1)), 30.0), 1);

    // Active operations at t = 15.0
    let active_ops = calendar.operations_active_at(15.0);
    assert_eq!(active_ops.len(), 2);
}

#[test]
fn test_resource_calendar_exclusive_collision_rejection() {
    let target = make_test_target_6q();
    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // op0 on q0 in [0.0, 30.0)
    let op0 = ScheduledOperation::new(
        OperationId(0),
        0,
        "x",
        vec![q(0)],
        0.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op0).is_ok());

    // Conflicting op on q0 in [10.0, 40.0) should be rejected
    let op_conflict = ScheduledOperation::new(
        OperationId(1),
        1,
        "h",
        vec![q(0)],
        10.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    let res = calendar.reserve(&op_conflict);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.conflicted_resource, Some(ResourceId::Qubit(q(0))));
    match *err.reason {
        ResourceConflictReason::QubitOccupied {
            qubit,
            blocking_op_id,
            blocking_interval,
            ..
        } => {
            assert_eq!(qubit, q(0));
            assert_eq!(blocking_op_id, OperationId(0));
            assert_eq!(blocking_interval, (0.0, 30.0));
        }
        other => panic!("expected QubitOccupied, got {other:?}"),
    }

    // Adjacent non-overlapping operation on q0 in [30.0, 60.0) should succeed
    let op_adjacent = ScheduledOperation::new(
        OperationId(2),
        2,
        "h",
        vec![q(0)],
        30.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op_adjacent).is_ok());
    assert_eq!(calendar.makespan(), 60.0);
}

#[test]
fn test_resource_calendar_aggregate_shared_capacity() {
    let mut target = make_test_target_6q();
    let shared_res = ResourceId::Custom("shared_bus".to_string());
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(shared_res.clone(), 3);

    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Schedule 3 operations on disjoint qubits sharing shared_bus (1 unit each) in [0.0, 50.0)
    for i in 0..3 {
        let op = ScheduledOperation::new(
            OperationId(i),
            i,
            "x",
            vec![q(i as u32)],
            0.0,
            50.0,
            vec![
                ResourceRequirement::exclusive(ResourceId::Qubit(q(i as u32))),
                ResourceRequirement::shared(shared_res.clone(), 1),
            ],
            vec![],
        );
        assert!(calendar.reserve(&op).is_ok());
    }

    assert_eq!(calendar.active_usage_at(&shared_res, 25.0), 3);

    // 4th operation requesting 1 unit in [10.0, 40.0) must fail with CapacityExceeded
    let op4 = ScheduledOperation::new(
        OperationId(3),
        3,
        "x",
        vec![q(3)],
        10.0,
        30.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(3))),
            ResourceRequirement::shared(shared_res.clone(), 1),
        ],
        vec![],
    );
    let res = calendar.reserve(&op4);
    assert!(res.is_err());
    let err = res.unwrap_err();
    match *err.reason {
        ResourceConflictReason::CapacityExceeded {
            resource,
            capacity,
            requested,
            active_usage,
            blocking_ops,
            ..
        } => {
            assert_eq!(resource, shared_res);
            assert_eq!(capacity, 3);
            assert_eq!(requested, 1);
            assert_eq!(active_usage, 3);
            assert_eq!(blocking_ops.len(), 3);
        }
        other => panic!("expected CapacityExceeded, got {other:?}"),
    }

    // Placing op4 at t = 50.0 (after earlier operations complete) must succeed!
    let op4_feasible = ScheduledOperation::new(
        OperationId(3),
        3,
        "x",
        vec![q(3)],
        50.0,
        30.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(3))),
            ResourceRequirement::shared(shared_res.clone(), 1),
        ],
        vec![],
    );
    assert!(calendar.reserve(&op4_feasible).is_ok());
    assert_eq!(calendar.makespan(), 80.0);
}

#[test]
fn test_resource_calendar_find_earliest_feasible_time_with_capacity() {
    let mut target = make_test_target_6q();
    let feedline = ResourceId::ReadoutLine(0);
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(feedline.clone(), 2);

    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Schedule 2 measurements on q0 and q1 on feedline 0 in [0.0, 500.0)
    let meas0 = ScheduledOperation::new(
        OperationId(0),
        0,
        "measure",
        vec![q(0)],
        0.0,
        500.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(0))),
            ResourceRequirement::shared(feedline.clone(), 1),
        ],
        vec![],
    );
    let meas1 = ScheduledOperation::new(
        OperationId(1),
        1,
        "measure",
        vec![q(1)],
        0.0,
        500.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::shared(feedline.clone(), 1),
        ],
        vec![],
    );
    assert!(calendar.reserve(&meas0).is_ok());
    assert!(calendar.reserve(&meas1).is_ok());

    // Third measurement on q2 also needs feedline 0
    let reqs = vec![
        ResourceRequirement::exclusive(ResourceId::Qubit(q(2))),
        ResourceRequirement::shared(feedline, 1),
    ];

    // Earliest feasible time starting from 0.0 should be exactly 500.0
    let earliest = calendar.find_earliest_feasible_time(0.0, 500.0, "measure", &[q(2)], &reqs);
    assert_eq!(earliest, 500.0);
}

#[test]
fn test_resource_calendar_find_latest_feasible_time_alap() {
    let target = make_test_target_6q();
    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Operation on q0 in [100.0, 130.0)
    let op_fixed = ScheduledOperation::new(
        OperationId(0),
        0,
        "x",
        vec![q(0)],
        100.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op_fixed).is_ok());

    let reqs = vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))];

    // We want to place an earlier operation of duration 30.0 with max_end = 100.0 and min_start = 0.0
    let latest = calendar.find_latest_feasible_time(100.0, 0.0, 30.0, "x", &[q(0)], &reqs);
    assert_eq!(latest, Some(70.0)); // [70.0, 100.0)

    // With max_end = 200.0, latest start is 200.0 - 30.0 = 170.0
    let latest_post = calendar.find_latest_feasible_time(200.0, 130.0, 30.0, "x", &[q(0)], &reqs);
    assert_eq!(latest_post, Some(170.0));
}

#[test]
fn test_readout_group_multiplexed_feedline_capacity() {
    let mut target = make_test_target_6q();
    let feedline = 0;
    let feedline_res = ResourceId::ReadoutLine(feedline);
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(feedline_res, 2);

    for idx in 0..4 {
        target
            .resources
            .as_mut()
            .unwrap()
            .map_qubit_readout_line(q(idx), feedline);
    }

    let mut manager = ResourceConstraintManager::from_target(&target).unwrap();
    let group = ReadoutGroup::new(0, feedline, [q(0), q(1), q(2), q(3)], 2)
        .with_name("multiplexed_feedline_0");
    manager.add_readout_group(group);

    // Build circuit measuring 4 independent qubits
    let mut builder = CircuitBuilder::new("meas_4q");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();
    builder.measure(qreg[3], creg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    // 4 measurements of 500ns on feedline of capacity 2 should be scheduled into 2 batches
    // Batch 1: [0.0, 500.0)
    // Batch 2: [500.0, 1000.0)
    assert_eq!(result.schedule.makespan(), 1000.0);
    assert_eq!(result.calendar.len(), 4);

    // Validate using independent ScheduleValidator
    let validator = ScheduleValidator::new();
    let val_res = validator.validate(&result.schedule, &circuit, &target);
    assert!(
        val_res.is_valid(),
        "validator failed: {:?}",
        val_res.violations
    );

    // Validate using ResourceCalendar::from_schedule
    let cal_res = ResourceCalendar::from_schedule(&result.schedule, &target);
    assert!(cal_res.is_ok(), "calendar validation failed: {cal_res:?}");
}

#[test]
fn test_readout_group_simultaneous_measurement_policy() {
    let mut target = make_test_target_6q();
    target
        .resources
        .as_mut()
        .unwrap()
        .map_qubit_readout_line(q(0), 0);
    target
        .resources
        .as_mut()
        .unwrap()
        .map_qubit_readout_line(q(1), 0);
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(ResourceId::ReadoutLine(0), 2);

    let group = ReadoutGroup::new(0, 0, [q(0), q(1)], 2)
        .with_sync_policy(ReadoutSynchronizationPolicy::SimultaneousOnly);

    let mut manager = ResourceConstraintManager::from_target(&target).unwrap();
    manager.add_readout_group(group);

    let mut builder = CircuitBuilder::new("sync_meas");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    let validator = ScheduleValidator::new();
    let val_res = validator.validate(&result.schedule, &circuit, &target);
    assert!(
        val_res.is_valid(),
        "simultaneous measurement validator failed: {:?}",
        val_res.violations
    );
}

#[test]
fn test_readout_group_strictly_serialized() {
    let mut target = make_test_target_6q();
    target
        .resources
        .as_mut()
        .unwrap()
        .map_qubit_readout_line(q(0), 0);
    target
        .resources
        .as_mut()
        .unwrap()
        .map_qubit_readout_line(q(1), 0);
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(ResourceId::ReadoutLine(0), 1);

    let group = ReadoutGroup::new(0, 0, [q(0), q(1)], 1)
        .with_sync_policy(ReadoutSynchronizationPolicy::StrictlySerialized);

    let mut manager = ResourceConstraintManager::from_target(&target).unwrap();
    manager.add_readout_group(group);

    let mut builder = CircuitBuilder::new("serialized_meas");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    // Capacity 1 means serialized: 500ns + 500ns = 1000ns
    assert_eq!(result.schedule.makespan(), 1000.0);
    let validator = ScheduleValidator::new();
    let val_res = validator.validate(&result.schedule, &circuit, &target);
    assert!(
        val_res.is_valid(),
        "strictly serialized validator failed: {:?}",
        val_res.violations
    );
}

#[test]
fn test_shared_control_channel_drive_lines() {
    let mut target = make_test_target_6q();
    // Qubits 0 and 2 share DriveLine(0) in Exclusive mode
    // Qubits 1 and 3 share DriveLine(1) in Exclusive mode
    let drive0 = ResourceId::DriveLine(0);
    let drive1 = ResourceId::DriveLine(1);

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
    target.resources.as_mut().unwrap().set_tuple_requirements(
        "x",
        [q(1)],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::exclusive(drive1.clone()),
        ],
    );
    target.resources.as_mut().unwrap().set_tuple_requirements(
        "x",
        [q(3)],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(3))),
            ResourceRequirement::exclusive(drive1.clone()),
        ],
    );

    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    // Circuit executes X on 0, 1, 2, 3
    let mut builder = CircuitBuilder::new("shared_drive");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.x(qreg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    // Qubits 0 and 1 execute concurrently at t = 0 (30ns)
    // Qubits 2 and 3 are serialized to t = 30 (30ns)
    // Total makespan: 60ns
    assert_eq!(result.schedule.makespan(), 60.0);

    let validator = ScheduleValidator::new();
    let val_res = validator.validate(&result.schedule, &circuit, &target);
    assert!(
        val_res.is_valid(),
        "strictly serialized validator failed: {:?}",
        val_res.violations
    );
}

#[test]
fn test_compatible_vs_disjoint_operations_concurrency_analysis() {
    let mut target = make_test_target_6q();
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

    // Add mutual exclusion rule between X on q1 and X on q3
    target
        .resources
        .as_mut()
        .unwrap()
        .add_mutual_exclusion(MutualExclusionRule::operation_pair(
            "cross_talk_avoidance",
            "x",
            Some(vec![q(1)]),
            "x",
            Some(vec![q(3)]),
        ));

    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    // 1. Disjoint operations on independent resources: X(q0) and X(q1)
    let analysis_disjoint = manager.analyze_concurrency(("x", &[q(0)]), ("x", &[q(1)]));
    assert!(analysis_disjoint.is_compatible());

    // 2. Same qubit collision: X(q0) and H(q0)
    let analysis_same_q = manager.analyze_concurrency(("x", &[q(0)]), ("h", &[q(0)]));
    assert!(analysis_same_q.has_conflict());
    assert!(analysis_same_q
        .inhibiting_constraints
        .contains(&InhibitingConstraint::SharedQubit(q(0))));

    // 3. Shared exclusive drive line: X(q0) and X(q2)
    let analysis_drive = manager.analyze_concurrency(("x", &[q(0)]), ("x", &[q(2)]));
    assert!(analysis_drive.has_conflict());
    assert!(analysis_drive
        .inhibiting_constraints
        .contains(&InhibitingConstraint::ExclusiveResource(drive0)));

    // 4. Mutual exclusion rule: X(q1) and X(q3)
    let analysis_mex = manager.analyze_concurrency(("x", &[q(1)]), ("x", &[q(3)]));
    assert!(analysis_mex.has_conflict());
    assert!(analysis_mex.inhibiting_constraints.iter().any(|c| matches!(
        c,
        InhibitingConstraint::MutualExclusion { rule, .. } if rule == "cross_talk_avoidance"
    )));
}

#[test]
fn test_explain_placement_blockers_and_scheduling_traces() {
    let mut target = make_test_target_6q();
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
        [q(1)],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::exclusive(drive0.clone()),
        ],
    );

    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    let mut builder = CircuitBuilder::new("delayed_gate");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    // Verify explainability trace was recorded for the delayed gate on q1
    assert_eq!(result.traces.len(), 1);
    let trace = &result.traces[0];
    assert_eq!(trace.op_name, "x");
    assert_eq!(trace.operands, vec![q(1)]);
    assert_eq!(trace.predecessor_bound, 0.0);
    assert_eq!(trace.scheduled_start, 30.0);
    assert_eq!(trace.delay, 30.0);

    // Blocker identifies DriveLine(0) and op#0
    assert!(!trace.blockers.is_empty());
    let blocker = &trace.blockers[0];
    assert_eq!(blocker.blocking_resource, drive0);
    assert_eq!(blocker.blocking_op_id, Some(OperationId(1)));
    assert_eq!(blocker.expected_release_time, 30.0);
}

#[test]
fn test_synthetic_resource_conflict_schedules_pass_validator() {
    // MANDATORY ROADMAP EXIT GATE: Synthetic resource-conflict schedules pass validator.
    let mut target = make_test_target_6q();

    // 1. Shared drive line across q0 and q2
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

    // 2. Multiplexed feedline with capacity 2 across q0, q1, q2, q3
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

    // 3. Mutual exclusion rule: spectator cross-talk between CX(0,1) and SX on q3
    target
        .resources
        .as_mut()
        .unwrap()
        .add_mutual_exclusion(MutualExclusionRule::operation_pair(
            "spectator_crosstalk",
            "cx",
            Some(vec![q(0), q(1)]),
            "h",
            Some(vec![q(3)]),
        ));

    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    // Build synthetic complex circuit triggering all resource constraints
    let mut builder = CircuitBuilder::new("synthetic_conflict");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    // Drive line conflict between q0 and q2
    builder.x(qreg[0]).unwrap();
    builder.x(qreg[2]).unwrap();
    // CX on (0, 1) and H on 3 (spectator exclusion)
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.h(qreg[3]).unwrap();
    // 4 measurements on feedline with capacity 2
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();
    builder.measure(qreg[3], creg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager);
    let result = scheduler.schedule(&circuit, &target).unwrap();

    // Verify with independent ScheduleValidator
    let validator = ScheduleValidator::new();
    let val_res = validator.validate(&result.schedule, &circuit, &target);
    assert!(
        val_res.is_valid(),
        "independent validator rejected synthetic schedule: {:?}",
        val_res.violations
    );

    // Verify with ResourceCalendar::from_schedule
    let cal_res = ResourceCalendar::from_schedule(&result.schedule, &target);
    assert!(
        cal_res.is_ok(),
        "resource calendar rejected synthetic schedule: {cal_res:?}"
    );
}

#[test]
fn test_validator_rejects_synthetic_capacity_tampering() {
    let target = make_test_target_6q();
    let feedline = ResourceId::ReadoutLine(0);

    // Build schedule manually with 3 overlapping measurements on capacity 2
    let mut schedule = Schedule::new("tampered", target.name.clone());
    schedule.add_operation(ScheduledOperation::new(
        OperationId(0),
        0,
        "measure",
        vec![q(0)],
        0.0,
        500.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(0))),
            ResourceRequirement::shared(feedline.clone(), 1),
        ],
        vec![],
    ));
    schedule.add_operation(ScheduledOperation::new(
        OperationId(1),
        1,
        "measure",
        vec![q(1)],
        0.0,
        500.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::shared(feedline.clone(), 1),
        ],
        vec![],
    ));
    schedule.add_operation(ScheduledOperation::new(
        OperationId(2),
        2,
        "measure",
        vec![q(2)],
        0.0,
        500.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(2))),
            ResourceRequirement::shared(feedline.clone(), 1),
        ],
        vec![],
    ));

    let cal_res = ResourceCalendar::from_schedule(&schedule, &target);
    assert!(cal_res.is_err());
    assert!(matches!(
        cal_res.unwrap_err(),
        ScheduleValidationError::CapacityExceeded { .. }
    ));
}

#[test]
fn test_validator_rejects_synthetic_exclusive_channel_tampering() {
    let target = make_test_target_6q();
    let drive = ResourceId::DriveLine(0);

    let mut schedule = Schedule::new("tampered", target.name.clone());
    schedule.add_operation(ScheduledOperation::new(
        OperationId(0),
        0,
        "x",
        vec![q(0)],
        0.0,
        30.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(0))),
            ResourceRequirement::exclusive(drive.clone()),
        ],
        vec![],
    ));
    schedule.add_operation(ScheduledOperation::new(
        OperationId(1),
        1,
        "x",
        vec![q(1)],
        10.0, // overlaps in [10.0, 40.0)
        30.0,
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(q(1))),
            ResourceRequirement::exclusive(drive.clone()),
        ],
        vec![],
    ));

    let cal_res = ResourceCalendar::from_schedule(&schedule, &target);
    assert!(cal_res.is_err());
    assert!(matches!(
        cal_res.unwrap_err(),
        ScheduleValidationError::ExclusiveResourceCollision { .. }
    ));
}

#[test]
fn test_validator_rejects_synthetic_mutual_exclusion_tampering() {
    let mut target = make_test_target_6q();
    target
        .resources
        .as_mut()
        .unwrap()
        .add_mutual_exclusion(MutualExclusionRule::operation_pair(
            "mex_rule",
            "x",
            Some(vec![q(0)]),
            "x",
            Some(vec![q(1)]),
        ));

    let mut schedule = Schedule::new("tampered", target.name.clone());
    schedule.add_operation(ScheduledOperation::new(
        OperationId(0),
        0,
        "x",
        vec![q(0)],
        0.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    ));
    schedule.add_operation(ScheduledOperation::new(
        OperationId(1),
        1,
        "x",
        vec![q(1)],
        10.0,
        30.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(1)))],
        vec![],
    ));

    let cal_res = ResourceCalendar::from_schedule(&schedule, &target);
    assert!(cal_res.is_err());
    assert!(matches!(
        cal_res.unwrap_err(),
        ScheduleValidationError::MutualExclusionViolation { .. }
    ));
}

#[test]
fn test_peak_resource_usage_and_timeline_queries() {
    let mut target = make_test_target_6q();
    let res = ResourceId::Custom("aux_channel".to_string());
    target
        .resources
        .as_mut()
        .unwrap()
        .set_capacity(res.clone(), 5);

    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Op 0 in [0, 40) amount 1
    // Op 1 in [10, 30) amount 2
    // Op 2 in [20, 50) amount 1
    calendar
        .reserve(&ScheduledOperation::new(
            OperationId(0),
            0,
            "x",
            vec![q(0)],
            0.0,
            40.0,
            vec![ResourceRequirement::shared(res.clone(), 1)],
            vec![],
        ))
        .unwrap();

    calendar
        .reserve(&ScheduledOperation::new(
            OperationId(1),
            1,
            "x",
            vec![q(1)],
            10.0,
            20.0,
            vec![ResourceRequirement::shared(res.clone(), 2)],
            vec![],
        ))
        .unwrap();

    calendar
        .reserve(&ScheduledOperation::new(
            OperationId(2),
            2,
            "x",
            vec![q(2)],
            20.0,
            30.0,
            vec![ResourceRequirement::shared(res.clone(), 1)],
            vec![],
        ))
        .unwrap();

    // At t = 25.0: op0(1) + op1(2) + op2(1) = 4
    assert_eq!(calendar.active_usage_at(&res, 25.0), 4);
    let (peak_val, peak_t) = calendar.peak_usage(&res);
    assert_eq!(peak_val, 4);
    assert_eq!(peak_t, 20.0);

    let timeline = calendar.timeline_for(&res);
    assert_eq!(timeline.len(), 3);
}

#[test]
fn test_reservation_removal_and_makespan_recalculation() {
    let target = make_test_target_6q();
    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    calendar
        .reserve(&ScheduledOperation::new(
            OperationId(0),
            0,
            "x",
            vec![q(0)],
            0.0,
            30.0,
            vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
            vec![],
        ))
        .unwrap();

    calendar
        .reserve(&ScheduledOperation::new(
            OperationId(1),
            1,
            "x",
            vec![q(1)],
            30.0,
            100.0,
            vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(1)))],
            vec![],
        ))
        .unwrap();

    assert_eq!(calendar.makespan(), 130.0);
    assert_eq!(calendar.len(), 2);

    // Remove op1
    assert!(calendar.remove_reservation(OperationId(1)));
    assert_eq!(calendar.len(), 1);
    assert_eq!(calendar.makespan(), 30.0);
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(1)), 50.0), 0);
}

#[test]
fn test_zero_duration_operations_on_resource_calendar() {
    let target = make_test_target_6q();
    let mut calendar = ResourceCalendar::new(target.resources.clone().unwrap());

    // Virtual gate on q0 at t = 30.0 (duration 0.0)
    let op_zero = ScheduledOperation::new(
        OperationId(0),
        0,
        "rz",
        vec![q(0)],
        30.0,
        0.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op_zero).is_ok());

    // Active at t = 30.0
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(0)), 30.0), 1);
    // Not active at t = 30.1
    assert_eq!(calendar.active_usage_at(&ResourceId::Qubit(q(0)), 30.1), 0);

    // Second zero-duration operation on q0 at same timestamp t = 30.0 should collide
    let op_zero2 = ScheduledOperation::new(
        OperationId(1),
        1,
        "rz",
        vec![q(0)],
        30.0,
        0.0,
        vec![ResourceRequirement::exclusive(ResourceId::Qubit(q(0)))],
        vec![],
    );
    assert!(calendar.reserve(&op_zero2).is_err());
}

#[test]
fn test_serde_json_round_trip_calendar_and_constraints() {
    let group = ReadoutGroup::new(0, 1, [q(0), q(1)], 2)
        .with_sync_policy(ReadoutSynchronizationPolicy::SimultaneousOnly)
        .with_name("line_1");

    let json = serde_json::to_string_pretty(&group).unwrap();
    let deserialized: ReadoutGroup = serde_json::from_str(&json).unwrap();
    assert_eq!(group, deserialized);

    let blocker = PlacementBlocker {
        candidate_time: 0.0,
        blocking_op_id: Some(OperationId(42)),
        blocking_op_name: Some("cx".to_string()),
        blocking_resource: ResourceId::DriveLine(2),
        constraint: InhibitingConstraint::ExclusiveResource(ResourceId::DriveLine(2)),
        expected_release_time: 100.0,
    };
    let json_b = serde_json::to_string(&blocker).unwrap();
    let des_b: PlacementBlocker = serde_json::from_str(&json_b).unwrap();
    assert_eq!(blocker, des_b);
}

#[test]
fn test_deterministic_reproducibility() {
    let target = make_test_target_6q();
    let manager = ResourceConstraintManager::from_target(&target).unwrap();

    let mut builder = CircuitBuilder::new("det_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    builder.h(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.x(qreg[2]).unwrap();
    builder.cx(qreg[2], qreg[3]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[1], creg[1]).unwrap();
    builder.measure(qreg[2], creg[2]).unwrap();
    builder.measure(qreg[3], creg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = ResourceConstrainedScheduler::new(manager.clone());
    let res1 = scheduler.schedule(&circuit, &target).unwrap();
    let res2 = scheduler.schedule(&circuit, &target).unwrap();

    assert_eq!(res1.schedule.makespan(), res2.schedule.makespan());
    assert_eq!(res1.schedule.len(), res2.schedule.len());
    for (op1, op2) in res1
        .schedule
        .operations()
        .iter()
        .zip(res2.schedule.operations().iter())
    {
        assert_eq!(op1.id, op2.id);
        assert_eq!(op1.start_time, op2.start_time);
        assert_eq!(op1.end_time, op2.end_time);
        assert_eq!(op1.name, op2.name);
    }
}
