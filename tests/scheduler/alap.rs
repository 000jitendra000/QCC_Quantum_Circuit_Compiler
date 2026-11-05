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

#[test]
fn test_adversarial_4_operation_counterexample_resolved() {
    // Exact 4-operation adversarial counterexample from Phase 56 audit:
    // gate_0: 80 ns, requires R2
    // gate_1: 20 ns, requires both R1 and R2
    // gate_2: 80 ns, requires both R1 and R2
    // gate_7: 60 ns, requires both R1 and R2
    // DAG dependencies on Q0: gate_0 -> gate_2 -> gate_7
    // gate_1 on Q1 is independent in the DAG
    // Requested makespan: 240 ns
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("gate_0", OperationDuration::known_ns(80.0));
    timing.set_default_duration("gate_1", OperationDuration::known_ns(20.0));
    timing.set_default_duration("gate_2", OperationDuration::known_ns(80.0));
    timing.set_default_duration("gate_7", OperationDuration::known_ns(60.0));

    let r1 = ResourceId::Custom("R1".to_string());
    let r2 = ResourceId::Custom("R2".to_string());

    let mut resources = ResourceModel::new();
    resources.set_capacity(r1.clone(), 1);
    resources.set_capacity(r2.clone(), 1);
    resources.set_default_requirements("gate_0", vec![ResourceRequirement::exclusive(r2.clone())]);
    resources.set_default_requirements(
        "gate_1",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r2.clone()),
        ],
    );
    resources.set_default_requirements(
        "gate_2",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r2.clone()),
        ],
    );
    resources.set_default_requirements(
        "gate_7",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r2.clone()),
        ],
    );

    let g0_desc = TargetOperationBuilder::new("gate_0", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(80.0)
        .build();
    let g1_desc = TargetOperationBuilder::new("gate_1", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(20.0)
        .build();
    let g2_desc = TargetOperationBuilder::new("gate_2", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(80.0)
        .build();
    let g7_desc = TargetOperationBuilder::new("gate_7", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(60.0)
        .build();

    let target = TargetBuilder::new("counterexample_target", 2)
        .add_operation(g0_desc)
        .add_operation(g1_desc)
        .add_operation(g2_desc)
        .add_operation(g7_desc)
        .timing(timing)
        .resources(resources)
        .build()
        .expect("target construction must succeed");

    let mut builder = CircuitBuilder::new("adversarial_4_op");
    let qreg = builder.add_quantum_register("q", 2).unwrap();

    // Create operations in the counterexample order:
    // gate_0 on Q0 (OpId 1)
    // gate_1 on Q1 (OpId 2) - independent in DAG
    // gate_2 on Q0 (OpId 3) - dependent on gate_0
    // gate_7 on Q0 (OpId 4) - dependent on gate_2
    let id_g0 = builder.custom_gate("gate_0", &[qreg[0]], vec![]).unwrap();
    let id_g1 = builder.custom_gate("gate_1", &[qreg[1]], vec![]).unwrap();
    let id_g2 = builder.custom_gate("gate_2", &[qreg[0]], vec![]).unwrap();
    let id_g7 = builder.custom_gate("gate_7", &[qreg[0]], vec![]).unwrap();

    let circuit = builder.finish().unwrap();

    // 1. Verify ASAP baseline
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(asap.makespan(), 240.0);
    assert_eq!(asap.get(id_g0).unwrap().start_time, 0.0);
    assert_eq!(asap.get(id_g1).unwrap().start_time, 80.0);
    assert_eq!(asap.get(id_g2).unwrap().start_time, 100.0);
    assert_eq!(asap.get(id_g7).unwrap().start_time, 180.0);

    // 2. Schedule under corrected ALAP with exact makespan 240ns
    let alap = AlapScheduler::new()
        .with_makespan(240.0)
        .schedule(&circuit, &target)
        .expect("corrected ALAP must succeed without negative slack");

    assert_eq!(alap.makespan(), 240.0);

    let op_g0 = alap.get(id_g0).unwrap();
    let op_g1 = alap.get(id_g1).unwrap();
    let op_g2 = alap.get(id_g2).unwrap();
    let op_g7 = alap.get(id_g7).unwrap();

    // Under RC-CPM resource order preservation:
    // gate_1 preserves precedence over gate_2 on R1, R2
    assert_eq!(op_g0.start_time, 0.0);
    assert_eq!(op_g0.end_time, 80.0);
    assert_eq!(op_g0.slack(), Some(0.0));

    assert_eq!(op_g1.start_time, 80.0);
    assert_eq!(op_g1.end_time, 100.0);
    assert_eq!(op_g1.slack(), Some(0.0));

    assert_eq!(op_g2.start_time, 100.0);
    assert_eq!(op_g2.end_time, 180.0);
    assert_eq!(op_g2.slack(), Some(0.0));

    assert_eq!(op_g7.start_time, 180.0);
    assert_eq!(op_g7.end_time, 240.0);
    assert_eq!(op_g7.slack(), Some(0.0));

    // 3. Independent validator verification (both physical and ALAP slack)
    let validator = ScheduleValidator::new();
    let phys_report = validator.validate(&alap, &circuit, &target);
    assert!(phys_report.is_valid(), "{}", phys_report.summary());

    let alap_report = validator.validate_alap(&alap, &circuit, &target);
    assert!(alap_report.is_valid(), "{}", alap_report.summary());

    // 4. Extended makespan 300ns (+60ns slack window)
    let alap_300 = AlapScheduler::new()
        .with_makespan(300.0)
        .schedule(&circuit, &target)
        .expect("ALAP with extended makespan must succeed");

    assert_eq!(alap_300.makespan(), 300.0);
    let alap_report_300 = validator.validate_alap(&alap_300, &circuit, &target);
    assert!(alap_report_300.is_valid(), "{}", alap_report_300.summary());

    for op in alap_300.operations() {
        assert!(
            op.slack().unwrap() >= 0.0,
            "op#{} had negative slack {}",
            op.id,
            op.slack().unwrap()
        );
    }
}

#[test]
fn test_adversarial_shared_exclusive_resources() {
    // Multi-resource competition:
    // Q0 has chain A1 (40ns) -> A2 (60ns), both use exclusive DriveLine(0)
    // Q1 has chain B1 (50ns) -> B2 (70ns), both use exclusive DriveLine(1)
    // Q2 has independent C1 (30ns), uses both DriveLine(0) and DriveLine(1)
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("a1", OperationDuration::known_ns(40.0));
    timing.set_default_duration("a2", OperationDuration::known_ns(60.0));
    timing.set_default_duration("b1", OperationDuration::known_ns(50.0));
    timing.set_default_duration("b2", OperationDuration::known_ns(70.0));
    timing.set_default_duration("c1", OperationDuration::known_ns(30.0));

    let dl0 = ResourceId::DriveLine(0);
    let dl1 = ResourceId::DriveLine(1);

    let mut resources = ResourceModel::new();
    resources.set_capacity(dl0.clone(), 1);
    resources.set_capacity(dl1.clone(), 1);
    resources.set_default_requirements("a1", vec![ResourceRequirement::exclusive(dl0.clone())]);
    resources.set_default_requirements("a2", vec![ResourceRequirement::exclusive(dl0.clone())]);
    resources.set_default_requirements("b1", vec![ResourceRequirement::exclusive(dl1.clone())]);
    resources.set_default_requirements("b2", vec![ResourceRequirement::exclusive(dl1.clone())]);
    resources.set_default_requirements(
        "c1",
        vec![
            ResourceRequirement::exclusive(dl0.clone()),
            ResourceRequirement::exclusive(dl1.clone()),
        ],
    );

    let target = TargetBuilder::new("multi_resource_target", 3)
        .add_operation(
            TargetOperationBuilder::new("a1", 1)
                .connectivity(OperationConnectivity::all_single_qubits(3))
                .duration(40.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("a2", 1)
                .connectivity(OperationConnectivity::all_single_qubits(3))
                .duration(60.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("b1", 1)
                .connectivity(OperationConnectivity::all_single_qubits(3))
                .duration(50.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("b2", 1)
                .connectivity(OperationConnectivity::all_single_qubits(3))
                .duration(70.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("c1", 1)
                .connectivity(OperationConnectivity::all_single_qubits(3))
                .duration(30.0)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("multi_compete");
    let qreg = builder.add_quantum_register("q", 3).unwrap();
    builder.custom_gate("a1", &[qreg[0]], vec![]).unwrap();
    builder.custom_gate("a2", &[qreg[0]], vec![]).unwrap();
    builder.custom_gate("b1", &[qreg[1]], vec![]).unwrap();
    builder.custom_gate("b2", &[qreg[1]], vec![]).unwrap();
    builder.custom_gate("c1", &[qreg[2]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    let scheduler = AlapScheduler::new();
    let schedule = scheduler.schedule(&circuit, &target).unwrap();

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&schedule, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    for op in schedule.operations() {
        assert!(
            op.slack().unwrap() >= 0.0,
            "operation {} on {:?} has negative slack {}",
            op.name,
            op.operands,
            op.slack().unwrap()
        );
    }
}

#[test]
fn test_independent_validator_detects_missing_slack() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("missing_slack_test");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap();
    builder.h(qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // 1. Valid schedule passes ALAP validation
    let report = ScheduleValidator::new().validate_alap(&schedule, &circuit, &target);
    assert!(report.is_valid());

    // 2. Strip slack from operation 0
    schedule.operations[0].slack = None;

    // ALAP validator MUST reject missing slack
    let alap_report = ScheduleValidator::new().validate_alap(&schedule, &circuit, &target);
    assert!(!alap_report.is_valid());
    assert!(alap_report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::MissingSlack { .. })));

    // Decoupling: physical schedule validation without ALAP slack checks still passes
    let phys_report = ScheduleValidator::new().validate(&schedule, &circuit, &target);
    assert!(phys_report.is_valid());
}

#[test]
fn test_independent_validator_detects_slack_mismatch_and_forgery() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("forgery_test");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let x0 = builder.x(qreg[0]).unwrap();
    builder.cx(qreg[0], qreg[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // Forge positive slack: replace legitimate slack with 999.0ns
    for op in &mut schedule.operations {
        if op.id == x0 {
            op.slack = Some(999.0);
        }
    }

    let report = ScheduleValidator::new().validate_alap(&schedule, &circuit, &target);
    assert!(!report.is_valid());
    let mismatch = report.violations.iter().find(|v| {
        matches!(
            v,
            ScheduleValidationError::SlackMismatch {
                op_id,
                actual_slack_ns,
                ..
            } if *op_id == x0 && (*actual_slack_ns - 999.0).abs() < 1e-9
        )
    });
    assert!(
        mismatch.is_some(),
        "validator must detect forged positive slack: {:?}",
        report.violations
    );
}

#[test]
fn test_independent_validator_detects_forged_negative_slack() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("forged_neg_slack_test");
    let qreg = builder.add_quantum_register("q", 1).unwrap();
    let x0 = builder.x(qreg[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();

    // Adversarially attach negative slack
    for op in &mut schedule.operations {
        if op.id == x0 {
            op.slack = Some(-15.0);
        }
    }

    // Both physical validator and ALAP validator must reject negative slack
    let phys_report = ScheduleValidator::new().validate(&schedule, &circuit, &target);
    assert!(!phys_report.is_valid());
    assert!(phys_report
        .violations
        .iter()
        .any(|v| matches!(v, ScheduleValidationError::NegativeSlack { op_id, slack_ns } if *op_id == x0 && (*slack_ns - (-15.0)).abs() < 1e-9)));

    let alap_report = ScheduleValidator::new().validate_alap(&schedule, &circuit, &target);
    assert!(!alap_report.is_valid());
    assert!(alap_report.violations.iter().any(
        |v| matches!(v, ScheduleValidationError::NegativeSlack { op_id, .. } if *op_id == x0)
    ));
}

#[test]
fn test_infeasible_makespan_vs_algorithmic_failure() {
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("infeasible_test");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    builder.x(qreg[0]).unwrap(); // 30ns
    builder.cx(qreg[0], qreg[1]).unwrap(); // 200ns
    let circuit = builder.finish().unwrap();

    // ASAP makespan is 230ns
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(asap.makespan(), 230.0);

    // Truly mathematically infeasible makespan: 200ns < 230ns minimum makespan
    let err = AlapScheduler::new()
        .with_makespan(200.0)
        .schedule(&circuit, &target)
        .unwrap_err();

    assert!(
        matches!(
            err,
            ScheduleError::InfeasibleMakespan {
                requested_ns,
                minimum_ns
            } if (requested_ns - 200.0).abs() < 1e-9 && (minimum_ns - 230.0).abs() < 1e-9
        ),
        "expected InfeasibleMakespan, got {err:?}"
    );
}

#[test]
fn test_aggregate_shared_capacity_three_independent_operations() {
    // 1. Audit case: At least 3 independent operations sharing a resource of capacity 10,
    // each requiring 4 units. Pairwise requirements (4+4=8 <= 10) do not exceed capacity,
    // but combined demand (4+4+4=12 > 10) does.
    // Also includes an operation requiring multiple resources (shared bus + exclusive drive line).
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("gate_a", OperationDuration::known_ns(30.0));
    timing.set_default_duration("gate_b", OperationDuration::known_ns(50.0));
    timing.set_default_duration("gate_c", OperationDuration::known_ns(40.0));
    timing.set_default_duration("gate_d", OperationDuration::known_ns(35.0));

    let bus = ResourceId::Custom("SharedBus".to_string());
    let dl0 = ResourceId::DriveLine(0);

    let mut resources = ResourceModel::new();
    resources.set_capacity(bus.clone(), 10);
    resources.set_capacity(dl0.clone(), 1);

    resources.set_default_requirements("gate_a", vec![ResourceRequirement::shared(bus.clone(), 4)]);
    resources.set_default_requirements("gate_b", vec![ResourceRequirement::shared(bus.clone(), 4)]);
    resources.set_default_requirements("gate_c", vec![ResourceRequirement::shared(bus.clone(), 4)]);
    resources.set_default_requirements(
        "gate_d",
        vec![
            ResourceRequirement::shared(bus.clone(), 4),
            ResourceRequirement::exclusive(dl0.clone()),
        ],
    );

    let target = TargetBuilder::new("aggregate_capacity_target", 4)
        .add_operation(
            TargetOperationBuilder::new("gate_a", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(30.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_b", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(50.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_c", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(40.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_d", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(35.0)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("aggregate_capacity_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let id_a = builder.custom_gate("gate_a", &[qreg[0]], vec![]).unwrap();
    let id_b = builder.custom_gate("gate_b", &[qreg[1]], vec![]).unwrap();
    let id_c = builder.custom_gate("gate_c", &[qreg[2]], vec![]).unwrap();
    let id_d = builder.custom_gate("gate_d", &[qreg[3]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    // 2. ASAP scheduling
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    let validator = ScheduleValidator::new();
    let asap_report = validator.validate(&asap, &circuit, &target);
    assert!(asap_report.is_valid(), "{}", asap_report.summary());

    // Verify aggregate usage never exceeds capacity 10 in ASAP
    for op in asap.operations() {
        assert!(op.start_time >= 0.0);
    }

    // 3. ALAP scheduling with derived makespan
    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(alap.makespan(), asap.makespan());

    let alap_report = validator.validate_alap(&alap, &circuit, &target);
    assert!(alap_report.is_valid(), "{}", alap_report.summary());

    // Verify all slacks are non-negative
    for op in alap.operations() {
        assert!(
            op.slack().unwrap() >= -1e-9,
            "operation {} has negative slack {}",
            op.name,
            op.slack().unwrap()
        );
    }

    // 4. ALAP scheduling with extended makespan (+60ns window)
    let extended_makespan = asap.makespan() + 60.0;
    let alap_ext = AlapScheduler::new()
        .with_makespan(extended_makespan)
        .schedule(&circuit, &target)
        .unwrap();
    assert_eq!(alap_ext.makespan(), extended_makespan);

    let alap_ext_report = validator.validate_alap(&alap_ext, &circuit, &target);
    assert!(alap_ext_report.is_valid(), "{}", alap_ext_report.summary());

    // Verify no invalid overlap (aggregate capacity <= 10 at all times)
    for t in 0..=(extended_makespan as usize) {
        let time = t as f64;
        let mut bus_usage = 0;
        for op in alap_ext.operations() {
            if op.start_time <= time && time < op.end_time {
                bus_usage += 4;
            }
        }
        assert!(
            bus_usage <= 10,
            "aggregate capacity 10 exceeded at {time}ns: usage={bus_usage}"
        );
    }

    assert!(alap.get(id_a).is_some());
    assert!(alap.get(id_b).is_some());
    assert!(alap.get(id_c).is_some());
    assert!(alap.get(id_d).is_some());
}

#[test]
fn test_augmented_dependency_graph_zero_duration_and_equal_timestamps() {
    // 2. Audit case: Zero-duration operations, equal timestamps, deterministic tie-breaking,
    // and multiple shared resources to verify that adding ASAP-derived resource precedence
    // edges cannot create cycles.
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("zero_dur_augmented_graph");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    // Add multiple zero-duration rz gates and calibrated single-qubit gates
    // at identical starting timestamps across independent qubits
    let z0 = builder.rz(qreg[0], 0.1).unwrap();
    let z1 = builder.rz(qreg[1], 0.2).unwrap();
    let z2 = builder.rz(qreg[2], 0.3).unwrap();
    let z3 = builder.rz(qreg[3], 0.4).unwrap();

    let x0 = builder.x(qreg[0]).unwrap();
    let x1 = builder.x(qreg[1]).unwrap();
    let x2 = builder.x(qreg[2]).unwrap();
    let x3 = builder.x(qreg[3]).unwrap();

    let circuit = builder.finish().unwrap();

    // ALAP schedule must resolve without any cycle detection error
    let alap = AlapScheduler::new()
        .schedule(&circuit, &target)
        .expect("augmented graph must remain strictly acyclic");

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    // All operations must have non-negative slack
    for op in alap.operations() {
        assert!(op.slack().unwrap() >= -1e-9);
    }

    assert!(alap.get(z0).unwrap().duration == 0.0);
    assert!(alap.get(z1).unwrap().duration == 0.0);
    assert!(alap.get(z2).unwrap().duration == 0.0);
    assert!(alap.get(z3).unwrap().duration == 0.0);
    assert!(alap.get(x0).unwrap().duration == 30.0);
    assert!(alap.get(x1).unwrap().duration == 30.0);
    assert!(alap.get(x2).unwrap().duration == 30.0);
    assert!(alap.get(x3).unwrap().duration == 30.0);
}

#[test]
fn test_tampered_alap_start_time_detected_by_independent_validator() {
    // 3. Audit case: Tampered ALAP start time that makes recorded slack incorrect,
    // verifying that validate_alap independently checks start_time against the ASAP baseline.
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("tampered_start_time_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let x0 = builder.x(qreg[0]).unwrap(); // 30ns
    let cx = builder.cx(qreg[0], qreg[1]).unwrap(); // 200ns
    let circuit = builder.finish().unwrap();

    let mut schedule = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    let validator = ScheduleValidator::new();

    // Baseline ALAP schedule is valid
    let report = validator.validate_alap(&schedule, &circuit, &target);
    assert!(report.is_valid());

    // Scenario A: Tamper start_time forward (e.g. shift x0 start from 0ns to 10ns),
    // but leave recorded slack untouched as Some(0.0).
    // Expected slack becomes 10 - 0 = 10ns, which does not match recorded 0ns.
    for op in &mut schedule.operations {
        if op.id == x0 {
            op.start_time += 10.0;
            op.end_time += 10.0;
        }
    }
    let report_a = validator.validate_alap(&schedule, &circuit, &target);
    assert!(!report_a.is_valid());
    assert!(
        report_a.violations.iter().any(|v| matches!(
            v,
            ScheduleValidationError::SlackMismatch { op_id, .. } if *op_id == x0
        )),
        "validator must detect slack mismatch from tampered start time: {:?}",
        report_a.violations
    );

    // Scenario B: Tamper start_time backwards (earlier than ASAP start),
    // while forging recorded slack to 0.0.
    // For cx, ASAP start is 30ns. Tamper cx start to 10ns (< 30ns).
    let mut schedule_b = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    for op in &mut schedule_b.operations {
        if op.id == cx {
            op.start_time = 10.0;
            op.end_time = 210.0;
            op.slack = Some(0.0); // Forged zero slack
        }
    }
    let report_b = validator.validate_alap(&schedule_b, &circuit, &target);
    assert!(!report_b.is_valid());
    assert!(
        report_b.violations.iter().any(|v| matches!(
            v,
            ScheduleValidationError::SlackMismatch { op_id, .. } if *op_id == cx
        )),
        "validator must detect slack mismatch when start precedes ASAP start: {:?}",
        report_b.violations
    );
    assert!(
        report_b.violations.iter().any(|v| matches!(
            v,
            ScheduleValidationError::NegativeSlack { op_id, .. } if *op_id == cx
        )),
        "validator must detect negative slack when start precedes ASAP start: {:?}",
        report_b.violations
    );
}

#[test]
fn test_augmented_graph_positive_predecessor_zero_duration_successor_touching_boundary() {
    // Audit regression test:
    // 1. Positive-duration predecessor u (30ns, [0, 30)) followed by zero-duration successor v1 (0ns, [30, 30))
    //    at the exact same completion timestamp (end(u) == end(v1) == 30.0ns).
    // 2. Chained zero-duration successor v2 (0ns, [30, 30)) following v1 at the same timestamp.
    // 3. Positive-duration successor w (30ns, [30, 60)) following v2.
    // 4. Exact boundary touching under half-open interval semantics [0, 30), [30, 30), [30, 60).
    // 5. Verification that ALAP reverse traversal orders (w, v2, v1, u) correctly, remains acyclic,
    //    assigns exact non-negative slacks, and passes independent validate_alap.
    let target = make_test_target_4q();
    let mut builder = CircuitBuilder::new("positive_pred_zero_succ_boundary");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    let u = builder.x(qreg[0]).unwrap(); // 30ns
    let v1 = builder.rz(qreg[0], 0.1).unwrap(); // 0ns virtual gate
    let v2 = builder.rz(qreg[0], 0.2).unwrap(); // 0ns virtual gate
    let w = builder.x(qreg[0]).unwrap(); // 30ns

    // Parallel 2-qubit gate on Q2, Q3 with 200ns duration defining makespan = 200ns
    let cx = builder.cx(qreg[2], qreg[3]).unwrap();

    let circuit = builder.finish().unwrap();

    // 1. ASAP baseline verification
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(asap.get(u).unwrap().start_time, 0.0);
    assert_eq!(asap.get(u).unwrap().end_time, 30.0);
    assert_eq!(asap.get(v1).unwrap().start_time, 30.0);
    assert_eq!(asap.get(v1).unwrap().end_time, 30.0);
    assert_eq!(asap.get(v2).unwrap().start_time, 30.0);
    assert_eq!(asap.get(v2).unwrap().end_time, 30.0);
    assert_eq!(asap.get(w).unwrap().start_time, 30.0);
    assert_eq!(asap.get(w).unwrap().end_time, 60.0);
    assert_eq!(asap.get(cx).unwrap().start_time, 0.0);
    assert_eq!(asap.get(cx).unwrap().end_time, 200.0);

    // End of positive-duration u equals end of zero-duration v1 and v2
    assert_eq!(
        asap.get(u).unwrap().end_time,
        asap.get(v1).unwrap().end_time
    );
    assert_eq!(
        asap.get(u).unwrap().end_time,
        asap.get(v2).unwrap().end_time
    );
    assert_eq!(asap.get(u).unwrap().duration, 30.0);
    assert_eq!(asap.get(v1).unwrap().duration, 0.0);

    // 2. ALAP scheduling
    let alap = AlapScheduler::new()
        .schedule(&circuit, &target)
        .expect("augmented graph must remain strictly acyclic");

    // With makespan = 200ns, available slack for Q0 operations is 200 - 60 = 140ns
    assert_eq!(alap.makespan(), 200.0);
    assert_eq!(alap.get(w).unwrap().start_time, 170.0);
    assert_eq!(alap.get(w).unwrap().end_time, 200.0);
    assert_eq!(alap.get(w).unwrap().slack().unwrap(), 140.0);

    assert_eq!(alap.get(v2).unwrap().start_time, 170.0);
    assert_eq!(alap.get(v2).unwrap().end_time, 170.0);
    assert_eq!(alap.get(v2).unwrap().slack().unwrap(), 140.0);

    assert_eq!(alap.get(v1).unwrap().start_time, 170.0);
    assert_eq!(alap.get(v1).unwrap().end_time, 170.0);
    assert_eq!(alap.get(v1).unwrap().slack().unwrap(), 140.0);

    assert_eq!(alap.get(u).unwrap().start_time, 140.0);
    assert_eq!(alap.get(u).unwrap().end_time, 170.0);
    assert_eq!(alap.get(u).unwrap().slack().unwrap(), 140.0);

    // Critical path operation (cx) has zero slack
    assert_eq!(alap.get(cx).unwrap().start_time, 0.0);
    assert_eq!(alap.get(cx).unwrap().slack().unwrap(), 0.0);

    // 3. Independent validator verification
    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    // 4. Adversarial check: shifting zero-duration v1 inside positive-duration u interval [140, 170)
    // causes an overlap violation on qubit 0
    let mut tampered = alap.clone();
    for op in &mut tampered.operations {
        if op.id == v1 {
            op.start_time = 150.0;
            op.end_time = 150.0;
        }
    }
    let tampered_report = validator.validate(&tampered, &circuit, &target);
    assert!(!tampered_report.is_valid());
    assert!(
        tampered_report.violations.iter().any(|v| matches!(
            v,
            ScheduleValidationError::QubitCollision { qubit, .. } if *qubit == PhysicalQubitId(0)
        )),
        "zero-duration gate inside occupied interval must be rejected as QubitCollision"
    );
}

#[test]
fn test_adversarial_sub_epsilon_durations_and_chain_acyclicity() {
    // Adversarial audit case 1 & 4:
    // - Positive durations smaller than epsilon: d = 0.2ns < epsilon (1.0ns).
    // - A chain of 5 operations competing for an exclusive hardware resource.
    // - Total chain span = 5 * 0.2ns = 1.0ns <= epsilon.
    // - Under epsilon-based precedence (u_end <= v_start + eps), op_5 would satisfy
    //   1.0 <= 0.0 + 1.0, creating an edge op_5 -> op_1 and a 5-cycle.
    // - Under exact ordering, op_5 -> op_1 is rejected because 1.0 > 0.0.
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    for name in &["sub_a", "sub_b", "sub_c", "sub_d", "sub_e"] {
        timing.set_default_duration(*name, OperationDuration::known_ns(0.2));
    }

    let shared_res = ResourceId::Custom("SubEpsExclusiveRes".to_string());
    let mut resources = ResourceModel::new();
    resources.set_capacity(shared_res.clone(), 1);
    for name in &["sub_a", "sub_b", "sub_c", "sub_d", "sub_e"] {
        resources.set_default_requirements(
            *name,
            vec![ResourceRequirement::exclusive(shared_res.clone())],
        );
    }

    let target = TargetBuilder::new("sub_eps_chain_target", 5)
        .add_operation(
            TargetOperationBuilder::new("sub_a", 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .duration(0.2)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("sub_b", 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .duration(0.2)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("sub_c", 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .duration(0.2)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("sub_d", 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .duration(0.2)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("sub_e", 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .duration(0.2)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("sub_eps_chain_circuit");
    let qreg = builder.add_quantum_register("q", 5).unwrap();
    let id_a = builder.custom_gate("sub_a", &[qreg[0]], vec![]).unwrap();
    let id_b = builder.custom_gate("sub_b", &[qreg[1]], vec![]).unwrap();
    let id_c = builder.custom_gate("sub_c", &[qreg[2]], vec![]).unwrap();
    let id_d = builder.custom_gate("sub_d", &[qreg[3]], vec![]).unwrap();
    let id_e = builder.custom_gate("sub_e", &[qreg[4]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    // 1. ASAP schedule
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert!((asap.makespan() - 1.0).abs() <= 1e-12);

    // 2. ALAP schedule with derived makespan (must not cycle)
    let alap = AlapScheduler::new()
        .schedule(&circuit, &target)
        .expect("exact ordering must remain strictly acyclic for sub-epsilon chains");
    assert!((alap.makespan() - 1.0).abs() <= 1e-12);

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    for op in alap.operations() {
        assert!(op.slack().unwrap() >= -1e-9);
    }

    // 3. ALAP schedule with extended makespan (+5.0ns window)
    let alap_ext = AlapScheduler::new()
        .with_makespan(6.0)
        .schedule(&circuit, &target)
        .unwrap();
    let report_ext = validator.validate_alap(&alap_ext, &circuit, &target);
    assert!(report_ext.is_valid(), "{}", report_ext.summary());

    assert!(alap.get(id_a).is_some());
    assert!(alap.get(id_b).is_some());
    assert!(alap.get(id_c).is_some());
    assert!(alap.get(id_d).is_some());
    assert!(alap.get(id_e).is_some());
}

#[test]
fn test_adversarial_operations_separated_by_less_than_epsilon() {
    // Adversarial audit case 2:
    // - Two independent operations u on Q0 and v on Q1 sharing an exclusive resource.
    // - Predecessor w on Q1 forces v to start at 10.0 + 1e-10 ns.
    // - u completes at 10.0 ns.
    // - Separation Delta = 1e-10 ns is strictly positive but < epsilon (1.0 ns).
    // - Exact ordering must recognize u_end < v_start (10.0 < 10.0000000001)
    //   and NOT create an inverted precedence.
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("gate_u", OperationDuration::known_ns(10.0));
    timing.set_default_duration("gate_w", OperationDuration::known_ns(10.0000000001));
    timing.set_default_duration("gate_v", OperationDuration::known_ns(10.0));

    let excl_laser = ResourceId::Custom("ExclLaser".to_string());
    let mut resources = ResourceModel::new();
    resources.set_capacity(excl_laser.clone(), 1);
    resources.set_default_requirements(
        "gate_u",
        vec![ResourceRequirement::exclusive(excl_laser.clone())],
    );
    resources.set_default_requirements("gate_w", vec![]);
    resources.set_default_requirements(
        "gate_v",
        vec![ResourceRequirement::exclusive(excl_laser.clone())],
    );

    let target = TargetBuilder::new("sub_eps_sep_target", 2)
        .add_operation(
            TargetOperationBuilder::new("gate_u", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .duration(10.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_w", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .duration(10.0000000001)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_v", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .duration(10.0)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("sub_eps_sep_circuit");
    let qreg = builder.add_quantum_register("q", 2).unwrap();
    let id_u = builder.custom_gate("gate_u", &[qreg[0]], vec![]).unwrap();
    let id_w = builder.custom_gate("gate_w", &[qreg[1]], vec![]).unwrap();
    let id_v = builder.custom_gate("gate_v", &[qreg[1]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    // In ASAP:
    // u runs on Q0: [0.0, 10.0)
    // w runs on Q1: [0.0, 10.0000000001)
    // v runs on Q1: [10.0000000001, 20.0000000001)
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert_eq!(asap.get(id_u).unwrap().end_time, 10.0);
    assert_eq!(asap.get(id_v).unwrap().start_time, 10.0000000001);

    // In ALAP: must resolve without cycles or inverted edges
    let alap = AlapScheduler::new().schedule(&circuit, &target).unwrap();
    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    for op in alap.operations() {
        assert!(op.slack().unwrap() >= -1e-9);
    }

    assert!(alap.get(id_u).is_some());
    assert!(alap.get(id_w).is_some());
    assert!(alap.get(id_v).is_some());
}

#[test]
fn test_adversarial_zero_duration_operations_equal_and_near_equal_timestamps() {
    // Adversarial audit case 3:
    // - Multiple instantaneous operations:
    //   z1, z2, z3 at identical timestamps T = 30.0ns.
    //   z4 at near-equal timestamp T = 30.0000000001ns (< epsilon separation).
    let mut builder = CircuitBuilder::new("zero_dur_near_equal_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    let _x0 = builder.x(qreg[0]).unwrap(); // 30ns
    let z1 = builder.rz(qreg[0], 0.1).unwrap(); // 0ns at 30.0ns
    let z2 = builder.rz(qreg[0], 0.2).unwrap(); // 0ns at 30.0ns
    let z3 = builder.rz(qreg[0], 0.3).unwrap(); // 0ns at 30.0ns

    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("x", OperationDuration::known_ns(30.0));
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0));
    timing.set_default_duration("near_x", OperationDuration::known_ns(30.0000000001));

    let mut resources = ResourceModel::new();
    resources.set_default_requirements("x", vec![]);
    resources.set_default_requirements("rz", vec![]);
    resources.set_default_requirements("near_x", vec![]);

    let custom_target = TargetBuilder::new("near_equal_target", 4)
        .add_operation(
            TargetOperationBuilder::new("x", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(30.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("rz", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(0.0)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("near_x", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(30.0000000001)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let _near_x = builder.custom_gate("near_x", &[qreg[1]], vec![]).unwrap();
    let z4 = builder.rz(qreg[1], 0.4).unwrap(); // 0ns at 30.0000000001ns

    let circuit = builder.finish().unwrap();

    let alap = AlapScheduler::new()
        .schedule(&circuit, &custom_target)
        .expect("exact ordering must handle equal and near-equal zero duration gates");

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &custom_target);
    assert!(report.is_valid(), "{}", report.summary());

    assert!(alap.get(z1).is_some());
    assert!(alap.get(z2).is_some());
    assert!(alap.get(z3).is_some());
    assert!(alap.get(z4).is_some());
}

#[test]
fn test_adversarial_multiple_resources_conflicting_ordering_constraints() {
    // Adversarial audit case 5:
    // Multiple resources imposing cross-resource ordering constraints:
    // op_a requires R1 and R2 (0.3ns)
    // op_b requires R2 and R3 (0.4ns)
    // op_c requires R1 and R3 (0.3ns)
    // op_d requires R1, R2, and R3 (0.2ns)
    // All durations are sub-epsilon (< 1.0ns).
    let r1 = ResourceId::Custom("Res1".to_string());
    let r2 = ResourceId::Custom("Res2".to_string());
    let r3 = ResourceId::Custom("Res3".to_string());

    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("op_a", OperationDuration::known_ns(0.3));
    timing.set_default_duration("op_b", OperationDuration::known_ns(0.4));
    timing.set_default_duration("op_c", OperationDuration::known_ns(0.3));
    timing.set_default_duration("op_d", OperationDuration::known_ns(0.2));

    let mut resources = ResourceModel::new();
    resources.set_capacity(r1.clone(), 1);
    resources.set_capacity(r2.clone(), 1);
    resources.set_capacity(r3.clone(), 1);

    resources.set_default_requirements(
        "op_a",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r2.clone()),
        ],
    );
    resources.set_default_requirements(
        "op_b",
        vec![
            ResourceRequirement::exclusive(r2.clone()),
            ResourceRequirement::exclusive(r3.clone()),
        ],
    );
    resources.set_default_requirements(
        "op_c",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r3.clone()),
        ],
    );
    resources.set_default_requirements(
        "op_d",
        vec![
            ResourceRequirement::exclusive(r1.clone()),
            ResourceRequirement::exclusive(r2.clone()),
            ResourceRequirement::exclusive(r3.clone()),
        ],
    );

    let target = TargetBuilder::new("multi_resource_conflict_target", 4)
        .add_operation(
            TargetOperationBuilder::new("op_a", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(0.3)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("op_b", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(0.4)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("op_c", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(0.3)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("op_d", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(0.2)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("multi_resource_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();
    let a = builder.custom_gate("op_a", &[qreg[0]], vec![]).unwrap();
    let b = builder.custom_gate("op_b", &[qreg[1]], vec![]).unwrap();
    let c = builder.custom_gate("op_c", &[qreg[2]], vec![]).unwrap();
    let d = builder.custom_gate("op_d", &[qreg[3]], vec![]).unwrap();
    let circuit = builder.finish().unwrap();

    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    assert!(asap.makespan() > 0.0);

    let alap = AlapScheduler::new()
        .schedule(&circuit, &target)
        .expect("multiple cross-resource conflicts must remain acyclic in ALAP");

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    for op in alap.operations() {
        assert!(op.slack().unwrap() >= -1e-9);
    }

    assert!(alap.get(a).is_some());
    assert!(alap.get(b).is_some());
    assert!(alap.get(c).is_some());
    assert!(alap.get(d).is_some());
}

#[test]
fn test_adversarial_signed_zero_representations_acyclicity_and_tie_breaking() {
    // Adversarial signed-zero test:
    // - Operations with duration -0.0 and +0.0 competing for exclusive resources at T = 0.0.
    // - Verifies that:
    //   1. exact_cmp normalizes -0.0 and +0.0 so that signed-zero does NOT distort precedence.
    //   2. Tie-breaking by deterministic sequence number is preserved.
    //   3. Augmented graph remains strictly acyclic.
    //   4. Stored start_time, duration, and slack are canonicalized to positive zero (+0.0).
    let neg_zero: f64 = -0.0;
    let pos_zero: f64 = 0.0;

    let res1 = ResourceId::Custom("SignedZeroRes1".to_string());
    let res2 = ResourceId::Custom("SignedZeroRes2".to_string());

    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("z_neg", OperationDuration::known_ns(neg_zero));
    timing.set_default_duration("z_pos", OperationDuration::known_ns(pos_zero));
    timing.set_default_duration("gate_x", OperationDuration::known_ns(30.0));

    let mut resources = ResourceModel::new();
    resources.set_capacity(res1.clone(), 1);
    resources.set_capacity(res2.clone(), 1);

    resources.set_default_requirements(
        "z_neg",
        vec![
            ResourceRequirement::exclusive(res1.clone()),
            ResourceRequirement::exclusive(res2.clone()),
        ],
    );
    resources.set_default_requirements(
        "z_pos",
        vec![
            ResourceRequirement::exclusive(res1.clone()),
            ResourceRequirement::exclusive(res2.clone()),
        ],
    );
    resources.set_default_requirements("gate_x", vec![]);

    let target = TargetBuilder::new("signed_zero_target", 4)
        .add_operation(
            TargetOperationBuilder::new("z_neg", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(neg_zero)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("z_pos", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(pos_zero)
                .build(),
        )
        .add_operation(
            TargetOperationBuilder::new("gate_x", 1)
                .connectivity(OperationConnectivity::all_single_qubits(4))
                .duration(30.0)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build()
        .unwrap();

    let mut builder = CircuitBuilder::new("signed_zero_circuit");
    let qreg = builder.add_quantum_register("q", 4).unwrap();

    // Four competing zero-duration gates with alternating -0.0 and +0.0 representations
    let op0 = builder.custom_gate("z_neg", &[qreg[0]], vec![]).unwrap();
    let op1 = builder.custom_gate("z_pos", &[qreg[1]], vec![]).unwrap();
    let op2 = builder.custom_gate("z_neg", &[qreg[2]], vec![]).unwrap();
    let op3 = builder.custom_gate("z_pos", &[qreg[3]], vec![]).unwrap();

    // Trailing positive-duration gates to define non-zero makespan
    let _x0 = builder.custom_gate("gate_x", &[qreg[0]], vec![]).unwrap();
    let _x1 = builder.custom_gate("gate_x", &[qreg[1]], vec![]).unwrap();

    let circuit = builder.finish().unwrap();

    // 1. ASAP baseline must not produce negative zero
    let asap = AsapScheduler::new().schedule(&circuit, &target).unwrap();
    for op in asap.operations() {
        assert!(
            !op.start_time.is_sign_negative() || op.start_time == 0.0,
            "ASAP start_time must not be -0.0: got {}",
            op.start_time
        );
        assert!(
            !op.duration.is_sign_negative() || op.duration == 0.0,
            "ASAP duration must not be -0.0"
        );
    }

    // 2. ALAP scheduler must remain strictly acyclic and preserve deterministic sequence order
    let alap = AlapScheduler::new()
        .schedule(&circuit, &target)
        .expect("signed-zero representations must remain strictly acyclic");

    for op in alap.operations() {
        assert!(
            !op.start_time.is_sign_negative() || op.start_time == 0.0,
            "ALAP start_time must not be -0.0"
        );
        if let Some(slack) = op.slack() {
            assert!(
                !slack.is_sign_negative() || slack == 0.0,
                "ALAP slack must not be -0.0"
            );
        }
    }

    let validator = ScheduleValidator::new();
    let report = validator.validate_alap(&alap, &circuit, &target);
    assert!(report.is_valid(), "{}", report.summary());

    assert!(alap.get(op0).is_some());
    assert!(alap.get(op1).is_some());
    assert!(alap.get(op2).is_some());
    assert!(alap.get(op3).is_some());
}

#[test]
fn test_adversarial_nan_and_infinite_timestamps_rejected() {
    // Adversarial NaN and Infinity test:
    // 1. Uncalibrated NaN/infinite duration is rejected at TargetBuilder::build boundary.
    // 2. Explicit NaN/infinite makespan in AlapScheduler is rejected.
    // 3. Tampered NaN/infinite slack is caught by ScheduleValidator.
    let mut timing = TimingModel::new();
    timing.clock_period_ns = Some(1.0);
    timing.set_default_duration("nan_gate", OperationDuration::known_ns(f64::NAN));
    timing.set_default_duration("inf_gate", OperationDuration::known_ns(f64::INFINITY));

    let mut resources = ResourceModel::new();
    resources.set_default_requirements("nan_gate", vec![]);
    resources.set_default_requirements("inf_gate", vec![]);

    // 1. TargetBuilder::build strictly rejects NaN and infinite durations
    let nan_target_res = TargetBuilder::new("nan_target", 2)
        .add_operation(
            TargetOperationBuilder::new("nan_gate", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .duration(f64::NAN)
                .build(),
        )
        .timing(timing.clone())
        .resources(resources.clone())
        .build();
    assert!(matches!(
        nan_target_res,
        Err(qcc::target::error::TargetError::InvalidDuration { .. })
    ));

    let inf_target_res = TargetBuilder::new("inf_target", 2)
        .add_operation(
            TargetOperationBuilder::new("inf_gate", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .duration(f64::INFINITY)
                .build(),
        )
        .timing(timing)
        .resources(resources)
        .build();
    assert!(matches!(
        inf_target_res,
        Err(qcc::target::error::TargetError::InvalidDuration { .. })
    ));

    // 2. Explicit NaN / Infinity makespan must be rejected
    let valid_target = make_test_target_4q();
    let mut builder_valid = CircuitBuilder::new("valid_circuit");
    let qreg_valid = builder_valid.add_quantum_register("q", 2).unwrap();
    let _x = builder_valid.x(qreg_valid[0]).unwrap();
    let circuit_valid = builder_valid.finish().unwrap();

    assert!(matches!(
        AlapScheduler::new()
            .with_makespan(f64::NAN)
            .schedule(&circuit_valid, &valid_target),
        Err(ScheduleError::Target(_))
    ));
    assert!(matches!(
        AlapScheduler::new()
            .with_makespan(f64::INFINITY)
            .schedule(&circuit_valid, &valid_target),
        Err(ScheduleError::Target(_))
    ));

    // 3. Validator rejects tampered NaN or Infinity slack
    let mut schedule = AlapScheduler::new()
        .schedule(&circuit_valid, &valid_target)
        .unwrap();
    for op in &mut schedule.operations {
        op.slack = Some(f64::NAN);
    }
    let validator = ScheduleValidator::new();
    let report_nan = validator.validate_alap(&schedule, &circuit_valid, &valid_target);
    assert!(!report_nan.is_valid());
    assert!(report_nan.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::InvalidTiming { reason, .. } if reason.contains("finite")
    )));

    for op in &mut schedule.operations {
        op.slack = Some(f64::INFINITY);
    }
    let report_inf = validator.validate_alap(&schedule, &circuit_valid, &valid_target);
    assert!(!report_inf.is_valid());
    assert!(report_inf.violations.iter().any(|v| matches!(
        v,
        ScheduleValidationError::InvalidTiming { reason, .. } if reason.contains("finite")
    )));
}
