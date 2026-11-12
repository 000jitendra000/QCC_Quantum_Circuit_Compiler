"""Automated Session 4 verification tests: corpus, fault injection, and fair baseline.

Tests real generator/campaign/benchmark/runner paths against schemas and invariants.
"""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from benchmark.run_baseline import (  # noqa: E402
    AGGREGATE_SCHEMA_PATH,
    DEFAULT_AGGREGATE_PATH,
    DEFAULT_TRIALS_PATH,
    TRIAL_SCHEMA_PATH,
    check_target_comparability,
    run_benchmark_baseline,
)
from cases_loader import (  # noqa: E402
    CASE_SCHEMA_PATH,
    DEFAULT_CASES_DIR,
    load_canonical_case,
    load_canonical_cases_jsonl,
    sha256_bytes,
)
from corpus.generate_corpus import (  # noqa: E402
    CORPUS_SUMMARY_SCHEMA_PATH,
    DEFAULT_CORPUS_JSONL,
    DEFAULT_SUMMARY_JSON,
    detect_duplicate_logical_hashes,
    generate_corpus,
)
from corpus.run_corpus import execute_corpus  # noqa: E402
from fault_injection.run_campaign import (  # noqa: E402
    FAULT_SUMMARY_SCHEMA_PATH,
    run_fault_campaign,
)
from verify_case import verify_canonical_case  # noqa: E402


class TestDeterministicCorpus(unittest.TestCase):
    """Verify M7 Phase 68 1,000+ corpus generation, schemas, and execution."""

    def test_committed_corpus_count_uniqueness_and_coverage(self) -> None:
        self.assertTrue(DEFAULT_CORPUS_JSONL.exists())
        self.assertTrue(DEFAULT_SUMMARY_JSON.exists())

        cases = load_canonical_cases_jsonl(DEFAULT_CORPUS_JSONL)
        self.assertGreaterEqual(len(cases), 1000)

        ids = [c.case_id for c in cases]
        self.assertEqual(len(set(ids)), len(cases))

        summary = json.loads(DEFAULT_SUMMARY_JSON.read_text(encoding="utf-8"))
        summary_schema = json.loads(CORPUS_SUMMARY_SCHEMA_PATH.read_text(encoding="utf-8"))
        jsonschema.validate(instance=summary, schema=summary_schema)

        self.assertEqual(summary["total_cases"], len(cases))
        self.assertEqual(summary["duplicate_logical_hashes"], 0)
        self.assertGreater(summary["by_verification_mode"].get("exhaustive_unitary", 0), 0)
        self.assertGreater(summary["by_verification_mode"].get("sampled_statevector", 0), 0)
        self.assertGreaterEqual(len(summary["by_family"]), 8)
        self.assertEqual(
            set(summary["by_topology"].keys()),
            {"linear", "linear-generic", "ring", "ring-generic"},
        )
        # Ensure sampled cases carry non-null sampling strategy and seed
        sampled_cases = [c for c in cases if c.verification_mode == "sampled_statevector"]
        self.assertTrue(all(c.generator_metadata["sampling_strategy"] for c in sampled_cases))
        self.assertTrue(all(isinstance(c.seed, int) for c in sampled_cases))

    def test_corpus_regeneration_bit_for_bit_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            s1 = generate_corpus(
                count=1000,
                master_seed=123456,
                output_jsonl=tmp_path / "c1.jsonl",
                output_summary=tmp_path / "s1.json",
            )
            s2 = generate_corpus(
                count=1000,
                master_seed=123456,
                output_jsonl=tmp_path / "c2.jsonl",
                output_summary=tmp_path / "s2.json",
            )
            self.assertEqual(s1["corpus_file_sha256"], s2["corpus_file_sha256"])
            self.assertEqual((tmp_path / "c1.jsonl").read_bytes(), (tmp_path / "c2.jsonl").read_bytes())

    def test_duplicate_detection_reports_identical_logical_qasm(self) -> None:
        sample = 'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nh q[0];\n'
        dup_report = detect_duplicate_logical_hashes(
            [
                {"case_id": "a", "logical_qasm": sample},
                {"case_id": "b", "logical_qasm": sample},
                {"case_id": "c", "logical_qasm": sample + "x q[0];\n"},
            ]
        )
        self.assertEqual(dup_report["distinct_logical_hashes"], 2)
        self.assertEqual(dup_report["duplicate_logical_hashes"], 1)
        self.assertEqual(len(dup_report["duplicate_groups"]), 1)

    def test_corpus_slice_execution_and_individual_case_reproducibility(self) -> None:
        cases = load_canonical_cases_jsonl(DEFAULT_CORPUS_JSONL)
        sampled_idx = next(
            idx for idx, c in enumerate(cases) if c.verification_mode == "sampled_statevector"
        )
        slice_cases = cases[:8] + [cases[sampled_idx]]
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            slice_jsonl = tmp_path / "slice.jsonl"
            slice_jsonl.write_text(
                "\n".join(json.dumps(c.raw_json, sort_keys=True) for c in slice_cases) + "\n",
                encoding="utf-8",
            )
            report = execute_corpus(
                corpus_jsonl=slice_jsonl,
                generation_summary_path=DEFAULT_SUMMARY_JSON,
                workers=2,
            )
            es = report["execution_summary"]
            self.assertEqual(es["executed_cases"], 9)
            self.assertEqual(es["exhaustive_failed"], 0)
            self.assertEqual(es["sampled_failed"], 0)
            # Exhaustive and sampled counters MUST remain strictly separate
            self.assertEqual(es["exhaustive_passed"], 8)
            self.assertEqual(es["sampled_passed"], 1)

            # Single corpus case replays deterministically
            r_one = verify_canonical_case(cases[sampled_idx], work_root=tmp_path / "replay")
            self.assertEqual(r_one["status"], "PASS")
            self.assertFalse(r_one["exhaustive"])


class TestFaultInjectionCampaign(unittest.TestCase):
    """Verify M7 Phase 67 systematic fault-injection campaign."""

    REQUIRED_FAMILIES = {
        "single_qubit_gate_mutation",
        "incorrect_rotation_angle",
        "missing_gate",
        "extra_gate",
        "reversed_cx_control_target",
        "incorrect_swap_behavior",
        "incorrect_initial_layout",
        "incorrect_final_layout",
        "incorrect_physical_qubit_count",
        "corrupted_emitted_qasm",
        "corrupted_manifest_hash",
        "corrupted_global_phase_metadata",
        "relative_phase_defect",
        "residual_ancilla_population",
        "residual_ancilla_entanglement",
        "unsupported_non_unitary_operation",
        "invalid_or_incomplete_layout_metadata",
    }

    def test_campaign_detects_all_17_families_deterministically_preserving_originals(self) -> None:
        orig_hashes = {
            p.name: sha256_bytes(p.read_bytes())
            for p in sorted(DEFAULT_CASES_DIR.glob("*.json"))
        }

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            art_dir = tmp_path / "fault_artifacts"
            s1 = run_fault_campaign(
                seed=20261113,
                output_summary_path=tmp_path / "fault1.json",
                failure_artifacts_root=art_dir,
            )
            s2 = run_fault_campaign(
                seed=20261113,
                output_summary_path=tmp_path / "fault2.json",
                failure_artifacts_root=None,
            )

            schema = json.loads(FAULT_SUMMARY_SCHEMA_PATH.read_text(encoding="utf-8"))
            jsonschema.validate(instance=s1, schema=schema)

            self.assertEqual(s1["results_sha256"], s2["results_sha256"])
            self.assertTrue(self.REQUIRED_FAMILIES.issubset(set(s1["by_mutation_family"].keys())))
            self.assertGreaterEqual(s1["attempted"], 35)
            self.assertEqual(s1["detected_correctly"], s1["attempted"])
            self.assertEqual(s1["misclassified"], 0)
            self.assertEqual(s1["undetected_semantic_defects"], 0)
            self.assertEqual(s1["detection_rate"], 1.0)

            # Confirm failure artifact bundles were materialized with mutation_description.json
            self.assertTrue((art_dir / "mut_revcx_03_bell_pair" / "mutation_description.json").exists())
            self.assertTrue((art_dir / "mut_revcx_03_bell_pair" / "comparison_result.json").exists())
            self.assertTrue((art_dir / "mut_revcx_03_bell_pair" / "physical_output.qasm").exists())

        # Confirm original canonical cases were untouched
        after_hashes = {
            p.name: sha256_bytes(p.read_bytes())
            for p in sorted(DEFAULT_CASES_DIR.glob("*.json"))
        }
        self.assertEqual(orig_hashes, after_hashes)


class TestFairQiskitBaseline(unittest.TestCase):
    """Verify M7 Phase 70 fair QCC-vs-Qiskit benchmark comparability & reporting."""

    def test_comparability_checks_and_unavailable_targets(self) -> None:
        self.assertTrue(check_target_comparability("linear", 4)["comparable"])
        self.assertTrue(check_target_comparability("ring", 4)["comparable"])

        unavail_gen = check_target_comparability("linear-generic", 4)
        self.assertFalse(unavail_gen["comparable"])
        self.assertEqual(unavail_gen["status"], "UNAVAILABLE")

        unavail_fe = check_target_comparability("frontend_only", 4)
        self.assertFalse(unavail_fe["comparable"])
        self.assertEqual(unavail_fe["status"], "UNAVAILABLE")

    def test_benchmark_runs_and_enforces_fairness_and_aggregates(self) -> None:
        case_lin = load_canonical_case(DEFAULT_CASES_DIR / "07_qft4_showcase_linear.json")
        case_ring = load_canonical_case(DEFAULT_CASES_DIR / "08_qft4_showcase_ring.json")
        case_unavail = dict(case_lin.raw_json)
        case_unavail["case_id"] = "unavail_generic"
        case_unavail["compiler_config"] = {
            "target": "linear-generic",
            "global_phase_policy": "auto",
            "seed": 1,
        }
        from cases_loader import load_canonical_case_from_dict

        workloads = [case_lin, case_ring, load_canonical_case_from_dict(case_unavail)]

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            trials1, agg1 = run_benchmark_baseline(
                workloads=workloads,
                trials_per_compiler=3,
                seed_base=500,
                trials_out_path=tmp_path / "t1.jsonl",
                aggregate_out_path=tmp_path / "a1.json",
            )
            trials2, agg2 = run_benchmark_baseline(
                workloads=workloads,
                trials_per_compiler=3,
                seed_base=500,
                trials_out_path=tmp_path / "t2.jsonl",
                aggregate_out_path=tmp_path / "a2.json",
            )

            trial_schema = json.loads(TRIAL_SCHEMA_PATH.read_text(encoding="utf-8"))
            agg_schema = json.loads(AGGREGATE_SCHEMA_PATH.read_text(encoding="utf-8"))
            jsonschema.validate(instance=agg1, schema=agg_schema)
            for row in trials1:
                jsonschema.validate(instance=row, schema=trial_schema)

            # Reproducible configuration hash
            self.assertEqual(agg1["benchmark_config_sha256"], agg2["benchmark_config_sha256"])

            # 2 comparable workloads * 3 trials * 2 compilers = 12 trials; 1 unavailable workload
            self.assertEqual(len(trials1), 12)
            self.assertEqual(len(agg1["unavailable_comparisons"]), 1)
            self.assertEqual(agg1["unavailable_comparisons"][0]["status"], "UNAVAILABLE")

            # Verify semantic gate passed on every trial and aggregates obey best <= median <= worst
            os_sum = agg1["overall_summary"]
            self.assertEqual(os_sum["qcc_verified_pass_trials"], 6)
            self.assertEqual(os_sum["qiskit_verified_pass_trials"], 6)
            for ca in agg1["case_aggregates"]:
                for side in ("qcc", "qiskit"):
                    s = ca[side]
                    for m in ("total_gates", "cx_count", "circuit_depth", "compile_wall_time_ms"):
                        st = s[m]
                        self.assertLessEqual(st["best"], st["median"])
                        self.assertLessEqual(st["median"], st["worst"])
                        self.assertAlmostEqual(st["spread"], st["worst"] - st["best"], places=9)
                        self.assertEqual(st["trial_count"], 3)

            # Verify honest fairness disclosures
            fd = agg1["fairness_disclosures"]
            self.assertTrue(fd["qcc_is_deterministic_single_config"])
            self.assertFalse(fd["equal_internal_compute_budget"])
            self.assertGreaterEqual(fd["qiskit_internal_sabre_layout_trials"], 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
