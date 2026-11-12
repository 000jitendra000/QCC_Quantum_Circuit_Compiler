"""Deterministic comparison-result serialization and failure-artifact preservation."""

from __future__ import annotations

import json
import pathlib
import sys
from typing import Any

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase, canonical_json_bytes, sha256_bytes  # noqa: E402
from runners.qcc_runner import PhysicalCircuitResult, QccCompileExecution  # noqa: E402


COMPARISON_RESULT_SCHEMA_PATH = ORACLE_DIR / "schema" / "comparison_result.schema.json"


def validate_comparison_result_schema(result: dict[str, Any]) -> None:
    """Validate a comparison result dict against comparison_result.schema.json."""
    schema = json.loads(COMPARISON_RESULT_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=result, schema=schema)


def comparison_result_digest(result: dict[str, Any]) -> str:
    """Return deterministic SHA-256 of result (excluding local artifact dir path)."""
    payload = dict(result)
    payload["failure_artifact_dir"] = None
    return sha256_bytes(canonical_json_bytes(payload))


def serialize_comparison_result(result: dict[str, Any]) -> str:
    """Serialize comparison result deterministically with sorted keys."""
    validate_comparison_result_schema(result)
    return json.dumps(result, indent=2, sort_keys=True) + "\n"


def write_failure_artifacts(
    *,
    artifact_dir: pathlib.Path,
    case: CanonicalCase,
    result: dict[str, Any],
    compile_exec: QccCompileExecution | None = None,
    physical_res: PhysicalCircuitResult | None = None,
    manifest_path: pathlib.Path | None = None,
    qasm_path: pathlib.Path | None = None,
) -> pathlib.Path:
    """Persist complete reproducible failure bundle into `artifact_dir`."""
    artifact_dir.mkdir(parents=True, exist_ok=True)

    (artifact_dir / "case.json").write_text(
        json.dumps(case.raw_json, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (artifact_dir / "logical_input.qasm").write_text(
        case.logical_qasm, encoding="utf-8"
    )

    resolved_manifest_path: pathlib.Path | None = manifest_path
    if resolved_manifest_path is None and compile_exec is not None and compile_exec.manifest_path.exists():
        resolved_manifest_path = compile_exec.manifest_path
    elif resolved_manifest_path is None and physical_res is not None and physical_res.manifest_path:
        cand = pathlib.Path(physical_res.manifest_path)
        if cand.exists():
            resolved_manifest_path = cand

    if resolved_manifest_path is not None and resolved_manifest_path.exists():
        (artifact_dir / "manifest.json").write_bytes(resolved_manifest_path.read_bytes())
    elif physical_res is not None and physical_res.manifest is not None:
        (artifact_dir / "manifest.json").write_text(
            json.dumps(physical_res.manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    resolved_qasm_path: pathlib.Path | None = qasm_path
    if resolved_qasm_path is None and compile_exec is not None and compile_exec.physical_qasm_path.exists():
        resolved_qasm_path = compile_exec.physical_qasm_path
    elif resolved_qasm_path is None and physical_res is not None and physical_res.physical_qasm_path:
        cand = pathlib.Path(physical_res.physical_qasm_path)
        if cand.exists():
            resolved_qasm_path = cand

    if resolved_qasm_path is not None and resolved_qasm_path.exists():
        (artifact_dir / "physical_output.qasm").write_bytes(resolved_qasm_path.read_bytes())
    elif physical_res is not None and physical_res.physical_qasm_text is not None:
        (artifact_dir / "physical_output.qasm").write_text(
            physical_res.physical_qasm_text, encoding="utf-8"
        )

    diag_lines = [
        f"case_id: {case.case_id}",
        f"status: {result['status']}",
        f"failure_category: {result['failure_category']}",
        f"compat_status: {result['compat_status']}",
        f"max_amplitude_error: {result['max_amplitude_error']}",
        f"common_global_phase_diff_radians: {result['common_global_phase_diff_radians']}",
        f"ancilla_zero_ok: {result['ancilla_zero_ok']}",
        f"max_residual_ancilla_population: {result['max_residual_ancilla_population']}",
        f"diagnostics: {json.dumps(result['diagnostics'], sort_keys=True)}",
    ]
    if compile_exec is not None:
        diag_lines.extend(
            [
                f"qcc_command: {' '.join(compile_exec.command)}",
                f"qcc_returncode: {compile_exec.returncode}",
                "--- qcc_stdout ---",
                compile_exec.stdout.strip(),
                "--- qcc_stderr ---",
                compile_exec.stderr.strip(),
            ]
        )
    (artifact_dir / "diagnostics.txt").write_text(
        "\n".join(diag_lines) + "\n", encoding="utf-8"
    )

    updated_result = dict(result)
    updated_result["failure_artifact_dir"] = str(artifact_dir)
    (artifact_dir / "comparison_result.json").write_text(
        serialize_comparison_result(updated_result), encoding="utf-8"
    )
    return artifact_dir
