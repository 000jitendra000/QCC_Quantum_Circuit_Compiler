"""Unit and schema validation tests for QCC independent optimizer validation benchmark (Phase 81)."""

from __future__ import annotations

import json
import pathlib
import sys
import unittest
from typing import Any

import jsonschema
from jsonschema import Draft202012Validator

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

SCHEMA_PATH = ORACLE_DIR / "schema" / "optimizer_validation.schema.json"
AGGREGATE_REPORT_PATH = ORACLE_DIR / "reports" / "optimizer_validation_aggregate.json"
CONTROL_A_REPORT_PATH = ORACLE_DIR / "reports" / "control_a_verification_results.json"

from benchmarks.optimizer_validation import (
    compute_aggregates,
)


class TestControlAVerification(unittest.TestCase):
    """Verifies that all Control A physical outputs passed independent Qiskit oracle verification."""

    def test_control_a_artifact_complete_and_all_pass(self) -> None:
        if not CONTROL_A_REPORT_PATH.exists():
            self.skipTest("Control A verification report does not exist")
        data = json.loads(CONTROL_A_REPORT_PATH.read_text(encoding="utf-8"))
        self.assertEqual(data["total_workloads_evaluated"], 130)
        self.assertEqual(data["pass_count"], 130)
        self.assertEqual(data["fail_count"], 0)
        self.assertEqual(data["clean_ancilla_ok_count"], 130)
        self.assertEqual(data["clean_ancilla_pass_rate"], 1.0)
        self.assertEqual(len(data["case_verifications"]), 130)
        for case in data["case_verifications"]:
            self.assertEqual(case["status"], "PASS")
            self.assertTrue(case["ancilla_zero_ok"])
            self.assertLessEqual(case["max_amplitude_error"], 1e-10)


class TestOptimizerValidationSchema(unittest.TestCase):
    """Validates optimizer validation benchmark schema and reports."""

    def setUp(self) -> None:
        self.schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(self.schema)
        self.validator = Draft202012Validator(self.schema)

    def test_schema_itself_is_valid_draft_2020_12(self) -> None:
        self.assertEqual(self.schema["$schema"], "https://json-schema.org/draft/2020-12/schema")
        self.assertEqual(self.schema["properties"]["schema_id"]["const"], "qcc.optimizer_validation_benchmark")

    def test_generated_aggregate_report_validates_cleanly(self) -> None:
        if not AGGREGATE_REPORT_PATH.exists():
            self.skipTest("Aggregate report does not exist yet")
        report = json.loads(AGGREGATE_REPORT_PATH.read_text(encoding="utf-8"))
        self.validator.validate(report)
        self.assertEqual(report["schema_id"], "qcc.optimizer_validation_benchmark")
        self.assertIn(report["outcome_classification"], [
            "A_CLEAR_BENEFIT",
            "B_LOGICAL_BENEFIT_ONLY",
            "C_MIXED_RESULT",
            "D_NO_MEASURABLE_BENEFIT",
            "E_NEGATIVE_RESULT",
            "F_INSUFFICIENT_EVIDENCE",
        ])

    def test_schema_rejects_missing_required_fields(self) -> None:
        if not AGGREGATE_REPORT_PATH.exists():
            self.skipTest("Aggregate report does not exist yet")
        report = json.loads(AGGREGATE_REPORT_PATH.read_text(encoding="utf-8"))
        bad_report = dict(report)
        del bad_report["outcome_classification"]
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(bad_report)

    def test_schema_rejects_invalid_outcome_classification(self) -> None:
        if not AGGREGATE_REPORT_PATH.exists():
            self.skipTest("Aggregate report does not exist yet")
        report = json.loads(AGGREGATE_REPORT_PATH.read_text(encoding="utf-8"))
        bad_report = dict(report)
        bad_report["outcome_classification"] = "INVALID_OUTCOME"
        with self.assertRaises(jsonschema.ValidationError):
            self.validator.validate(bad_report)


class TestAggregateClassificationLogic(unittest.TestCase):
    """Tests outcome classification and recommendation logic under synthetic distributions."""

    def _make_dummy_case(
        self,
        case_id: str,
        opp_type: str,
        gates_delta: int,
        two_qubit_delta: int,
        swaps_delta: int,
        depth_delta: int,
        ops_removed: int,
        oracle_status: str = "PASS",
        ctrl_time: float = 10.0,
        treat_time: float = 10.5,
    ) -> dict[str, Any]:
        return {
            "case_id": case_id,
            "family": "test_family",
            "target": "linear",
            "logical_qubits": 2,
            "opportunity_type": opp_type,
            "control": {
                "total_gates": 20,
                "two_qubit_gates": 5,
                "router_swaps": 2,
                "final_emitted_swaps": 2,
                "circuit_depth": 10,
                "compile_time_ms_median": ctrl_time,
                "compile_time_ms_min": ctrl_time - 1.0,
                "compile_time_ms_max": ctrl_time + 1.0,
                "compile_time_ms_spread": 2.0,
            },
            "treatment": {
                "total_gates": 20 + gates_delta,
                "two_qubit_gates": 5 + two_qubit_delta,
                "router_swaps": 2 + swaps_delta,
                "final_emitted_swaps": 2 + swaps_delta,
                "circuit_depth": 10 + depth_delta,
                "compile_time_ms_median": treat_time,
                "compile_time_ms_min": treat_time - 1.0,
                "compile_time_ms_max": treat_time + 1.0,
                "compile_time_ms_spread": 2.0,
            },
            "delta": {
                "gates_delta": gates_delta,
                "two_qubit_delta": two_qubit_delta,
                "swaps_delta": swaps_delta,
                "depth_delta": depth_delta,
                "compile_time_delta_ms": treat_time - ctrl_time,
                "compile_time_overhead_ratio": treat_time / ctrl_time,
            },
            "pass_diagnostics": {
                "status": "ran" if ops_removed > 0 else "not_executed",
                "operations_removed": ops_removed,
                "duration_ms": 0.5,
            },
            "oracle_verification": {
                "status": oracle_status,
                "ancilla_zero_ok": True,
                "max_amplitude_error": 0.0,
                "strict_phase_accounting": "PASS",
            },
        }

    def test_classify_insufficient_evidence_when_few_cases(self) -> None:
        cases = [self._make_dummy_case(f"c{i}", "positive", -2, -2, 0, -1, 2) for i in range(5)]
        agg, oracle, outcome, rec = compute_aggregates(cases)
        self.assertEqual(outcome, "F_INSUFFICIENT_EVIDENCE")
        self.assertFalse(rec["enable_by_default"])

    def test_classify_negative_when_oracle_fails(self) -> None:
        cases = [self._make_dummy_case(f"c{i}", "positive", -2, -2, 0, -1, 2) for i in range(25)]
        cases[3]["oracle_verification"]["status"] = "MISMATCH"
        agg, oracle, outcome, rec = compute_aggregates(cases)
        self.assertEqual(outcome, "E_NEGATIVE_RESULT")
        self.assertFalse(rec["enable_by_default"])

    def test_classify_mixed_result_when_regressions_exist(self) -> None:
        cases = [self._make_dummy_case(f"c{i}", "positive", -2, -2, 0, -1, 2) for i in range(24)]
        # Add 1 case with a worsened swap
        cases.append(self._make_dummy_case("c24", "positive", +2, +1, +1, +1, 2))
        agg, oracle, outcome, rec = compute_aggregates(cases)
        self.assertEqual(outcome, "C_MIXED_RESULT")
        self.assertFalse(rec["enable_by_default"])

    def test_classify_clear_benefit_when_strictly_positive(self) -> None:
        cases = [self._make_dummy_case(f"c{i}", "positive", -2, -2, 0, -1, 2) for i in range(25)]
        agg, oracle, outcome, rec = compute_aggregates(cases)
        self.assertEqual(outcome, "A_CLEAR_BENEFIT")
        self.assertTrue(rec["enable_by_default"])


if __name__ == "__main__":
    unittest.main()

