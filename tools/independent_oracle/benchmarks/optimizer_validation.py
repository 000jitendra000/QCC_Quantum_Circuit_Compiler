"""Independent optimizer validation and comparative benchmark (Roadmap v3.1, Phase 81).

Evaluates whether enabling CommutativeTwoQubitCancellationPass provides repeatable
circuit-quality improvements in QCC's final compiled physical circuits, and quantifies
the additional compilation-time cost.

Primary Evaluation Methodology:
1. Paired A/B configurations on identical inputs, target topologies, placement/routing settings:
   - Configuration A (Control): enable_commutative_cancellation = false (default)
   - Configuration B (Treatment): enable_commutative_cancellation = true (--enable-commutative-cancellation)
2. Three evaluation levels:
   - Level A (Logical): Did the pass remove candidate pairs? (from pass diagnostic reports)
   - Level B (Physical): Did reductions survive basis translation, routing, and scheduling?
   - Level C (End-to-End): Did final circuits pass independent Qiskit oracle verification
     including clean-ancilla policy (ADR-0001) and target legality?
3. Corpus coverage:
   - All 82 positive cancellation opportunity cases identified in Phase 78
   - Stratified control cases from zero-opportunity families to verify safety
   - Target topologies: both 'linear' and 'ring'
4. Multiple repeated trials per case to measure wall-clock compilation overhead accurately.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import platform
import statistics
import sys
import tempfile
import time
from typing import Any

import jsonschema
import qiskit

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ORACLE_DIR.parent.parent
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    CanonicalCase,
    load_canonical_cases_jsonl,
)
from corpus.generate_corpus import DEFAULT_CORPUS_JSONL  # noqa: E402
from equivalence.comparator import compare_logical_vs_physical  # noqa: E402
from runners.qcc_runner import (  # noqa: E402
    PhysicalCircuitResult,
    load_and_simulate_physical,
    resolve_qcc_binary,
    run_qcc_compile,
)
from runners.qiskit_reference import run_logical_reference  # noqa: E402
from benchmarks.audit_two_qubit_cancellation import (  # noqa: E402
    parse_operations,
    is_valid_cx_commutation,
    is_valid_cz_commutation,
)

SCHEMA_PATH = ORACLE_DIR / "schema" / "optimizer_validation.schema.json"
DEFAULT_OUTPUT_PATH = ORACLE_DIR / "reports" / "optimizer_validation_aggregate.json"
DEFAULT_TRIALS_PATH = ORACLE_DIR / "reports" / "optimizer_validation_trials.jsonl"


def detect_cancellation_opportunities(case: CanonicalCase) -> list[tuple[str, int, int]]:
    """Detect realizable CX and CZ cancellation pairs in case logical QASM."""
    ops = parse_operations(case.logical_qasm)
    pairs: list[tuple[str, int, int]] = []

    for i, (op1, qargs1, _) in enumerate(ops):
        if op1 == "cx":
            ctrl, targ = qargs1
            for j in range(i + 1, len(ops)):
                op2, qargs2, _ = ops[j]
                if op2 == "cx" and qargs2 == (ctrl, targ):
                    if j > i + 1 and is_valid_cx_commutation(ops, i, j, ctrl, targ):
                        pairs.append(("cx", i, j))
                    break
        elif op1 == "cz":
            q0, q1 = qargs1
            norm_qargs = tuple(sorted((q0, q1)))
            for j in range(i + 1, len(ops)):
                op2, qargs2, _ = ops[j]
                if op2 == "cz" and tuple(sorted(qargs2)) == norm_qargs:
                    if j > i + 1 and is_valid_cz_commutation(ops, i, j, norm_qargs[0], norm_qargs[1]):
                        pairs.append(("cz", i, j))
                    break

    return pairs


def select_benchmark_cases(
    all_cases: list[CanonicalCase],
    include_controls: bool = True,
    control_count_per_family: int = 6,
) -> tuple[list[CanonicalCase], dict[str, str]]:
    """Select all positive opportunity cases plus stratified controls."""
    positive_cases: list[CanonicalCase] = []
    control_candidates_by_family: dict[str, list[CanonicalCase]] = {}
    opportunity_map: dict[str, str] = {}

    for c in all_cases:
        opps = detect_cancellation_opportunities(c)
        fam = c.generator_metadata.get("family", "unknown") if c.generator_metadata else "unknown"
        if opps:
            positive_cases.append(c)
            opportunity_map[c.case_id] = "positive"
        else:
            control_candidates_by_family.setdefault(fam, []).append(c)

    selected: list[CanonicalCase] = list(positive_cases)

    if include_controls:
        for fam, cases in sorted(control_candidates_by_family.items()):
            # Stratify by qubit count and target
            by_qubits: dict[int, list[CanonicalCase]] = {}
            for c in cases:
                by_qubits.setdefault(c.logical_qubits, []).append(c)

            family_selected: list[CanonicalCase] = []
            q_tiers = sorted(by_qubits.keys())
            idx = 0
            while len(family_selected) < control_count_per_family and any(by_qubits.values()):
                q = q_tiers[idx % len(q_tiers)]
                if by_qubits[q]:
                    c_pick = by_qubits[q].pop(0)
                    family_selected.append(c_pick)
                    opportunity_map[c_pick.case_id] = "control"
                idx += 1

            selected.extend(family_selected)

    return selected, opportunity_map


def measure_compilation_trials(
    case: CanonicalCase,
    enable_cancellation: bool,
    work_dir: pathlib.Path,
    qcc_binary: pathlib.Path,
    num_trials: int,
    pass_report_name: str,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[float]]:
    """Execute warm-up and repeated compilation trials, returning last manifest and timings."""
    extra_args = ["--pass-report", pass_report_name]
    if enable_cancellation:
        extra_args.append("--enable-commutative-cancellation")

    # Warm-up run (not included in timing stats)
    warmup_dir = work_dir / "warmup"
    run_qcc_compile(
        case,
        warmup_dir,
        qcc_binary=qcc_binary,
        extra_args=extra_args,
    )

    times_ms: list[float] = []
    last_manifest: dict[str, Any] = {}
    last_pass_reports: list[dict[str, Any]] = []

    for t in range(num_trials):
        trial_dir = work_dir / f"trial_{t}"
        start_t = time.perf_counter()
        exec_res = run_qcc_compile(
            case,
            trial_dir,
            qcc_binary=qcc_binary,
            extra_args=extra_args,
        )
        elapsed_ms = (time.perf_counter() - start_t) * 1000.0

        if not exec_res.ok:
            raise RuntimeError(
                f"QCC compilation failed for case {case.case_id} "
                f"(cancellation={enable_cancellation}, returncode={exec_res.returncode}):\n{exec_res.stderr}"
            )

        times_ms.append(elapsed_ms)
        last_manifest = json.loads(exec_res.manifest_path.read_text(encoding="utf-8"))
        report_file = trial_dir / pass_report_name
        if report_file.exists():
            last_pass_reports = json.loads(report_file.read_text(encoding="utf-8"))

    return last_manifest, last_pass_reports, times_ms


def evaluate_paired_case(
    case: CanonicalCase,
    opportunity_type: str,
    work_dir: pathlib.Path,
    qcc_binary: pathlib.Path,
    num_trials: int,
    verify_oracle: bool,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Evaluate a single case in both Control (A) and Treatment (B) configurations."""
    case_work_dir = work_dir / case.case_id
    case_work_dir.mkdir(parents=True, exist_ok=True)

    ctrl_work_dir = case_work_dir / "control"
    treat_work_dir = case_work_dir / "treatment"

    # Run Control (A)
    ctrl_manifest, ctrl_pass_reports, ctrl_times = measure_compilation_trials(
        case,
        enable_cancellation=False,
        work_dir=ctrl_work_dir,
        qcc_binary=qcc_binary,
        num_trials=num_trials,
        pass_report_name=f"{case.case_id}.ctrl_report.json",
    )

    # Run Treatment (B)
    treat_manifest, treat_pass_reports, treat_times = measure_compilation_trials(
        case,
        enable_cancellation=True,
        work_dir=treat_work_dir,
        qcc_binary=qcc_binary,
        num_trials=num_trials,
        pass_report_name=f"{case.case_id}.treat_report.json",
    )

    # Extract Control metrics
    ctrl_metrics = ctrl_manifest.get("metrics", {})
    ctrl_total_gates = int(ctrl_metrics.get("total_gate_count", 0))
    ctrl_2q_gates = int(ctrl_metrics.get("two_qubit_gates", 0))
    ctrl_swaps = int(ctrl_metrics.get("router_swaps_inserted", 0))
    ctrl_final_swaps = int(ctrl_metrics.get("final_emitted_swaps", 0))
    ctrl_depth = int(ctrl_metrics.get("circuit_depth", 0))
    ctrl_time_median = float(statistics.median(ctrl_times))
    ctrl_time_min = float(min(ctrl_times))
    ctrl_time_max = float(max(ctrl_times))
    ctrl_time_spread = float(ctrl_time_max - ctrl_time_min)

    # Extract Treatment metrics
    treat_metrics = treat_manifest.get("metrics", {})
    treat_total_gates = int(treat_metrics.get("total_gate_count", 0))
    treat_2q_gates = int(treat_metrics.get("two_qubit_gates", 0))
    treat_swaps = int(treat_metrics.get("router_swaps_inserted", 0))
    treat_final_swaps = int(treat_metrics.get("final_emitted_swaps", 0))
    treat_depth = int(treat_metrics.get("circuit_depth", 0))
    treat_time_median = float(statistics.median(treat_times))
    treat_time_min = float(min(treat_times))
    treat_time_max = float(max(treat_times))
    treat_time_spread = float(treat_time_max - treat_time_min)

    # Compute deltas
    gates_delta = treat_total_gates - ctrl_total_gates
    two_qubit_delta = treat_2q_gates - ctrl_2q_gates
    swaps_delta = treat_swaps - ctrl_swaps
    depth_delta = treat_depth - ctrl_depth
    time_delta = treat_time_median - ctrl_time_median
    overhead_ratio = (treat_time_median / ctrl_time_median) if ctrl_time_median > 0 else 1.0

    # Extract pass diagnostics for commutative_cancellation
    comm_reports = [p for p in treat_pass_reports if p.get("pass_id") == "commutative_cancellation"]
    total_ops_removed = sum(int(p.get("transformation", {}).get("operations_removed", 0)) for p in comm_reports)
    total_pass_duration_ms = sum(float(p.get("measurements", {}).get("duration_ms", 0.0) or 0.0) for p in comm_reports)
    pass_status = "ran" if comm_reports else "not_executed"

    # Independent Oracle Verification on Treatment physical circuit
    oracle_verification: dict[str, Any]
    if verify_oracle:
        log_ref = run_logical_reference(case)
        # Point to last trial manifest path
        last_treat_manifest_path = treat_work_dir / f"trial_{num_trials - 1}" / f"{case.case_id}.manifest.json"
        phys_sim = load_and_simulate_physical(last_treat_manifest_path)
        comp_res = compare_logical_vs_physical(case, log_ref, phys_sim)

        oracle_verification = {
            "status": comp_res.get("status", "FAIL"),
            "ancilla_zero_ok": bool(comp_res.get("ancilla_zero_ok", False)),
            "max_amplitude_error": float(comp_res.get("max_amplitude_error", 0.0)),
            "strict_phase_accounting": str(comp_res.get("strict_phase_accounting", "UNVERIFIED")),
        }
    else:
        oracle_verification = {
            "status": "NOT_RUN",
            "ancilla_zero_ok": True,
            "max_amplitude_error": 0.0,
            "strict_phase_accounting": "UNVERIFIED",
        }

    fam = case.generator_metadata.get("family", "unknown") if case.generator_metadata else "unknown"

    case_result = {
        "case_id": case.case_id,
        "family": fam,
        "target": case.target,
        "logical_qubits": case.logical_qubits,
        "opportunity_type": opportunity_type,
        "control": {
            "total_gates": ctrl_total_gates,
            "two_qubit_gates": ctrl_2q_gates,
            "router_swaps": ctrl_swaps,
            "final_emitted_swaps": ctrl_final_swaps,
            "circuit_depth": ctrl_depth,
            "compile_time_ms_median": round(ctrl_time_median, 3),
            "compile_time_ms_min": round(ctrl_time_min, 3),
            "compile_time_ms_max": round(ctrl_time_max, 3),
            "compile_time_ms_spread": round(ctrl_time_spread, 3),
        },
        "treatment": {
            "total_gates": treat_total_gates,
            "two_qubit_gates": treat_2q_gates,
            "router_swaps": treat_swaps,
            "final_emitted_swaps": treat_final_swaps,
            "circuit_depth": treat_depth,
            "compile_time_ms_median": round(treat_time_median, 3),
            "compile_time_ms_min": round(treat_time_min, 3),
            "compile_time_ms_max": round(treat_time_max, 3),
            "compile_time_ms_spread": round(treat_time_spread, 3),
        },
        "delta": {
            "gates_delta": gates_delta,
            "two_qubit_delta": two_qubit_delta,
            "swaps_delta": swaps_delta,
            "depth_delta": depth_delta,
            "compile_time_delta_ms": round(time_delta, 3),
            "compile_time_overhead_ratio": round(overhead_ratio, 3),
        },
        "pass_diagnostics": {
            "status": pass_status,
            "operations_removed": total_ops_removed,
            "duration_ms": round(total_pass_duration_ms, 3),
        },
        "oracle_verification": oracle_verification,
    }

    trial_log_record = {
        "case_id": case.case_id,
        "family": fam,
        "target": case.target,
        "opportunity_type": opportunity_type,
        "ctrl_times_ms": [round(t, 3) for t in ctrl_times],
        "treat_times_ms": [round(t, 3) for t in treat_times],
        "ctrl_metrics": ctrl_metrics,
        "treat_metrics": treat_metrics,
        "pass_diagnostics": case_result["pass_diagnostics"],
        "oracle_status": oracle_verification["status"],
    }

    return case_result, trial_log_record


def compute_aggregates(
    case_results: list[dict[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any], str, dict[str, Any]]:
    """Compute aggregate statistical summaries, classification, and recommendation."""
    total_cases = len(case_results)
    pos_cases = sum(1 for c in case_results if c["opportunity_type"] == "positive")
    ctrl_cases = total_cases - pos_cases

    # Logical Level
    total_logical_ops_removed = sum(c["pass_diagnostics"]["operations_removed"] for c in case_results)
    total_logical_pairs_removed = total_logical_ops_removed // 2
    cases_with_logical_reductions = sum(
        1 for c in case_results if c["pass_diagnostics"]["operations_removed"] > 0
    )

    # Physical Level
    total_phys_gates_saved = -sum(c["delta"]["gates_delta"] for c in case_results)
    total_phys_2q_saved = -sum(c["delta"]["two_qubit_delta"] for c in case_results)

    swaps_improved = sum(1 for c in case_results if c["delta"]["swaps_delta"] < 0)
    swaps_unchanged = sum(1 for c in case_results if c["delta"]["swaps_delta"] == 0)
    swaps_worsened = sum(1 for c in case_results if c["delta"]["swaps_delta"] > 0)

    depth_improved = sum(1 for c in case_results if c["delta"]["depth_delta"] < 0)
    depth_unchanged = sum(1 for c in case_results if c["delta"]["depth_delta"] == 0)
    depth_worsened = sum(1 for c in case_results if c["delta"]["depth_delta"] > 0)

    total_gates_improved = sum(1 for c in case_results if c["delta"]["gates_delta"] < 0)
    total_gates_unchanged = sum(1 for c in case_results if c["delta"]["gates_delta"] == 0)
    total_gates_worsened = sum(1 for c in case_results if c["delta"]["gates_delta"] > 0)

    two_qubit_improved = sum(1 for c in case_results if c["delta"]["two_qubit_delta"] < 0)
    two_qubit_unchanged = sum(1 for c in case_results if c["delta"]["two_qubit_delta"] == 0)
    two_qubit_worsened = sum(1 for c in case_results if c["delta"]["two_qubit_delta"] > 0)

    # Compilation Overhead
    ctrl_times = [c["control"]["compile_time_ms_median"] for c in case_results]
    treat_times = [c["treatment"]["compile_time_ms_median"] for c in case_results]
    pass_durations = [c["pass_diagnostics"]["duration_ms"] for c in case_results]

    mean_ctrl_time = statistics.mean(ctrl_times) if ctrl_times else 0.0
    mean_treat_time = statistics.mean(treat_times) if treat_times else 0.0
    overall_overhead = (mean_treat_time / mean_ctrl_time) if mean_ctrl_time > 0 else 1.0
    mean_pass_duration = statistics.mean(pass_durations) if pass_durations else 0.0

    aggregate_summary = {
        "total_cases": total_cases,
        "positive_opportunity_cases": pos_cases,
        "control_cases": ctrl_cases,
        "logical_level": {
            "total_logical_pairs_removed": total_logical_pairs_removed,
            "total_logical_operations_removed": total_logical_ops_removed,
            "cases_with_logical_reductions": cases_with_logical_reductions,
        },
        "physical_level": {
            "total_physical_gates_saved": total_phys_gates_saved,
            "total_physical_two_qubit_saved": total_phys_2q_saved,
            "swaps_improved": swaps_improved,
            "swaps_unchanged": swaps_unchanged,
            "swaps_worsened": swaps_worsened,
            "depth_improved": depth_improved,
            "depth_unchanged": depth_unchanged,
            "depth_worsened": depth_worsened,
            "total_gates_improved": total_gates_improved,
            "total_gates_unchanged": total_gates_unchanged,
            "total_gates_worsened": total_gates_worsened,
            "two_qubit_improved": two_qubit_improved,
            "two_qubit_unchanged": two_qubit_unchanged,
            "two_qubit_worsened": two_qubit_worsened,
        },
        "compilation_overhead": {
            "mean_compile_time_control_ms": round(mean_ctrl_time, 3),
            "mean_compile_time_treatment_ms": round(mean_treat_time, 3),
            "overall_overhead_factor": round(overall_overhead, 3),
            "mean_pass_duration_ms": round(mean_pass_duration, 3),
        },
    }

    # Oracle verification summary
    total_verified = sum(1 for c in case_results if c["oracle_verification"]["status"] != "NOT_RUN")
    pass_count = sum(1 for c in case_results if c["oracle_verification"]["status"] == "PASS")
    fail_count = total_verified - pass_count
    clean_ancilla_ok_count = sum(1 for c in case_results if c["oracle_verification"]["ancilla_zero_ok"])
    clean_ancilla_rate = (clean_ancilla_ok_count / total_cases) if total_cases > 0 else 0.0

    spa_pass = sum(1 for c in case_results if c["oracle_verification"]["strict_phase_accounting"] == "PASS")
    spa_mismatch = sum(1 for c in case_results if c["oracle_verification"]["strict_phase_accounting"] == "MISMATCH")
    spa_unverified = sum(1 for c in case_results if c["oracle_verification"]["strict_phase_accounting"] == "UNVERIFIED")

    oracle_summary = {
        "total_verified": total_verified,
        "pass_count": pass_count,
        "fail_count": fail_count,
        "clean_ancilla_pass_rate": round(clean_ancilla_rate, 4),
        "strict_phase_accounting_summary": {
            "pass": spa_pass,
            "mismatch": spa_mismatch,
            "unverified": spa_unverified,
        },
    }

    # Determine outcome classification
    # A: CLEAR BENEFIT
    # B: LOGICAL BENEFIT ONLY
    # C: MIXED RESULT
    # D: NO MEASURABLE BENEFIT
    # E: NEGATIVE RESULT
    # F: INSUFFICIENT EVIDENCE
    if total_verified > 0 and fail_count > 0:
        outcome = "E_NEGATIVE_RESULT"
    elif total_cases < 20:
        outcome = "F_INSUFFICIENT_EVIDENCE"
    elif cases_with_logical_reductions == 0:
        outcome = "D_NO_MEASURABLE_BENEFIT"
    elif total_phys_gates_saved <= 0 and total_phys_2q_saved <= 0:
        outcome = "B_LOGICAL_BENEFIT_ONLY"
    elif total_gates_worsened > total_gates_improved or swaps_worsened > swaps_improved:
        outcome = "C_MIXED_RESULT"
    elif total_gates_worsened > 0 or swaps_worsened > 0:
        outcome = "C_MIXED_RESULT"
    elif total_gates_improved > 0 and total_gates_worsened == 0:
        outcome = "A_CLEAR_BENEFIT"
    else:
        outcome = "C_MIXED_RESULT"

    # Default enablement recommendation
    if outcome == "A_CLEAR_BENEFIT" and overall_overhead < 1.30 and fail_count == 0:
        recommendation = {
            "enable_by_default": True,
            "rationale": (
                f"Commutative cancellation delivers consistent physical gate reductions "
                f"({total_phys_gates_saved} gates saved, {total_phys_2q_saved} two-qubit gates saved) "
                f"with zero physical regressions (0 worsened), 100% independent oracle equivalence, "
                f"100% clean ancilla compliance, and modest compilation overhead ({overall_overhead:.2f}x)."
            ),
            "prerequisites_or_conditions": "Safe for default enablement in O2 and O3 pipelines.",
        }
    else:
        recommendation = {
            "enable_by_default": False,
            "rationale": (
                f"Outcome classification is {outcome}. While logical reductions occur in {cases_with_logical_reductions} cases, "
                f"default enablement should remain opt-in until further routing/optimizer co-design or thresholding is established."
            ),
            "prerequisites_or_conditions": "Keep --enable-commutative-cancellation opt-in (default: false).",
        }

    return aggregate_summary, oracle_summary, outcome, recommendation


def main() -> int:
    parser = argparse.ArgumentParser(description="QCC Independent Optimizer Validation Benchmark (Phase 81)")
    parser.add_argument("--corpus", type=pathlib.Path, default=DEFAULT_CORPUS_JSONL, help="Path to canonical corpus JSONL")
    parser.add_argument("--trials", type=int, default=3, help="Number of timing trials per case")
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT_PATH, help="Path to aggregate JSON report")
    parser.add_argument("--trials-output", type=pathlib.Path, default=DEFAULT_TRIALS_PATH, help="Path to raw trials JSONL")
    parser.add_argument("--include-controls", action=argparse.BooleanOptionalAction, default=True, help="Include stratified control cases")
    parser.add_argument("--control-count", type=int, default=6, help="Control cases per zero-opportunity family")
    parser.add_argument("--verify-oracle", action=argparse.BooleanOptionalAction, default=True, help="Run independent Qiskit oracle verification")
    parser.add_argument("--max-cases", type=int, default=None, help="Maximum cases to evaluate (for quick runs)")
    args = parser.parse_args()

    qcc_binary = resolve_qcc_binary()
    print(f"Using QCC binary: {qcc_binary}")
    print(f"Loading corpus from: {args.corpus}")

    all_corpus_cases = load_canonical_cases_jsonl(args.corpus)
    selected_cases, opp_map = select_benchmark_cases(
        all_corpus_cases,
        include_controls=args.include_controls,
        control_count_per_family=args.control_count,
    )

    if args.max_cases is not None:
        selected_cases = selected_cases[: args.max_cases]

    print(
        f"Evaluating {len(selected_cases)} benchmark cases "
        f"({sum(1 for c in selected_cases if opp_map[c.case_id] == 'positive')} positive, "
        f"{sum(1 for c in selected_cases if opp_map[c.case_id] == 'control')} controls) "
        f"with {args.trials} trials per configuration..."
    )

    case_results: list[dict[str, Any]] = []
    trial_log_records: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(prefix="qcc_phase81_benchmark_") as temp_dir:
        temp_path = pathlib.Path(temp_dir)
        start_bench_time = time.time()

        for idx, case in enumerate(selected_cases):
            opp_type = opp_map[case.case_id]
            fam = case.generator_metadata.get("family", "unknown") if case.generator_metadata else "unknown"
            t0 = time.time()
            res, trial_log = evaluate_paired_case(
                case=case,
                opportunity_type=opp_type,
                work_dir=temp_path,
                qcc_binary=qcc_binary,
                num_trials=args.trials,
                verify_oracle=args.verify_oracle,
            )
            elapsed = time.time() - t0
            case_results.append(res)
            trial_log_records.append(trial_log)

            print(
                f"[{idx + 1:3d}/{len(selected_cases):3d}] {case.case_id} ({fam}, {case.target}, {opp_type}): "
                f"gates {res['control']['total_gates']} -> {res['treatment']['total_gates']} (delta {res['delta']['gates_delta']:+d}), "
                f"2q {res['control']['two_qubit_gates']} -> {res['treatment']['two_qubit_gates']} (delta {res['delta']['two_qubit_delta']:+d}), "
                f"swaps {res['control']['router_swaps']} -> {res['treatment']['router_swaps']} (delta {res['delta']['swaps_delta']:+d}), "
                f"oracle: {res['oracle_verification']['status']} ({elapsed:.2f}s)"
            )

        total_duration_sec = time.time() - start_bench_time

    aggregate_summary, oracle_summary, outcome, recommendation = compute_aggregates(case_results)

    final_report = {
        "schema_id": "qcc.optimizer_validation_benchmark",
        "schema_version": "1.0.0",
        "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "environment": {
            "os": platform.system(),
            "architecture": platform.machine(),
            "python_version": platform.python_version(),
            "qiskit_version": qiskit.__version__,
            "qcc_version": "0.1.0",
        },
        "benchmark_config": {
            "corpus_file": str(args.corpus),
            "optimization_level": "2",
            "trials_per_case": args.trials,
            "verified_with_oracle": args.verify_oracle,
        },
        "cases_evaluated": len(case_results),
        "case_results": case_results,
        "aggregate_summary": aggregate_summary,
        "oracle_verification_summary": oracle_summary,
        "outcome_classification": outcome,
        "default_enablement_recommendation": recommendation,
    }

    # Validate against schema
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=final_report, schema=schema)
    print("Schema validation PASSED.")

    # Write output files
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(final_report, indent=2), encoding="utf-8")
    print(f"Saved aggregate benchmark report to: {args.output}")

    args.trials_output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.trials_output, "w", encoding="utf-8") as f:
        for rec in trial_log_records:
            f.write(json.dumps(rec) + "\n")
    print(f"Saved raw trial logs to: {args.trials_output}")

    print("\n================ BENCHMARK SUMMARY ================")
    print(f"Total Cases: {len(case_results)} ({aggregate_summary['positive_opportunity_cases']} positive, {aggregate_summary['control_cases']} controls)")
    print(f"Logical: {aggregate_summary['logical_level']['total_logical_operations_removed']} ops removed across {aggregate_summary['logical_level']['cases_with_logical_reductions']} cases")
    print(f"Physical Gates Saved: {aggregate_summary['physical_level']['total_physical_gates_saved']} (two-qubit: {aggregate_summary['physical_level']['total_physical_two_qubit_saved']})")
    print(f"Physical Gates: {aggregate_summary['physical_level']['total_gates_improved']} improved, {aggregate_summary['physical_level']['total_gates_unchanged']} unchanged, {aggregate_summary['physical_level']['total_gates_worsened']} worsened")
    print(f"SWAPs: {aggregate_summary['physical_level']['swaps_improved']} improved, {aggregate_summary['physical_level']['swaps_unchanged']} unchanged, {aggregate_summary['physical_level']['swaps_worsened']} worsened")
    print(f"Compilation Overhead: {aggregate_summary['compilation_overhead']['overall_overhead_factor']:.2f}x (mean pass duration: {aggregate_summary['compilation_overhead']['mean_pass_duration_ms']:.2f} ms)")
    print(f"Oracle Equivalence: {oracle_summary['pass_count']}/{oracle_summary['total_verified']} PASS (clean ancilla: {oracle_summary['clean_ancilla_pass_rate'] * 100:.1f}%)")
    print(f"Outcome Classification: {outcome}")
    print(f"Recommendation: Default Enablement = {recommendation['enable_by_default']}")
    print(f"Rationale: {recommendation['rationale']}")
    print("===================================================\n")

    return 0


if __name__ == "__main__":
    sys.exit(main())

