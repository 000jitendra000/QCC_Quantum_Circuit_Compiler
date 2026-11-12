"""QCC black-box compiler runner and independent physical-circuit simulator (M7 Phases 63 & 66).

Responsibilities:
1. Invoke the QCC CLI as a black box with `--emit-manifest` inside an isolated
   working/artifact directory, never scraping stdout for layout/metrics.
2. Independently validate manifest schema, SHA-256 of emitted physical QASM,
   and layout invariants, then import & simulate the physical OpenQASM 3 file
   using Qiskit via the Session 2 compatibility adapter.
Never calls QCC's internal equivalence verifier to establish oracle pass/fail.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import pathlib
import subprocess
import sys
from typing import Any

import jsonschema
import qiskit
from qiskit.quantum_info import Operator, Statevector

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase, sha256_bytes  # noqa: E402
from equivalence.layout import ValidatedLayout, validate_manifest_layout  # noqa: E402
from qasm3_compat import (  # noqa: E402
    CompatStatus,
    MANIFEST_SCHEMA_PATH,
    probe_qasm3_compatibility,
)


DEFAULT_QCC_DEBUG_BIN = REPO_ROOT / "target" / "debug" / "qcc"
DEFAULT_QCC_RELEASE_BIN = REPO_ROOT / "target" / "release" / "qcc"


@dataclass(frozen=True)
class QccCompileExecution:
    """Raw subprocess execution result from invoking the QCC CLI."""

    ok: bool
    returncode: int
    command: tuple[str, ...]
    stdout: str
    stderr: str
    logical_input_path: pathlib.Path
    manifest_path: pathlib.Path
    physical_qasm_path: pathlib.Path


@dataclass(frozen=True)
class PhysicalCircuitResult:
    """Validated physical manifest + independently simulated Qiskit physical circuit."""

    ok: bool
    status: str  # "OK", "UNSUPPORTED", "INVALID_INPUT", "ERROR"
    failure_category: str | None
    manifest: dict[str, Any] | None = None
    manifest_sha256: str | None = None
    manifest_path: str | None = None
    physical_qasm_sha256: str | None = None
    physical_qasm_path: str | None = None
    physical_qasm_text: str | None = None
    layout: ValidatedLayout | None = None
    physical_qubits: int | None = None
    global_phase_qiskit: float | None = None
    gate_counts: dict[str, int] = field(default_factory=dict)
    compat_status: str | None = None
    circuit: qiskit.QuantumCircuit | None = None
    unitary_operator: Operator | None = None
    error_detail: str | None = None

    def evolve_state(self, physical_statevector: Statevector) -> Statevector:
        """Independently evolve a 2^m physical Statevector via Qiskit."""
        if self.circuit is None:
            raise RuntimeError("Cannot evolve physical state without imported QuantumCircuit")
        return physical_statevector.evolve(self.circuit)


def resolve_qcc_binary(qcc_binary: pathlib.Path | None = None) -> pathlib.Path:
    """Locate an existing compiled QCC binary cleanly."""
    if qcc_binary is not None:
        if not qcc_binary.exists():
            raise FileNotFoundError(f"QCC binary not found at {qcc_binary}")
        return qcc_binary
    if DEFAULT_QCC_DEBUG_BIN.exists():
        return DEFAULT_QCC_DEBUG_BIN
    if DEFAULT_QCC_RELEASE_BIN.exists():
        return DEFAULT_QCC_RELEASE_BIN
    raise FileNotFoundError(
        "No compiled `qcc` binary found in target/debug/qcc or target/release/qcc. "
        "Run `cargo build` first."
    )


def run_qcc_compile(
    case: CanonicalCase,
    work_dir: pathlib.Path,
    *,
    qcc_binary: pathlib.Path | None = None,
) -> QccCompileExecution:
    """Run `qcc compile --emit-manifest` as a black box into `work_dir`."""
    work_dir.mkdir(parents=True, exist_ok=True)
    binary = resolve_qcc_binary(qcc_binary)

    logical_name = f"{case.case_id}.logical.qasm"
    physical_name = f"{case.case_id}.physical.qasm"
    manifest_name = f"{case.case_id}.manifest.json"
    logical_path = work_dir / logical_name
    physical_path = work_dir / physical_name
    manifest_path = work_dir / manifest_name
    logical_path.write_text(case.logical_qasm, encoding="utf-8")

    cmd: list[str] = [
        str(binary),
        "compile",
        "--target",
        case.target,
        "--global-phase",
        case.global_phase_policy,
    ]
    for param_name, param_val in sorted(case.parameter_bindings.items()):
        cmd.extend(["-b", f"{param_name}={repr(float(param_val))}"])
    cmd.extend(
        [
            "-o",
            physical_name,
            "--emit-manifest",
            manifest_name,
            logical_name,
        ]
    )

    proc = subprocess.run(
        cmd,
        cwd=work_dir,
        capture_output=True,
        text=True,
        check=False,
    )
    ok = proc.returncode == 0 and manifest_path.exists() and physical_path.exists()
    return QccCompileExecution(
        ok=ok,
        returncode=proc.returncode,
        command=tuple(cmd),
        stdout=proc.stdout,
        stderr=proc.stderr,
        logical_input_path=logical_path,
        manifest_path=manifest_path,
        physical_qasm_path=physical_path,
    )


def load_and_simulate_physical(
    manifest_path: pathlib.Path,
    *,
    qasm_override_path: pathlib.Path | None = None,
    compute_operator: bool = True,
) -> PhysicalCircuitResult:
    """Independently validate manifest, check QASM SHA-256, import and simulate physical QASM."""
    if not manifest_path.exists():
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="manifest_missing",
            manifest_path=str(manifest_path),
            error_detail=f"Manifest file does not exist: {manifest_path}",
        )

    manifest_bytes = manifest_path.read_bytes()
    manifest_sha = sha256_bytes(manifest_bytes)

    try:
        manifest = json.loads(manifest_bytes.decode("utf-8"))
        schema = json.loads(MANIFEST_SCHEMA_PATH.read_text(encoding="utf-8"))
        jsonschema.validate(instance=manifest, schema=schema)
    except Exception as exc:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="manifest_schema_invalid",
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    if manifest.get("compilation_mode") != "target_compiled" or manifest.get("layout") is None:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="manifest_missing_target_layout",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            error_detail="Manifest must have compilation_mode='target_compiled' and non-null layout",
        )

    try:
        validated_layout = validate_manifest_layout(manifest)
    except Exception as exc:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="manifest_invalid_layout",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            error_detail=f"{type(exc).__name__}: {exc}",
        )

    emitted_info = manifest["emitted_circuit"]
    if qasm_override_path is not None:
        qasm_path = qasm_override_path
    else:
        raw_qasm_rel = emitted_info.get("path")
        if not raw_qasm_rel:
            return PhysicalCircuitResult(
                ok=False,
                status="INVALID_INPUT",
                failure_category="manifest_missing_emitted_path",
                manifest=manifest,
                manifest_sha256=manifest_sha,
                manifest_path=str(manifest_path),
                layout=validated_layout,
                error_detail="Manifest emitted_circuit.path is null",
            )
        p = pathlib.Path(raw_qasm_rel)
        if p.is_absolute():
            qasm_path = p
        elif (manifest_path.parent / p).exists():
            qasm_path = manifest_path.parent / p
        else:
            qasm_path = REPO_ROOT / p

    if not qasm_path.exists():
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="physical_qasm_missing",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_path=str(qasm_path),
            layout=validated_layout,
            error_detail=f"Emitted physical QASM file not found: {qasm_path}",
        )

    qasm_bytes = qasm_path.read_bytes()
    actual_qasm_sha = sha256_bytes(qasm_bytes)
    expected_qasm_sha = emitted_info.get("sha256")
    if expected_qasm_sha is not None and actual_qasm_sha != expected_qasm_sha:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="qasm_manifest_hash_mismatch",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            layout=validated_layout,
            error_detail=(
                f"SHA-256 mismatch for {qasm_path}: "
                f"manifest={expected_qasm_sha}, actual={actual_qasm_sha}"
            ),
        )

    qasm_text = qasm_bytes.decode("utf-8")
    compat = probe_qasm3_compatibility(qasm_text, compute_operator=compute_operator)

    if compat.status == CompatStatus.PARSE_ERROR:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="physical_qasm_parse_error",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            physical_qasm_text=qasm_text,
            layout=validated_layout,
            compat_status=compat.status.value,
            error_detail=compat.error_detail,
        )

    if compat.status in (
        CompatStatus.UNSUPPORTED_NON_UNITARY,
        CompatStatus.UNSUPPORTED_UNBOUND_PARAM,
        CompatStatus.UNSUPPORTED_CONSTRUCT,
    ):
        return PhysicalCircuitResult(
            ok=False,
            status="UNSUPPORTED",
            failure_category=f"physical_{compat.status.value}",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            physical_qasm_text=qasm_text,
            layout=validated_layout,
            physical_qubits=compat.num_qubits,
            global_phase_qiskit=compat.global_phase_qiskit,
            gate_counts=compat.count_ops,
            compat_status=compat.status.value,
            error_detail=compat.error_detail,
        )

    if compat.circuit is None:
        return PhysicalCircuitResult(
            ok=False,
            status="ERROR",
            failure_category="physical_missing_circuit",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            physical_qasm_text=qasm_text,
            layout=validated_layout,
            compat_status=compat.status.value,
            error_detail="Compatibility probe returned no QuantumCircuit",
        )

    if compat.num_qubits != validated_layout.num_physical:
        return PhysicalCircuitResult(
            ok=False,
            status="INVALID_INPUT",
            failure_category="physical_qubit_count_mismatch",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            physical_qasm_text=qasm_text,
            layout=validated_layout,
            physical_qubits=compat.num_qubits,
            global_phase_qiskit=compat.global_phase_qiskit,
            gate_counts=compat.count_ops,
            compat_status=compat.status.value,
            error_detail=(
                f"Physical QASM qubit count ({compat.num_qubits}) != "
                f"manifest physical_qubits ({validated_layout.num_physical})"
            ),
        )

    if compute_operator and compat.operator is None:
        return PhysicalCircuitResult(
            ok=False,
            status="ERROR",
            failure_category="physical_operator_unavailable",
            manifest=manifest,
            manifest_sha256=manifest_sha,
            manifest_path=str(manifest_path),
            physical_qasm_sha256=actual_qasm_sha,
            physical_qasm_path=str(qasm_path),
            physical_qasm_text=qasm_text,
            layout=validated_layout,
            physical_qubits=compat.num_qubits,
            global_phase_qiskit=compat.global_phase_qiskit,
            gate_counts=compat.count_ops,
            compat_status=compat.status.value,
            error_detail=compat.error_detail or "Operator simulation failed",
        )

    return PhysicalCircuitResult(
        ok=True,
        status="OK",
        failure_category=None,
        manifest=manifest,
        manifest_sha256=manifest_sha,
        manifest_path=str(manifest_path),
        physical_qasm_sha256=actual_qasm_sha,
        physical_qasm_path=str(qasm_path),
        physical_qasm_text=qasm_text,
        layout=validated_layout,
        physical_qubits=compat.num_qubits,
        global_phase_qiskit=compat.global_phase_qiskit,
        gate_counts=compat.count_ops,
        compat_status=compat.status.value,
        circuit=compat.circuit,
        unitary_operator=compat.operator if compute_operator else None,
        error_detail=None,
    )
