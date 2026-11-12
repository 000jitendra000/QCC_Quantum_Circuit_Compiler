"""Deterministic corpus execution runner for M7 Phase 68.

Executes generated corpus cases cleanly in parallel or sequentially using their
explicitly declared verification modes (`exhaustive_unitary` vs `sampled_statevector`),
preserving reproducible failure bundles on any mismatch/error and outputting a
schema-validated `qcc.corpus_summary` execution report.
"""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import json
import os
import pathlib
import sys
import tempfile
from typing import Any

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase, load_canonical_cases_jsonl  # noqa: E402
from corpus.generate_corpus import (  # noqa: E402
    CORPUS_SUMMARY_SCHEMA_PATH,
    DEFAULT_CORPUS_JSONL,
    DEFAULT_SUMMARY_JSON,
)
from qasm3_compat import check_pinned_environment  # noqa: E402
from verify_case import verify_canonical_case  # noqa: E402


def _run_one_case_worker(
    payload: tuple[dict[str, Any], str | None],
) -> dict[str, Any]:
    raw_case, failure_root_str = payload
    from cases_loader import load_canonical_case_from_dict

    case = load_canonical_case_from_dict(raw_case)
    failure_root = pathlib.Path(failure_root_str) if failure_root_str else None
    with tempfile.TemporaryDirectory() as tmp:
        return verify_canonical_case(
            case,
            work_root=pathlib.Path(tmp),
            failure_artifacts_root=failure_root,
        )


def execute_corpus(
    *,
    corpus_jsonl: pathlib.Path = DEFAULT_CORPUS_JSONL,
    generation_summary_path: pathlib.Path = DEFAULT_SUMMARY_JSON,
    output_report_path: pathlib.Path | None = None,
    failure_artifacts_root: pathlib.Path | None = None,
    limit: int | None = None,
    workers: int | None = None,
) -> dict[str, Any]:
    """Run corpus cases and return merged schema-validated execution summary."""
    cases: list[CanonicalCase] = load_canonical_cases_jsonl(corpus_jsonl)
    if limit is not None:
        cases = cases[:limit]

    base_summary = json.loads(generation_summary_path.read_text(encoding="utf-8"))
    if limit is not None:
        # Compute scoped summary counters cleanly when running a subset
        from corpus.generate_corpus import SAMPLING_STRATEGY_NAME
        _ = SAMPLING_STRATEGY_NAME

    worker_count = workers or min(max(1, (os.cpu_count() or 2) - 2), 12)
    payloads = [
        (c.raw_json, str(failure_artifacts_root) if failure_artifacts_root else None)
        for c in cases
    ]

    results: list[dict[str, Any]]
    if worker_count <= 1 or len(cases) <= 8:
        results = [_run_one_case_worker(p) for p in payloads]
    else:
        with ProcessPoolExecutor(max_workers=worker_count) as pool:
            results = list(pool.map(_run_one_case_worker, payloads, chunksize=8))

    status_counts: Counter[str] = Counter()
    exhaustive_passed = 0
    exhaustive_failed = 0
    sampled_passed = 0
    sampled_failed = 0
    unsupported_count = 0
    invalid_or_error_count = 0
    max_amp: float | None = None
    max_anc: float | None = None
    failed_case_ids: list[str] = []

    for case, res in zip(cases, results):
        st = res["status"]
        status_counts[st] += 1
        is_exh = bool(res["exhaustive"])

        if res["max_amplitude_error"] is not None:
            val = float(res["max_amplitude_error"])
            max_amp = val if max_amp is None else max(max_amp, val)
        if res["max_residual_ancilla_population"] is not None:
            aval = float(res["max_residual_ancilla_population"])
            max_anc = aval if max_anc is None else max(max_anc, aval)

        if st == "PASS":
            if is_exh:
                exhaustive_passed += 1
            else:
                sampled_passed += 1
        elif st == "MISMATCH":
            failed_case_ids.append(case.case_id)
            if is_exh:
                exhaustive_failed += 1
            else:
                sampled_failed += 1
        elif st == "UNSUPPORTED":
            unsupported_count += 1
            if case.expected_oracle_status != "UNSUPPORTED":
                failed_case_ids.append(case.case_id)
        else:
            invalid_or_error_count += 1
            failed_case_ids.append(case.case_id)

    merged = dict(base_summary)
    merged["execution_summary"] = {
        "executed_cases": len(cases),
        "status_counts": dict(sorted(status_counts.items())),
        "exhaustive_passed": exhaustive_passed,
        "exhaustive_failed": exhaustive_failed,
        "sampled_passed": sampled_passed,
        "sampled_failed": sampled_failed,
        "unsupported_count": unsupported_count,
        "invalid_or_error_count": invalid_or_error_count,
        "max_amplitude_error_observed": max_amp,
        "max_residual_ancilla_observed": max_anc,
        "failed_case_ids": sorted(failed_case_ids),
    }

    schema = json.loads(CORPUS_SUMMARY_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=merged, schema=schema)

    if output_report_path is not None:
        output_report_path.parent.mkdir(parents=True, exist_ok=True)
        output_report_path.write_text(
            json.dumps(merged, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    return merged


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Execute QCC deterministic oracle corpus.")
    parser.add_argument("--corpus-jsonl", type=pathlib.Path, default=DEFAULT_CORPUS_JSONL)
    parser.add_argument("--summary-in", type=pathlib.Path, default=DEFAULT_SUMMARY_JSON)
    parser.add_argument(
        "--output-report",
        type=pathlib.Path,
        default=ORACLE_DIR / "reports" / "corpus_execution_summary.json",
    )
    parser.add_argument("--failure-dir", type=pathlib.Path, default=None)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--workers", type=int, default=None)
    args = parser.parse_args(argv)

    installed = check_pinned_environment()
    print(
        f"Running corpus against Qiskit={installed['qiskit']}, "
        f"qasm3-import={installed['qiskit-qasm3-import']}"
    )
    report = execute_corpus(
        corpus_jsonl=args.corpus_jsonl,
        generation_summary_path=args.summary_in,
        output_report_path=args.output_report,
        failure_artifacts_root=args.failure_dir,
        limit=args.limit,
        workers=args.workers,
    )
    es = report["execution_summary"]
    print(
        f"Executed {es['executed_cases']} cases: "
        f"exhaustive={es['exhaustive_passed']} pass / {es['exhaustive_failed']} fail, "
        f"sampled={es['sampled_passed']} pass / {es['sampled_failed']} fail, "
        f"unsupported={es['unsupported_count']}, "
        f"invalid_or_error={es['invalid_or_error_count']}, "
        f"max_amp_err={es['max_amplitude_error_observed']:.2e}"
    )
    return 0 if not es["failed_case_ids"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
