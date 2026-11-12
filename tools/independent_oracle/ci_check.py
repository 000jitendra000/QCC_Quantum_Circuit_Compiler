"""Deterministic CI execution harness for QCC Independent Verification Oracle (M7 Phase 69).

Runs the exact pinned Python/Qiskit oracle checks invoked by `.github/workflows/independent-oracle.yml`:
- `--stage core`: environment/pip check, all 7 Draft 2020-12 JSON schemas + committed artifacts,
  Session 2 QASM3 compatibility checks, 10 canonical cases, all 41 Python oracle unit tests,
  and bit-for-bit corpus regeneration verification against committed `corpus_v1.jsonl`.
- `--stage campaign`: full 1,080-case corpus execution (exhaustive + sampled modes),
  systematic 17-family (40-trial) fault-injection campaign, and semantically verified
  QCC-vs-Qiskit benchmark baseline schema & semantic validation (no flaky runtime thresholds).
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile
from typing import Any

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parent
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from benchmark.run_baseline import run_benchmark_baseline  # noqa: E402
from corpus.generate_corpus import (  # noqa: E402
    DEFAULT_CORPUS_JSONL,
    DEFAULT_SUMMARY_JSON,
    generate_corpus,
)
from corpus.run_corpus import execute_corpus  # noqa: E402
from fault_injection.run_campaign import run_fault_campaign  # noqa: E402
from qasm3_compat import (  # noqa: E402
    check_pinned_environment,
    validate_manifest_consistency,
)
from verify_case import verify_all_cases  # noqa: E402


def validate_all_schemas_and_committed_artifacts() -> dict[str, int]:
    """Validate all 7 oracle JSON schemas and all committed fixtures/cases/reports."""
    schema_dir = ORACLE_DIR / "schema"
    schemas: dict[str, dict[str, Any]] = {}
    for schema_path in sorted(schema_dir.glob("*.schema.json")):
        obj = json.loads(schema_path.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(obj)
        schemas[schema_path.name] = obj

    # 1. Session 2 real QCC manifests
    manifest_schema = schemas["compile_manifest.schema.json"]
    s2_manifests = sorted((ORACLE_DIR / "fixtures" / "session2").glob("*.manifest.json"))
    for p in s2_manifests:
        jsonschema.validate(instance=json.loads(p.read_text(encoding="utf-8")), schema=manifest_schema)

    # 2. Canonical cases
    case_schema = schemas["canonical_case.schema.json"]
    case_files = sorted((ORACLE_DIR / "cases").glob("*.json"))
    for p in case_files:
        jsonschema.validate(instance=json.loads(p.read_text(encoding="utf-8")), schema=case_schema)

    # 3. 1,080-case corpus JSONL + summary
    corpus_validator = jsonschema.Draft202012Validator(case_schema)
    corpus_count = 0
    for line in DEFAULT_CORPUS_JSONL.read_text(encoding="utf-8").splitlines():
        if line.strip():
            corpus_validator.validate(json.loads(line))
            corpus_count += 1
    corpus_sum_schema = schemas["corpus_summary.schema.json"]
    jsonschema.validate(
        instance=json.loads(DEFAULT_SUMMARY_JSON.read_text(encoding="utf-8")),
        schema=corpus_sum_schema,
    )

    # 4. Committed Session 4 reports
    jsonschema.validate(
        instance=json.loads(
            (ORACLE_DIR / "reports" / "corpus_execution_summary.json").read_text(encoding="utf-8")
        ),
        schema=corpus_sum_schema,
    )
    jsonschema.validate(
        instance=json.loads(
            (ORACLE_DIR / "reports" / "fault_campaign_summary.json").read_text(encoding="utf-8")
        ),
        schema=schemas["fault_campaign_summary.schema.json"],
    )
    jsonschema.validate(
        instance=json.loads(
            (ORACLE_DIR / "reports" / "benchmark_aggregate.json").read_text(encoding="utf-8")
        ),
        schema=schemas["benchmark_aggregate.schema.json"],
    )
    trial_validator = jsonschema.Draft202012Validator(schemas["benchmark_trial.schema.json"])
    trial_count = 0
    for line in (ORACLE_DIR / "reports" / "benchmark_trials.jsonl").read_text(encoding="utf-8").splitlines():
        if line.strip():
            trial_validator.validate(json.loads(line))
            trial_count += 1

    return {
        "schemas": len(schemas),
        "session2_manifests": len(s2_manifests),
        "canonical_cases": len(case_files),
        "corpus_cases": corpus_count,
        "benchmark_trials": trial_count,
    }


def run_core_stage(artifact_dir: pathlib.Path) -> None:
    """Execute CI Stage 1: pinned env, schemas, compat, canonical cases, unittests, corpus determinism."""
    artifact_dir.mkdir(parents=True, exist_ok=True)
    failures_dir = artifact_dir / "failures"

    # 1. Pinned dependency verification + pip check
    installed = check_pinned_environment()
    pip_bin = pathlib.Path(sys.executable).parent / "pip"
    if pip_bin.exists():
        pip_proc = subprocess.run(
            [str(pip_bin), "check"], capture_output=True, text=True, check=False
        )
        if pip_proc.returncode != 0:
            raise RuntimeError(f"pip check failed:\n{pip_proc.stdout}\n{pip_proc.stderr}")
    print(
        f"[CI core] Environment OK: qiskit={installed['qiskit']}, "
        f"qiskit-qasm3-import={installed['qiskit-qasm3-import']}, openqasm3={installed['openqasm3']}"
    )

    # 2. All schemas and committed artifacts
    counts = validate_all_schemas_and_committed_artifacts()
    print(f"[CI core] Schemas & committed artifacts valid: {counts}")

    # 3. Session 2 real QCC QASM compatibility checks
    for mf in sorted((ORACLE_DIR / "fixtures" / "session2").glob("*.manifest.json")):
        validate_manifest_consistency(mf)
    print("[CI core] Session 2 real QCC QASM compatibility checks PASSED (5 fixtures).")

    # 4. Canonical cases end-to-end with failure artifact capture
    from cases_loader import DEFAULT_CASES_DIR, load_all_canonical_cases

    canonical_cases = load_all_canonical_cases(DEFAULT_CASES_DIR)
    canonical_results = verify_all_cases(
        cases_dir=DEFAULT_CASES_DIR,
        failure_artifacts_root=failures_dir / "canonical",
    )
    canonical_summary = []
    for case, res in zip(canonical_cases, canonical_results):
        ok = res["status"] == case.expected_oracle_status
        canonical_summary.append(
            {
                "case_id": case.case_id,
                "expected": case.expected_oracle_status,
                "actual": res["status"],
                "ok": ok,
            }
        )
        if not ok:
            raise AssertionError(
                f"Canonical case {case.case_id} mismatch: expected={case.expected_oracle_status}, got={res['status']}"
            )
    (artifact_dir / "canonical_cases_summary.json").write_text(
        json.dumps(canonical_summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"[CI core] Canonical cases PASSED ({len(canonical_cases)}/{len(canonical_cases)}).")

    # 5. Corpus regeneration determinism verification against committed corpus_v1.jsonl
    committed_sha = json.loads(DEFAULT_SUMMARY_JSON.read_text(encoding="utf-8"))[
        "corpus_file_sha256"
    ]
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = pathlib.Path(tmp)
        s1 = generate_corpus(
            output_jsonl=tmp_path / "regen1.jsonl",
            output_summary=tmp_path / "regen1_sum.json",
        )
        s2 = generate_corpus(
            output_jsonl=tmp_path / "regen2.jsonl",
            output_summary=tmp_path / "regen2_sum.json",
        )
        if s1["corpus_file_sha256"] != committed_sha or s2["corpus_file_sha256"] != committed_sha:
            raise AssertionError(
                f"Corpus regeneration hash mismatch: committed={committed_sha}, "
                f"regen1={s1['corpus_file_sha256']}, regen2={s2['corpus_file_sha256']}"
            )
    print(f"[CI core] Corpus regeneration bit-for-bit identical ({committed_sha[:16]}...).")

    # 6. Full Python oracle unittest suite (41 tests)
    test_proc = subprocess.run(
        [sys.executable, "-m", "unittest", "discover", "-s", str(ORACLE_DIR / "tests"), "-v"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    (artifact_dir / "unittest_output.log").write_text(
        test_proc.stdout + "\n" + test_proc.stderr, encoding="utf-8"
    )
    if test_proc.returncode != 0:
        raise RuntimeError(
            f"Python oracle unit tests failed (rc={test_proc.returncode}):\n"
            f"{test_proc.stdout}\n{test_proc.stderr}"
        )
    print("[CI core] Full Python oracle unittest suite PASSED.")


def run_campaign_stage(artifact_dir: pathlib.Path, *, benchmark_trials: int = 2) -> None:
    """Execute CI Stage 2: 1,080 corpus run, 17-family fault injection, and benchmark check."""
    artifact_dir.mkdir(parents=True, exist_ok=True)
    failures_dir = artifact_dir / "failures"

    # 1. Full 1,080-case corpus execution (exhaustive + sampled clearly separated)
    corpus_report = execute_corpus(
        output_report_path=artifact_dir / "ci_corpus_execution_summary.json",
        failure_artifacts_root=failures_dir / "corpus",
    )
    es = corpus_report["execution_summary"]
    if es["failed_case_ids"]:
        raise AssertionError(f"Corpus failures detected: {es['failed_case_ids']}")
    print(
        f"[CI campaign] Corpus PASSED: {es['executed_cases']} cases "
        f"(exhaustive={es['exhaustive_passed']}, sampled={es['sampled_passed']})"
    )

    # 2. Full 17-family (40-trial) fault-injection campaign
    fault_report = run_fault_campaign(
        output_summary_path=artifact_dir / "ci_fault_campaign_summary.json",
        failure_artifacts_root=failures_dir / "fault_injection",
    )
    if fault_report["misclassified"] != 0 or fault_report["undetected_semantic_defects"] != 0:
        raise AssertionError(
            f"Fault injection regression: misclassified={fault_report['misclassified']}, "
            f"undetected={fault_report['undetected_semantic_defects']}"
        )
    print(
        f"[CI campaign] Fault injection PASSED: "
        f"{fault_report['detected_correctly']}/{fault_report['attempted']} detected (0 undetected)"
    )

    # 3. Semantically verified QCC-vs-Qiskit benchmark check (no flaky wall-clock assertions)
    trials, agg = run_benchmark_baseline(
        trials_per_compiler=benchmark_trials,
        trials_out_path=artifact_dir / "ci_benchmark_trials.jsonl",
        aggregate_out_path=artifact_dir / "ci_benchmark_aggregate.json",
    )
    os_sum = agg["overall_summary"]
    if (
        os_sum["qcc_verified_pass_trials"] != os_sum["total_qcc_trials"]
        or os_sum["qiskit_verified_pass_trials"] != os_sum["total_qiskit_trials"]
    ):
        raise AssertionError(f"Benchmark semantic verification failure: {os_sum}")
    print(
        f"[CI campaign] Benchmark semantic + schema check PASSED: "
        f"{os_sum['total_workloads']} workloads x {benchmark_trials} trials ({len(trials)} records)"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run QCC Independent Oracle CI checks.")
    parser.add_argument(
        "--stage",
        choices=("core", "campaign", "all"),
        default="all",
        help="CI stage to run cleanly.",
    )
    parser.add_argument(
        "--artifact-dir",
        type=pathlib.Path,
        default=pathlib.Path(tempfile.gettempdir()) / "qcc_oracle_ci_artifacts",
        help="Directory for structured CI summaries and failure artifacts.",
    )
    parser.add_argument(
        "--benchmark-trials",
        type=int,
        default=2,
        help="Declared trials per compiler for CI benchmark semantic & schema smoke check.",
    )
    args = parser.parse_args(argv)

    if args.stage in ("core", "all"):
        run_core_stage(args.artifact_dir)
    if args.stage in ("campaign", "all"):
        run_campaign_stage(args.artifact_dir, benchmark_trials=args.benchmark_trials)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

