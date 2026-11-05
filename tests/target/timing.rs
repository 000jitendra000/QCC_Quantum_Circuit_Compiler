//! Integration tests for Phase 54: Target Timing and Resource Model.
//!
//! Verifies:
//! 1. Explicit time and duration units with safe conversions.
//! 2. The Explicit Unknown Principle: unknown durations are preserved as `None` / `Unknown`, never zero.
//! 3. Operation resource requirements, target capacities, and conflict detection.
//! 4. Hardware concurrency constraints: disconnected gates cannot run concurrently if sharing resources.
//! 5. Measurement and reset timing profiles with multi-stage characterization and exclusion groups.
//! 6. Calibration provenance, ISO 8601 timestamp parsing, and freshness/staleness enforcement.
//! 7. Inconsistent, out-of-bounds, or invalid timing data validation and structured error reporting.
//! 8. Complete Serde JSON round-trip fidelity.

use qcc::target::{
    parse_timestamp_to_epoch_seconds, DurationUnit, HardwareDuration, MutualExclusionRule,
    OperationConnectivity, OperationDuration, PhysicalQubitId, Provenance, QubitCalibration,
    ReadoutTiming, ResetTiming, ResourceConflict, ResourceId, ResourceModel, ResourceRequirement,
    Target, TargetCalibration, TargetError, TargetOpId, TargetOperationDescriptor, TimingModel,
    TupleCalibration,
};

// ============================================================================
// Fixture Helpers
// ============================================================================

/// Creates a standard 4-qubit target fixture with 1Q and 2Q operations.
fn create_test_target_base() -> Target {
    let mut target = Target::new("test_target_4q", 4);
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let rz = TargetOperationDescriptor::builder("rz", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(0.0) // Virtual Z is known 0 ns
        .build();
    target.add_operation(rz);

    let sx = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(32.0) // Nominal 32 ns
        .build();
    target.add_operation(sx);

    let x = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(32.0)
        .build();
    target.add_operation(x);

    let cx = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit([
            (p0, p1),
            (p1, p2),
            (p2, p3),
        ]))
        .duration(280.0) // Nominal 280 ns
        .build();
    target.add_operation(cx);

    let measure = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(800.0) // Nominal 800 ns
        .build();
    target.add_operation(measure);

    let reset = TargetOperationDescriptor::builder("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .duration(600.0)
        .build();
    target.add_operation(reset);

    target
}

// ============================================================================
// 1. Time and Duration Units Tests
// ============================================================================

#[test]
fn test_duration_unit_conversions() {
    let p_ns = DurationUnit::Picoseconds
        .as_nanoseconds(2500.0, None)
        .unwrap();
    assert!((p_ns - 2.5).abs() < 1e-9);

    let ns = DurationUnit::Nanoseconds
        .as_nanoseconds(125.0, None)
        .unwrap();
    assert!((ns - 125.0).abs() < 1e-9);

    let us_ns = DurationUnit::Microseconds
        .as_nanoseconds(1.5, None)
        .unwrap();
    assert!((us_ns - 1500.0).abs() < 1e-9);

    let ms_ns = DurationUnit::Milliseconds
        .as_nanoseconds(2.0, None)
        .unwrap();
    assert!((ms_ns - 2_000_000.0).abs() < 1e-9);

    let s_ns = DurationUnit::Seconds.as_nanoseconds(0.001, None).unwrap();
    assert!((s_ns - 1_000_000.0).abs() < 1e-9);
}

#[test]
fn test_duration_unit_clock_cycles() {
    let clock_period_ns = 2.5; // 400 MHz DAC
    let cycle_ns = DurationUnit::Cycles
        .as_nanoseconds(16.0, Some(clock_period_ns))
        .unwrap();
    assert!((cycle_ns - 40.0).abs() < 1e-9);

    // Missing clock period must fail cleanly
    let err = DurationUnit::Cycles.as_nanoseconds(16.0, None).unwrap_err();
    assert!(matches!(err, TargetError::InvalidDurationUnit { .. }));

    // Non-positive clock period must fail
    let err_zero = DurationUnit::Cycles
        .as_nanoseconds(16.0, Some(0.0))
        .unwrap_err();
    assert!(matches!(err_zero, TargetError::InconsistentTiming { .. }));

    let err_neg = DurationUnit::Cycles
        .as_nanoseconds(16.0, Some(-1.0))
        .unwrap_err();
    assert!(matches!(err_neg, TargetError::InconsistentTiming { .. }));
}

#[test]
fn test_duration_unit_parsing_and_display() {
    assert_eq!(
        DurationUnit::from_str_unit("ns").unwrap(),
        DurationUnit::Nanoseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("nanoseconds").unwrap(),
        DurationUnit::Nanoseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("us").unwrap(),
        DurationUnit::Microseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("µs").unwrap(),
        DurationUnit::Microseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("ms").unwrap(),
        DurationUnit::Milliseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("s").unwrap(),
        DurationUnit::Seconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("ps").unwrap(),
        DurationUnit::Picoseconds
    );
    assert_eq!(
        DurationUnit::from_str_unit("dt").unwrap(),
        DurationUnit::Cycles
    );
    assert_eq!(
        DurationUnit::from_str_unit("cycles").unwrap(),
        DurationUnit::Cycles
    );

    let err = DurationUnit::from_str_unit("lightyear").unwrap_err();
    assert!(matches!(err, TargetError::InvalidDurationUnit { .. }));

    assert_eq!(format!("{}", DurationUnit::Nanoseconds), "ns");
    assert_eq!(format!("{}", DurationUnit::Microseconds), "µs");
    assert_eq!(format!("{}", DurationUnit::Cycles), "cycles");
}

#[test]
fn test_hardware_duration_validation() {
    let valid_dur = HardwareDuration::nanoseconds(45.0);
    assert!(valid_dur.validate("sx", None).is_ok());

    let zero_dur = HardwareDuration::nanoseconds(0.0);
    assert!(zero_dur.validate("rz", None).is_ok());

    let neg_dur = HardwareDuration::nanoseconds(-10.0);
    assert!(matches!(
        neg_dur.validate("invalid", None).unwrap_err(),
        TargetError::InvalidDuration { .. }
    ));

    let nan_dur = HardwareDuration::nanoseconds(f64::NAN);
    assert!(matches!(
        nan_dur.validate("invalid_nan", None).unwrap_err(),
        TargetError::InvalidDuration { .. }
    ));

    let inf_dur = HardwareDuration::nanoseconds(f64::INFINITY);
    assert!(matches!(
        inf_dur.validate("invalid_inf", None).unwrap_err(),
        TargetError::InvalidDuration { .. }
    ));
}

// ============================================================================
// 2. Explicit Unknown Principle Tests
// ============================================================================

#[test]
fn test_explicit_unknown_preservation() {
    let unk = OperationDuration::Unknown;
    assert!(unk.is_unknown());
    assert!(!unk.is_known());
    assert!(!unk.is_estimated());
    assert_eq!(unk.to_nanoseconds(None).unwrap(), None);
    assert_eq!(unk.exact_nanoseconds(None).unwrap(), None);

    // Formatted display explicitly notes unknown
    assert_eq!(format!("{unk}"), "unknown");

    let known = OperationDuration::Known(HardwareDuration::nanoseconds(120.0));
    assert!(known.is_known());
    assert!(!known.is_unknown());
    assert_eq!(known.to_nanoseconds(None).unwrap(), Some(120.0));
    assert_eq!(known.exact_nanoseconds(None).unwrap(), Some(120.0));

    let estimated = OperationDuration::Estimated(HardwareDuration::microseconds(1.2));
    assert!(estimated.is_estimated());
    assert_eq!(estimated.to_nanoseconds(None).unwrap(), Some(1200.0));
    assert_eq!(estimated.exact_nanoseconds(None).unwrap(), None); // Not exact
}

#[test]
fn test_target_duration_query_preserves_unknown() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let mut target = Target::new("unknown_test", 3);
    // Operation without nominal duration
    let custom_op = TargetOperationDescriptor::builder("custom_gate", 1)
        .connectivity(OperationConnectivity::all_single_qubits(3))
        .build();
    target.add_operation(custom_op);

    // No timing model or calibration provided: MUST return Unknown, never 0.0!
    let dur = target.operation_duration("custom_gate", &[p0]);
    assert_eq!(dur, OperationDuration::Unknown);
    assert_eq!(target.tuple_duration("custom_gate", &[p0]), None);

    // Non-existent operation also returns Unknown
    let unk_op = target.operation_duration("non_existent", &[p1]);
    assert_eq!(unk_op, OperationDuration::Unknown);
    assert_eq!(target.tuple_duration("non_existent", &[p1]), None);

    // Add tuple calibration for (custom_gate, p0) but not p2
    let mut cal = TargetCalibration::new();
    cal.set_tuple(
        "custom_gate",
        vec![p0],
        TupleCalibration::new().with_duration(75.0),
    );
    target.calibration = Some(cal);

    assert_eq!(target.tuple_duration("custom_gate", &[p0]), Some(75.0));
    assert_eq!(target.tuple_duration("custom_gate", &[p2]), None); // Still Unknown!
}

// ============================================================================
// 3. TimingModel Hierarchy and Overrides
// ============================================================================

#[test]
fn test_timing_model_hierarchy_and_cycles() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    let mut timing = TimingModel::new().with_clock_period_ns(2.0); // 2 ns dt
    timing.set_default_duration(
        "sx",
        OperationDuration::Estimated(HardwareDuration::cycles(16.0)),
    ); // 32 ns
    timing.set_tuple_duration(
        "sx",
        vec![p1],
        OperationDuration::Known(HardwareDuration::cycles(18.0)), // 36 ns on p1
    );

    // Check p0 (falls back to default duration)
    let dur_p0 = timing.duration_of(&TargetOpId::new("sx"), &[p0]);
    assert!(dur_p0.is_estimated());
    assert_eq!(
        timing.duration_ns(&TargetOpId::new("sx"), &[p0]).unwrap(),
        Some(32.0)
    );

    // Check p1 (tuple override takes precedence)
    let dur_p1 = timing.duration_of(&TargetOpId::new("sx"), &[p1]);
    assert!(dur_p1.is_known());
    assert_eq!(
        timing.duration_ns(&TargetOpId::new("sx"), &[p1]).unwrap(),
        Some(36.0)
    );

    // Check unconfigured gate: preserves Unknown
    let dur_cz = timing.duration_of(&TargetOpId::new("cz"), &[p0, p1]);
    assert_eq!(dur_cz, OperationDuration::Unknown);
    assert_eq!(
        timing
            .duration_ns(&TargetOpId::new("cz"), &[p0, p1])
            .unwrap(),
        None
    );
}

// ============================================================================
// 4. Measurement and Reset Timing Models
// ============================================================================

#[test]
fn test_readout_timing_validation() {
    let p0 = PhysicalQubitId::new(0);

    // Consistent readout: pulse 300 ns, acquisition 600 ns, total 800 ns
    let readout = ReadoutTiming::new(
        OperationDuration::known_ns(300.0),
        OperationDuration::known_ns(600.0),
        OperationDuration::known_ns(800.0),
    )
    .with_depletion(OperationDuration::known_ns(200.0));
    assert!(readout.validate(p0, None).is_ok());

    // Inconsistent readout: pulse 900 ns exceeds total 800 ns!
    let bad_readout = ReadoutTiming::new(
        OperationDuration::known_ns(900.0),
        OperationDuration::known_ns(600.0),
        OperationDuration::known_ns(800.0),
    );
    let err = bad_readout.validate(p0, None).unwrap_err();
    assert!(matches!(err, TargetError::InconsistentTiming { .. }));
}

#[test]
fn test_reset_timing_validation() {
    let p0 = PhysicalQubitId::new(0);

    // Consistent reset: pulse 250 ns, total 500 ns with 250 ns settling
    let reset = ResetTiming::new(
        OperationDuration::known_ns(250.0),
        OperationDuration::known_ns(500.0),
    )
    .with_settling(OperationDuration::known_ns(250.0));
    assert!(reset.validate(p0, None).is_ok());

    // Inconsistent reset: pulse 600 ns exceeds total 500 ns!
    let bad_reset = ResetTiming::new(
        OperationDuration::known_ns(600.0),
        OperationDuration::known_ns(500.0),
    );
    let err = bad_reset.validate(p0, None).unwrap_err();
    assert!(matches!(err, TargetError::InconsistentTiming { .. }));
}

// ============================================================================
// 5. Hardware Resource Modeling and Concurrency Constraints
// ============================================================================

#[test]
fn test_disjoint_gates_sharing_drive_line_conflict() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let mut resources = ResourceModel::new();
    let drive_bus = ResourceId::DriveLine(0);
    resources.set_capacity(drive_bus.clone(), 1); // Only 1 operation at a time on drive bus 0

    // Gate on (p0) and gate on (p2) are disjoint in qubits, but both require exclusive drive bus 0!
    resources.set_tuple_requirements(
        "sx",
        vec![p0],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(p0)),
            ResourceRequirement::exclusive(drive_bus.clone()),
        ],
    );
    resources.set_tuple_requirements(
        "sx",
        vec![p2],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(p2)),
            ResourceRequirement::exclusive(drive_bus.clone()),
        ],
    );

    // Gate on (p1) uses separate drive bus 1
    let drive_bus_1 = ResourceId::DriveLine(1);
    resources.set_tuple_requirements(
        "sx",
        vec![p1],
        vec![
            ResourceRequirement::exclusive(ResourceId::Qubit(p1)),
            ResourceRequirement::exclusive(drive_bus_1),
        ],
    );

    let op_sx = TargetOpId::new("sx");

    // Concurrency between p0 and p2 MUST fail due to shared drive line, even though qubits are disjoint!
    let conflict = resources
        .check_concurrency((&op_sx, &[p0]), (&op_sx, &[p2]))
        .unwrap_err();
    assert!(
        matches!(conflict, ResourceConflict::ExclusiveCollision { resource, .. } if resource == drive_bus)
    );

    // Concurrency between p0 and p1 must succeed (independent qubits, independent drive lines)
    assert!(resources
        .check_concurrency((&op_sx, &[p0]), (&op_sx, &[p1]))
        .is_ok());

    // Qubit overlap (p0 and p0) fails automatically
    let q_conflict = resources
        .check_concurrency((&op_sx, &[p0]), (&op_sx, &[p0]))
        .unwrap_err();
    assert!(matches!(
        q_conflict,
        ResourceConflict::ExclusiveCollision { .. }
    ));
}

#[test]
fn test_multiplexed_readout_capacity_limits() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);

    let mut resources = ResourceModel::new();
    let feedline = ResourceId::ReadoutLine(0);
    // Feedline can support at most 2 simultaneous readout pulses
    resources.set_capacity(feedline.clone(), 2);
    resources.map_qubit_readout_line(p0, 0);
    resources.map_qubit_readout_line(p1, 0);
    resources.map_qubit_readout_line(p2, 0);

    let op_meas = TargetOpId::new("measure");

    // Two simultaneous readouts on the feedline are allowed (capacity 2)
    assert!(resources
        .check_concurrency((&op_meas, &[p0]), (&op_meas, &[p1]))
        .is_ok());

    // If feedline capacity was only 1, two simultaneous readouts must fail
    resources.set_capacity(feedline.clone(), 1);
    let conflict = resources
        .check_concurrency((&op_meas, &[p0]), (&op_meas, &[p1]))
        .unwrap_err();
    assert!(
        matches!(conflict, ResourceConflict::CapacityExceeded { resource, requested: 2, capacity: 1 } if resource == feedline)
    );
}

#[test]
fn test_mutual_exclusion_rules() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);
    let p2 = PhysicalQubitId::new(2);
    let p3 = PhysicalQubitId::new(3);

    let mut resources = ResourceModel::new();

    // Cross-talk rule: CX(p0, p1) cannot run concurrently with CX(p2, p3)
    let rule = MutualExclusionRule::operation_pair(
        "cx_crosstalk_p01_p23",
        "cx",
        Some(vec![p0, p1]),
        "cx",
        Some(vec![p2, p3]),
    );
    resources.add_mutual_exclusion(rule);

    let op_cx = TargetOpId::new("cx");

    // Competing pair violates mutual exclusion
    let conflict = resources
        .check_concurrency((&op_cx, &[p0, p1]), (&op_cx, &[p2, p3]))
        .unwrap_err();
    assert!(
        matches!(conflict, ResourceConflict::MutualExclusion { ref rule, .. } if rule == "cx_crosstalk_p01_p23")
    );

    // Reverse order also detected
    let conflict_rev = resources
        .check_concurrency((&op_cx, &[p2, p3]), (&op_cx, &[p0, p1]))
        .unwrap_err();
    assert!(matches!(
        conflict_rev,
        ResourceConflict::MutualExclusion { .. }
    ));

    // Global exclusive rule: reset stops everything else
    let global_rule = MutualExclusionRule::global_exclusive("global_reset", "reset");
    resources.add_mutual_exclusion(global_rule);

    let op_reset = TargetOpId::new("reset");
    let global_conflict = resources
        .check_concurrency((&op_reset, &[p0]), (&op_cx, &[p2, p3]))
        .unwrap_err();
    assert!(matches!(
        global_conflict,
        ResourceConflict::MutualExclusion { .. }
    ));

    // Readout exclusion group: p0 and p2 cannot be measured at the same time
    let readout_excl =
        MutualExclusionRule::readout_exclusion("spectator_readout_clash", vec![p0, p2]);
    resources.add_mutual_exclusion(readout_excl);

    let op_meas = TargetOpId::new("measure");
    let r_conflict = resources
        .check_concurrency((&op_meas, &[p0]), (&op_meas, &[p2]))
        .unwrap_err();
    assert!(
        matches!(r_conflict, ResourceConflict::ReadoutExclusion { qubit1, qubit2, .. } if (qubit1 == p0 && qubit2 == p2) || (qubit1 == p2 && qubit2 == p0))
    );
}

// ============================================================================
// 6. Calibration Provenance, Timestamps, and Freshness / Staleness
// ============================================================================

#[test]
fn test_iso8601_timestamp_parsing() {
    // Standard UTC
    let ep1 = parse_timestamp_to_epoch_seconds("1970-01-01T00:00:00Z").unwrap();
    assert_eq!(ep1, 0);

    let ep2 = parse_timestamp_to_epoch_seconds("2026-11-04T05:00:00Z").unwrap();
    assert!(ep2 > 0);

    // Offset notation
    let ep3 = parse_timestamp_to_epoch_seconds("2026-11-04T07:00:00+02:00").unwrap();
    assert_eq!(ep2, ep3); // 07:00 +02:00 is same instant as 05:00 Z

    // Date-only defaults to 00:00:00 UTC
    let ep4 = parse_timestamp_to_epoch_seconds("1970-01-01").unwrap();
    assert_eq!(ep4, 0);

    // Fractional seconds ignored gracefully
    let ep5 = parse_timestamp_to_epoch_seconds("1970-01-01T00:00:00.543Z").unwrap();
    assert_eq!(ep5, 0);

    // Raw integer string
    let ep6 = parse_timestamp_to_epoch_seconds("1793768400").unwrap();
    assert_eq!(ep6, 1793768400);

    // Malformed timestamps must fail
    assert!(parse_timestamp_to_epoch_seconds("").is_err());
    assert!(parse_timestamp_to_epoch_seconds("invalid-date").is_err());
    assert!(parse_timestamp_to_epoch_seconds("2026-13-01T00:00:00Z").is_err()); // Month 13
    assert!(parse_timestamp_to_epoch_seconds("2026-02-30T00:00:00Z").is_err()); // Feb 30
}

#[test]
fn test_calibration_staleness_and_expiration() {
    let cal_ts = "2026-11-04T00:00:00Z";
    let now_fresh = "2026-11-04T02:00:00Z"; // 2 hours = 7200 seconds later
    let now_stale = "2026-11-05T12:00:00Z"; // 36 hours later

    let prov = Provenance::new("cryo_bench")
        .with_timestamp(cal_ts)
        .with_method("randomized_benchmarking");

    // Fresh within 24 hours (86400s)
    assert!(prov
        .validate_freshness(now_fresh, 86400, "qubit p[0]")
        .is_ok());
    assert!(!prov.is_stale(now_fresh, 86400).unwrap());

    // Stale when 36 hours have elapsed
    let stale_err = prov
        .validate_freshness(now_stale, 86400, "qubit p[0]")
        .unwrap_err();
    assert!(
        matches!(stale_err, TargetError::StaleCalibration { ref context, age_seconds, max_age_seconds, .. } if context == "qubit p[0]" && age_seconds > 86400 && max_age_seconds == 86400)
    );
    assert!(prov.is_stale(now_stale, 86400).unwrap());

    // Expiration via valid_until
    let expiring_prov = Provenance::new("daily_cal")
        .with_timestamp(cal_ts)
        .with_valid_until("2026-11-04T01:00:00Z"); // Expired at 01:00

    let exp_err = expiring_prov
        .validate_freshness(now_fresh, 86400, "expired_test")
        .unwrap_err();
    assert!(matches!(exp_err, TargetError::StaleCalibration { .. }));

    // Future timestamp clock skew check: calibration timestamp 2 hours in the future
    let future_ref = "2026-11-03T22:00:00Z";
    let fut_err = prov
        .validate_freshness(future_ref, 86400, "future_test")
        .unwrap_err();
    assert!(matches!(fut_err, TargetError::InvalidTimestamp { .. }));
}

#[test]
fn test_target_calibration_batch_freshness() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    let mut cal = TargetCalibration::new();
    let qcal = QubitCalibration::new();
    cal.set_qubit(p0, qcal);

    let fresh_prov = Provenance::new("bench").with_timestamp("2026-11-04T00:00:00Z");
    let stale_prov = Provenance::new("bench").with_timestamp("2026-11-01T00:00:00Z");

    let tcal0 = TupleCalibration::new()
        .with_duration(30.0)
        .with_provenance(fresh_prov);
    let tcal1 = TupleCalibration::new()
        .with_duration(30.0)
        .with_provenance(stale_prov);

    cal.set_tuple("sx", vec![p0], tcal0);
    cal.set_tuple("sx", vec![p1], tcal1);

    // Batch freshness check catches stale tuple calibration on p1
    let ref_ts = "2026-11-04T12:00:00Z";
    let err = cal.validate_freshness(ref_ts, 86400).unwrap_err();
    assert!(matches!(err, TargetError::StaleCalibration { .. }));
}

// ============================================================================
// 7. Target Validation of Timing and Resources
// ============================================================================

#[test]
fn test_target_timing_and_resource_validation() {
    let mut target = create_test_target_base();

    let mut timing = TimingModel::new().with_clock_period_ns(2.0);
    timing.set_default_duration("sx", OperationDuration::known_ns(32.0));
    target.timing = Some(timing);

    let mut resources = ResourceModel::new();
    resources.set_capacity(ResourceId::DriveLine(0), 1);
    target.resources = Some(resources);

    assert!(target.validate().is_ok());

    // Validation fails if timing references unknown operation
    let mut bad_timing = TimingModel::new();
    bad_timing.set_default_duration("unknown_gate", OperationDuration::known_ns(50.0));
    target.timing = Some(bad_timing);
    let err = target.validate().unwrap_err();
    assert!(
        matches!(err, TargetError::UnknownOperation { ref operation, .. } if operation == "unknown_gate")
    );

    // Validation fails if timing references out-of-bounds qubit
    let mut oob_timing = TimingModel::new();
    let oob_p = PhysicalQubitId::new(99);
    oob_timing.set_tuple_duration("sx", vec![oob_p], OperationDuration::known_ns(32.0));
    target.timing = Some(oob_timing);
    let err_oob = target.validate().unwrap_err();
    assert!(matches!(err_oob, TargetError::QubitOutOfBounds { qubit, .. } if qubit == oob_p));

    // Validation fails if resource declared with zero capacity
    target.timing = None;
    let mut zero_res = ResourceModel::new();
    zero_res.set_capacity(ResourceId::DriveLine(1), 0);
    target.resources = Some(zero_res);
    let err_cap = target.validate().unwrap_err();
    assert!(matches!(err_cap, TargetError::InvalidResourceModel { .. }));
}

// ============================================================================
// 8. Serde JSON Round-Trip Fidelity
// ============================================================================

#[test]
fn test_timing_and_resource_serde_round_trip() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    let mut target = create_test_target_base();

    // Configure timing model
    let mut timing = TimingModel::new().with_clock_period_ns(2.222);
    timing.set_default_duration(
        "sx",
        OperationDuration::Estimated(HardwareDuration::cycles(16.0)),
    );
    timing.set_tuple_duration(
        "cx",
        vec![p0, p1],
        OperationDuration::Known(HardwareDuration::nanoseconds(260.0)),
    );
    timing.set_readout_timing(
        p0,
        ReadoutTiming::new(
            OperationDuration::known_ns(300.0),
            OperationDuration::known_ns(600.0),
            OperationDuration::known_ns(800.0),
        ),
    );
    target.timing = Some(timing);

    // Configure resource model
    let mut resources = ResourceModel::new();
    resources.set_capacity(ResourceId::ReadoutLine(0), 4);
    resources.map_qubit_readout_line(p0, 0);
    resources.map_qubit_readout_line(p1, 0);
    resources.add_mutual_exclusion(MutualExclusionRule::readout_exclusion(
        "readout_guard",
        vec![p0, p1],
    ));
    target.resources = Some(resources);

    // Serialize to JSON
    let json = target.to_json().expect("serialization should succeed");
    assert!(json.contains("clock_period_ns"));
    assert!(json.contains("readout_guard"));

    // Deserialize and validate
    let restored = Target::from_json(&json).expect("deserialization and validation should succeed");
    assert_eq!(restored.name, target.name);
    assert_eq!(restored.timing, target.timing);
    assert_eq!(restored.resources, target.resources);

    // Duration queries match on deserialized target
    assert_eq!(restored.tuple_duration("cx", &[p0, p1]), Some(260.0));
    let sx_dur = restored.operation_duration("sx", &[p0]);
    assert!(sx_dur.is_estimated());
}

#[test]
fn test_overflow_duration_conversion_fails() {
    let huge_dur = HardwareDuration::seconds(f64::MAX);
    let err = huge_dur.to_nanoseconds(None).unwrap_err();
    assert!(matches!(err, TargetError::InvalidDuration { .. }));
}

#[test]
fn test_missing_resource_metadata_rejects_unrestricted_concurrency() {
    let p0 = PhysicalQubitId::new(0);
    let p1 = PhysicalQubitId::new(1);

    // Target with NO resource model: must NOT assume unrestricted concurrency!
    let target_no_res = create_test_target_base();
    assert_eq!(target_no_res.operation_resources("sx", &[p0]), None);

    let conflict = target_no_res
        .check_concurrency(("sx", &[p0]), ("sx", &[p1]))
        .unwrap_err();
    assert!(matches!(
        conflict,
        ResourceConflict::MissingResourceModel { ref target } if target == "test_target_4q"
    ));

    // Target WITH resource model, but operation has NO declared metadata:
    let mut target_res = create_test_target_base();
    let res = ResourceModel::new();
    target_res.resources = Some(res);

    let conflict_op = target_res
        .check_concurrency(("sx", &[p0]), ("sx", &[p1]))
        .unwrap_err();
    assert!(matches!(
        conflict_op,
        ResourceConflict::MissingOperationResources { ref op, .. } if op == "sx"
    ));
}

#[test]
fn test_invalid_expiration_preceding_calibration_fails() {
    let prov = Provenance::new("bench")
        .with_timestamp("2026-11-04T12:00:00Z")
        .with_valid_until("2026-11-04T11:00:00Z"); // 1 hour BEFORE calibration!

    let err = prov
        .validate_freshness("2026-11-04T12:30:00Z", 86400, "expiration_check")
        .unwrap_err();
    assert!(matches!(err, TargetError::InvalidTimestamp { .. }));
}

#[test]
fn test_provenance_full_fidelity_serde() {
    let prov = Provenance::new("cryo_rig_alpha")
        .with_timestamp("2026-11-04T08:00:00Z")
        .with_method("randomized_benchmarking")
        .with_valid_until("2026-11-05T08:00:00Z");

    let json = serde_json::to_string(&prov).expect("serialization succeeds");
    assert!(json.contains("cryo_rig_alpha"));
    assert!(json.contains("valid_until"));

    let restored: Provenance = serde_json::from_str(&json).expect("deserialization succeeds");
    assert_eq!(restored, prov);
}
