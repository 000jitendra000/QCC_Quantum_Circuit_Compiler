"""OpenQASM 3 compatibility probe and semantics-preserving Qiskit importer for QCC.

M7 Session 2 deliverable:
- Verifies pinned Python/Qiskit/OpenQASM 3 environment reproducibility.
- Inspects real QCC-emitted physical OpenQASM 3 against `openqasm3` and
  `qiskit-qasm3-import` without modifying QCC's Rust compiler or calling QCC's
  internal equivalence checker.
- Implements a strictly scoped, provably semantics-preserving AST register
  alpha-renaming adapter when global register identifiers (most notably QCC's
  `qubit[N] p;`) collide with `stdgates.inc` gate symbols (e.g. `p(λ)`) in
  `qiskit-qasm3-import`.
- Never silently drops `gphase`, gates, or unsupported operations; classifies
  malformed or non-unitary/unsupported constructs explicitly.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
import hashlib
from importlib import metadata
import json
import math
import pathlib
import re
from typing import Any

import numpy as np
import openqasm3
from openqasm3 import ast
import openqasm3.parser
import qiskit
import qiskit.qasm3
from qiskit.quantum_info import Operator, Statevector
import qiskit_qasm3_import
import jsonschema


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
REQUIREMENTS_PATH = pathlib.Path(__file__).resolve().parent / "requirements.txt"
MANIFEST_SCHEMA_PATH = (
    pathlib.Path(__file__).resolve().parent / "schema" / "compile_manifest.schema.json"
)

# Exact pinned package versions verified in Session 2.
PINNED_PACKAGES: dict[str, str] = {
    "qiskit": "2.6.2",
    "qiskit-qasm3-import": "0.6.0",
    "openqasm3": "1.0.1",
    "jsonschema": "4.26.0",
    "numpy": "2.5.4",
    "scipy": "1.18.1",
    "rustworkx": "0.18.1",
    "antlr4-python3-runtime": "4.13.2",
    "dill": "0.4.1",
    "stevedore": "5.10.0",
    "attrs": "26.1.0",
    "referencing": "0.37.0",
    "jsonschema-specifications": "2025.9.1",
    "rpds-py": "2026.9.1",
    "typing-extensions": "4.16.0",
}

# Symbols populated in Scope.GLOBAL by `qiskit-qasm3-import==0.6.0` on
# `include "stdgates.inc";` plus Scope.BUILTIN pre-populated symbols.
STDGATES_SYMBOLS: frozenset[str] = frozenset(
    {
        "p",
        "x",
        "y",
        "z",
        "h",
        "s",
        "sdg",
        "t",
        "tdg",
        "sx",
        "rx",
        "ry",
        "rz",
        "cx",
        "cy",
        "cz",
        "cp",
        "crx",
        "cry",
        "crz",
        "ch",
        "swap",
        "ccx",
        "cswap",
        "cu",
        "CX",
        "phase",
        "cphase",
        "id",
        "u1",
        "u2",
        "u3",
    }
)

BUILTIN_SYMBOLS: frozenset[str] = frozenset(
    {"U", "pi", "π", "tau", "τ", "euler", "ℇ"}
)

RESERVED_IMPORTER_SYMBOLS: frozenset[str] = STDGATES_SYMBOLS | BUILTIN_SYMBOLS

_SAFE_TOP_LEVEL_STATEMENTS = (
    ast.Include,
    ast.QubitDeclaration,
    ast.ClassicalDeclaration,
    ast.IODeclaration,
    ast.QuantumGate,
    ast.QuantumPhase,
    ast.QuantumMeasurementStatement,
    ast.QuantumReset,
    ast.QuantumBarrier,
)


class CompatStatus(str, Enum):
    """Explicit classification of OpenQASM 3 compatibility with pinned Qiskit."""

    SUPPORTED_DIRECT = "supported_direct"
    SUPPORTED_WITH_REGISTER_RENAME = "supported_with_register_rename"
    UNSUPPORTED_NON_UNITARY = "unsupported_non_unitary"
    UNSUPPORTED_UNBOUND_PARAM = "unsupported_unbound_param"
    UNSUPPORTED_CONSTRUCT = "unsupported_construct"
    PARSE_ERROR = "parse_error"


@dataclass(frozen=True)
class Qasm3CompatResult:
    """Detailed compatibility report for a single OpenQASM 3 program."""

    status: CompatStatus
    parse_ok: bool
    raw_construct_ok: bool
    adapted_construct_ok: bool
    simulate_sv_ok: bool
    simulate_op_ok: bool
    num_qubits: int | None = None
    num_clbits: int | None = None
    global_phase_qiskit: float | None = None
    count_ops: dict[str, int] = field(default_factory=dict)
    fallback_applied: bool = False
    renamed_registers: dict[str, str] = field(default_factory=dict)
    adapted_qasm: str | None = None
    raw_error: str | None = None
    error_detail: str | None = None
    circuit: qiskit.QuantumCircuit | None = None
    statevector: Statevector | None = None
    operator: Operator | None = None


def check_pinned_environment() -> dict[str, str]:
    """Return installed package versions and verify all PINNED_PACKAGES match."""
    installed: dict[str, str] = {}
    mismatches: list[str] = []
    for pkg, expected in PINNED_PACKAGES.items():
        actual = metadata.version(pkg)
        installed[pkg] = actual
        if actual != expected:
            mismatches.append(f"{pkg}: expected=={expected}, got=={actual}")
    if mismatches:
        raise RuntimeError(
            "Pinned oracle environment mismatch:\n  " + "\n  ".join(mismatches)
        )
    return installed


def _collect_all_identifiers(program: ast.Program) -> set[str]:
    """Collect every ast.Identifier name appearing anywhere in the AST."""
    found: set[str] = set()

    def walk(node: Any) -> None:
        if isinstance(node, ast.Identifier):
            found.add(node.name)
            return
        if isinstance(node, list):
            for item in node:
                walk(item)
            return
        if isinstance(node, ast.QASMNode):
            for field_name in getattr(node, "__dict__", {}):
                if field_name == "span":
                    continue
                walk(getattr(node, field_name))

    walk(program)
    return found


def _fresh_register_name(orig: str, forbidden: set[str]) -> str:
    """Choose a deterministic `[a-z]\\w*` identifier absent from `forbidden`."""
    sanitized = re.sub(r"\W", "_", orig) or "r"
    base = f"qcc_reg_{sanitized}"
    candidate = base
    idx = 1
    while candidate in forbidden:
        candidate = f"{base}_{idx}"
        idx += 1
    forbidden.add(candidate)
    return candidate


def _rename_operand_expr(expr: Any, rename_map: dict[str, str]) -> None:
    """Rename register references inside a qubit/bit operand expression in place."""
    if isinstance(expr, ast.Identifier):
        if expr.name in rename_map:
            expr.name = rename_map[expr.name]
    elif isinstance(expr, ast.IndexedIdentifier):
        _rename_operand_expr(expr.name, rename_map)
    elif isinstance(expr, list):
        for item in expr:
            _rename_operand_expr(item, rename_map)


def _extract_gate_signature(program: ast.Program) -> list[tuple[str, int, int]]:
    """Extract ordered (kind_or_gate_name, arity, num_params) for invariance check."""
    sig: list[tuple[str, int, int]] = []
    for stmt in program.statements:
        if isinstance(stmt, ast.QuantumGate):
            sig.append((f"gate:{stmt.name.name}", len(stmt.qubits), len(stmt.arguments)))
        elif isinstance(stmt, ast.QuantumPhase):
            sig.append(("gphase", len(stmt.qubits), 1))
        elif isinstance(stmt, ast.QuantumMeasurementStatement):
            sig.append(("measure", 1, 0))
        elif isinstance(stmt, ast.QuantumReset):
            sig.append(("reset", 1, 0))
        elif isinstance(stmt, ast.QuantumBarrier):
            sig.append(("barrier", len(stmt.qubits), 0))
    return sig


def adapt_colliding_registers(qasm_text: str) -> tuple[str, dict[str, str]]:
    """Alpha-rename top-level register declarations colliding with stdgates/builtins.

    Only top-level QubitDeclaration and ClassicalDeclaration identifiers whose
    names collide with RESERVED_IMPORTER_SYMBOLS are renamed, and only operand/target
    references are rewritten (`QuantumGate.name` is never altered). Refuses and
    raises ValueError if custom gate definitions, subroutines, calibrations, or
    aliases are present so scope semantics are never guessed.
    """
    program = openqasm3.parser.parse(qasm_text)

    uses_stdgates = any(
        isinstance(stmt, ast.Include) and stmt.filename == "stdgates.inc"
        for stmt in program.statements
    )
    reserved = RESERVED_IMPORTER_SYMBOLS if uses_stdgates else BUILTIN_SYMBOLS

    # Check if any top-level declaration collides; if none, return untouched text.
    colliding: list[str] = []
    for stmt in program.statements:
        if isinstance(stmt, ast.QubitDeclaration):
            if stmt.qubit.name in reserved:
                colliding.append(stmt.qubit.name)
        elif isinstance(stmt, ast.ClassicalDeclaration):
            if stmt.identifier.name in reserved:
                colliding.append(stmt.identifier.name)

    if not colliding:
        return qasm_text, {}

    # Fail closed if any statement outside the flat top-level circuit subset
    # (e.g. gate definitions, subroutines, control flow, aliases, calibrations)
    # is present alongside a register collision.
    for stmt in program.statements:
        if not isinstance(stmt, _SAFE_TOP_LEVEL_STATEMENTS):
            raise ValueError(
                f"Refusing AST register alpha-renaming with {type(stmt).__name__} present"
            )

    used_identifiers = _collect_all_identifiers(program) | set(RESERVED_IMPORTER_SYMBOLS)
    rename_map: dict[str, str] = {}
    for name in colliding:
        if name not in rename_map:
            rename_map[name] = _fresh_register_name(name, used_identifiers)

    before_sig = _extract_gate_signature(program)

    for stmt in program.statements:
        if isinstance(stmt, ast.QubitDeclaration):
            if stmt.qubit.name in rename_map:
                stmt.qubit.name = rename_map[stmt.qubit.name]
        elif isinstance(stmt, ast.ClassicalDeclaration):
            if stmt.identifier.name in rename_map:
                stmt.identifier.name = rename_map[stmt.identifier.name]
        elif isinstance(stmt, ast.QuantumGate):
            _rename_operand_expr(stmt.qubits, rename_map)
        elif isinstance(stmt, ast.QuantumPhase):
            _rename_operand_expr(stmt.qubits, rename_map)
        elif isinstance(stmt, ast.QuantumMeasurementStatement):
            _rename_operand_expr(stmt.measure.qubit, rename_map)
            if stmt.target is not None:
                _rename_operand_expr(stmt.target, rename_map)
        elif isinstance(stmt, ast.QuantumReset):
            _rename_operand_expr(stmt.qubits, rename_map)
        elif isinstance(stmt, ast.QuantumBarrier):
            _rename_operand_expr(stmt.qubits, rename_map)

    adapted = openqasm3.dumps(program)
    reparsed = openqasm3.parser.parse(adapted)
    after_sig = _extract_gate_signature(reparsed)
    if before_sig != after_sig:
        raise ValueError("AST register renaming altered statement signature")

    return adapted, rename_map


def _is_symbol_collision_error(err: Exception) -> bool:
    msg = str(err)
    return "already inserted in symbol table" in msg and "Scope.GLOBAL" in msg


def probe_qasm3_compatibility(
    qasm_text: str, *, compute_operator: bool = True
) -> Qasm3CompatResult:
    """Probe OpenQASM 3 parse, Qiskit import, and simulation compatibility."""
    try:
        openqasm3.parser.parse(qasm_text)
    except Exception as exc:
        return Qasm3CompatResult(
            status=CompatStatus.PARSE_ERROR,
            parse_ok=False,
            raw_construct_ok=False,
            adapted_construct_ok=False,
            simulate_sv_ok=False,
            simulate_op_ok=False,
            raw_error=f"{type(exc).__name__}: {exc}",
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    raw_error: str | None = None
    raw_construct_ok = False
    adapted_construct_ok = False
    fallback_applied = False
    renamed_registers: dict[str, str] = {}
    adapted_qasm: str | None = None
    qc: qiskit.QuantumCircuit | None = None

    try:
        qc = qiskit.qasm3.loads(qasm_text)
        raw_construct_ok = True
        adapted_construct_ok = True
    except Exception as exc:
        raw_error = f"{type(exc).__name__}: {exc}"
        if _is_symbol_collision_error(exc):
            try:
                adapted_qasm, rename_map = adapt_colliding_registers(qasm_text)
                if rename_map:
                    qc = qiskit.qasm3.loads(adapted_qasm)
                    adapted_construct_ok = True
                    fallback_applied = True
                    renamed_registers = rename_map
            except Exception as adapt_exc:
                return Qasm3CompatResult(
                    status=CompatStatus.UNSUPPORTED_CONSTRUCT,
                    parse_ok=True,
                    raw_construct_ok=False,
                    adapted_construct_ok=False,
                    simulate_sv_ok=False,
                    simulate_op_ok=False,
                    raw_error=raw_error,
                    error_detail=f"{type(adapt_exc).__name__}: {adapt_exc}",
                )
        if not adapted_construct_ok or qc is None:
            return Qasm3CompatResult(
                status=CompatStatus.UNSUPPORTED_CONSTRUCT,
                parse_ok=True,
                raw_construct_ok=False,
                adapted_construct_ok=False,
                simulate_sv_ok=False,
                simulate_op_ok=False,
                raw_error=raw_error,
                error_detail=raw_error,
            )

    sv: Statevector | None = None
    op: Operator | None = None
    sv_err: str | None = None
    op_err: str | None = None

    try:
        sv = Statevector.from_instruction(qc)
    except Exception as exc:
        sv_err = f"{type(exc).__name__}: {exc}"

    if compute_operator:
        try:
            op = Operator(qc)
        except Exception as exc:
            op_err = f"{type(exc).__name__}: {exc}"

    count_ops = {str(k): int(v) for k, v in qc.count_ops().items()}
    has_non_unitary = int(qc.num_clbits) > 0 or any(
        k in count_ops for k in ("measure", "reset")
    )

    if qc.parameters:
        status = CompatStatus.UNSUPPORTED_UNBOUND_PARAM
        error_detail = op_err or sv_err
    elif has_non_unitary or (compute_operator and op_err is not None):
        status = CompatStatus.UNSUPPORTED_NON_UNITARY
        error_detail = op_err or sv_err
    elif sv is not None and (op is not None if compute_operator else True):
        status = (
            CompatStatus.SUPPORTED_WITH_REGISTER_RENAME
            if fallback_applied
            else CompatStatus.SUPPORTED_DIRECT
        )
        error_detail = None
    else:
        status = CompatStatus.UNSUPPORTED_CONSTRUCT
        error_detail = op_err or sv_err

    return Qasm3CompatResult(
        status=status,
        parse_ok=True,
        raw_construct_ok=raw_construct_ok,
        adapted_construct_ok=adapted_construct_ok,
        simulate_sv_ok=sv is not None,
        simulate_op_ok=op is not None,
        num_qubits=int(qc.num_qubits),
        num_clbits=int(qc.num_clbits),
        global_phase_qiskit=float(qc.global_phase),
        count_ops=count_ops,
        fallback_applied=fallback_applied,
        renamed_registers=renamed_registers,
        adapted_qasm=adapted_qasm,
        raw_error=raw_error,
        error_detail=error_detail,
        circuit=qc,
        statevector=sv,
        operator=op,
    )


def validate_manifest_consistency(
    manifest_path: pathlib.Path, qasm_path: pathlib.Path | None = None
) -> tuple[dict[str, Any], Qasm3CompatResult]:
    """Validate manifest JSON against schema and check Qiskit import consistency."""
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    schema = json.loads(MANIFEST_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=manifest, schema=schema)

    if qasm_path is None:
        emitted_rel = manifest["emitted_circuit"]["path"]
        if not emitted_rel:
            raise ValueError("Manifest has no emitted_circuit.path")
        qasm_path = REPO_ROOT / emitted_rel

    qasm_bytes = qasm_path.read_bytes()
    actual_sha256 = hashlib.sha256(qasm_bytes).hexdigest()
    expected_sha256 = manifest["emitted_circuit"]["sha256"]
    if expected_sha256 is not None and actual_sha256 != expected_sha256:
        raise ValueError(
            f"SHA-256 mismatch for {qasm_path}: expected {expected_sha256}, got {actual_sha256}"
        )

    qasm_text = qasm_bytes.decode("utf-8")
    compat = probe_qasm3_compatibility(qasm_text)

    if compat.num_qubits != manifest["physical_qubits"]:
        raise ValueError(
            f"Physical qubit count mismatch: manifest={manifest['physical_qubits']}, "
            f"qiskit_imported={compat.num_qubits}"
        )

    if compat.count_ops != manifest["metrics"]["per_gate_counts"]:
        raise ValueError(
            f"Per-gate counts mismatch: manifest={manifest['metrics']['per_gate_counts']}, "
            f"qiskit_imported={compat.count_ops}"
        )

    manifest_phase = float(manifest["global_phase"]["radians"])
    if compat.global_phase_qiskit is None:
        raise ValueError("Missing Qiskit global_phase")
    phase_diff = abs(
        np.exp(1j * compat.global_phase_qiskit) - np.exp(1j * manifest_phase)
    )
    if phase_diff > 1e-12:
        raise ValueError(
            f"Global phase unitary scalar mismatch: "
            f"manifest={manifest_phase}, qiskit={compat.global_phase_qiskit}, diff={phase_diff}"
        )

    return manifest, compat


if __name__ == "__main__":
    installed = check_pinned_environment()
    print(f"Pinned Python oracle environment OK (qiskit={installed['qiskit']}, "
          f"qiskit-qasm3-import={installed['qiskit-qasm3-import']}, "
          f"openqasm3={installed['openqasm3']})")
    fixtures_dir = pathlib.Path(__file__).resolve().parent / "fixtures" / "session2"
    for mf in sorted(fixtures_dir.glob("*.manifest.json")):
        manifest, res = validate_manifest_consistency(mf)
        print(
            f"  [OK] {mf.name}: status={res.status.value}, "
            f"qubits={res.num_qubits}, ops={res.count_ops}, "
            f"raw_collision={res.raw_error is not None}, "
            f"renamed={res.renamed_registers}, "
            f"phase_qiskit={res.global_phase_qiskit:.12g} "
            f"(manifest={manifest['global_phase']['radians']:.12g})"
        )
