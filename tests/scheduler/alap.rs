//! Comprehensive integration tests for Phase 56: ALAP Scheduler & Operation Slack.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::node::OperationId;
use qcc::scheduler::alap::AlapScheduler;
use qcc::scheduler::asap::AsapScheduler;
use qcc::scheduler::validator::{ScheduleValidationError, ScheduleValidator};
use qcc::scheduler::ScheduleError;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::resources::{ResourceId, ResourceModel, ResourceRequirement};
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
    timing.set_default_duration("z", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0)); // virtual Z
    timing.set_default_duration("cx", OperationDuration::known_ns(200.0));
    timing.set_default_duration("measure", OperationDuration::known_ns(1000.0));
    timing.set_default_duration("reset", OperationDuration::known_ns(500.0));

    let mut resources = ResourceModel::new();
    // Default requirements: exclusive qubit locks (and coupler for 2Q)
    resources.set_default_requirements("x", vec![]);
    resources.set_default_requirements("h", vec![]);
    resources.set_default_requirements("z", vec![]);
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

    let z_desc = TargetOperationBuilder::new("z", 1)
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
        .add_operation(z_desc)
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
fn test_linear_dag_zero_slack() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("linear_dag");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    let creg = builder.add_classical_register("c", 1).unwrap();

    let h_id = builder.h(qreg[0]).unwrap();
    let x_id = builder.x(qreg[0]).unwrap();
    let z_id = builder.z(qreg[0]).unwrap();
    let m_id = builder.measure(qreg[0], creg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // In a strictly linear DAG, critical path is the entire circuit
    assert_eq!(asap.makespan(), alap.makespan());
    assert_eq!(asap.len(), 4);
    assert_eq!(alap.len(), 4);

    for op in alap.operations() {
        let asap_op = asap.get(op.id).unwrap();
        assert_eq!(op.start_time, asap_op.start_time);
        assert_eq!(op.end_time, asap_op.end_time);
        assert_eq!(
            op.slack(),
            Some(0.0),
            "operation op#{} must have 0.0 slack",
            op.id.0
        );
    }

    assert_eq!(alap.get(h_id).unwrap().start_time, 0.0);
    assert_eq!(alap.get(x_id).unwrap().start_time, 30.0);
    assert_eq!(alap.get(z_id).unwrap().start_time, 60.0);
    assert_eq!(alap.get(m_id).unwrap().start_time, 90.0);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_parallel_independent_paths_positive_slack() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("parallel_paths");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();

    // Qubit 0 path: H (30ns) -> X (30ns) -> Z (30ns) -> CX(0, 1) (200ns) -> Meas(0) (1000ns)
    // Qubit 1 path: H (30ns) -> CX(0, 1) -> Meas(1) (1000ns)
    let h0 = builder.h(qreg[0]).unwrap();
    let x0 = builder.x(qreg[0]).unwrap();
    let z0 = builder.z(qreg[0]).unwrap();

    let h1 = builder.h(qreg[1]).unwrap();

    let cx = builder.cx(qreg[0], qreg[1]).unwrap();
    let _m0 = builder.measure(qreg[0], creg[0]).unwrap();
    let _m1 = builder.measure(qreg[1], creg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    assert_eq!(asap.makespan(), alap.makespan());

    // In ASAP: h1 starts at 0.0ns, ends at 30.0ns. CX starts at 90.0ns.
    assert_eq!(asap.get(h1).unwrap().start_time, 0.0);
    assert_eq!(asap.get(cx).unwrap().start_time, 90.0);

    // In ALAP: h1 is delayed to finish right when CX starts at 90.0ns!
    // So h1 ALAP start time is 90.0 - 30.0 = 60.0ns.
    let alap_h1 = alap.get(h1).unwrap();
    assert_eq!(alap_h1.start_time, 60.0);
    assert_eq!(alap_h1.end_time, 90.0);
    assert_eq!(alap_h1.slack(), Some(60.0));

    // Operations on qubit 0 (critical path) have 0.0 slack
    assert_eq!(alap.get(h0).unwrap().slack(), Some(0.0));
    assert_eq!(alap.get(x0).unwrap().slack(), Some(0.0));
    assert_eq!(alap.get(z0).unwrap().slack(), Some(0.0));
    assert_eq!(alap.get(cx).unwrap().slack(), Some(0.0));

    let crit_path = alap.critical_path();
    let crit_ids: Vec<OperationId> = crit_path.iter().map(|o| o.id).collect();
    assert!(crit_ids.contains(&h0));
    assert!(crit_ids.contains(&x0));
    assert!(crit_ids.contains(&z0));
    assert!(crit_ids.contains(&cx));
    assert!(!crit_ids.contains(&h1), "h1 must not be on critical path");

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_multi_successor_synchronization() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("multi_successor");
    let qreg = builder.add_quantum_register("q", 3).unwrap();

    // Node h0 feeds into cx(0, 1) AND cx(0, 2)
    let h0 = builder.h(qreg[0]).unwrap();
    // Path 1 on q1: X -> X -> CX(0, 1)
    let _x1_1 = builder.x(qreg[1]).unwrap();
    let _x1_2 = builder.x(qreg[1]).unwrap();
    let cx01 = builder.cx(qreg[0], qreg[1]).unwrap();

    // Path 2 on q2: X -> X -> X -> X -> CX(0, 2) (starts later)
    let _x2_1 = builder.x(qreg[2]).unwrap();
    let _x2_2 = builder.x(qreg[2]).unwrap();
    let _x2_3 = builder.x(qreg[2]).unwrap();
    let _x2_4 = builder.x(qreg[2]).unwrap();
    let _cx02 = builder.cx(qreg[0], qreg[2]).unwrap();

    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // CX(0, 1) must wait for h0 and x1_2 (60ns). CX(0, 2) waits for CX(0, 1) on q0.
    // In ALAP, h0 must complete before the earliest of its successors starts.
    let cx01_start = alap.get(cx01).unwrap().start_time;
    let h0_end = alap.get(h0).unwrap().end_time;
    assert!(
        h0_end <= cx01_start,
        "h0 end ({h0_end}) must be <= cx01 start ({cx01_start})"
    );

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_explicit_makespan_policy_extended_window() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("explicit_makespan");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    let h0 = builder.h(qreg[0]).unwrap(); // 30ns
    let x0 = builder.x(qreg[0]).unwrap(); // 30ns
    let circuit = builder.finish().unwrap();

    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(asap.makespan(), 60.0);

    // Request an extended makespan of 200.0 ns
    let alap = AlapScheduler::new()
        .with_makespan(200.0)
        .schedule(&circuit, &target)
        .unwrap();

    assert_eq!(alap.requested_makespan(), Some(200.0));
    assert_eq!(alap.makespan(), 200.0);

    // Operations should be shifted to the end of the window:
    // x0 runs from 170.0 to 200.0 ns
    // h0 runs from 140.0 to 170.0 ns
    let alap_x0 = alap.get(x0).unwrap();
    let alap_h0 = alap.get(h0).unwrap();

    assert_eq!(alap_h0.start_time, 140.0);
    assert_eq!(alap_h0.end_time, 170.0);
    assert_eq!(alap_x0.start_time, 170.0);
    assert_eq!(alap_x0.end_time, 200.0);

    // Slack is 140.0 ns (start_alap 140 - start_asap 0)
    assert_eq!(alap_h0.slack(), Some(140.0));
    assert_eq!(alap_x0.slack(), Some(140.0));

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_explicit_makespan_policy_infeasible_window() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("infeasible_makespan");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.h(qreg[0]).unwrap(); // 30ns
    builder.x(qreg[0]).unwrap(); // 30ns
    let circuit = builder.finish().unwrap();

    // Minimum makespan is 60.0 ns. Requesting 50.0 ns must fail!
    let err = AlapScheduler::new()
        .with_makespan(50.0)
        .schedule(&circuit, &target)
        .unwrap_err();

    match err {
        ScheduleError::InfeasibleMakespan {
            requested_ns,
            minimum_ns,
        } => {
            assert_eq!(requested_ns, 50.0);
            assert_eq!(minimum_ns, 60.0);
        }
        other => panic!("expected InfeasibleMakespan, got {other:?}"),
    }
}

#[test]
fn test_unequal_durations_and_virtual_gates_alap() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("unequal_durations");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Virtual RZ has 0ns duration
    let rz0 = builder.rz(qreg[0], 0.5).unwrap();
    let h0 = builder.h(qreg[0]).unwrap(); // 30ns
    let cx = builder.cx(qreg[0], qreg[1]).unwrap(); // 200ns
    let rz1 = builder.rz(qreg[1], 1.0).unwrap(); // 0ns
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    let rz0_op = alap.get(rz0).unwrap();
    let h0_op = alap.get(h0).unwrap();
    let cx_op = alap.get(cx).unwrap();
    let rz1_op = alap.get(rz1).unwrap();

    assert_eq!(rz0_op.duration, 0.0);
    assert_eq!(rz1_op.duration, 0.0);
    assert_eq!(h0_op.duration, 30.0);
    assert_eq!(cx_op.duration, 200.0);

    // In ALAP, rz0 should end right at h0 start
    assert_eq!(rz0_op.start_time, h0_op.start_time);
    assert_eq!(h0_op.end_time, cx_op.start_time);
    assert_eq!(cx_op.end_time, rz1_op.start_time);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_resource_constrained_shared_drive_line_alap() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // Set shared drive line for X on q0 and X on q1
    let drive_line_0 = ResourceId::DriveLine(0);
    resources.set_tuple_requirements(
        "x",
        [q(0)],
        vec![ResourceRequirement::exclusive(drive_line_0.clone())],
    );
    resources.set_tuple_requirements(
        "x",
        [q(1)],
        vec![ResourceRequirement::exclusive(drive_line_0)],
    );
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("shared_drive");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    // Two independent operations on q0 and q1 that contest the same drive line
    let x0 = builder.x(qreg[0]).unwrap();
    let x1 = builder.x(qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    let op0 = alap.get(x0).unwrap();
    let op1 = alap.get(x1).unwrap();

    // Must be serialized because of the exclusive drive line!
    assert!(
        !op0.overlaps(op1),
        "operations contending for exclusive drive line must not overlap"
    );

    assert_eq!(alap.makespan(), 60.0);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_resource_constrained_multiplexed_feedline_capacity_alap() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // 4 qubits sharing a readout line of capacity 2 (at most 2 concurrent measurements)
    let readout_line = ResourceId::ReadoutLine(1);
    resources.set_capacity(readout_line.clone(), 2);

    for idx in 0..4 {
        resources.set_tuple_requirements(
            "measure",
            [q(idx)],
            vec![ResourceRequirement::shared(readout_line.clone(), 1)],
        );
    }
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("parallel_readouts");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let creg = builder.add_classical_register("c", 4).unwrap();
    for idx in 0..4 {
        builder.measure(qreg[idx], creg[idx]).unwrap();
    }
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // With 4 measurements of 1000ns and capacity 2, makespan must be 2000ns
    assert_eq!(alap.makespan(), 2000.0);

    // Two measurements finish at 2000ns, two finish at 1000ns
    let count_at_1000 = alap
        .operations()
        .iter()
        .filter(|o| o.end_time == 1000.0)
        .count();
    let count_at_2000 = alap
        .operations()
        .iter()
        .filter(|o| o.end_time == 2000.0)
        .count();
    assert_eq!(count_at_1000, 2);
    assert_eq!(count_at_2000, 2);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_barrier_synchronization_alap() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("barrier_sync");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    let h0 = builder.h(qreg[0]).unwrap(); // 30ns
    let bar = builder.barrier(&[qreg[0], qreg[1]]).unwrap(); // 0ns
    let x1 = builder.x(qreg[1]).unwrap(); // 30ns
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    let h0_op = alap.get(h0).unwrap();
    let bar_op = alap.get(bar).unwrap();
    let x1_op = alap.get(x1).unwrap();

    // Barrier must be >= h0_op.end_time
    assert!(bar_op.start_time >= h0_op.end_time);
    // x1 must be >= bar_op.end_time
    assert!(x1_op.start_time >= bar_op.end_time);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_deterministic_reproducibility() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("deterministic_test");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();

    builder.h(qreg[0]).unwrap();
    builder.x(qreg[1]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    builder.z(qreg[2]).unwrap();
    builder.cx(qreg[1], qreg[2]).unwrap();
    builder.measure(qreg[0], creg[0]).unwrap();
    builder.measure(qreg[2], creg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AlapScheduler::new();
    let baseline = scheduler.schedule(&circuit, &target).unwrap();

    for _ in 0..10 {
        let repeated = scheduler.schedule(&circuit, &target).unwrap();
        assert_eq!(
            baseline, repeated,
            "ALAP scheduling must be 100% deterministic"
        );
    }
}

#[test]
fn test_independent_validator_rejects_exceeded_window() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("window_exceeded");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap(); // 30ns
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    // Tamper with requested makespan: request 20ns when makespan is 30ns
    schedule.requested_makespan = Some(20.0);

    let report = ScheduleValidator::new().validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::WindowExceeded {
            requested_ns,
            actual_ns
        } if *requested_ns == 20.0 && *actual_ns == 30.0
    )));
}

#[test]
fn test_independent_validator_rejects_negative_slack() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("negative_slack_test");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    let x0 = builder.x(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    // Tamper: assign negative slack to operation
    for op in &mut schedule.operations {
        if op.id == x0 {
            op.slack = Some(-10.0);
        }
    }

    let report = ScheduleValidator::new().validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::NegativeSlack { op_id, slack_ns }
        if *op_id == x0 && *slack_ns == -10.0
    )));
}

#[test]
fn test_classical_wire_hazards_raw_war_waw_alap() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("classical_hazards");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let creg = builder.add_classical_register("c", 2).unwrap();

    // WAW hazard: Measure writes c0, subsequent measure also writes c0
    let m0 = builder.measure(qreg[0], creg[0]).unwrap();
    let m1 = builder.measure(qreg[1], creg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    let op_m0 = alap.get(m0).unwrap();
    let op_m1 = alap.get(m1).unwrap();

    // m1 depends on m0 through WAW hazard on bit c0!
    assert!(op_m1.start_time >= op_m0.end_time);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_unknown_duration_rejection_alap() {
    let mut target = make_test_target_4q();
    // Remove duration for z from timing model and target operation descriptor
    if let Some(timing) = target.timing.as_mut() {
        timing.set_default_duration("z", OperationDuration::Unknown);
    }
    if let Some(op) = target
        .operations
        .get_mut(&qcc::target::operation::TargetOpId::new("z"))
    {
        op.duration = None;
    }

    let mut builder = CircuitBuilder::new("unknown_dur");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.z(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let err = AlapScheduler::new()
        .schedule(&circuit, &target)
        .unwrap_err();

    match err {
        ScheduleError::UnknownDuration { op_name, .. } => {
            assert_eq!(op_name, "z");
        }
        other => panic!("expected UnknownDuration, got {other:?}"),
    }
}

#[test]
fn test_missing_resource_model_rejection_alap() {
    let mut target = make_test_target_4q();
    target.resources = None;

    let mut builder = CircuitBuilder::new("missing_resources");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let err = AlapScheduler::new()
        .schedule(&circuit, &target)
        .unwrap_err();

    match err {
        ScheduleError::MissingResourceModel { target: name } => {
            assert_eq!(name, "test_target_4q");
        }
        other => panic!("expected MissingResourceModel, got {other:?}"),
    }
}

#[test]
fn test_empty_circuit_alap() {
    let target = make_test_target_4q();
    let builder = CircuitBuilder::new("empty_circuit");
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(alap.makespan(), 0.0);
    assert!(alap.is_empty());
    assert_eq!(alap.len(), 0);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_schedule_slack_api_and_critical_path() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("slack_api_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Critical path on q0: 30 + 30 + 30 = 90ns
    let h0 = builder.h(qreg[0]).unwrap();
    let x0 = builder.x(qreg[0]).unwrap();
    let z0 = builder.z(qreg[0]).unwrap();

    // Shorter path on q1: 30ns
    let h1 = builder.h(qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(alap.makespan(), 90.0);

    assert_eq!(alap.slack(h0), Some(0.0));
    assert_eq!(alap.slack(x0), Some(0.0));
    assert_eq!(alap.slack(z0), Some(0.0));
    assert_eq!(alap.slack(h1), Some(60.0));

    let slacks = alap.slacks();
    assert_eq!(slacks.len(), 4);
    assert_eq!(slacks.get(&h1), Some(&60.0));

    let crit = alap.critical_path();
    assert_eq!(crit.len(), 3);
    let crit_ids: Vec<OperationId> = crit.iter().map(|o| o.id).collect();
    assert!(crit_ids.contains(&h0));
    assert!(crit_ids.contains(&x0));
    assert!(crit_ids.contains(&z0));
    assert!(!crit_ids.contains(&h1));

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_mutual_exclusion_crosstalk_alap() {
    let mut target = make_test_target_4q();
    let mut resources = target.resources.clone().unwrap();

    // Cross-talk rule: CX(0, 1) and CX(2, 3) cannot run concurrently
    resources.add_mutual_exclusion(qcc::target::resources::MutualExclusionRule::operation_pair(
        "cx_cross_talk",
        "cx",
        Some(vec![q(0), q(1)]),
        "cx",
        Some(vec![q(2), q(3)]),
    ));
    target.resources = Some(resources);

    let mut builder = CircuitBuilder::new("crosstalk_alap");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let cx1 = builder.cx(qreg[0], qreg[1]).unwrap();
    let cx2 = builder.cx(qreg[2], qreg[3]).unwrap();
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    let op1 = alap.get(cx1).unwrap();
    let op2 = alap.get(cx2).unwrap();

    // Because of cross-talk mutual exclusion, they must not overlap
    assert!(
        !op1.overlaps(op2),
        "operations with cross-talk rule must not overlap in ALAP"
    );
    assert_eq!(alap.makespan(), 400.0);

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}

#[test]
fn test_stale_calibration_rejection_alap() {
    let mut target = make_test_target_4q();
    let prov = qcc::target::calibration::Provenance::new("rigetti_api")
        .with_timestamp("2026-11-01T00:00:00Z");
    target.provenance = Some(prov);

    let mut builder = CircuitBuilder::new("stale_cal");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    builder.x(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    // Scheduler with reference timestamp 10 days later (> 86,400s max age)
    let scheduler = AlapScheduler::new().with_freshness_check("2026-11-11T00:00:00Z", Some(86_400));
    let sched_err = scheduler.schedule(&circuit, &target).unwrap_err();
    assert!(matches!(sched_err, ScheduleError::Target(_)));

    // Normal schedule without freshness check validates cleanly if validator does not check freshness
    let normal_scheduler = AlapScheduler::new();
    let schedule = normal_scheduler.schedule(&circuit, &target).unwrap();

    // Validator configured with stale reference timestamp rejects schedule
    let validator =
        ScheduleValidator::new().with_freshness_check("2026-11-11T00:00:00Z", Some(86_400));
    let report = validator.validate(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::StaleCalibration { .. })));
}

#[test]
fn test_reset_and_long_duration_alap() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("reset_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // q0: Reset (500ns) -> H (30ns)
    // q1: H (30ns)
    let r0 = builder.reset(qreg[0]).unwrap();
    let h0 = builder.h(qreg[0]).unwrap();
    let h1 = builder.h(qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(alap.makespan(), 530.0);

    let r0_op = alap.get(r0).unwrap();
    let h0_op = alap.get(h0).unwrap();
    let h1_op = alap.get(h1).unwrap();

    assert_eq!(r0_op.start_time, 0.0);
    assert_eq!(r0_op.end_time, 500.0);
    assert_eq!(h0_op.start_time, 500.0);
    assert_eq!(h0_op.end_time, 530.0);

    // h1 on q1 has duration 30ns and ends at 530ns (ALAP makespan)
    assert_eq!(h1_op.start_time, 500.0);
    assert_eq!(h1_op.end_time, 530.0);
    assert_eq!(h1_op.slack(), Some(500.0));

    let report = ScheduleValidator::new().validate(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());
}
