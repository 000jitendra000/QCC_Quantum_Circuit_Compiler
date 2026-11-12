"""Neutral canonical test-case loader and validator for QCC Independent Oracle."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import pathlib
from typing import Any

import jsonschema


ORACLE_DIR = pathlib.Path(__file__).resolve().parent
REPO_ROOT = ORACLE_DIR.parent.parent
CASE_SCHEMA_PATH = ORACLE_DIR / "schema" / "canonical_case.schema.json"
DEFAULT_CASES_DIR = ORACLE_DIR / "cases"


@dataclass(frozen=True)
class CanonicalCase:
    """Validated canonical circuit case shared as a neutral input specification."""

    case_id: str
    description: str
    logical_qubits: int
    logical_qasm: str
    logical_qasm_source: str
    parameter_bindings: dict[str, float]
    target: str
    global_phase_policy: str
    seed: int
    verification_mode: str
    sampled_state_count: int
    expected_oracle_status: str
    limitations: tuple[str, ...]
    raw_json: dict[str, Any]
    case_sha256: str
    logical_qasm_sha256: str


def canonical_json_bytes(obj: Any) -> bytes:
    """Serialize JSON deterministically without timestamps for stable hashing."""
    return (
        json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        .encode("utf-8")
    )


def sha256_bytes(data: bytes) -> str:
    """Return lowercase hexadecimal SHA-256 digest."""
    return hashlib.sha256(data).hexdigest()


def load_canonical_case_from_dict(
    raw: dict[str, Any], *, base_dir: pathlib.Path | None = None
) -> CanonicalCase:
    """Validate a canonical case dictionary against canonical_case.schema.json."""
    schema = json.loads(CASE_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=raw, schema=schema)

    if "logical_qasm" in raw and raw["logical_qasm"] is not None:
        qasm_text = raw["logical_qasm"]
        qasm_source = "<inline>"
    else:
        rel_path = pathlib.Path(raw["logical_qasm_path"])
        resolved = rel_path if rel_path.is_absolute() else (base_dir or REPO_ROOT) / rel_path
        qasm_text = resolved.read_text(encoding="utf-8")
        qasm_source = str(raw["logical_qasm_path"])

    bindings = {str(k): float(v) for k, v in sorted(raw["parameter_bindings"].items())}
    cfg = raw["compiler_config"]

    return CanonicalCase(
        case_id=raw["case_id"],
        description=raw["description"],
        logical_qubits=int(raw["logical_qubits"]),
        logical_qasm=qasm_text,
        logical_qasm_source=qasm_source,
        parameter_bindings=bindings,
        target=cfg["target"],
        global_phase_policy=cfg["global_phase_policy"],
        seed=int(cfg["seed"]),
        verification_mode=raw["verification_mode"],
        sampled_state_count=int(raw.get("sampled_state_count", 16)),
        expected_oracle_status=raw["expected_oracle_status"],
        limitations=tuple(raw["limitations"]),
        raw_json=raw,
        case_sha256=sha256_bytes(canonical_json_bytes(raw)),
        logical_qasm_sha256=sha256_bytes(qasm_text.encode("utf-8")),
    )


def load_canonical_case(case_path: pathlib.Path) -> CanonicalCase:
    """Load and validate a canonical case JSON file."""
    raw = json.loads(case_path.read_text(encoding="utf-8"))
    return load_canonical_case_from_dict(raw)


def load_all_canonical_cases(
    cases_dir: pathlib.Path = DEFAULT_CASES_DIR,
) -> list[CanonicalCase]:
    """Load all canonical case files in deterministic sorted filename order."""
    return [load_canonical_case(p) for p in sorted(cases_dir.glob("*.json"))]
