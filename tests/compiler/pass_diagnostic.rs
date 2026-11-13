//! Integration tests for per-pass diagnostic and report schema (Roadmap v3.1, Phase 71).

use std::collections::BTreeMap;

use qcc::builder::CircuitBuilder;
use qcc::compiler::{
    CircuitFingerprints, CircuitSnapshotMetrics, GlobalPhaseAdjustment, PassDiagnosticReport,
    PassDiagnosticValidationError, PassMeasurements, PassStatus, PassVerification,
    PassVerificationStatus, TransformationAccounting, VerifierProvenance,
    PASS_DIAGNOSTIC_SCHEMA_ID, PASS_DIAGNOSTIC_SCHEMA_VERSION,
};
use qcc::optimizer::report::CircuitMetrics;

#[test]
fn test_pass_diagnostic_roundtrip_all_fields_populated() {
    let mut per_gate_before = BTreeMap::new();
    per_gate_before.insert("h".to_string(), 2);
    per_gate_before.insert("cx".to_string(), 1);

    let mut per_gate_after = BTreeMap::new();
    per_gate_after.insert("h".to_string(), 2);
    per_gate_after.insert("cx".to_string(), 1);

    let before = CircuitSnapshotMetrics {
        total_gate_count: Some(3),
        per_gate_counts: Some(per_gate_before),
        cx_count: Some(1),
        logical_swaps: Some(0),
        router_swaps_inserted: Some(0),
        final_emitted_swaps: Some(0),
        circuit_depth: Some(2),
        cost_metrics: Some({
            let mut cm = BTreeMap::new();
            cm.insert("estimated_latency_ns".to_string(), 120.0);
            cm
        }),
    };

    let after = CircuitSnapshotMetrics {
        total_gate_count: Some(3),
        per_gate_counts: Some(per_gate_after),
        cx_count: Some(1),
        logical_swaps: Some(0),
        router_swaps_inserted: Some(0),
        final_emitted_swaps: Some(0),
        circuit_depth: Some(2),
        cost_metrics: Some({
            let mut cm = BTreeMap::new();
            cm.insert("estimated_latency_ns".to_string(), 120.0);
            cm
        }),
    };

    let measurements = PassMeasurements {
        before: Some(before),
        after: Some(after),
        duration_ms: Some(1.45),
    };

    let fingerprints = CircuitFingerprints::sha256(
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
        Some("ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb".to_string()),
    );

    let transformation = TransformationAccounting {
        operations_added: 0,
        operations_removed: 0,
        operations_rewritten: 0,
        global_phase_adjustment: Some(GlobalPhaseAdjustment::new(0.0, "0")),
        analysis_invalidations: vec![],
    };

    let mut tolerances = BTreeMap::new();
    tolerances.insert("atol".to_string(), 1e-10);
    tolerances.insert("rtol".to_string(), 1e-8);

    let verification = PassVerification::internal_passed(
        "qcc_statevector_unitary",
        "0.1.0",
        "exhaustive_unitary",
        Some(tolerances),
    );

    let mut pass_config = BTreeMap::new();
    pass_config.insert("max_iterations".to_string(), serde_json::json!(10));
    pass_config.insert("merge_rotations".to_string(), serde_json::json!(true));

    let report = PassDiagnosticReport::new(
        "rotation_merging",
        "Same-Axis Rotation Merging",
        1,
        PassStatus::Ran,
        pass_config,
        fingerprints,
        measurements,
        transformation,
        verification,
    );

    assert_eq!(report.schema_id, PASS_DIAGNOSTIC_SCHEMA_ID);
    assert_eq!(report.schema_version, PASS_DIAGNOSTIC_SCHEMA_VERSION);
    assert_eq!(report.status, PassStatus::Ran);
    assert_eq!(report.verification.status, PassVerificationStatus::Passed);
    assert_eq!(
        report.verification.provenance,
        Some(VerifierProvenance::Internal)
    );

    // Invariant validation
    assert!(report.validate_invariants().is_ok());

    // JSON serialization roundtrip
    let json_str = report.to_json().expect("Serialization must succeed");
    assert!(json_str.contains("\"schema_id\": \"qcc.pass_diagnostic\""));
    assert!(json_str.contains("\"schema_version\": \"1.0.0\""));
    assert!(json_str.contains("\"pass_id\": \"rotation_merging\""));
    assert!(json_str.contains("\"semantic_equivalence_guaranteed\": false"));

    let deserialized =
        PassDiagnosticReport::from_json(&json_str).expect("Deserialization must succeed");
    assert_eq!(report, deserialized);
    assert!(deserialized.validate_invariants().is_ok());
}

#[test]
fn test_pass_diagnostic_no_op_pass() {
    let report = PassDiagnosticReport::new(
        "identity_elimination",
        "Identity Gate Elimination",
        0,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(
            Some("1111111111111111111111111111111111111111111111111111111111111111".to_string()),
            Some("1111111111111111111111111111111111111111111111111111111111111111".to_string()),
        ),
        PassMeasurements {
            before: None,
            after: None,
            duration_ms: Some(0.02),
        },
        TransformationAccounting::no_op(),
        PassVerification::not_run(),
    );

    assert_eq!(report.transformation.operations_added, 0);
    assert_eq!(report.transformation.operations_removed, 0);
    assert_eq!(report.transformation.operations_rewritten, 0);
    assert!(report.transformation.global_phase_adjustment.is_none());
    assert!(report.transformation.analysis_invalidations.is_empty());
    assert_eq!(report.verification.status, PassVerificationStatus::NotRun);
    assert!(report.validate_invariants().is_ok());

    let json_str = report.to_json().unwrap();
    let roundtrip = PassDiagnosticReport::from_json(&json_str).unwrap();
    assert_eq!(report, roundtrip);
}

#[test]
fn test_pass_diagnostic_skipped_pass() {
    let report = PassDiagnosticReport::new(
        "sabre_routing",
        "SABRE Qubit Routing",
        3,
        PassStatus::Skipped,
        BTreeMap::new(),
        CircuitFingerprints::sha256(None, None),
        PassMeasurements {
            before: None,
            after: None,
            duration_ms: None,
        },
        TransformationAccounting::no_op(),
        PassVerification::not_run(),
    );

    assert_eq!(report.status, PassStatus::Skipped);
    assert_eq!(report.verification.status, PassVerificationStatus::NotRun);
    assert!(report.measurements.before.is_none());
    assert!(report.measurements.after.is_none());
    assert!(report.measurements.duration_ms.is_none());
    assert!(report.validate_invariants().is_ok());

    let json_str = report.to_json().unwrap();
    assert!(json_str.contains("\"status\": \"skipped\""));
    assert!(json_str.contains("\"status\": \"not_run\""));
    let roundtrip = PassDiagnosticReport::from_json(&json_str).unwrap();
    assert_eq!(report, roundtrip);
}

#[test]
fn test_pass_diagnostic_unsupported_pass() {
    let report = PassDiagnosticReport::new(
        "unitary_equivalence_pass",
        "Unitary Equivalence Verification",
        4,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(Some("abc".to_string()), Some("abc".to_string())),
        PassMeasurements::default(),
        TransformationAccounting::no_op(),
        PassVerification::unsupported(
            "Circuit contains terminal measurement and classical feedforward (non-unitary)",
        ),
    );

    assert_eq!(
        report.verification.status,
        PassVerificationStatus::Unsupported
    );
    assert!(report
        .verification
        .detail
        .as_ref()
        .unwrap()
        .contains("non-unitary"));
    assert!(report.validate_invariants().is_ok());

    let json_str = report.to_json().unwrap();
    let roundtrip = PassDiagnosticReport::from_json(&json_str).unwrap();
    assert_eq!(report, roundtrip);
}

#[test]
fn test_pass_diagnostic_failed_verification() {
    let report = PassDiagnosticReport::new(
        "commutation_reordering",
        "DAG Commutation Reordering",
        2,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(Some("aaa".to_string()), Some("bbb".to_string())),
        PassMeasurements::default(),
        TransformationAccounting {
            operations_added: 0,
            operations_removed: 0,
            operations_rewritten: 2,
            global_phase_adjustment: None,
            analysis_invalidations: vec!["depth_cache".to_string()],
        },
        PassVerification::failed(
            "qcc_statevector_unitary",
            "Pointwise statevector amplitude mismatch max_diff=0.7071 > atol=1e-10",
            VerifierProvenance::Internal,
        ),
    );

    assert_eq!(report.verification.status, PassVerificationStatus::Failed);
    assert_eq!(
        report.verification.provenance,
        Some(VerifierProvenance::Internal)
    );
    assert!(report
        .verification
        .detail
        .as_ref()
        .unwrap()
        .contains("mismatch"));
    assert!(report.validate_invariants().is_ok());

    let json_str = report.to_json().unwrap();
    let roundtrip = PassDiagnosticReport::from_json(&json_str).unwrap();
    assert_eq!(report, roundtrip);
}

#[test]
fn test_pass_diagnostic_independent_oracle_provenance() {
    let report = PassDiagnosticReport::new(
        "physical_routing_and_scheduling",
        "Physical Routing and Scheduling",
        5,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(Some("in".to_string()), Some("out".to_string())),
        PassMeasurements::default(),
        TransformationAccounting {
            operations_added: 4,
            operations_removed: 0,
            operations_rewritten: 0,
            global_phase_adjustment: None,
            analysis_invalidations: vec!["routing_layout".to_string()],
        },
        PassVerification::oracle_passed(
            "qiskit_independent_oracle",
            "2.6.2",
            "exhaustive_unitary",
            None,
        ),
    );

    assert_eq!(report.verification.status, PassVerificationStatus::Passed);
    assert_eq!(
        report.verification.provenance,
        Some(VerifierProvenance::IndependentOracle)
    );
    assert!(report.validate_invariants().is_ok());

    let json_str = report.to_json().unwrap();
    let roundtrip = PassDiagnosticReport::from_json(&json_str).unwrap();
    assert_eq!(report, roundtrip);
}

#[test]
fn test_invariant_rejection_semantic_equivalence_guaranteed_true() {
    let mut report = PassDiagnosticReport::new(
        "test_pass",
        "Test Pass",
        0,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(None, None),
        PassMeasurements::default(),
        TransformationAccounting::no_op(),
        PassVerification::not_run(),
    );

    report.fingerprints.semantic_equivalence_guaranteed = true;
    let err = report.validate_invariants().unwrap_err();
    assert_eq!(
        err,
        PassDiagnosticValidationError::SemanticEquivalenceGuaranteedTrue
    );
}

#[test]
fn test_invariant_rejection_skipped_pass_with_passed_verification() {
    let report = PassDiagnosticReport::new(
        "test_pass",
        "Test Pass",
        0,
        PassStatus::Skipped,
        BTreeMap::new(),
        CircuitFingerprints::sha256(None, None),
        PassMeasurements::default(),
        TransformationAccounting::no_op(),
        PassVerification::internal_passed("internal", "1.0", "mode", None),
    );

    let err = report.validate_invariants().unwrap_err();
    assert_eq!(
        err,
        PassDiagnosticValidationError::SkippedPassWithRunVerification
    );
}

#[test]
fn test_invariant_rejection_passed_without_provenance() {
    let mut report = PassDiagnosticReport::new(
        "test_pass",
        "Test Pass",
        0,
        PassStatus::Ran,
        BTreeMap::new(),
        CircuitFingerprints::sha256(None, None),
        PassMeasurements::default(),
        TransformationAccounting::no_op(),
        PassVerification::internal_passed("internal", "1.0", "mode", None),
    );

    report.verification.provenance = None;
    let err = report.validate_invariants().unwrap_err();
    assert_eq!(err, PassDiagnosticValidationError::PassedWithoutProvenance);
}

#[test]
fn test_from_circuit_metrics_helper() {
    let mut builder = CircuitBuilder::new("test_metrics");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let metrics = CircuitMetrics::compute(&circuit);
    let snapshot =
        CircuitSnapshotMetrics::from_circuit_metrics(&metrics, Some(0), Some(1), Some(1), None);

    assert_eq!(snapshot.total_gate_count, Some(2));
    assert_eq!(snapshot.cx_count, Some(1));
    assert_eq!(snapshot.logical_swaps, Some(0));
    assert_eq!(snapshot.router_swaps_inserted, Some(1));
    assert_eq!(snapshot.final_emitted_swaps, Some(1));
    assert_eq!(snapshot.circuit_depth, Some(2));
    let counts = snapshot.per_gate_counts.unwrap();
    assert_eq!(counts.get("h"), Some(&1));
    assert_eq!(counts.get("cx"), Some(&1));
}

#[test]
fn test_null_handling_for_optional_snapshot_metrics() {
    let snapshot = CircuitSnapshotMetrics {
        total_gate_count: None,
        per_gate_counts: None,
        cx_count: None,
        logical_swaps: None,
        router_swaps_inserted: None,
        final_emitted_swaps: None,
        circuit_depth: None,
        cost_metrics: None,
    };

    let json_val = serde_json::to_value(&snapshot).unwrap();
    assert!(json_val.get("total_gate_count").unwrap().is_null());
    assert!(json_val.get("per_gate_counts").unwrap().is_null());
    assert!(json_val.get("cx_count").unwrap().is_null());
    assert!(json_val.get("logical_swaps").unwrap().is_null());
    assert!(json_val.get("router_swaps_inserted").unwrap().is_null());
    assert!(json_val.get("final_emitted_swaps").unwrap().is_null());
    assert!(json_val.get("circuit_depth").unwrap().is_null());
    assert!(json_val.get("cost_metrics").unwrap().is_null());

    let roundtrip: CircuitSnapshotMetrics = serde_json::from_value(json_val).unwrap();
    assert_eq!(snapshot, roundtrip);
}
