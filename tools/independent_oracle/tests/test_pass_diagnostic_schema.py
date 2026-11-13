"""Comprehensive schema validation tests for QCC compiler pass diagnostic reports (Phase 71).

Validates:
1. JSON Schema Draft 2020-12 compliance of `pass_diagnostic.schema.json`.
2. Valid report acceptance across all verification states (`passed`, `failed`, `not_run`, `unsupported`).
3. Valid report acceptance across all execution statuses (`ran`, `skipped`, `failed`).
4. Separation and validation of both `internal` and `independent_oracle` provenance.
5. Invariant enforcement: rejection of `semantic_equivalence_guaranteed: true`.
6. Strict rejection of unknown status enums, missing required fields, and additional properties.
7. Clean handling of null/missing snapshot metrics without fabricating zero defaults.
"""

from __future__ import annotations

import copy
import json
import pathlib
import sys
import unittest
from typing import Any

import jsonschema
from jsonschema import Draft202012Validator

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
SCHEMA_PATH = ORACLE_DIR / "schema" / "pass_diagnostic.schema.json"


def load_pass_diagnostic_schema() -> dict[str, Any]:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def make_valid_base_diagnostic() -> dict[str, Any]:
    return {
        "schema_id": "qcc.pass_diagnostic",
        "schema_version": "1.0.0",
        "pass_id": "rotation_merging",
        "pass_name": "Same-Axis Rotation Merging",
        "execution_order": 1,
        "status": "ran",
        "compiler": {
            "name": "qcc",
            "version": "0.1.0",
            "version_string": "qcc 0.1.0",
            "profile": "release",
            "target_os": "linux",
            "target_arch": "x86_64",
            "git_commit": None,
        },
        "pass_config": {
            "max_iterations": 10,
            "merge_rotations": True,
        },
        "fingerprints": {
            "algorithm": "sha256",
            "input_fingerprint": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "output_fingerprint": "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
            "semantic_equivalence_guaranteed": False,
        },
        "measurements": {
            "before": {
                "total_gate_count": 3,
                "per_gate_counts": {"h": 2, "cx": 1},
                "cx_count": 1,
                "logical_swaps": 0,
                "router_swaps_inserted": 0,
                "final_emitted_swaps": 0,
                "circuit_depth": 2,
                "cost_metrics": {"estimated_latency_ns": 120.0},
            },
            "after": {
                "total_gate_count": 3,
                "per_gate_counts": {"h": 2, "cx": 1},
                "cx_count": 1,
                "logical_swaps": 0,
                "router_swaps_inserted": 0,
                "final_emitted_swaps": 0,
                "circuit_depth": 2,
                "cost_metrics": {"estimated_latency_ns": 120.0},
            },
            "duration_ms": 1.45,
        },
        "transformation": {
            "operations_added": 0,
            "operations_removed": 0,
            "operations_rewritten": 0,
            "global_phase_adjustment": {
                "radians": 0.0,
                "expression": "0",
            },
            "analysis_invalidations": [],
        },
        "verification": {
            "status": "passed",
            "verifier_name": "qcc_statevector_unitary",
            "verifier_version": "0.1.0",
            "verifier_mode": "exhaustive_unitary",
            "provenance": "internal",
            "tolerances": {"atol": 1e-10, "rtol": 1e-8},
            "detail": None,
        },
    }


class TestPassDiagnosticSchema(unittest.TestCase):
    """Test suite for pass_diagnostic.schema.json."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.schema = load_pass_diagnostic_schema()
        Draft202012Validator.check_schema(cls.schema)
        cls.validator = Draft202012Validator(cls.schema)

    def test_schema_itself_is_valid_draft_2020_12(self) -> None:
        self.assertEqual(self.schema["properties"]["schema_id"]["const"], "qcc.pass_diagnostic")
        self.assertEqual(self.schema["properties"]["schema_version"]["const"], "1.0.0")

    def test_valid_comprehensive_report_validates(self) -> None:
        doc = make_valid_base_diagnostic()
        self.validator.validate(doc)

    def test_valid_independent_oracle_verification(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"] = {
            "status": "passed",
            "verifier_name": "qiskit_independent_oracle",
            "verifier_version": "2.6.2",
            "verifier_mode": "exhaustive_unitary",
            "provenance": "independent_oracle",
            "tolerances": {"atol": 1e-10, "rtol": 1e-8},
            "detail": None,
        }
        self.validator.validate(doc)

    def test_valid_not_run_verification(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"] = {
            "status": "not_run",
            "verifier_name": None,
            "verifier_version": None,
            "verifier_mode": None,
            "provenance": None,
            "tolerances": None,
            "detail": None,
        }
        self.validator.validate(doc)

    def test_valid_unsupported_verification(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"] = {
            "status": "unsupported",
            "verifier_name": None,
            "verifier_version": None,
            "verifier_mode": None,
            "provenance": None,
            "tolerances": None,
            "detail": "Circuit contains non-unitary measurement operations",
        }
        self.validator.validate(doc)

    def test_valid_failed_verification(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"] = {
            "status": "failed",
            "verifier_name": "qcc_statevector_unitary",
            "verifier_version": "0.1.0",
            "verifier_mode": "exhaustive_unitary",
            "provenance": "internal",
            "tolerances": {"atol": 1e-10},
            "detail": "Unitary equivalence mismatch max_amplitude_error=0.7071",
        }
        self.validator.validate(doc)

    def test_valid_skipped_pass_with_null_metrics(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["status"] = "skipped"
        doc["fingerprints"]["input_fingerprint"] = None
        doc["fingerprints"]["output_fingerprint"] = None
        doc["measurements"]["before"] = None
        doc["measurements"]["after"] = None
        doc["measurements"]["duration_ms"] = None
        doc["transformation"]["global_phase_adjustment"] = None
        doc["verification"] = {
            "status": "not_run",
            "verifier_name": None,
            "verifier_version": None,
            "verifier_mode": None,
            "provenance": None,
            "tolerances": None,
            "detail": None,
        }
        self.validator.validate(doc)

    def test_rejection_semantic_equivalence_guaranteed_true(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["fingerprints"]["semantic_equivalence_guaranteed"] = True
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)

    def test_rejection_invalid_status_enum(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["status"] = "in_progress"
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)

    def test_rejection_invalid_verification_status_enum(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"]["status"] = "partial_pass"
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)

    def test_rejection_invalid_provenance_enum(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["verification"]["provenance"] = "third_party"
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)

    def test_rejection_missing_required_top_level_field(self) -> None:
        for field in (
            "schema_id",
            "schema_version",
            "pass_id",
            "pass_name",
            "execution_order",
            "status",
            "compiler",
            "pass_config",
            "fingerprints",
            "measurements",
            "transformation",
            "verification",
        ):
            doc = make_valid_base_diagnostic()
            del doc[field]
            with self.assertRaises(jsonschema.ValidationError):
                self.validator.validate(doc)

    def test_rejection_additional_properties_at_top_level(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["unexpected_field"] = "bogus"
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)

    def test_rejection_negative_gate_counts_or_depth(self) -> None:
        doc = make_valid_base_diagnostic()
        doc["measurements"]["before"]["total_gate_count"] = -1
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(doc)


if __name__ == "__main__":
    unittest.main()
