"""Layout-aware, common-global-phase, clean-ancilla semantic comparator (M7 Phase 66).

Compares independently constructed Qiskit logical references against QCC's
independently simulated physical circuits over the full 2^m physical Hilbert space.

Mathematical contract:
- Little-endian indexing (`q[0]` LSB).
- Logical inputs embedded via `initial_logical_to_physical` with all spare physical
  qubits initialized strictly to |0>.
- Reference outputs embedded via `final_logical_to_physical` with all spare physical
  qubits strictly set to |0> (`unused_physical_qubits_final`, ADR-0001).
- Never traces out spare physical qubits; never compares probabilities only.
- Estimates ONE common scalar global phase factor `e^{i * phi}` jointly across the
  entire compared subspace (`np.vdot(B, A)`) — never per-state phase fitting.
- Computes informational `strict_phase_accounting` (`PASS` / `MISMATCH` / `UNVERIFIED`)
  separately without overriding authoritative common-phase semantic equivalence.
"""

from __future__ import annotations

from dataclasses import dataclass
import pathlib
import sys
from typing import Any

import numpy as np
from qiskit.quantum_info import Statevector

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase  # noqa: E402
from equivalence.artifacts import (  # noqa: E402
    validate_comparison_result_schema,
    write_failure_artifacts,
)
from equivalence.layout import (  # noqa: E402
    build_basis_embedding_indices,
    compute_max_residual_ancilla_population_matrix,
    embed_logical_columns,
    embed_logical_statevector,
)
from runners.qcc_runner import PhysicalCircuitResult, QccCompileExecution  # noqa: E402
from runners.qiskit_reference import LogicalReferenceResult  # noqa: E402


# Upper bound for exhaustive 2^m x 2^n operator comparison in Session 3.
# Circuits with physical_qubits > MAX_EXHAUSTIVE_PHYSICAL_QUBITS must use
# `sampled_statevector` so the oracle never attempts intractable O(4^m) allocations.
MAX_EXHAUSTIVE_PHYSICAL_QUBITS = 8

# Justified tolerances for IEEE-754 f64 OpenQASM text serialization (~15-17 digits)
# across composed single/two-qubit rotations and statevector inner products.
DEFAULT_AMPLITUDE_ATOL = 1e-10
DEFAULT_ANCILLA_ATOL = 1e-12
DEFAULT_PHASE_ATOL = 1e-10


@dataclass(frozen=True)
class OracleTolerances:
    """Explicit numerical comparison tolerances."""

    amplitude_atol: float = DEFAULT_AMPLITUDE_ATOL
    ancilla_atol: float = DEFAULT_ANCILLA_ATOL
    phase_atol: float = DEFAULT_PHASE_ATOL

    def as_dict(self) -> dict[str, float]:
        return {
            "amplitude_atol": self.amplitude_atol,
            "ancilla_atol": self.ancilla_atol,
            "phase_atol": self.phase_atol,
        }


def _wrapped_angle_diff(angle_a: float, angle_b: float) -> float:
    """Return shortest signed angular difference in (-pi, pi]."""
    return float(np.angle(np.exp(1j * (angle_a - angle_b))))


def _build_sampled_input_states(
    num_logical: int, seed: int, random_count: int
) -> np.ndarray:
    """Construct deterministic basis + seeded random complex states (shape 2^n x K)."""
    dim = 1 << num_logical
    columns: list[np.ndarray] = []

    basis_indices = [0]
    for q in range(num_logical):
        idx = 1 << q
        if idx not in basis_indices:
            basis_indices.append(idx)
    if (dim - 1) not in basis_indices:
        basis_indices.append(dim - 1)

    for idx in basis_indices:
        vec = np.zeros(dim, dtype=np.complex128)
        vec[idx] = 1.0 + 0.0j
        columns.append(vec)

    plus_state = np.full(dim, 1.0 / np.sqrt(dim), dtype=np.complex128)
    columns.append(plus_state)

    rng = np.random.default_rng(seed)
    for _ in range(random_count):
        re = rng.normal(size=dim)
        im = rng.normal(size=dim)
        vec = (re + 1j * im).astype(np.complex128)
        norm = np.linalg.norm(vec)
        if norm == 0.0:
            vec[0] = 1.0 + 0.0j
        else:
            vec /= norm
        columns.append(vec)

    return np.column_stack(columns)


def _str_layout_dict(layout_map: dict[int, int] | None) -> dict[str, int] | None:
    if layout_map is None:
        return None
    return {str(k): int(v) for k, v in sorted(layout_map.items())}


def _manifest_identity_from_physical(
    physical_res: PhysicalCircuitResult | None,
) -> dict[str, Any] | None:
    if physical_res is None or physical_res.manifest_sha256 is None:
        return None
    manifest = physical_res.manifest or {}
    compiler_info = manifest.get("compiler") or {}
    target_info = manifest.get("target") or {}
    return {
        "sha256": physical_res.manifest_sha256,
        "path": physical_res.manifest_path,
        "compiler_version": compiler_info.get("version_string") or compiler_info.get("version"),
        "target_spec": target_info.get("cli_target_spec") or target_info.get("name"),
    }


def _reference_identity(
    case: CanonicalCase, logical_ref: LogicalReferenceResult | None
) -> dict[str, Any]:
    return {
        "case_sha256": case.case_sha256,
        "logical_qasm_sha256": case.logical_qasm_sha256,
        "global_phase_radians": (
            logical_ref.global_phase_radians if logical_ref is not None else None
        ),
        "gate_counts": dict(sorted(logical_ref.gate_counts.items()))
        if logical_ref is not None
        else {},
    }


def _physical_identity(
    physical_res: PhysicalCircuitResult | None,
) -> dict[str, Any] | None:
    if physical_res is None:
        return None
    return {
        "qasm_sha256": physical_res.physical_qasm_sha256,
        "path": physical_res.physical_qasm_path,
        "global_phase_radians": physical_res.global_phase_qiskit,
        "gate_counts": dict(sorted(physical_res.gate_counts.items())),
    }


def _make_unverified_phase_info(detail: str) -> dict[str, Any]:
    return {
        "status": "UNVERIFIED",
        "informational_only": True,
        "manifest_vs_qasm_delta_radians": None,
        "unitary_common_phase_delta_radians": None,
        "detail": detail,
    }


def _evaluate_strict_phase_accounting(
    *,
    manifest: dict[str, Any],
    qasm_global_phase: float | None,
    common_phase_diff: float,
    overlap_norm: float,
    semantic_amp_ok: bool,
    ancilla_ok: bool,
    phase_atol: float,
) -> dict[str, Any]:
    """Compute informational strict phase-accounting diagnostic without overriding equivalence."""
    gp_info = manifest.get("global_phase") or {}
    if not gp_info.get("is_numeric") or gp_info.get("radians") is None:
        return _make_unverified_phase_info("Manifest global_phase is symbolic/non-numeric.")
    if qasm_global_phase is None:
        return _make_unverified_phase_info("Physical QASM global_phase unavailable.")

    manifest_phase = float(gp_info["radians"])
    manifest_vs_qasm_delta = abs(_wrapped_angle_diff(manifest_phase, qasm_global_phase))

    if manifest_vs_qasm_delta > phase_atol:
        return {
            "status": "MISMATCH",
            "informational_only": True,
            "manifest_vs_qasm_delta_radians": manifest_vs_qasm_delta,
            "unitary_common_phase_delta_radians": (
                abs(common_phase_diff) if semantic_amp_ok and overlap_norm > 1e-12 else None
            ),
            "detail": (
                f"Manifest global_phase ({manifest_phase}) differs from emitted physical "
                f"QASM global_phase ({qasm_global_phase}) by {manifest_vs_qasm_delta:.3e} rad."
            ),
        }

    if not (semantic_amp_ok and ancilla_ok) or overlap_norm <= 1e-12:
        return {
            "status": "UNVERIFIED",
            "informational_only": True,
            "manifest_vs_qasm_delta_radians": manifest_vs_qasm_delta,
            "unitary_common_phase_delta_radians": None,
            "detail": "Semantic or ancilla mismatch prevents strict phase reconstruction.",
        }

    unitary_delta = abs(common_phase_diff)
    if unitary_delta > phase_atol:
        return {
            "status": "MISMATCH",
            "informational_only": True,
            "manifest_vs_qasm_delta_radians": manifest_vs_qasm_delta,
            "unitary_common_phase_delta_radians": unitary_delta,
            "detail": (
                f"Unitary common global phase residual {unitary_delta:.3e} rad exceeds "
                f"phase_atol={phase_atol:.1e} despite matching amplitude subspace."
            ),
        }

    return {
        "status": "PASS",
        "informational_only": True,
        "manifest_vs_qasm_delta_radians": manifest_vs_qasm_delta,
        "unitary_common_phase_delta_radians": unitary_delta,
        "detail": "Manifest phase matches physical QASM and reconstructed unitary phase residual is within tolerance.",
    }


def _finalize_result(
    *,
    case: CanonicalCase,
    result: dict[str, Any],
    compile_exec: QccCompileExecution | None = None,
    physical_res: PhysicalCircuitResult | None = None,
    failure_artifacts_root: pathlib.Path | None = None,
) -> dict[str, Any]:
    if result["status"] != "PASS" and failure_artifacts_root is not None:
        artifact_dir = write_failure_artifacts(
            artifact_dir=failure_artifacts_root / case.case_id,
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
        )
        result["failure_artifact_dir"] = str(artifact_dir)
    validate_comparison_result_schema(result)
    return result


def compare_logical_vs_physical(
    case: CanonicalCase,
    logical_ref: LogicalReferenceResult,
    physical_res: PhysicalCircuitResult,
    *,
    tolerances: OracleTolerances = OracleTolerances(),
    compile_exec: QccCompileExecution | None = None,
    failure_artifacts_root: pathlib.Path | None = None,
) -> dict[str, Any]:
    """Compare an independent logical reference against an independently simulated physical circuit."""
    exhaustive = case.verification_mode == "exhaustive_unitary"

    # 1. Propagate logical reference non-OK outcomes before physical comparison
    if not logical_ref.ok:
        result = {
            "schema_id": "qcc.comparison_result",
            "schema_version": "1.0.0",
            "case_id": case.case_id,
            "status": logical_ref.status,
            "failure_category": logical_ref.failure_category,
            "verification_mode": case.verification_mode,
            "exhaustive": exhaustive,
            "logical_qubits": logical_ref.logical_qubits or case.logical_qubits,
            "physical_qubits": physical_res.physical_qubits,
            "initial_layout": None,
            "final_layout": None,
            "manifest_identity": _manifest_identity_from_physical(physical_res),
            "reference_identity": _reference_identity(case, logical_ref),
            "physical_identity": _physical_identity(physical_res),
            "max_amplitude_error": None,
            "frobenius_norm_error": None,
            "common_global_phase_diff_radians": None,
            "ancilla_zero_ok": None,
            "max_residual_ancilla_population": None,
            "strict_phase_accounting": _make_unverified_phase_info(
                f"Logical reference did not produce a unitary simulator model ({logical_ref.status})."
            ),
            "compat_status": logical_ref.compat_status,
            "tolerances": tolerances.as_dict(),
            "diagnostics": {
                "stage": "logical_reference",
                "error_detail": logical_ref.error_detail,
            },
            "failure_artifact_dir": None,
        }
        return _finalize_result(
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
            failure_artifacts_root=failure_artifacts_root,
        )

    # 2. Propagate QCC subprocess compile failures (when compile_exec provided)
    if compile_exec is not None and not compile_exec.ok:
        result = {
            "schema_id": "qcc.comparison_result",
            "schema_version": "1.0.0",
            "case_id": case.case_id,
            "status": "ERROR",
            "failure_category": "qcc_compile_failed",
            "verification_mode": case.verification_mode,
            "exhaustive": exhaustive,
            "logical_qubits": case.logical_qubits,
            "physical_qubits": None,
            "initial_layout": None,
            "final_layout": None,
            "manifest_identity": None,
            "reference_identity": _reference_identity(case, logical_ref),
            "physical_identity": None,
            "max_amplitude_error": None,
            "frobenius_norm_error": None,
            "common_global_phase_diff_radians": None,
            "ancilla_zero_ok": None,
            "max_residual_ancilla_population": None,
            "strict_phase_accounting": _make_unverified_phase_info(
                "QCC compilation failed before producing physical artifacts."
            ),
            "compat_status": logical_ref.compat_status,
            "tolerances": tolerances.as_dict(),
            "diagnostics": {
                "stage": "qcc_compile",
                "returncode": compile_exec.returncode,
                "stderr": compile_exec.stderr.strip(),
            },
            "failure_artifact_dir": None,
        }
        return _finalize_result(
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
            failure_artifacts_root=failure_artifacts_root,
        )

    # 3. Propagate physical validation/import failures (manifest invalid, hash mismatch, unsupported)
    if not physical_res.ok or physical_res.layout is None or physical_res.manifest is None:
        result = {
            "schema_id": "qcc.comparison_result",
            "schema_version": "1.0.0",
            "case_id": case.case_id,
            "status": physical_res.status,
            "failure_category": physical_res.failure_category,
            "verification_mode": case.verification_mode,
            "exhaustive": exhaustive,
            "logical_qubits": case.logical_qubits,
            "physical_qubits": physical_res.physical_qubits,
            "initial_layout": _str_layout_dict(
                physical_res.layout.initial_logical_to_physical
                if physical_res.layout is not None
                else None
            ),
            "final_layout": _str_layout_dict(
                physical_res.layout.final_logical_to_physical
                if physical_res.layout is not None
                else None
            ),
            "manifest_identity": _manifest_identity_from_physical(physical_res),
            "reference_identity": _reference_identity(case, logical_ref),
            "physical_identity": _physical_identity(physical_res),
            "max_amplitude_error": None,
            "frobenius_norm_error": None,
            "common_global_phase_diff_radians": None,
            "ancilla_zero_ok": None,
            "max_residual_ancilla_population": None,
            "strict_phase_accounting": _make_unverified_phase_info(
                f"Physical runner did not succeed ({physical_res.failure_category})."
            ),
            "compat_status": physical_res.compat_status,
            "tolerances": tolerances.as_dict(),
            "diagnostics": {
                "stage": "physical_runner",
                "error_detail": physical_res.error_detail,
            },
            "failure_artifact_dir": None,
        }
        return _finalize_result(
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
            failure_artifacts_root=failure_artifacts_root,
        )

    layout = physical_res.layout
    n = layout.num_logical
    m = layout.num_physical

    if n != case.logical_qubits:
        result = {
            "schema_id": "qcc.comparison_result",
            "schema_version": "1.0.0",
            "case_id": case.case_id,
            "status": "INVALID_INPUT",
            "failure_category": "logical_qubit_count_mismatch",
            "verification_mode": case.verification_mode,
            "exhaustive": exhaustive,
            "logical_qubits": n,
            "physical_qubits": m,
            "initial_layout": _str_layout_dict(layout.initial_logical_to_physical),
            "final_layout": _str_layout_dict(layout.final_logical_to_physical),
            "manifest_identity": _manifest_identity_from_physical(physical_res),
            "reference_identity": _reference_identity(case, logical_ref),
            "physical_identity": _physical_identity(physical_res),
            "max_amplitude_error": None,
            "frobenius_norm_error": None,
            "common_global_phase_diff_radians": None,
            "ancilla_zero_ok": None,
            "max_residual_ancilla_population": None,
            "strict_phase_accounting": _make_unverified_phase_info(
                "Manifest logical_qubits disagrees with canonical case."
            ),
            "compat_status": physical_res.compat_status,
            "tolerances": tolerances.as_dict(),
            "diagnostics": {
                "stage": "pre_compare",
                "error_detail": f"Case declares {case.logical_qubits}, manifest has {n}",
            },
            "failure_artifact_dir": None,
        }
        return _finalize_result(
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
            failure_artifacts_root=failure_artifacts_root,
        )

    if exhaustive and m > MAX_EXHAUSTIVE_PHYSICAL_QUBITS:
        result = {
            "schema_id": "qcc.comparison_result",
            "schema_version": "1.0.0",
            "case_id": case.case_id,
            "status": "UNSUPPORTED",
            "failure_category": "exhaustive_width_limit_exceeded",
            "verification_mode": case.verification_mode,
            "exhaustive": True,
            "logical_qubits": n,
            "physical_qubits": m,
            "initial_layout": _str_layout_dict(layout.initial_logical_to_physical),
            "final_layout": _str_layout_dict(layout.final_logical_to_physical),
            "manifest_identity": _manifest_identity_from_physical(physical_res),
            "reference_identity": _reference_identity(case, logical_ref),
            "physical_identity": _physical_identity(physical_res),
            "max_amplitude_error": None,
            "frobenius_norm_error": None,
            "common_global_phase_diff_radians": None,
            "ancilla_zero_ok": None,
            "max_residual_ancilla_population": None,
            "strict_phase_accounting": _make_unverified_phase_info(
                "Exhaustive comparison refused above MAX_EXHAUSTIVE_PHYSICAL_QUBITS."
            ),
            "compat_status": physical_res.compat_status,
            "tolerances": tolerances.as_dict(),
            "diagnostics": {
                "stage": "pre_compare",
                "error_detail": (
                    f"physical_qubits={m} > MAX_EXHAUSTIVE_PHYSICAL_QUBITS={MAX_EXHAUSTIVE_PHYSICAL_QUBITS}; "
                    "use sampled_statevector explicitly"
                ),
            },
            "failure_artifact_dir": None,
        }
        return _finalize_result(
            case=case,
            result=result,
            compile_exec=compile_exec,
            physical_res=physical_res,
            failure_artifacts_root=failure_artifacts_root,
        )

    # 4. Construct 2^m x K physical simulated matrix A and expected embedded matrix B
    sampled_labels: list[str] | None = None
    if exhaustive:
        assert logical_ref.unitary_operator is not None
        assert physical_res.unitary_operator is not None
        u_ref = np.asarray(logical_ref.unitary_operator.data, dtype=np.complex128)
        u_phys = np.asarray(physical_res.unitary_operator.data, dtype=np.complex128)
        init_perm = build_basis_embedding_indices(layout.initial_logical_to_physical, n)
        phys_cols = u_phys[:, init_perm]
        expected_cols = embed_logical_columns(
            u_ref,
            layout.final_logical_to_physical,
            num_logical=n,
            num_physical=m,
        )
        num_columns = 1 << n
    else:
        inputs = _build_sampled_input_states(
            n, case.seed, case.sampled_state_count
        )
        num_columns = inputs.shape[1]
        sampled_labels = [f"state_{idx}" for idx in range(num_columns)]
        phys_list: list[np.ndarray] = []
        exp_list: list[np.ndarray] = []
        for col_idx in range(num_columns):
            psi_log = inputs[:, col_idx]
            phi_ref = logical_ref.evolve_state(Statevector(psi_log)).data
            psi_phys_in = embed_logical_statevector(
                psi_log,
                layout.initial_logical_to_physical,
                num_logical=n,
                num_physical=m,
            )
            psi_phys_out = physical_res.evolve_state(Statevector(psi_phys_in)).data
            phi_phys_exp = embed_logical_statevector(
                phi_ref,
                layout.final_logical_to_physical,
                num_logical=n,
                num_physical=m,
            )
            phys_list.append(np.asarray(psi_phys_out, dtype=np.complex128))
            exp_list.append(np.asarray(phi_phys_exp, dtype=np.complex128))
        phys_cols = np.column_stack(phys_list)
        expected_cols = np.column_stack(exp_list)

    # 5. Estimate ONE single common global phase across ALL 2^m x K amplitudes jointly
    overlap = np.vdot(expected_cols, phys_cols)
    overlap_norm = float(abs(overlap))
    if overlap_norm > 1e-15:
        common_phase_factor = overlap / overlap_norm
        common_phase_diff = float(np.angle(common_phase_factor))
    else:
        common_phase_factor = 1.0 + 0.0j
        common_phase_diff = 0.0

    diff = phys_cols - common_phase_factor * expected_cols
    abs_diff = np.abs(diff)
    worst_flat = int(np.argmax(abs_diff))
    worst_phys_idx, worst_col_idx = divmod(worst_flat, num_columns)
    max_amp_error = float(abs_diff[worst_phys_idx, worst_col_idx])
    frob_error = float(np.linalg.norm(diff))

    # 6. Strict ADR-0001 clean-ancilla verification over full 2^m state
    max_ancilla_pop = compute_max_residual_ancilla_population_matrix(
        phys_cols, layout.unused_physical_final
    )
    ancilla_ok = max_ancilla_pop <= tolerances.ancilla_atol
    amp_ok = max_amp_error <= tolerances.amplitude_atol

    if not ancilla_ok:
        status = "MISMATCH"
        failure_category = "residual_ancilla_contamination"
    elif not amp_ok:
        status = "MISMATCH"
        failure_category = "unitary_or_layout_mismatch"
    else:
        status = "PASS"
        failure_category = None

    strict_phase_info = _evaluate_strict_phase_accounting(
        manifest=physical_res.manifest,
        qasm_global_phase=physical_res.global_phase_qiskit,
        common_phase_diff=common_phase_diff,
        overlap_norm=overlap_norm,
        semantic_amp_ok=amp_ok,
        ancilla_ok=ancilla_ok,
        phase_atol=tolerances.phase_atol,
    )

    diagnostics: dict[str, Any] = {
        "compared_columns": num_columns,
        "physical_dimension": 1 << m,
        "unused_physical_qubits_initial": list(layout.unused_physical_initial),
        "unused_physical_qubits_final": list(layout.unused_physical_final),
        "overlap_magnitude": overlap_norm,
        "worst_physical_basis_index": worst_phys_idx,
        "worst_input_column_index": worst_col_idx,
        "worst_physical_amplitude": [
            float(np.real(phys_cols[worst_phys_idx, worst_col_idx])),
            float(np.imag(phys_cols[worst_phys_idx, worst_col_idx])),
        ],
        "worst_expected_amplitude_phased": [
            float(
                np.real(
                    common_phase_factor * expected_cols[worst_phys_idx, worst_col_idx]
                )
            ),
            float(
                np.imag(
                    common_phase_factor * expected_cols[worst_phys_idx, worst_col_idx]
                )
            ),
        ],
    }
    if sampled_labels is not None:
        diagnostics["sampled_state_count"] = len(sampled_labels)
        diagnostics["seed"] = case.seed

    result = {
        "schema_id": "qcc.comparison_result",
        "schema_version": "1.0.0",
        "case_id": case.case_id,
        "status": status,
        "failure_category": failure_category,
        "verification_mode": case.verification_mode,
        "exhaustive": exhaustive,
        "logical_qubits": n,
        "physical_qubits": m,
        "initial_layout": _str_layout_dict(layout.initial_logical_to_physical),
        "final_layout": _str_layout_dict(layout.final_logical_to_physical),
        "manifest_identity": _manifest_identity_from_physical(physical_res),
        "reference_identity": _reference_identity(case, logical_ref),
        "physical_identity": _physical_identity(physical_res),
        "max_amplitude_error": max_amp_error,
        "frobenius_norm_error": frob_error,
        "common_global_phase_diff_radians": common_phase_diff,
        "ancilla_zero_ok": ancilla_ok,
        "max_residual_ancilla_population": max_ancilla_pop,
        "strict_phase_accounting": strict_phase_info,
        "compat_status": physical_res.compat_status,
        "tolerances": tolerances.as_dict(),
        "diagnostics": diagnostics,
        "failure_artifact_dir": None,
    }
    return _finalize_result(
        case=case,
        result=result,
        compile_exec=compile_exec,
        physical_res=physical_res,
        failure_artifacts_root=failure_artifacts_root,
    )
