"""Independent Qiskit logical-reference runner (M7 Phase 64).

Constructs and simulates the logical reference circuit purely from neutral
canonical inputs using the pinned Qiskit stack and Session 2 OpenQASM 3
compatibility layer. Never inspects QCC outputs or calls QCC code.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import pathlib
import sys
from typing import Mapping

import openqasm3
from openqasm3 import ast
import openqasm3.parser
import qiskit
import qiskit.qasm3
from qiskit.quantum_info import Operator, Statevector

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase, sha256_bytes  # noqa: E402
from qasm3_compat import CompatStatus, adapt_colliding_registers  # noqa: E402


@dataclass(frozen=True)
class LogicalReferenceResult:
    """Result returned by the independent Qiskit logical-reference runner."""

    ok: bool
    status: str  # "OK", "UNSUPPORTED", "INVALID_INPUT", "ERROR"
    failure_category: str | None
    logical_qubits: int | None
    global_phase_radians: float | None
    gate_counts: dict[str, int] = field(default_factory=dict)
    logical_qasm_sha256: str = ""
    compat_status: str | None = None
    circuit: qiskit.QuantumCircuit | None = None
    unitary_operator: Operator | None = None
    error_detail: str | None = None

    def evolve_state(self, logical_statevector: Statevector) -> Statevector:
        """Independently evolve a 2^n logical Statevector via Qiskit."""
        if self.circuit is None:
            raise RuntimeError("Cannot evolve logical state without bound QuantumCircuit")
        return logical_statevector.evolve(self.circuit)


def _inject_bound_input_declarations(
    qasm_text: str, bindings: Mapping[str, float]
) -> str:
    """Declare `input float[64] <name>;` AST nodes for explicit parameter bindings.

    Only declares parameter identifiers explicitly supplied in `bindings`.
    Any undeclared or unbound symbol in the source remains undeclared and will
    be rejected explicitly by Qiskit's importer.
    """
    if not bindings:
        return qasm_text

    program = openqasm3.parser.parse(qasm_text)
    existing_decls: set[str] = set()
    insert_idx = 0
    for idx, stmt in enumerate(program.statements):
        if isinstance(stmt, ast.Include):
            insert_idx = idx + 1
        if isinstance(stmt, ast.IODeclaration):
            existing_decls.add(stmt.identifier.name)
        elif isinstance(stmt, ast.ClassicalDeclaration):
            existing_decls.add(stmt.identifier.name)

    new_nodes = []
    for param_name in sorted(bindings):
        if param_name not in existing_decls:
            new_nodes.append(
                ast.IODeclaration(
                    io_identifier=ast.IOKeyword.input,
                    type=ast.FloatType(size=ast.IntegerLiteral(value=64)),
                    identifier=ast.Identifier(param_name),
                )
            )

    if not new_nodes:
        return qasm_text

    program.statements = (
        list(program.statements[:insert_idx])
        + new_nodes
        + list(program.statements[insert_idx:])
    )
    return openqasm3.dumps(program)


def run_logical_reference(
    case: CanonicalCase, *, compute_operator: bool | None = None
) -> LogicalReferenceResult:
    """Independently parse, bind, validate, and construct Qiskit logical reference."""
    qasm_sha = sha256_bytes(case.logical_qasm.encode("utf-8"))
    should_compute_op = (
        (case.verification_mode == "exhaustive_unitary")
        if compute_operator is None
        else compute_operator
    )

    # 1. Syntax check on unmodified logical QASM first
    try:
        openqasm3.parser.parse(case.logical_qasm)
    except Exception as exc:
        return LogicalReferenceResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="logical_qasm_parse_error",
            logical_qubits=None,
            global_phase_radians=None,
            logical_qasm_sha256=qasm_sha,
            compat_status=CompatStatus.PARSE_ERROR.value,
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    # 2. Inject AST declarations strictly for explicit parameter bindings
    try:
        importable_qasm = _inject_bound_input_declarations(
            case.logical_qasm, case.parameter_bindings
        )
    except Exception as exc:
        return LogicalReferenceResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="logical_parameter_injection_error",
            logical_qubits=None,
            global_phase_radians=None,
            logical_qasm_sha256=qasm_sha,
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    # 3. Import into Qiskit (using Session 2 register rename only on Scope.GLOBAL collision)
    compat_status = CompatStatus.SUPPORTED_DIRECT.value
    try:
        qc = qiskit.qasm3.loads(importable_qasm)
    except Exception as exc:
        msg = str(exc)
        if "already inserted in symbol table" in msg and "Scope.GLOBAL" in msg:
            try:
                adapted, rename_map = adapt_colliding_registers(importable_qasm)
                if not rename_map:
                    raise ValueError(msg)
                qc = qiskit.qasm3.loads(adapted)
                compat_status = CompatStatus.SUPPORTED_WITH_REGISTER_RENAME.value
            except Exception as adapt_exc:
                return LogicalReferenceResult(
                    ok=False,
                    status="UNSUPPORTED",
                    failure_category="logical_qasm_unsupported",
                    logical_qubits=None,
                    global_phase_radians=None,
                    logical_qasm_sha256=qasm_sha,
                    compat_status=CompatStatus.UNSUPPORTED_CONSTRUCT.value,
                    error_detail=f"{type(adapt_exc).__name__}: {adapt_exc}",
                )
        elif "Undefined symbol" in msg:
            return LogicalReferenceResult(
                ok=False,
                status="UNSUPPORTED",
                failure_category="logical_unbound_parameter",
                logical_qubits=None,
                global_phase_radians=None,
                logical_qasm_sha256=qasm_sha,
                compat_status=CompatStatus.UNSUPPORTED_UNBOUND_PARAM.value,
                error_detail=f"{type(exc).__name__}: {exc}",
            )
        else:
            return LogicalReferenceResult(
                ok=False,
                status="UNSUPPORTED",
                failure_category="logical_qasm_unsupported",
                logical_qubits=None,
                global_phase_radians=None,
                logical_qasm_sha256=qasm_sha,
                compat_status=CompatStatus.UNSUPPORTED_CONSTRUCT.value,
                error_detail=f"{type(exc).__name__}: {exc}",
            )

    # 4. Validate logical qubit count consistency against canonical case
    if int(qc.num_qubits) != case.logical_qubits:
        return LogicalReferenceResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="logical_qubit_count_mismatch",
            logical_qubits=int(qc.num_qubits),
            global_phase_radians=float(qc.global_phase),
            logical_qasm_sha256=qasm_sha,
            compat_status=compat_status,
            error_detail=(
                f"Logical QASM has {qc.num_qubits} qubits, "
                f"canonical case declares {case.logical_qubits}"
            ),
        )

    # 5. Bind declared numeric parameters and fail closed on any remaining unbound parameters
    if qc.parameters:
        bind_map = {
            param: case.parameter_bindings[param.name]
            for param in qc.parameters
            if param.name in case.parameter_bindings
        }
        unbound = [p.name for p in qc.parameters if p.name not in case.parameter_bindings]
        if unbound:
            return LogicalReferenceResult(
                ok=False,
                status="UNSUPPORTED",
                failure_category="logical_unbound_parameter",
                logical_qubits=int(qc.num_qubits),
                global_phase_radians=None,
                logical_qasm_sha256=qasm_sha,
                compat_status=CompatStatus.UNSUPPORTED_UNBOUND_PARAM.value,
                error_detail=f"Unbound logical parameters remaining: {unbound}",
            )
        qc = qc.assign_parameters(bind_map)

    count_ops = {str(k): int(v) for k, v in qc.count_ops().items()}

    # 6. Reject non-unitary/classical operations outside unitary oracle domain
    if int(qc.num_clbits) > 0 or any(k in count_ops for k in ("measure", "reset")):
        return LogicalReferenceResult(
            ok=False,
            status="UNSUPPORTED",
            failure_category="logical_non_unitary",
            logical_qubits=int(qc.num_qubits),
            global_phase_radians=float(qc.global_phase),
            gate_counts=count_ops,
            logical_qasm_sha256=qasm_sha,
            compat_status=CompatStatus.UNSUPPORTED_NON_UNITARY.value,
            error_detail=f"Logical circuit contains non-unitary/classical ops: {count_ops}",
        )

    # 7. Probe Statevector / Operator cleanly to verify unitary simulability
    op: Operator | None = None
    try:
        if should_compute_op:
            op = Operator(qc)
        else:
            # Verify Statevector kernel accepts the circuit cleanly without O(4^n) Operator
            zero = Statevector.from_int(0, 1 << int(qc.num_qubits))
            zero.evolve(qc)
    except Exception as exc:
        status = (
            "UNSUPPORTED"
            if "Cannot apply" in str(exc) or "classical bits" in str(exc)
            else "ERROR"
        )
        return LogicalReferenceResult(
            ok=False,
            status=status,
            failure_category="logical_simulation_rejected",
            logical_qubits=int(qc.num_qubits),
            global_phase_radians=None,
            gate_counts=count_ops,
            logical_qasm_sha256=qasm_sha,
            compat_status=CompatStatus.UNSUPPORTED_NON_UNITARY.value,
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    return LogicalReferenceResult(
        ok=True,
        status="OK",
        failure_category=None,
        logical_qubits=int(qc.num_qubits),
        global_phase_radians=float(qc.global_phase),
        gate_counts=count_ops,
        logical_qasm_sha256=qasm_sha,
        compat_status=compat_status,
        circuit=qc,
        unitary_operator=op,
        error_detail=None,
    )
