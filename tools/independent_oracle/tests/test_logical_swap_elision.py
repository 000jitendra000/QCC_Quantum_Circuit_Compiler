"""Automated independent oracle verification tests for Logical SWAP Elision (ADR-0006).

Validates:
1. QCC with `--elide-swaps` compiles circuits with terminal, start, middle, and chained swaps.
2. Independent Qiskit reference simulation confirms exact mathematical equivalence (PASS, error < 1e-14).
3. QFT4 showcase exhibits expected 6 CX gate reduction (27 -> 21 CX) while passing oracle.
4. Mandatory Fault Injection: Omitting or corrupting final layout permutation in manifest fails closed.
"""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import load_canonical_case_from_dict
from equivalence.comparator import OracleTolerances, compare_logical_vs_physical
from runners.qcc_runner import load_and_simulate_physical, run_qcc_compile
from runners.qiskit_reference import run_logical_reference


def _make_case(
    case_id: str,
    logical_qasm: str,
    logical_qubits: int,
    *,
    target: str = "linear",
    seed: int = 42,
):
    raw = {
        "schema_id": "qcc.oracle_case",
        "schema_version": "1.0.0",
        "case_id": case_id,
        "description": f"Test case for logical swap elision: {case_id}",
        "logical_qubits": logical_qubits,
        "logical_qasm": logical_qasm,
        "parameter_bindings": {},
        "compiler_config": {
            "target": target,
            "global_phase_policy": "auto",
            "seed": seed,
        },
        "verification_mode": "exhaustive_unitary",
        "expected_oracle_status": "PASS",
        "limitations": [],
    }
    return load_canonical_case_from_dict(raw)


class TestLogicalSwapElisionOracle(unittest.TestCase):
    """Independent oracle tests verifying QCC `--elide-swaps` correctness."""

    def test_terminal_swap_elision_04_logical_swap(self) -> None:
        qasm = """OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
h q[0];
t q[0];
x q[1];
rz(pi / 5) q[1];
swap q[0], q[1];
"""
        case = _make_case("04_swap_test", qasm, 2)
        with tempfile.TemporaryDirectory(prefix="qcc_swap_elide_") as tmp:
            work_dir = pathlib.Path(tmp)

            # Compile with --elide-swaps
            exec_res = run_qcc_compile(
                case,
                work_dir,
                extra_args=["--elide-swaps"],
            )
            self.assertTrue(exec_res.ok, f"Compilation failed: {exec_res.stderr}")

            # Independent physical simulation
            phys_res = load_and_simulate_physical(exec_res.manifest_path)
            self.assertTrue(phys_res.ok, f"Physical import failed: {phys_res.error_detail}")

            # Assert 0 CX emitted
            cx_count = phys_res.gate_counts.get("cx", 0)
            self.assertEqual(cx_count, 0, f"Expected 0 CX with --elide-swaps, got {cx_count}")

            # Independent logical reference simulation
            log_ref = run_logical_reference(case)
            self.assertTrue(log_ref.ok)

            # Compare logical reference vs physical circuit under composed layout
            comparison = compare_logical_vs_physical(case, log_ref, phys_res)
            self.assertEqual(comparison["status"], "PASS")
            self.assertLess(
                comparison["max_amplitude_error"],
                1e-14,
                f"Max amplitude error too large: {comparison['max_amplitude_error']}",
            )

    def test_start_and_middle_swaps(self) -> None:
        circuits = [
            # Start swap
            (
                "start_swap",
                """OPENQASM 3.0;
include "stdgates.inc";
qubit[2] q;
swap q[0], q[1];
h q[0];
x q[1];
""",
                2,
            ),
            # Middle swap
            (
                "middle_swap",
                """OPENQASM 3.0;
include "stdgates.inc";
qubit[2] q;
h q[0];
swap q[0], q[1];
x q[0];
""",
                2,
            ),
            # Consecutive chained swaps
            (
                "swap_chain",
                """OPENQASM 3.0;
include "stdgates.inc";
qubit[3] q;
h q[0];
t q[1];
swap q[0], q[1];
swap q[1], q[2];
x q[0];
""",
                3,
            ),
        ]

        for cid, qasm, n in circuits:
            case = _make_case(cid, qasm, n)
            with tempfile.TemporaryDirectory(prefix=f"qcc_elide_{cid}_") as tmp:
                work_dir = pathlib.Path(tmp)
                exec_res = run_qcc_compile(case, work_dir, extra_args=["--elide-swaps"])
                self.assertTrue(exec_res.ok, f"Compilation failed for {cid}: {exec_res.stderr}")

                phys_res = load_and_simulate_physical(exec_res.manifest_path)
                self.assertTrue(phys_res.ok, f"Physical simulation failed for {cid}: {phys_res.error_detail}")

                log_ref = run_logical_reference(case)
                self.assertTrue(log_ref.ok)

                comp = compare_logical_vs_physical(case, log_ref, phys_res)
                self.assertEqual(comp["status"], "PASS", f"Oracle failed for {cid}: {comp.get('error_detail')}")
                self.assertLess(comp["max_amplitude_error"], 1e-14)

    def test_qft4_showcase_elide_swaps(self) -> None:
        qasm_path = REPO_ROOT / "examples" / "showcase.qasm"
        self.assertTrue(qasm_path.exists())
        qasm = qasm_path.read_text(encoding="utf-8")

        case = _make_case("qft4_elide_test", qasm, 4)
        with tempfile.TemporaryDirectory(prefix="qcc_qft4_elide_") as tmp:
            work_dir = pathlib.Path(tmp)

            exec_res = run_qcc_compile(case, work_dir, extra_args=["--elide-swaps"])
            self.assertTrue(exec_res.ok, f"Compilation failed: {exec_res.stderr}")

            phys_res = load_and_simulate_physical(exec_res.manifest_path)
            self.assertTrue(phys_res.ok, f"Physical import failed: {phys_res.error_detail}")

            # Verify CX reduction: 21 CX instead of baseline 27 CX (-6 CX)
            cx_count = phys_res.gate_counts.get("cx", 0)
            self.assertEqual(cx_count, 21, f"Expected 21 CX for QFT4 with --elide-swaps, got {cx_count}")

            # Independent oracle verification
            log_ref = run_logical_reference(case)
            self.assertTrue(log_ref.ok)

            comp = compare_logical_vs_physical(case, log_ref, phys_res)
            self.assertEqual(comp["status"], "PASS")
            self.assertLess(comp["max_amplitude_error"], 1e-14)

    def test_mandatory_fault_injection_omitted_layout_fails_closed(self) -> None:
        """Assert that if final layout permutation is omitted or corrupted, oracle MUST fail."""
        qasm = """OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
h q[0];
t q[0];
swap q[0], q[1];
"""
        case = _make_case("fi_swap_test", qasm, 2)
        with tempfile.TemporaryDirectory(prefix="qcc_fi_elide_") as tmp:
            work_dir = pathlib.Path(tmp)

            exec_res = run_qcc_compile(case, work_dir, extra_args=["--elide-swaps"])
            self.assertTrue(exec_res.ok)

            # Untampered run passes oracle
            phys_res_valid = load_and_simulate_physical(exec_res.manifest_path)
            log_ref = run_logical_reference(case)
            comp_valid = compare_logical_vs_physical(case, log_ref, phys_res_valid)
            self.assertEqual(comp_valid["status"], "PASS")

            # FAULT INJECTION: Tamper with manifest final layout (set to identity, omitting swap)
            manifest_data = json.loads(exec_res.manifest_path.read_text(encoding="utf-8"))
            manifest_data["layout"]["final_logical_to_physical"] = {"0": 0, "1": 1}
            exec_res.manifest_path.write_text(json.dumps(manifest_data, indent=2), encoding="utf-8")

            # Re-simulate with tampered manifest
            phys_res_tampered = load_and_simulate_physical(exec_res.manifest_path)
            self.assertTrue(phys_res_tampered.ok)

            comp_tampered = compare_logical_vs_physical(case, log_ref, phys_res_tampered)
            self.assertEqual(
                comp_tampered["status"],
                "MISMATCH",
                "Independent oracle MUST fail closed (MISMATCH) when final layout permutation is corrupted",
            )
            self.assertGreater(
                comp_tampered["max_amplitude_error"],
                0.5,
                f"Error difference must be large, got {comp_tampered['max_amplitude_error']}",
            )


if __name__ == "__main__":
    unittest.main()
