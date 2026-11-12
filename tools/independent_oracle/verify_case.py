"""End-to-end independent oracle verification entrypoint (M7 Session 3).

Orchestrates:
1. Canonical case loading & validation (`cases_loader.py`).
2. Independent Qiskit logical-reference runner (`runners/qiskit_reference.py`).
3. Black-box QCC CLI compilation & independent physical runner (`runners/qcc_runner.py`).
4. Layout-aware, single-common-phase, clean-ancilla semantic comparator (`equivalence/comparator.py`).
5. Deterministic comparison results & failure artifacts (`equivalence/artifacts.py`).
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import tempfile
from typing import Any

ORACLE_DIR = pathlib.Path(__file__).resolve().parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    CanonicalCase,
    DEFAULT_CASES_DIR,
    load_all_canonical_cases,
    load_canonical_case,
)
from equivalence.comparator import (  # noqa: E402
    OracleTolerances,
    compare_logical_vs_physical,
)
from qasm3_compat import check_pinned_environment  # noqa: E402
from runners.qcc_runner import (  # noqa: E402
    PhysicalCircuitResult,
    load_and_simulate_physical,
    run_qcc_compile,
)
from runners.qiskit_reference import run_logical_reference  # noqa: E402


def verify_canonical_case(
    case: CanonicalCase,
    *,
    work_root: pathlib.Path,
    failure_artifacts_root: pathlib.Path | None = None,
    qcc_binary: pathlib.Path | None = None,
    tolerances: OracleTolerances = OracleTolerances(),
) -> dict[str, Any]:
    """Run independent logical reference, QCC compile + physical simulation, and compare."""
    compute_op = case.verification_mode == "exhaustive_unitary"

    # Step 1: Independent logical reference runner
    logical_ref = run_logical_reference(case, compute_operator=compute_op)
    if not logical_ref.ok:
        empty_phys = PhysicalCircuitResult(
            ok=False,
            status=logical_ref.status,
            failure_category=logical_ref.failure_category,
            error_detail="Skipped QCC physical comparison because logical reference was not runnable.",
        )
        return compare_logical_vs_physical(
            case,
            logical_ref,
            empty_phys,
            tolerances=tolerances,
            compile_exec=None,
            failure_artifacts_root=failure_artifacts_root,
        )

    # Step 2: Black-box QCC compilation + independent physical simulation
    case_work = work_root / case.case_id
    compile_exec = run_qcc_compile(case, case_work, qcc_binary=qcc_binary)
    if not compile_exec.ok:
        failed_phys = PhysicalCircuitResult(
            ok=False,
            status="ERROR",
            failure_category="qcc_compile_failed",
            error_detail=compile_exec.stderr.strip() or f"qcc exited with {compile_exec.returncode}",
        )
        return compare_logical_vs_physical(
            case,
            logical_ref,
            failed_phys,
            tolerances=tolerances,
            compile_exec=compile_exec,
            failure_artifacts_root=failure_artifacts_root,
        )

    physical_res = load_and_simulate_physical(
        compile_exec.manifest_path,
        compute_operator=compute_op,
    )

    # Step 3: Layout-aware full physical-space comparison
    return compare_logical_vs_physical(
        case,
        logical_ref,
        physical_res,
        tolerances=tolerances,
        compile_exec=compile_exec,
        failure_artifacts_root=failure_artifacts_root,
    )


def verify_all_cases(
    *,
    cases_dir: pathlib.Path = DEFAULT_CASES_DIR,
    work_root: pathlib.Path | None = None,
    failure_artifacts_root: pathlib.Path | None = None,
    qcc_binary: pathlib.Path | None = None,
) -> list[dict[str, Any]]:
    """Run all canonical cases and verify each actual status matches expected_oracle_status."""
    cases = load_all_canonical_cases(cases_dir)
    results: list[dict[str, Any]] = []
    if work_root is None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            for case in cases:
                results.append(
                    verify_canonical_case(
                        case,
                        work_root=tmp_path,
                        failure_artifacts_root=failure_artifacts_root,
                        qcc_binary=qcc_binary,
                    )
                )
    else:
        for case in cases:
            results.append(
                verify_canonical_case(
                    case,
                    work_root=work_root,
                    failure_artifacts_root=failure_artifacts_root,
                    qcc_binary=qcc_binary,
                )
            )
    return results


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run QCC Independent Verification Oracle on canonical cases."
    )
    parser.add_argument(
        "--case",
        type=pathlib.Path,
        help="Path to a single canonical case JSON file (defaults to all cases in cases/).",
    )
    parser.add_argument(
        "--failure-dir",
        type=pathlib.Path,
        default=None,
        help="Optional directory to write failure artifact bundles.",
    )
    args = parser.parse_args(argv)

    installed = check_pinned_environment()
    print(
        f"Oracle environment OK: qiskit={installed['qiskit']}, "
        f"qiskit-qasm3-import={installed['qiskit-qasm3-import']}, "
        f"openqasm3={installed['openqasm3']}"
    )

    if args.case is not None:
        cases = [load_canonical_case(args.case)]
        with tempfile.TemporaryDirectory() as tmp:
            results = [
                verify_canonical_case(
                    cases[0],
                    work_root=pathlib.Path(tmp),
                    failure_artifacts_root=args.failure_dir,
                )
            ]
    else:
        cases = load_all_canonical_cases(DEFAULT_CASES_DIR)
        results = verify_all_cases(failure_artifacts_root=args.failure_dir)

    mismatches = 0
    for case, res in zip(cases, results):
        expected = case.expected_oracle_status
        actual = res["status"]
        matched = expected == actual
        if not matched:
            mismatches += 1
        err_str = (
            f"{res['max_amplitude_error']:.2e}"
            if res["max_amplitude_error"] is not None
            else "n/a"
        )
        phase_acct = res["strict_phase_accounting"]["status"]
        print(
            f"  [{'OK' if matched else 'FAIL'}] {case.case_id}: "
            f"status={actual} (expected={expected}), "
            f"q=({res['logical_qubits']}->{res['physical_qubits']}), "
            f"max_amp_err={err_str}, "
            f"ancilla_ok={res['ancilla_zero_ok']}, "
            f"strict_phase_acct={phase_acct}"
        )

    return 0 if mismatches == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
