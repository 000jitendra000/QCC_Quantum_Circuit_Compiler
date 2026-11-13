"""Supplemental verification of Control A physical outputs via independent Qiskit oracle (Phase 81).

Verifies all 130 Control A physical outputs against the canonical logical reference
using the independent Qiskit oracle:
- Full 2^{n_phys} physical statevector evolution
- Initial and final layout map handling
- Strict ADR-0001 clean ancilla zero-state verification
- Single common global phase equivalence
- Verification of zero mismatches, zero unsupported cases, zero unexecuted cases
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import sys
import tempfile
import time
from typing import Any

import qiskit

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import CanonicalCase, load_canonical_cases_jsonl  # noqa: E402
from corpus.generate_corpus import DEFAULT_CORPUS_JSONL  # noqa: E402
from equivalence.comparator import compare_logical_vs_physical  # noqa: E402
from runners.qcc_runner import (  # noqa: E402
    load_and_simulate_physical,
    resolve_qcc_binary,
    run_qcc_compile,
)
from runners.qiskit_reference import run_logical_reference  # noqa: E402
from benchmarks.optimizer_validation import select_benchmark_cases  # noqa: E402

OUTPUT_PATH = ORACLE_DIR / "reports" / "control_a_verification_results.json"


def main() -> int:
    parser = argparse.ArgumentParser(description="Supplemental verification of Control A physical outputs")
    parser.add_argument("--corpus", type=pathlib.Path, default=DEFAULT_CORPUS_JSONL, help="Path to corpus JSONL")
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT_PATH, help="Path to output JSON results")
    args = parser.parse_args()

    qcc_binary = resolve_qcc_binary()
    print(f"Using QCC binary: {qcc_binary}")
    print(f"Loading corpus from: {args.corpus}")

    all_cases = load_canonical_cases_jsonl(args.corpus)
    selected_cases, opp_map = select_benchmark_cases(all_cases, include_controls=True, control_count_per_family=6)

    print(f"Verifying all {len(selected_cases)} Control A outputs via independent Qiskit oracle...")

    results: list[dict[str, Any]] = []
    pass_count = 0
    fail_count = 0
    ancilla_ok_count = 0
    t_start = time.time()

    with tempfile.TemporaryDirectory(prefix="qcc_ctrl_a_verify_") as tmp_dir:
        tmp_path = pathlib.Path(tmp_dir)

        for idx, case in enumerate(selected_cases):
            work_dir = tmp_path / case.case_id
            work_dir.mkdir(parents=True, exist_ok=True)

            # Compile Control A (default baseline: enable_commutative_cancellation = false)
            exec_res = run_qcc_compile(
                case,
                work_dir,
                qcc_binary=qcc_binary,
                extra_args=["--pass-report", "ctrl_pass_report.json"],
            )

            if not exec_res.ok:
                raise RuntimeError(f"Control compilation failed for {case.case_id}: {exec_res.stderr}")

            # Independent logical reference in Qiskit
            log_ref = run_logical_reference(case)
            if not log_ref.ok:
                raise RuntimeError(f"Logical reference failed for {case.case_id}: {log_ref.error_detail}")

            # Independent physical simulation in Qiskit
            phys_sim = load_and_simulate_physical(exec_res.manifest_path)
            if not phys_sim.ok:
                raise RuntimeError(f"Physical simulation failed for {case.case_id}: {phys_sim.error_detail}")

            # Independent semantic comparator
            comp_res = compare_logical_vs_physical(case, log_ref, phys_sim, compile_exec=exec_res)

            status = comp_res.get("status", "FAIL")
            ancilla_zero_ok = bool(comp_res.get("ancilla_zero_ok", False))
            max_amp_err = float(comp_res.get("max_amplitude_error", 0.0))
            spa = str(comp_res.get("strict_phase_accounting", "UNVERIFIED"))

            if status == "PASS":
                pass_count += 1
            else:
                fail_count += 1

            if ancilla_zero_ok:
                ancilla_ok_count += 1

            rec = {
                "case_id": case.case_id,
                "family": case.generator_metadata.get("family", "unknown") if case.generator_metadata else "unknown",
                "target": case.target,
                "logical_qubits": case.logical_qubits,
                "status": status,
                "ancilla_zero_ok": ancilla_zero_ok,
                "max_amplitude_error": max_amp_err,
                "strict_phase_accounting": spa,
                "physical_sha256": comp_res.get("physical_identity", {}).get("physical_qasm_sha256"),
            }
            results.append(rec)

            if (idx + 1) % 25 == 0 or idx + 1 == len(selected_cases):
                print(f"[{idx + 1:3d}/{len(selected_cases):3d}] Control A oracle verification in progress... ({pass_count} PASS, {fail_count} FAIL)")

    elapsed_sec = time.time() - t_start
    print(f"\nCompleted in {elapsed_sec:.2f}s.")
    print(f"Control A Verification Results: {pass_count}/{len(selected_cases)} PASS ({pass_count/len(selected_cases)*100:.1f}%)")
    print(f"ADR-0001 Clean Ancilla Compliance: {ancilla_ok_count}/{len(selected_cases)} OK ({ancilla_ok_count/len(selected_cases)*100:.1f}%)")
    print(f"Failures: {fail_count}")

    output_summary = {
        "benchmark": "Control A Supplemental Independent Oracle Verification",
        "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "total_workloads_evaluated": len(results),
        "pass_count": pass_count,
        "fail_count": fail_count,
        "clean_ancilla_ok_count": ancilla_ok_count,
        "clean_ancilla_pass_rate": round(ancilla_ok_count / len(results), 4) if results else 0.0,
        "case_verifications": results,
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output_summary, indent=2), encoding="utf-8")
    print(f"Saved Control A verification results to: {args.output}")

    if fail_count > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

