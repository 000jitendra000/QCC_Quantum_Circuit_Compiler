//! Phase 85: Integrated Upgrade Acceptance Test Suite (Milestone M12).
//!
//! Validates the coherent, end-to-end integration of:
//! - M8: Compiler introspection and per-pass diagnostics
//! - M9: Multi-candidate routing search with deterministic ranking and fallback
//! - M10: Commutative two-qubit cancellation with opt-in default preservation
//! - End-to-end target legality and ADR-0001 clean-ancilla compliance

use std::fs;
use std::path::Path;
use std::process::Command;

const QCC_BIN: &str = env!("CARGO_BIN_EXE_qcc");

#[test]
fn test_cli_acceptance_ex1_pass_diagnostics() {
    let report_path = "target/acceptance_ex1_diag.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/routing.qasm")
        .arg("--target")
        .arg("linear")
        .arg("--pass-summary")
        .arg("--pass-report")
        .arg(report_path)
        .arg("--inspect-pass")
        .arg("inverse_cancellation")
        .output()
        .expect("Execute qcc compile with diagnostics");

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Pass Execution Summary:"));
    assert!(stderr.contains("inverse_cancellation"));
    assert!(stderr.contains("identity_elimination"));

    assert!(Path::new(report_path).exists());
    let report_text = fs::read_to_string(report_path).expect("read report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report_text).expect("parse pass report json");
    assert!(report_json.is_array());
    assert!(!report_json.as_array().unwrap().is_empty());

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_acceptance_ex2_multi_candidate_routing() {
    let manifest_path = "target/acceptance_ex2_manifest.json";
    let _ = fs::remove_file(manifest_path);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/routing.qasm")
        .arg("--target")
        .arg("ring")
        .arg("--routing-candidates")
        .arg("4")
        .arg("--routing-objective")
        .arg("min-swaps")
        .arg("--emit-manifest")
        .arg(manifest_path)
        .output()
        .expect("Execute qcc compile with multi-candidate routing");

    assert_eq!(output.status.code(), Some(0));
    assert!(Path::new(manifest_path).exists());
    let manifest_text = fs::read_to_string(manifest_path).expect("read manifest");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).expect("parse manifest");

    // On 3-qubit ring, all 3 CX interactions are adjacent; router SWAPs must be 0
    assert_eq!(
        manifest["metrics"]["router_swaps_inserted"].as_u64(),
        Some(0)
    );
    assert_eq!(manifest["metrics"]["cx_count"].as_u64(), Some(3));
    let router_name = manifest["routing"]["router_name"].as_str().unwrap_or("");
    assert!(
        router_name.contains("MultiCandidate"),
        "Router name must indicate MultiCandidate routing"
    );

    let _ = fs::remove_file(manifest_path);
}

#[test]
fn test_cli_acceptance_ex3_ex4_optimizer_default_vs_enabled() {
    let control_manifest_path = "target/acceptance_control_manifest.json";
    let treatment_manifest_path = "target/acceptance_treatment_manifest.json";
    let _ = fs::remove_file(control_manifest_path);
    let _ = fs::remove_file(treatment_manifest_path);

    // EX-4: Control - pass disabled by default
    let control_output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/commutative_cancellation.qasm")
        .arg("--target")
        .arg("linear")
        .arg("--emit-manifest")
        .arg(control_manifest_path)
        .output()
        .expect("Execute qcc compile control");

    assert_eq!(control_output.status.code(), Some(0));
    let control_manifest_text =
        fs::read_to_string(control_manifest_path).expect("read control manifest");
    let control_manifest: serde_json::Value =
        serde_json::from_str(&control_manifest_text).expect("parse control manifest");

    assert_eq!(
        control_manifest["metrics"]["cx_count"].as_u64(),
        Some(3),
        "Control must retain all 3 CX gates"
    );
    assert_eq!(
        control_manifest["metrics"]["total_operations"].as_u64(),
        Some(4)
    );

    // EX-3: Treatment - pass explicitly enabled
    let treatment_output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/commutative_cancellation.qasm")
        .arg("--target")
        .arg("linear")
        .arg("--enable-commutative-cancellation")
        .arg("--emit-manifest")
        .arg(treatment_manifest_path)
        .output()
        .expect("Execute qcc compile treatment");

    assert_eq!(treatment_output.status.code(), Some(0));
    let treatment_manifest_text =
        fs::read_to_string(treatment_manifest_path).expect("read treatment manifest");
    let treatment_manifest: serde_json::Value =
        serde_json::from_str(&treatment_manifest_text).expect("parse treatment manifest");

    assert_eq!(
        treatment_manifest["metrics"]["cx_count"].as_u64(),
        Some(1),
        "Treatment must cancel 2 commuting CX gates, leaving 1 CX"
    );
    assert_eq!(
        treatment_manifest["metrics"]["total_operations"].as_u64(),
        Some(2),
        "Treatment must leave only 2 operations"
    );

    let _ = fs::remove_file(control_manifest_path);
    let _ = fs::remove_file(treatment_manifest_path);
}

#[test]
fn test_cli_acceptance_ex5_comprehensive_pipeline() {
    let report_path = "target/acceptance_ex5_report.json";
    let manifest_path = "target/acceptance_ex5_manifest.json";
    let _ = fs::remove_file(report_path);
    let _ = fs::remove_file(manifest_path);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/showcase.qasm")
        .arg("--target")
        .arg("ring")
        .arg("--enable-commutative-cancellation")
        .arg("--routing-candidates")
        .arg("4")
        .arg("--routing-objective")
        .arg("min-swaps")
        .arg("--pass-summary")
        .arg("--pass-report")
        .arg(report_path)
        .arg("--emit-manifest")
        .arg(manifest_path)
        .output()
        .expect("Execute comprehensive compilation");

    assert_eq!(output.status.code(), Some(0));
    assert!(Path::new(report_path).exists());
    assert!(Path::new(manifest_path).exists());

    let manifest_text = fs::read_to_string(manifest_path).expect("read manifest");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).expect("parse manifest");
    assert_eq!(manifest["metrics"]["is_unitary"].as_bool(), Some(true));

    let _ = fs::remove_file(report_path);
    let _ = fs::remove_file(manifest_path);
}
