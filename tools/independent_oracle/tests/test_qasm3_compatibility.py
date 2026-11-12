"""Automated OpenQASM 3 compatibility test suite for QCC Independent Oracle (M7 Session 2).

Verifies:
1. Exact pinned Python/Qiskit dependency versions against requirements.txt.
2. Direct import/simulation compatibility (and raw collision behavior) on
   unmodified physical OpenQASM 3 + manifests emitted by QCC.
3. Construct-level semantic preservation for rz, sx, cx, gphase, parameter
   expressions, scalar/multi-register declarations, and register alpha-renaming.
4. Explicit fail-closed classification (never silently dropping phase or ops)
   for malformed QASM, non-unitary ops, unbound parameters, and unsupported gates.
5. Consistency with manifest schema (`physical_qubits`, `global_phase`, gate counts)
   and deterministic reproducibility.
"""

from __future__ import annotations

import math
import pathlib
import re
import sys
import tempfile
import unittest

import numpy as np
import qiskit.qasm3

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from qasm3_compat import (  # noqa: E402
    CompatStatus,
    PINNED_PACKAGES,
    REQUIREMENTS_PATH,
    adapt_colliding_registers,
    check_pinned_environment,
    probe_qasm3_compatibility,
    validate_manifest_consistency,
)


FIXTURES_DIR = ORACLE_DIR / "fixtures" / "session2"


class TestPinnedEnvironment(unittest.TestCase):
    """Verify pinned Python/Qiskit runtime reproducibility."""

    def test_installed_versions_match_pinned_map(self) -> None:
        installed = check_pinned_environment()
        for pkg, expected in PINNED_PACKAGES.items():
            self.assertEqual(installed[pkg], expected)

    def test_requirements_txt_uses_exact_pins(self) -> None:
        lines = [
            line.strip()
            for line in REQUIREMENTS_PATH.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.strip().startswith("#")
        ]
        parsed: dict[str, str] = {}
        for line in lines:
            match = re.fullmatch(r"([A-Za-z0-9_.\-]+)==([A-Za-z0-9_.+\-]+)", line)
            self.assertIsNotNone(
                match, msg=f"Requirement must use exact '==' pin, got: {line!r}"
            )
            assert match is not None
            parsed[match.group(1).lower().replace("_", "-")] = match.group(2)

        for pkg, expected in PINNED_PACKAGES.items():
            norm = pkg.lower().replace("_", "-")
            self.assertIn(norm, parsed, msg=f"Missing pinned dependency {pkg} in requirements.txt")
            self.assertEqual(parsed[norm], expected)


class TestRealQccEmittedFixtures(unittest.TestCase):
    """Verify unmodified real QCC physical OpenQASM 3 + manifests."""

    EXPECTED_FIXTURES = (
        "showcase_linear.manifest.json",
        "showcase_ring.manifest.json",
        "routing_linear.manifest.json",
        "rotations_linear.manifest.json",
        "global_phase_linear.manifest.json",
    )

    def test_all_real_qcc_fixtures_validate_and_simulate(self) -> None:
        for manifest_name in self.EXPECTED_FIXTURES:
            manifest_path = FIXTURES_DIR / manifest_name
            with self.subTest(fixture=manifest_name):
                self.assertTrue(manifest_path.exists())
                manifest, compat = validate_manifest_consistency(manifest_path)

                self.assertTrue(compat.parse_ok)
                # Raw QCC physical QASM uses `qubit[N] p;` + `include "stdgates.inc";`,
                # which collides with `stdgates.inc`'s `p` gate in qiskit-qasm3-import 0.6.0.
                self.assertFalse(compat.raw_construct_ok)
                self.assertIsNotNone(compat.raw_error)
                self.assertIn("Symbol 'p' already inserted", compat.raw_error or "")

                self.assertTrue(compat.adapted_construct_ok)
                self.assertTrue(compat.fallback_applied)
                self.assertEqual(compat.renamed_registers, {"p": "qcc_reg_p"})
                self.assertEqual(compat.status, CompatStatus.SUPPORTED_WITH_REGISTER_RENAME)

                self.assertTrue(compat.simulate_sv_ok)
                self.assertTrue(compat.simulate_op_ok)
                assert compat.statevector is not None
                assert compat.operator is not None

                dim = 1 << manifest["physical_qubits"]
                self.assertEqual(len(compat.statevector.data), dim)
                self.assertAlmostEqual(
                    float(np.linalg.norm(compat.statevector.data)), 1.0, places=12
                )
                self.assertEqual(compat.operator.data.shape, (dim, dim))
                self.assertTrue(compat.operator.is_unitary())

    def test_showcase_direct_cli_matches_committed_fixture(self) -> None:
        fresh_dir = REPO_ROOT / "target" / "debug" / "qcc"
        if not fresh_dir.exists():
            # Build binary only if absent? Keep unit tests offline/fast; compile if built.
            return
        import subprocess

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            out_qasm = tmp_path / "showcase.qasm"
            out_manifest = tmp_path / "showcase.manifest.json"
            proc = subprocess.run(
                [
                    str(fresh_dir),
                    "compile",
                    "--target",
                    "linear",
                    "-o",
                    str(out_qasm),
                    "--emit-manifest",
                    str(out_manifest),
                    str(REPO_ROOT / "examples" / "showcase.qasm"),
                ],
                cwd=REPO_ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            manifest, compat = validate_manifest_consistency(out_manifest, out_qasm)
            self.assertEqual(manifest["physical_qubits"], 4)
            self.assertEqual(compat.status, CompatStatus.SUPPORTED_WITH_REGISTER_RENAME)


class TestOpenQasm3ConstructSemantics(unittest.TestCase):
    """Test construct-by-construct parsing, construction, simulation, and semantics."""

    def test_rz_gate_unitary_matches_definition(self) -> None:
        theta = 1.25
        res = probe_qasm3_compatibility(
            f'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nrz({theta}) q[0];\n'
        )
        self.assertEqual(res.status, CompatStatus.SUPPORTED_DIRECT)
        assert res.operator is not None
        expected = np.diag([np.exp(-0.5j * theta), np.exp(0.5j * theta)])
        self.assertLess(np.max(np.abs(res.operator.data - expected)), 1e-12)

    def test_sx_gate_unitary_matches_definition(self) -> None:
        res = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nsx q[0];\n'
        )
        self.assertEqual(res.status, CompatStatus.SUPPORTED_DIRECT)
        assert res.operator is not None
        expected = 0.5 * np.array([[1 + 1j, 1 - 1j], [1 - 1j, 1 + 1j]])
        self.assertLess(np.max(np.abs(res.operator.data - expected)), 1e-12)

    def test_cx_operand_order_and_little_endian_convention(self) -> None:
        # CX(ctrl=q[0], targ=q[1]) in little-endian basis |q1 q0> flips |01> (idx 1) <-> |11> (idx 3)
        res_01 = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\ncx q[0], q[1];\n'
        )
        self.assertEqual(res_01.status, CompatStatus.SUPPORTED_DIRECT)
        assert res_01.operator is not None
        expected_01 = np.array(
            [
                [1, 0, 0, 0],
                [0, 0, 0, 1],
                [0, 0, 1, 0],
                [0, 1, 0, 0],
            ],
            dtype=complex,
        )
        self.assertLess(np.max(np.abs(res_01.operator.data - expected_01)), 1e-12)

        # Reversed operands CX(ctrl=q[1], targ=q[0]) flips |10> (idx 2) <-> |11> (idx 3)
        res_10 = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\ncx q[1], q[0];\n'
        )
        self.assertEqual(res_10.status, CompatStatus.SUPPORTED_DIRECT)
        assert res_10.operator is not None
        expected_10 = np.array(
            [
                [1, 0, 0, 0],
                [0, 1, 0, 0],
                [0, 0, 0, 1],
                [0, 0, 1, 0],
            ],
            dtype=complex,
        )
        self.assertLess(np.max(np.abs(res_10.operator.data - expected_10)), 1e-12)

    def test_gphase_preserved_across_circuit_operator_and_statevector(self) -> None:
        for angle in (0.0, math.pi / 4.0, 10.210176124166829, -1.5e-10):
            with self.subTest(angle=angle):
                res = probe_qasm3_compatibility(
                    f'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\n'
                    f"gphase({angle});\nsx q[0];\n"
                )
                self.assertEqual(res.status, CompatStatus.SUPPORTED_DIRECT)
                assert res.operator is not None and res.statevector is not None
                expected_sx = 0.5 * np.array([[1 + 1j, 1 - 1j], [1 - 1j, 1 + 1j]])
                expected_op = np.exp(1j * angle) * expected_sx
                self.assertLess(np.max(np.abs(res.operator.data - expected_op)), 1e-12)
                self.assertLess(np.max(np.abs(res.statevector.data - expected_op[:, 0])), 1e-12)

    def test_cumulative_gphase_preserved(self) -> None:
        res = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\n'
            "gphase(0.3);\nsx q[0];\ngphase(0.4);\n"
        )
        self.assertEqual(res.status, CompatStatus.SUPPORTED_DIRECT)
        assert res.global_phase_qiskit is not None and res.operator is not None
        self.assertAlmostEqual(res.global_phase_qiskit, 0.7, places=12)
        expected = np.exp(0.7j) * 0.5 * np.array([[1 + 1j, 1 - 1j], [1 - 1j, 1 + 1j]])
        self.assertLess(np.max(np.abs(res.operator.data - expected)), 1e-12)

    def test_parameter_expressions_supported(self) -> None:
        exprs = [
            ("pi / 4", math.pi / 4.0),
            ("3 * pi / 4 - 0.5", 3.0 * math.pi / 4.0 - 0.5),
            ("-(pi / 4)", -math.pi / 4.0),
            ("π / 2", math.pi / 2.0),
            ("1e-12", 1e-12),
        ]
        for expr_str, val in exprs:
            with self.subTest(expr=expr_str):
                res = probe_qasm3_compatibility(
                    f'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nrz({expr_str}) q[0];\n'
                )
                self.assertEqual(res.status, CompatStatus.SUPPORTED_DIRECT)
                assert res.operator is not None
                expected = np.diag([np.exp(-0.5j * val), np.exp(0.5j * val)])
                self.assertLess(np.max(np.abs(res.operator.data - expected)), 1e-12)

    def test_scalar_and_multi_register_declarations(self) -> None:
        scalar = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit q;\nsx q;\n'
        )
        self.assertEqual(scalar.status, CompatStatus.SUPPORTED_DIRECT)
        self.assertEqual(scalar.num_qubits, 1)

        multi = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] a;\nqubit[1] b;\ncx a[0], b[0];\n'
        )
        self.assertEqual(multi.status, CompatStatus.SUPPORTED_DIRECT)
        self.assertEqual(multi.num_qubits, 3)

    def test_register_alpha_rename_preserves_phase_gate_call(self) -> None:
        # Register `p` collides with `stdgates.inc` gate `p(λ)`.
        # Alpha-renaming must rename operand `p[0]` -> `qcc_reg_p[0]` while keeping gate `p(0.5)`!
        qasm = 'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] p;\np(0.5) p[0];\n'
        res = probe_qasm3_compatibility(qasm)
        self.assertEqual(res.status, CompatStatus.SUPPORTED_WITH_REGISTER_RENAME)
        self.assertEqual(res.renamed_registers, {"p": "qcc_reg_p"})
        self.assertEqual(res.count_ops, {"p": 1})
        assert res.operator is not None
        expected = np.diag([1.0, np.exp(0.5j)])
        self.assertLess(np.max(np.abs(res.operator.data - expected)), 1e-12)

    def test_reproducibility(self) -> None:
        qasm = (FIXTURES_DIR / "showcase_linear.qasm").read_text(encoding="utf-8")
        r1 = probe_qasm3_compatibility(qasm)
        r2 = probe_qasm3_compatibility(qasm)
        self.assertEqual(r1.adapted_qasm, r2.adapted_qasm)
        assert r1.statevector is not None and r2.statevector is not None
        assert r1.operator is not None and r2.operator is not None
        self.assertTrue(np.array_equal(r1.statevector.data, r2.statevector.data))
        self.assertTrue(np.array_equal(r1.operator.data, r2.operator.data))


class TestFailureAndUnsupportedClassification(unittest.TestCase):
    """Ensure errors and unsupported constructs fail closed and never silently drop ops."""

    def test_malformed_qasm_rejected(self) -> None:
        res = probe_qasm3_compatibility("OPENQASM 3.0;\nqubit[1] q;\nrz(1.0 q[0];\n")
        self.assertEqual(res.status, CompatStatus.PARSE_ERROR)
        self.assertFalse(res.parse_ok)
        self.assertFalse(res.simulate_sv_ok)
        self.assertFalse(res.simulate_op_ok)

    def test_non_unitary_measure_and_reset_classified_unsupported(self) -> None:
        meas_assign = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nbit[1] c;\nsx q[0];\nc[0] = measure q[0];\n'
        )
        self.assertEqual(meas_assign.status, CompatStatus.UNSUPPORTED_NON_UNITARY)
        self.assertTrue(meas_assign.parse_ok)
        self.assertFalse(meas_assign.simulate_op_ok)

        reset = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\nreset q[0];\n'
        )
        self.assertEqual(reset.status, CompatStatus.UNSUPPORTED_NON_UNITARY)
        self.assertFalse(reset.simulate_op_ok)

    def test_unbound_parameter_classified_unsupported(self) -> None:
        res = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "stdgates.inc";\ninput float theta;\nqubit[1] q;\nrz(theta) q[0];\n'
        )
        self.assertEqual(res.status, CompatStatus.UNSUPPORTED_UNBOUND_PARAM)
        self.assertTrue(res.adapted_construct_ok)
        self.assertFalse(res.simulate_sv_ok)
        self.assertFalse(res.simulate_op_ok)

    def test_qiskit_missing_stdgates_classified_unsupported(self) -> None:
        for gate_stmt in ("sxdg q[0];", "i q[0];", "u(0.1, 0.2, 0.3) q[0];"):
            with self.subTest(gate=gate_stmt):
                res = probe_qasm3_compatibility(
                    f'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[1] q;\n{gate_stmt}\n'
                )
                self.assertEqual(res.status, CompatStatus.UNSUPPORTED_CONSTRUCT)
                self.assertTrue(res.parse_ok)
                self.assertFalse(res.adapted_construct_ok)

    def test_non_stdgates_include_classified_unsupported(self) -> None:
        res = probe_qasm3_compatibility(
            'OPENQASM 3.0;\ninclude "custom.inc";\nqubit[1] q;\n'
        )
        self.assertEqual(res.status, CompatStatus.UNSUPPORTED_CONSTRUCT)

    def test_refuses_rename_with_custom_gate_scope(self) -> None:
        with self.assertRaises(ValueError):
            adapt_colliding_registers(
                'OPENQASM 3.0;\ninclude "stdgates.inc";\n'
                "gate mygate q { sx q; }\nqubit[1] p;\nmygate p[0];\n"
            )

    def test_manifest_qubit_count_mismatch_rejected(self) -> None:
        bad_qasm_text = 'OPENQASM 3.0;\ninclude "stdgates.inc";\nqubit[2] q;\nsx q[0];\n'
        manifest_path = FIXTURES_DIR / "showcase_linear.manifest.json"
        with tempfile.TemporaryDirectory() as tmp:
            bad_qasm = pathlib.Path(tmp) / "wrong_width.qasm"
            bad_qasm.write_text(bad_qasm_text, encoding="utf-8")
            with self.assertRaises(ValueError):
                validate_manifest_consistency(manifest_path, bad_qasm)


if __name__ == "__main__":
    unittest.main(verbosity=2)
