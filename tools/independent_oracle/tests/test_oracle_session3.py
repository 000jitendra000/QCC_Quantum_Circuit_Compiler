"""Comprehensive Session 3 tests: canonical cases, independent runners, layout/endian, and comparator.

Covers all M7 Phases 62-66 requirements:
1. Canonical case schema validation & deterministic loading.
2. Separate independent Qiskit logical-reference runner & black-box QCC physical runner.
3. Little-endian layout & physical embedding invariants (`equivalence/layout.py`).
4. Authoritative single-common-phase semantic comparison & strict ADR-0001 ancilla zero-state checks
   in both `exhaustive_unitary` and `sampled_statevector` modes.
5. Fault detection: bad physical gate, reversed CX, wrong initial layout, wrong final layout,
   relative phase mismatch (non-repairable with one common phase), nonzero phase metadata corruption
   vs informational strict phase accounting, residual ancilla population, residual ancilla entanglement,
   QASM/manifest SHA-256 mismatch, invalid manifest schema, unsupported/non-unitary inputs.
6. Parameter binding, deterministic serialization, schema validation, and failure artifact bundles.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys
import tempfile
import unittest

import numpy as np

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    DEFAULT_CASES_DIR,
    load_all_canonical_cases,
    load_canonical_case_from_dict,
)
from equivalence.artifacts import (  # noqa: E402
    comparison_result_digest,
    serialize_comparison_result,
    validate_comparison_result_schema,
)
from equivalence.comparator import (  # noqa: E402
    OracleTolerances,
    compare_logical_vs_physical,
)
from equivalence.layout import (  # noqa: E402
    compute_residual_ancilla_population,
    embed_logical_statevector,
    map_logical_basis_index,
    parse_and_validate_single_layout,
)
from runners.qcc_runner import (  # noqa: E402
    load_and_simulate_physical,
    run_qcc_compile,
)
from runners.qiskit_reference import run_logical_reference  # noqa: E402
from verify_case import verify_all_cases, verify_canonical_case  # noqa: E402


def _make_inline_case(
    case_id: str,
    logical_qasm: str,
    logical_qubits: int,
    *,
    bindings: dict[str, float] | None = None,
    verification_mode: str = "exhaustive_unitary",
    expected_status: str = "PASS",
    sampled_state_count: int = 8,
):
    raw = {
        "schema_id": "qcc.oracle_case",
        "schema_version": "1.0.0",
        "case_id": case_id,
        "description": f"Synthetic test case {case_id}",
        "logical_qubits": logical_qubits,
        "logical_qasm": logical_qasm,
        "parameter_bindings": bindings or {},
        "compiler_config": {
            "target": "linear",
            "global_phase_policy": "auto",
            "seed": 42,
        },
        "verification_mode": verification_mode,
        "sampled_state_count": sampled_state_count,
        "expected_oracle_status": expected_status,
        "limitations": [],
    }
    return load_canonical_case_from_dict(raw)


def _rewrite_manifest_with_qasm(
    manifest_path: pathlib.Path,
    qasm_path: pathlib.Path,
    new_qasm_text: str,
    *,
    update_sha256: bool = True,
    manifest_mutator=None,
) -> None:
    """Test helper mutating a physical QASM and/or manifest on disk."""
    raw_bytes = new_qasm_text.encode("utf-8")
    qasm_path.write_bytes(raw_bytes)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if update_sha256:
        manifest["emitted_circuit"]["sha256"] = hashlib.sha256(raw_bytes).hexdigest()
        manifest["emitted_circuit"]["byte_length"] = len(raw_bytes)
    if manifest_mutator is not None:
        manifest_mutator(manifest)
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")


class TestLayoutAndEndianNormalization(unittest.TestCase):
    """Independent tests for Phase 65 little-endian & layout embedding helpers."""

    def test_identity_reversed_and_nontrivial_permutations(self) -> None:
        # 2 logical qubits: q0=LSB, q1=MSB -> index k = 2*q1 + q0
        # Basis |q1 q0> = |10> -> logical_idx = 2
        self.assertEqual(map_logical_basis_index(2, {0: 0, 1: 1}, 2), 2)
        # Reversed layout {0: 1, 1: 0}: logical |q1=1, q0=0> -> p0=1, p1=0 -> physical index 1
        self.assertEqual(map_logical_basis_index(2, {0: 1, 1: 0}, 2), 1)
        # Spare physical qubits: logical 2 -> physical 4 with mapping {0: 1, 1: 3}
        # logical index 3 (|11>) -> physical bits 1 and 3 -> (1<<1)|(1<<3) = 10
        self.assertEqual(map_logical_basis_index(3, {0: 1, 1: 3}, 2), 10)

    def test_embed_logical_statevector_sets_ancillas_to_zero(self) -> None:
        # 1-qubit |-> mapped to physical qubit p[1] out of 2 physical qubits
        logical = np.array([1.0, -1.0]) / np.sqrt(2.0)
        embedded = embed_logical_statevector(logical, {0: 1}, num_logical=1, num_physical=2)
        # p[0] must be |0>, so indices 1 (|01>) and 3 (|11>) must be zero!
        expected = np.array([1.0, 0.0, -1.0, 0.0]) / np.sqrt(2.0)
        self.assertLess(np.max(np.abs(embedded - expected)), 1e-15)
        self.assertAlmostEqual(compute_residual_ancilla_population(embedded, (0,)), 0.0)

    def test_non_injective_and_out_of_range_layouts_rejected(self) -> None:
        with self.assertRaises(ValueError):
            parse_and_validate_single_layout({0: 1, 1: 1}, num_logical=2, num_physical=3)
        with self.assertRaises(ValueError):
            parse_and_validate_single_layout({0: 0, 1: 3}, num_logical=2, num_physical=3)
        with self.assertRaises(ValueError):
            parse_and_validate_single_layout({0: 0}, num_logical=2, num_physical=2)


class TestCanonicalCasesEndToEnd(unittest.TestCase):
    """Run the full Session 3 canonical corpus end-to-end against QCC and Qiskit."""

    def test_all_ten_canonical_cases_match_expected_status_and_reproduce(self) -> None:
        cases = load_all_canonical_cases(DEFAULT_CASES_DIR)
        self.assertEqual(len(cases), 10)

        with tempfile.TemporaryDirectory() as tmp1, tempfile.TemporaryDirectory() as tmp2:
            r1 = verify_all_cases(work_root=pathlib.Path(tmp1))
            r2 = verify_all_cases(work_root=pathlib.Path(tmp2))

        for case, res1, res2 in zip(cases, r1, r2):
            with self.subTest(case_id=case.case_id):
                validate_comparison_result_schema(res1)
                self.assertEqual(res1["status"], case.expected_oracle_status)
                if case.expected_oracle_status == "PASS":
                    self.assertIsNotNone(res1["max_amplitude_error"])
                    self.assertLessEqual(res1["max_amplitude_error"], 1e-10)
                    self.assertTrue(res1["ancilla_zero_ok"])
                    self.assertLessEqual(res1["max_residual_ancilla_population"], 1e-12)
                    self.assertEqual(res1["strict_phase_accounting"]["status"], "PASS")
                else:
                    self.assertNotEqual(res1["status"], "PASS")

                # Deterministic serialization check (excluding temp work_dir paths)
                n1 = dict(res1)
                n2 = dict(res2)
                for obj in (n1, n2):
                    if obj["manifest_identity"]:
                        obj["manifest_identity"] = {
                            k: v for k, v in obj["manifest_identity"].items() if k != "path"
                        }
                    if obj["physical_identity"]:
                        obj["physical_identity"] = {
                            k: v for k, v in obj["physical_identity"].items() if k != "path"
                        }
                self.assertEqual(comparison_result_digest(n1), comparison_result_digest(n2))

    def test_sampled_statevector_mode_equivalent_and_non_exhaustive_flag(self) -> None:
        case = _make_inline_case(
            "sampled_qft3",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[3] q;\nh q[0];\ncx q[0], q[2];\nrz(0.3) q[1];\ncx q[2], q[1];\n',
            3,
            verification_mode="sampled_statevector",
            sampled_state_count=6,
        )
        with tempfile.TemporaryDirectory() as tmp:
            res = verify_canonical_case(case, work_root=pathlib.Path(tmp))
        self.assertEqual(res["status"], "PASS")
        self.assertFalse(res["exhaustive"])
        self.assertEqual(res["verification_mode"], "sampled_statevector")
        self.assertLess(res["max_amplitude_error"], 1e-12)
        self.assertTrue(res["ancilla_zero_ok"])


class TestFaultDetectionAndCompliance(unittest.TestCase):
    """Verify the comparator detects injected semantic, layout, phase, ancilla, and contract faults."""

    def _compile_case(self, case, tmp_path: pathlib.Path):
        logical_ref = run_logical_reference(case)
        self.assertTrue(logical_ref.ok)
        compile_exec = run_qcc_compile(case, tmp_path)
        self.assertTrue(compile_exec.ok, msg=compile_exec.stderr)
        return logical_ref, compile_exec

    def test_detects_deliberately_incorrect_physical_gate(self) -> None:
        case = _make_inline_case(
            "fault_bad_gate",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n',
            2,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)
            wrong_qasm = 'OPENQASM 3.0;\ninclude "stdgates.inc";\n\nqubit[2] p;\n\nx p[0];\ncx p[0], p[1];\n'
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path, compile_exec.physical_qasm_path, wrong_qasm
            )
            phys = load_and_simulate_physical(compile_exec.manifest_path)
            res = compare_logical_vs_physical(
                case,
                logical_ref,
                phys,
                compile_exec=compile_exec,
                failure_artifacts_root=tmp_path / "artifacts",
            )
            self.assertEqual(res["status"], "MISMATCH")
            self.assertEqual(res["failure_category"], "unitary_or_layout_mismatch")
            self.assertIsNotNone(res["failure_artifact_dir"])
            art = pathlib.Path(res["failure_artifact_dir"])
            for expected_file in (
                "case.json",
                "logical_input.qasm",
                "physical_output.qasm",
                "manifest.json",
                "comparison_result.json",
                "diagnostics.txt",
            ):
                self.assertTrue((art / expected_file).exists(), msg=expected_file)

    def test_detects_reversed_cx_control_target(self) -> None:
        case = _make_inline_case(
            "fault_reversed_cx",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\ncx q[0], q[1];\n',
            2,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)
            raw = compile_exec.physical_qasm_path.read_text(encoding="utf-8")
            self.assertIn("cx p[0], p[1];", raw)
            mutated = raw.replace("cx p[0], p[1];", "cx p[1], p[0];")
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path, compile_exec.physical_qasm_path, mutated
            )
            phys = load_and_simulate_physical(compile_exec.manifest_path)
            res = compare_logical_vs_physical(case, logical_ref, phys)
            self.assertEqual(res["status"], "MISMATCH")
            self.assertGreater(res["max_amplitude_error"], 0.9)

    def test_detects_incorrect_initial_and_final_layouts(self) -> None:
        case = _make_inline_case(
            "fault_layouts",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n',
            2,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)
            orig_manifest = json.loads(compile_exec.manifest_path.read_text(encoding="utf-8"))

            # Wrong initial layout only
            bad_init = json.loads(json.dumps(orig_manifest))
            bad_init["layout"]["initial_logical_to_physical"] = {"0": 1, "1": 0}
            init_path = tmp_path / "bad_init.manifest.json"
            init_path.write_text(json.dumps(bad_init), encoding="utf-8")
            res_init = compare_logical_vs_physical(
                case, logical_ref, load_and_simulate_physical(init_path)
            )
            self.assertEqual(res_init["status"], "MISMATCH")

            # Wrong final layout only
            bad_final = json.loads(json.dumps(orig_manifest))
            bad_final["layout"]["final_logical_to_physical"] = {"0": 1, "1": 0}
            final_path = tmp_path / "bad_final.manifest.json"
            final_path.write_text(json.dumps(bad_final), encoding="utf-8")
            res_final = compare_logical_vs_physical(
                case, logical_ref, load_and_simulate_physical(final_path)
            )
            self.assertEqual(res_final["status"], "MISMATCH")

    def test_detects_relative_phase_mismatch_not_repairable_with_one_common_phase(self) -> None:
        # Both circuits map |0> -> |0> and |1> -> +/- |1>, so computational-basis
        # probabilities are IDENTICAL (100% pass for histogram-only verifiers!), yet
        # a single common phase cannot reconcile +1 on |0> and -1 on |1>.
        case = _make_inline_case(
            "fault_relative_phase",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\n',
            1,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)
            # Apply rz(pi) on physical p[0]: diag(-i, +i) = -i * Z -> relative sign flip!
            z_qasm = 'OPENQASM 3.0;\ninclude "stdgates.inc";\n\nqubit[2] p;\n\nrz(3.141592653589793) p[0];\n'
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path, compile_exec.physical_qasm_path, z_qasm
            )
            phys = load_and_simulate_physical(compile_exec.manifest_path)
            res = compare_logical_vs_physical(case, logical_ref, phys)
            self.assertEqual(res["status"], "MISMATCH")
            self.assertGreater(res["max_amplitude_error"], 1.0)

    def test_common_phase_equivalence_vs_informational_strict_phase_accounting(self) -> None:
        case = _make_inline_case(
            "phase_accounting_split",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\ngphase(0.25);\nh q[0];\n',
            1,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)

            # Corrupt ONLY manifest recorded global_phase radians cleanly -> physical QASM
            # still implements the exact unitary up to common phase, so semantic verdict
            # remains PASS while strict phase accounting reports MISMATCH.
            def corrupt_manifest_phase(m: dict) -> None:
                m["global_phase"]["radians"] = float(m["global_phase"]["radians"]) + 0.5

            qasm_txt = compile_exec.physical_qasm_path.read_text(encoding="utf-8")
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path,
                compile_exec.physical_qasm_path,
                qasm_txt,
                update_sha256=False,
                manifest_mutator=corrupt_manifest_phase,
            )
            phys = load_and_simulate_physical(compile_exec.manifest_path)
            res = compare_logical_vs_physical(case, logical_ref, phys)
            self.assertEqual(res["status"], "PASS")
            self.assertEqual(res["strict_phase_accounting"]["status"], "MISMATCH")
            self.assertTrue(res["strict_phase_accounting"]["informational_only"])

    def test_detects_residual_ancilla_population_and_entanglement(self) -> None:
        case = _make_inline_case(
            "fault_ancilla_policy",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nh q[0];\n',
            1,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)
            clean_qasm = compile_exec.physical_qasm_path.read_text(encoding="utf-8")

            # 1. Residual ancilla |1> population: add `rz(pi) p[1]; sx p[1]; sx p[1];` (X on ancilla p[1])
            pop_qasm = clean_qasm + "sx p[1];\nsx p[1];\n"
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path, compile_exec.physical_qasm_path, pop_qasm
            )
            res_pop = compare_logical_vs_physical(
                case, logical_ref, load_and_simulate_physical(compile_exec.manifest_path)
            )
            self.assertEqual(res_pop["status"], "MISMATCH")
            self.assertEqual(res_pop["failure_category"], "residual_ancilla_contamination")
            self.assertFalse(res_pop["ancilla_zero_ok"])
            self.assertAlmostEqual(res_pop["max_residual_ancilla_population"], 1.0, places=10)

            # 2. Residual ancilla entanglement: CX from logical p[0] into spare ancilla p[1]
            ent_qasm = clean_qasm + "cx p[0], p[1];\n"
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path, compile_exec.physical_qasm_path, ent_qasm
            )
            res_ent = compare_logical_vs_physical(
                case, logical_ref, load_and_simulate_physical(compile_exec.manifest_path)
            )
            self.assertEqual(res_ent["status"], "MISMATCH")
            self.assertEqual(res_ent["failure_category"], "residual_ancilla_contamination")
            self.assertFalse(res_ent["ancilla_zero_ok"])
            self.assertAlmostEqual(res_ent["max_residual_ancilla_population"], 0.5, places=10)

    def test_detects_qasm_manifest_hash_mismatch_and_invalid_manifest_schema(self) -> None:
        case = _make_inline_case(
            "fault_contract",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nh q[0];\n',
            2,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            logical_ref, compile_exec = self._compile_case(case, tmp_path)

            # Mutate QASM without updating SHA-256 -> INVALID_INPUT + failure artifact bundle
            orig_qasm = compile_exec.physical_qasm_path.read_text(encoding="utf-8")
            _rewrite_manifest_with_qasm(
                compile_exec.manifest_path,
                compile_exec.physical_qasm_path,
                orig_qasm + "// tampered\n",
                update_sha256=False,
            )
            res_hash = compare_logical_vs_physical(
                case,
                logical_ref,
                load_and_simulate_physical(compile_exec.manifest_path),
                failure_artifacts_root=tmp_path / "hash_artifacts",
            )
            self.assertEqual(res_hash["status"], "INVALID_INPUT")
            self.assertEqual(res_hash["failure_category"], "qasm_manifest_hash_mismatch")
            self.assertTrue((tmp_path / "hash_artifacts" / case.case_id / "manifest.json").exists())
            self.assertTrue(
                (tmp_path / "hash_artifacts" / case.case_id / "physical_output.qasm").exists()
            )

            # Corrupt manifest schema (unknown top-level property) -> INVALID_INPUT
            bad_manifest = json.loads(compile_exec.manifest_path.read_text(encoding="utf-8"))
            bad_manifest["bogus_field"] = 123
            bad_path = tmp_path / "bad_schema.manifest.json"
            bad_path.write_text(json.dumps(bad_manifest), encoding="utf-8")
            res_schema = compare_logical_vs_physical(
                case, logical_ref, load_and_simulate_physical(bad_path)
            )
            self.assertEqual(res_schema["status"], "INVALID_INPUT")
            self.assertEqual(res_schema["failure_category"], "manifest_schema_invalid")

    def test_unsupported_non_unitary_and_missing_parameter_bindings(self) -> None:
        reset_case = _make_inline_case(
            "unsupported_reset",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nreset q[0];\n',
            1,
            expected_status="UNSUPPORTED",
        )
        unbound_case = _make_inline_case(
            "unsupported_unbound",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nrz(theta) q[0];\n',
            1,
            bindings={},
            expected_status="UNSUPPORTED",
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            res_reset = verify_canonical_case(reset_case, work_root=tmp_path)
            res_unbound = verify_canonical_case(unbound_case, work_root=tmp_path)
            self.assertEqual(res_reset["status"], "UNSUPPORTED")
            self.assertEqual(res_unbound["status"], "UNSUPPORTED")
            # Wrong concrete binding vs reference must fail cleanly as MISMATCH when swapped
            param_case_a = _make_inline_case(
                "param_a",
                'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nrz(theta) q[0];\ncx q[0], q[1];\n',
                2,
                bindings={"theta": 0.75},
            )
            param_case_b = _make_inline_case(
                "param_b",
                'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nrz(theta) q[0];\ncx q[0], q[1];\n',
                2,
                bindings={"theta": 1.75},
            )
            exec_a = run_qcc_compile(param_case_a, tmp_path / "p_mismatch")
            self.assertTrue(exec_a.ok)
            res_wrong_bind = compare_logical_vs_physical(
                param_case_b,
                run_logical_reference(param_case_b),
                load_and_simulate_physical(exec_a.manifest_path),
            )
            self.assertEqual(res_wrong_bind["status"], "MISMATCH")

    def test_serialized_comparison_result_deterministic_and_schema_valid(self) -> None:
        case = _make_inline_case(
            "serial_det",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n',
            2,
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            res = verify_canonical_case(case, work_root=tmp_path)
            s1 = serialize_comparison_result(res)
            s2 = serialize_comparison_result(res)
            self.assertEqual(s1, s2)
            self.assertIn('"schema_id": "qcc.comparison_result"', s1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
