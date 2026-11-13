#!/usr/bin/env python3
"""
test_cancellation_scanner.py

Unit tests for the Phase 78 fail-closed read-only cancellation scanner
(tools/independent_oracle/benchmarks/audit_two_qubit_cancellation.py).
"""

import unittest
from tools.independent_oracle.benchmarks.audit_two_qubit_cancellation import (
    parse_operations,
    is_valid_cx_commutation,
    is_valid_cz_commutation,
)

class TestCancellationScanner(unittest.TestCase):
    def test_unknown_or_unsupported_gate_blocks(self):
        """Unknown or uncatalogued gates must block cancellation fail-closed, even if disjoint."""
        # Custom unknown gate on disjoint wire q[2]
        qasm = """
        cx q[0], q[1];
        custom_gate q[2];
        cx q[0], q[1];
        """
        ops = parse_operations(qasm)
        self.assertEqual(ops[1][0], "unsupported")
        self.assertFalse(is_valid_cx_commutation(ops, 0, 2, 0, 1))

        # Unknown parameterized gate
        qasm2 = """
        cx q[0], q[1];
        unknown_rot(1.23) q[2];
        cx q[0], q[1];
        """
        ops2 = parse_operations(qasm2)
        self.assertEqual(ops2[1][0], "unsupported")
        self.assertFalse(is_valid_cx_commutation(ops2, 0, 2, 0, 1))

    def test_non_unitary_and_barriers_block(self):
        """Barriers, measurement, and reset must strictly block cancellation."""
        for kw in ["barrier", "measure", "reset"]:
            qasm = f"""
            cx q[0], q[1];
            {kw} q[0];
            cx q[0], q[1];
            """
            ops = parse_operations(qasm)
            self.assertFalse(is_valid_cx_commutation(ops, 0, 2, 0, 1))

    def test_control_flow_blocks(self):
        """Control-flow constructs must be parsed as unsupported and block cancellation."""
        qasm = """
        cx q[0], q[1];
        if (c == 1) x q[2];
        cx q[0], q[1];
        """
        ops = parse_operations(qasm)
        self.assertEqual(ops[1][0], "unsupported")
        self.assertFalse(is_valid_cx_commutation(ops, 0, 2, 0, 1))

    def test_cx_requires_ordered_operands(self):
        """CX(c, t) requires ordered operand identity: CX(0, 1) and CX(1, 0) do not cancel."""
        qasm = """
        cx q[0], q[1];
        cx q[1], q[0];
        """
        ops = parse_operations(qasm)
        # Verify that ops[1] has reversed operands (1, 0) != (0, 1)
        self.assertNotEqual(ops[0][1], ops[1][1])
        # An intervening reversed CX touches both wires and must not commute
        qasm_intervene = """
        cx q[0], q[1];
        cx q[1], q[0];
        cx q[0], q[1];
        """
        ops_int = parse_operations(qasm_intervene)
        self.assertFalse(is_valid_cx_commutation(ops_int, 0, 2, 0, 1))

    def test_cz_symmetric_operands(self):
        """CZ(a, b) is symmetric: CZ(0, 1) and CZ(1, 0) represent the identical unitary."""
        # Commuting intervening Rz on wire 0
        qasm = """
        cz q[0], q[1];
        rz(0.5) q[0];
        cz q[1], q[0];
        """
        ops = parse_operations(qasm)
        self.assertTrue(is_valid_cz_commutation(ops, 0, 2, 0, 1))

    def test_cx_commutation_rules(self):
        """CX control commutes with Z-axis, target commutes with X-axis, Ry blocks both."""
        # Rz on control wire (q[0]): commutes
        qasm1 = "cx q[0], q[1]; rz(0.5) q[0]; cx q[0], q[1];"
        self.assertTrue(is_valid_cx_commutation(parse_operations(qasm1), 0, 2, 0, 1))

        # Rx on control wire (q[0]): blocks
        qasm2 = "cx q[0], q[1]; rx(0.5) q[0]; cx q[0], q[1];"
        self.assertFalse(is_valid_cx_commutation(parse_operations(qasm2), 0, 2, 0, 1))

        # Rx on target wire (q[1]): commutes
        qasm3 = "cx q[0], q[1]; rx(0.5) q[1]; cx q[0], q[1];"
        self.assertTrue(is_valid_cx_commutation(parse_operations(qasm3), 0, 2, 0, 1))

        # Rz on target wire (q[1]): blocks
        qasm4 = "cx q[0], q[1]; rz(0.5) q[1]; cx q[0], q[1];"
        self.assertFalse(is_valid_cx_commutation(parse_operations(qasm4), 0, 2, 0, 1))

        # Ry on either wire: blocks
        qasm5 = "cx q[0], q[1]; ry(0.5) q[0]; cx q[0], q[1];"
        self.assertFalse(is_valid_cx_commutation(parse_operations(qasm5), 0, 2, 0, 1))
        qasm6 = "cx q[0], q[1]; ry(0.5) q[1]; cx q[0], q[1];"
        self.assertFalse(is_valid_cx_commutation(parse_operations(qasm6), 0, 2, 0, 1))

    def test_cz_commutation_rules(self):
        """CZ commutes with Z-axis on either operand, but NOT with X or Y (never generalize CX)."""
        # Rz on either operand: commutes
        qasm1 = "cz q[0], q[1]; rz(0.5) q[1]; cz q[0], q[1];"
        self.assertTrue(is_valid_cz_commutation(parse_operations(qasm1), 0, 2, 0, 1))

        # Rx on either operand: blocks (contrast with CX target where Rx commutes!)
        qasm2 = "cz q[0], q[1]; rx(0.5) q[1]; cz q[0], q[1];"
        self.assertFalse(is_valid_cz_commutation(parse_operations(qasm2), 0, 2, 0, 1))

        # Ry on either operand: blocks
        qasm3 = "cz q[0], q[1]; ry(0.5) q[0]; cz q[0], q[1];"
        self.assertFalse(is_valid_cz_commutation(parse_operations(qasm3), 0, 2, 0, 1))

if __name__ == "__main__":
    unittest.main()
