"""Fair, semantically verified QCC-vs-Qiskit compilation-cost baseline (M7 Phase 70).

Methodology & Fairness Guardrails:
1. Both compilers receive the exact same logical input circuit, physical qubit count
   `m = max(n, 2)`, undirected coupling graph (`linear` or `ring`), and native basis
   `["rz", "sx", "cx"]`.
2. Non-comparable targets (`linear-generic`, `ring-generic`, `frontend_only`) are
   explicitly classified `UNAVAILABLE` with documented reasons rather than compared
   under misaligned bases or cost weights.
3. Both QCC outputs AND Qiskit outputs are independently semantically verified using the
   Session 3 layout-aware, common-phase, clean-ancilla comparator BEFORE counting any
   cost metrics.
4. QCC CLI exposes no randomized seed and runs one deterministic `SabreBidirectionalRouter`
   configuration (`O2`). Repeated external trials N measure wall-clock timing repeatability
   of that single deterministic configuration, NOT independent search trials.
5. Qiskit `optimization_level=2` internal SABRE trial budgets (`layout_trials`,
   `swap_trials`, `max_iterations`, routing `trials`) are inspected dynamically from the
   preset pass manager and explicitly disclosed (`equal_internal_compute_budget = false`).
6. Compilation wall-clock time is measured strictly separately from semantic verification.
   CLI process spawn overhead on QCC vs in-process Python execution on Qiskit is
   explicitly disclosed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import statistics
import sys
import tempfile
import time
from typing import Any

import jsonschema
import qiskit
from qiskit.quantum_info import Operator
from qiskit.transpiler import CouplingMap
from qiskit.transpiler.passes import SabreLayout, SabreSwap
from qiskit.transpiler.preset_passmanagers import generate_preset_pass_manager

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    CanonicalCase,
    DEFAULT_CASES_DIR,
    canonical_json_bytes,
    load_canonical_case,
    load_canonical_cases_jsonl,
    sha256_bytes,
)
from corpus.generate_corpus import DEFAULT_CORPUS_JSONL  # noqa: E402
from equivalence.comparator import compare_logical_vs_physical  # noqa: E402
from equivalence.layout import validate_manifest_layout  # noqa: E402
from qasm3_compat import check_pinned_environment  # noqa: E402
from runners.qcc_runner import (  # noqa: E402
    PhysicalCircuitResult,
    load_and_simulate_physical,
    run_qcc_compile,
)
from runners.qiskit_reference import run_logical_reference  # noqa: E402


TRIAL_SCHEMA_PATH = ORACLE_DIR / "schema" / "benchmark_trial.schema.json"
AGGREGATE_SCHEMA_PATH = ORACLE_DIR / "schema" / "benchmark_aggregate.schema.json"
DEFAULT_TRIALS_PATH = ORACLE_DIR / "reports" / "benchmark_trials.jsonl"
DEFAULT_AGGREGATE_PATH = ORACLE_DIR / "reports" / "benchmark_aggregate.json"

DEFAULT_NUM_TRIALS = 5
DEFAULT_SEED_BASE = 1000
STANDARD_BASIS_GATES = ["rz", "sx", "cx"]
TWO_Q_GATES = frozenset({"cx", "cz", "swap", "cy", "ch", "cp", "crx", "cry", "crz"})


def check_target_comparability(target_spec: str, num_logical: int) -> dict[str, Any]:
    """Classify whether `target_spec` admits a fair QCC-vs-Qiskit baseline."""
    norm = target_spec.lower()
    if norm in ("linear", "line", "ring"):
        return {
            "comparable": True,
            "target_spec": "linear" if norm in ("linear", "line") else "ring",
            "physical_qubits": max(num_logical, 2),
            "basis_gates": list(STANDARD_BASIS_GATES),
            "status": "COMPARABLE",
            "reason": None,
        }
    if norm in ("linear-generic", "ring-generic"):
        return {
            "comparable": False,
            "target_spec": norm,
            "physical_qubits": max(num_logical, 2),
            "basis_gates": None,
            "status": "UNAVAILABLE",
            "reason": (
                f"Target '{norm}' includes heterogeneous native gates (h..tdg, sxdg, cx, cz, "
                "swap with 150/300 duration weights) lacking a 1:1 calibrated Qiskit "
                "basis/importer equivalent (`sxdg` unsupported in qiskit-qasm3-import)."
            ),
        }
    return {
        "comparable": False,
        "target_spec": norm,
        "physical_qubits": None,
        "basis_gates": None,
        "status": "UNAVAILABLE",
        "reason": f"Target '{norm}' is not a routed hardware topology.",
    }


def _build_qiskit_coupling_map(target_spec: str, num_physical: int) -> CouplingMap:
    """Match src/cli/compile.rs `create_standard_target` undirected topology exactly."""
    norm = target_spec.lower()
    if norm in ("linear", "line") or (norm == "ring" and num_physical <= 2):
        return CouplingMap.from_line(num_physical, bidirectional=True)
    if norm == "ring":
        return CouplingMap.from_ring(num_physical, bidirectional=True)
    raise ValueError(f"Unsupported comparable target_spec: {target_spec}")


def _extract_qiskit_sabre_metadata(pm: Any) -> dict[str, int]:
    """Inspect preset PassManager dynamically to report Qiskit's internal SABRE trials."""
    info: dict[str, int] = {
        "sabre_layout_trials": 1,
        "sabre_swap_trials": 1,
        "sabre_layout_max_iterations": 1,
        "sabre_routing_trials": 1,
    }

    def visit(obj: Any) -> None:
        if isinstance(obj, SabreLayout):
            if obj.layout_trials is not None:
                info["sabre_layout_trials"] = int(obj.layout_trials)
            if obj.swap_trials is not None:
                info["sabre_swap_trials"] = int(obj.swap_trials)
            if obj.max_iterations is not None:
                info["sabre_layout_max_iterations"] = int(obj.max_iterations)
        elif isinstance(obj, SabreSwap):
            if obj.trials is not None:
                info["sabre_routing_trials"] = int(obj.trials)
        elif hasattr(obj, "tasks"):
            for sub in obj.tasks:
                visit(sub)

    for stage_name in getattr(pm, "stages", ()):
        stage_pm = getattr(pm, stage_name, None)
        if stage_pm is not None:
            try:
                visit(stage_pm.to_flow_controller())
            except Exception:
                pass
    return info


def _count_circuit_gate_classes(counts: dict[str, int]) -> tuple[int, int, int, int, int]:
    """Return (single_q, multi_q, total_gates, cx_count, swap_count)."""
    single_q = 0
    multi_q = 0
    cx_count = 0
    swap_count = 0
    for raw_name, cnt in counts.items():
        name = str(raw_name).lower()
        if name in ("barrier", "measure", "reset", "delay"):
            continue
        if name in TWO_Q_GATES:
            multi_q += int(cnt)
            if name == "cx":
                cx_count += int(cnt)
            elif name == "swap":
                swap_count += int(cnt)
        else:
            single_q += int(cnt)
    return single_q, multi_q, single_q + multi_q, cx_count, swap_count


def _build_qiskit_physical_result(
    tqc: qiskit.QuantumCircuit,
    num_logical: int,
    num_physical: int,
    *,
    compute_operator: bool,
) -> PhysicalCircuitResult:
    """Wrap transpiled Qiskit circuit into PhysicalCircuitResult for Session 3 comparator."""
    if tqc.layout is not None:
        init_idx = tqc.layout.initial_index_layout(filter_ancillas=True)
        final_idx = tqc.layout.final_index_layout(filter_ancillas=True)
        init_map = {str(j): int(init_idx[j]) for j in range(num_logical)}
        final_map = {str(j): int(final_idx[j]) for j in range(num_logical)}
    else:
        init_map = {str(j): j for j in range(num_logical)}
        final_map = {str(j): j for j in range(num_logical)}

    used_init = set(init_map.values())
    used_final = set(final_map.values())
    unused_init = sorted(p for p in range(num_physical) if p not in used_init)
    unused_final = sorted(p for p in range(num_physical) if p not in used_final)

    synth_manifest: dict[str, Any] = {
        "logical_qubits": num_logical,
        "physical_qubits": num_physical,
        "layout": {
            "mapping_direction": "logical_to_physical",
            "qubit_indexing_convention": "little_endian_q0_lsb",
            "num_logical": num_logical,
            "num_physical": num_physical,
            "initial_logical_to_physical": init_map,
            "final_logical_to_physical": final_map,
            "unused_physical_qubits_initial": unused_init,
            "unused_physical_qubits_final": unused_final,
        },
        "global_phase": {
            "radians": float(tqc.global_phase),
            "expression": repr(float(tqc.global_phase)),
            "is_numeric": True,
            "policy": "auto",
            "emitted_in_openqasm": True,
        },
        "compiler": {
            "version": qiskit.__version__,
            "version_string": f"qiskit {qiskit.__version__}",
        },
        "target": {
            "name": "qiskit_transpiler",
            "cli_target_spec": "qiskit_opt2",
        },
    }
    validated_layout = validate_manifest_layout(synth_manifest)
    op = Operator(tqc) if compute_operator else None
    counts = {str(k): int(v) for k, v in tqc.count_ops().items()}

    return PhysicalCircuitResult(
        ok=True,
        status="OK",
        failure_category=None,
        manifest=synth_manifest,
        manifest_sha256="qiskit_in_memory",
        manifest_path=None,
        physical_qasm_sha256=None,
        physical_qasm_path=None,
        physical_qasm_text=None,
        layout=validated_layout,
        physical_qubits=num_physical,
        global_phase_qiskit=float(tqc.global_phase),
        gate_counts=counts,
        compat_status="qiskit_transpiler_circuit",
        circuit=tqc,
        unitary_operator=op,
        error_detail=None,
    )


def _metric_stats(values: list[float | int]) -> dict[str, float | int]:
    s = sorted(values)
    return {
        "best": s[0],
        "median": float(statistics.median(s)),
        "worst": s[-1],
        "spread": float(s[-1] - s[0]) if isinstance(s[0], float) else int(s[-1] - s[0]),
        "trial_count": len(s),
    }


def _select_default_workloads() -> list[CanonicalCase]:
    """Select balanced representative linear & ring benchmark suite across families and widths."""
    # Canonical single-file cases + curated high-pressure corpus workloads
    selected: list[CanonicalCase] = []
    for fname in (
        "02_cx_both_directions.json",
        "03_bell_pair.json",
        "04_logical_swap.json",
        "05_nontrivial_routing.json",
        "06_nonzero_global_phase_spare_ancilla.json",
        "07_qft4_showcase_linear.json",
        "08_qft4_showcase_ring.json",
        "09_parameterized_bound.json",
    ):
        selected.append(load_canonical_case(DEFAULT_CASES_DIR / fname))

    if DEFAULT_CORPUS_JSONL.exists():
        corpus_cases = load_canonical_cases_jsonl(DEFAULT_CORPUS_JSONL)
        picked_keys: set[tuple[str, str, int]] = set()
        for c in corpus_cases:
            meta = c.generator_metadata or {}
            fam = meta.get("family", "")
            if c.target not in ("linear", "ring"):
                continue
            if fam in ("ghz_entanglement", "qft_like_nonadjacent", "logical_swap_stress", "random_layered_dense"):
                if c.logical_qubits in (4, 5, 6):
                    key = (fam, c.target, c.logical_qubits)
                    if key not in picked_keys:
                        picked_keys.add(key)
                        selected.append(c)
                        if len(selected) >= 18:
                            break
    return selected


def run_benchmark_baseline(
    *,
    workloads: list[CanonicalCase] | None = None,
    trials_per_compiler: int = DEFAULT_NUM_TRIALS,
    seed_base: int = DEFAULT_SEED_BASE,
    trials_out_path: pathlib.Path | None = DEFAULT_TRIALS_PATH,
    aggregate_out_path: pathlib.Path | None = DEFAULT_AGGREGATE_PATH,
    qcc_binary: pathlib.Path | None = None,
    qcc_routing_candidates: int | None = None,
    qcc_routing_objective: str | None = None,
    qcc_elide_swaps: bool = False,
    qcc_extra_args: list[str] | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Run paired QCC-vs-Qiskit benchmark trials with semantic verification."""
    if trials_per_compiler < 1:
        raise ValueError("trials_per_compiler must be >= 1")

    cases = workloads if workloads is not None else _select_default_workloads()
    trial_schema = json.loads(TRIAL_SCHEMA_PATH.read_text(encoding="utf-8"))
    agg_schema = json.loads(AGGREGATE_SCHEMA_PATH.read_text(encoding="utf-8"))
    trial_validator = jsonschema.Draft202012Validator(trial_schema)

    seed_schedule = [seed_base + i for i in range(trials_per_compiler)]
    trials: list[dict[str, Any]] = []
    case_aggregates: list[dict[str, Any]] = []
    unavailable_comparisons: list[dict[str, Any]] = []

    sample_pm = generate_preset_pass_manager(
        optimization_level=2,
        basis_gates=STANDARD_BASIS_GATES,
        coupling_map=CouplingMap.from_line(4, bidirectional=True),
        seed_transpiler=seed_base,
    )
    qiskit_sabre_meta = _extract_qiskit_sabre_metadata(sample_pm)

    qcc_version_observed: str | None = None
    effective_candidates = qcc_routing_candidates if qcc_routing_candidates is not None else 1
    qcc_config_observed: dict[str, Any] = {
        "optimization_level": "O2 (CompilerPipeline::default)",
        "router": "SabreBidirectionalRouter",
        "placement_strategy": "Weighted" if effective_candidates <= 1 else "Diversified",
        "is_deterministic": True,
        "cli_seed_flag_supported": False,
        "internal_search_trials": effective_candidates,
        "routing_candidates": effective_candidates,
        "routing_objective": qcc_routing_objective if qcc_routing_objective is not None else "min-swaps",
        "elide_swaps": bool(qcc_elide_swaps),
    }
    qiskit_config_observed: dict[str, Any] = {
        "qiskit_version": qiskit.__version__,
        "optimization_level": 2,
        "basis_gates": STANDARD_BASIS_GATES,
        "layout_method": "default (VF2Layout -> SabreLayout fallback)",
        "routing_method": "default (SabreSwap decay heuristic)",
        **qiskit_sabre_meta,
    }

    cx_wins_qcc = 0
    cx_ties = 0
    cx_wins_qiskit = 0
    depth_wins_qcc = 0
    depth_ties = 0
    depth_wins_qiskit = 0
    qcc_verified_pass_trials = 0
    qiskit_verified_pass_trials = 0
    comparable_workloads_count = 0

    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = pathlib.Path(tmp)

        for case in cases:
            comp_check = check_target_comparability(case.target, case.logical_qubits)
            if not comp_check["comparable"]:
                unavailable_comparisons.append(
                    {
                        "target_spec": f"{case.case_id} ({case.target})",
                        "status": "UNAVAILABLE",
                        "reason": comp_check["reason"],
                    }
                )
                continue

            lref = run_logical_reference(case)
            if not lref.ok or lref.circuit is None:
                unavailable_comparisons.append(
                    {
                        "target_spec": f"{case.case_id} ({case.target})",
                        "status": "UNAVAILABLE",
                        "reason": f"Logical reference unsupported for unitary benchmark: {lref.error_detail}",
                    }
                )
                continue

            comparable_workloads_count += 1
            family = (
                (case.generator_metadata or {}).get("family")
                or case.case_id
            )
            num_logical = case.logical_qubits
            num_physical = comp_check["physical_qubits"]
            compute_op = case.verification_mode == "exhaustive_unitary"
            logical_swaps_count = int(lref.gate_counts.get("swap", 0))
            cm = _build_qiskit_coupling_map(case.target, num_physical)

            qcc_case_trials: list[dict[str, Any]] = []
            qiskit_case_trials: list[dict[str, Any]] = []

            extra_cmd_args = list(qcc_extra_args or [])
            if qcc_elide_swaps and "--elide-swaps" not in extra_cmd_args:
                extra_cmd_args.append("--elide-swaps")

            # --- QCC Trials (repeated deterministic executions for wall-clock repeatability) ---
            for t_idx in range(trials_per_compiler):
                t0 = time.perf_counter()
                cexec = run_qcc_compile(
                    case,
                    tmp_root / "qcc" / case.case_id / f"t{t_idx}",
                    qcc_binary=qcc_binary,
                    routing_candidates=qcc_routing_candidates,
                    routing_objective=qcc_routing_objective,
                    extra_args=extra_cmd_args,
                )
                compile_ms = (time.perf_counter() - t0) * 1000.0

                if cexec.ok:
                    phys_res = load_and_simulate_physical(
                        cexec.manifest_path, compute_operator=compute_op
                    )
                    manifest = phys_res.manifest or {}
                    metrics = manifest.get("metrics") or {}
                    compiler_meta = manifest.get("compiler") or {}
                    if qcc_version_observed is None:
                        qcc_version_observed = compiler_meta.get("version_string") or "qcc"

                    comp_res = compare_logical_vs_physical(case, lref, phys_res)
                    sem_status = comp_res["status"]
                    if sem_status == "PASS":
                        qcc_verified_pass_trials += 1

                    record = {
                        "schema_id": "qcc.benchmark_trial",
                        "schema_version": "1.0.0",
                        "case_id": case.case_id,
                        "family": family,
                        "target": case.target,
                        "trial_index": t_idx,
                        "compiler": "qcc",
                        "compiler_version": compiler_meta.get("version_string") or "qcc 0.1.0",
                        "seed": None,
                        "is_deterministic": True,
                        "internal_sabre_trials": effective_candidates,
                        "compile_success": True,
                        "compile_wall_time_ms": round(compile_ms, 4),
                        "physical_qubits": int(manifest["physical_qubits"]),
                        "single_qubit_gates": int(metrics["single_qubit_gates"]),
                        "multi_qubit_gates": int(metrics["two_qubit_gates"]),
                        "total_gates": int(metrics["total_gate_count"]),
                        "cx_count": int(metrics["cx_count"]),
                        "logical_swaps": int(metrics["logical_swaps"]),
                        "router_swaps_inserted": int(metrics["router_swaps_inserted"]),
                        "final_emitted_swaps": int(metrics["final_emitted_swaps"]),
                        "circuit_depth": int(metrics["circuit_depth"]),
                        "semantic_status": sem_status,
                        "max_amplitude_error": comp_res["max_amplitude_error"],
                        "ancilla_zero_ok": comp_res["ancilla_zero_ok"],
                        "compiler_options": dict(manifest.get("config") or qcc_config_observed),
                    }
                else:
                    record = {
                        "schema_id": "qcc.benchmark_trial",
                        "schema_version": "1.0.0",
                        "case_id": case.case_id,
                        "family": family,
                        "target": case.target,
                        "trial_index": t_idx,
                        "compiler": "qcc",
                        "compiler_version": qcc_version_observed or "qcc",
                        "seed": None,
                        "is_deterministic": True,
                        "internal_sabre_trials": effective_candidates,
                        "compile_success": False,
                        "compile_wall_time_ms": round(compile_ms, 4),
                        "physical_qubits": None,
                        "single_qubit_gates": None,
                        "multi_qubit_gates": None,
                        "total_gates": None,
                        "cx_count": None,
                        "logical_swaps": logical_swaps_count,
                        "router_swaps_inserted": None,
                        "final_emitted_swaps": None,
                        "circuit_depth": None,
                        "semantic_status": "ERROR",
                        "max_amplitude_error": None,
                        "ancilla_zero_ok": None,
                        "compiler_options": dict(qcc_config_observed),
                    }
                trial_validator.validate(record)
                trials.append(record)
                qcc_case_trials.append(record)

            # --- Qiskit Trials (same external trial count N, pinned explicit seed_transpiler) ---
            for t_idx, t_seed in enumerate(seed_schedule):
                pm = generate_preset_pass_manager(
                    optimization_level=2,
                    basis_gates=STANDARD_BASIS_GATES,
                    coupling_map=cm,
                    seed_transpiler=t_seed,
                )
                # Ensure `num_physical` wires in circuit when logical_qubits < num_physical (e.g. 1 -> 2)
                qc_in = lref.circuit
                t0 = time.perf_counter()
                tqc = pm.run(qc_in)
                compile_ms = (time.perf_counter() - t0) * 1000.0

                counts = {str(k): int(v) for k, v in tqc.count_ops().items()}
                single_q, multi_q, total_g, cx_c, swap_c = _count_circuit_gate_classes(counts)
                phys_res = _build_qiskit_physical_result(
                    tqc, num_logical, int(tqc.num_qubits), compute_operator=compute_op
                )
                comp_res = compare_logical_vs_physical(case, lref, phys_res)
                sem_status = comp_res["status"]
                if sem_status == "PASS":
                    qiskit_verified_pass_trials += 1

                record = {
                    "schema_id": "qcc.benchmark_trial",
                    "schema_version": "1.0.0",
                    "case_id": case.case_id,
                    "family": family,
                    "target": case.target,
                    "trial_index": t_idx,
                    "compiler": "qiskit",
                    "compiler_version": f"qiskit {qiskit.__version__}",
                    "seed": int(t_seed),
                    "is_deterministic": False,
                    "internal_sabre_trials": int(qiskit_sabre_meta["sabre_layout_trials"]),
                    "compile_success": True,
                    "compile_wall_time_ms": round(compile_ms, 4),
                    "physical_qubits": int(tqc.num_qubits),
                    "single_qubit_gates": single_q,
                    "multi_qubit_gates": multi_q,
                    "total_gates": total_g,
                    "cx_count": cx_c,
                    "logical_swaps": logical_swaps_count,
                    "router_swaps_inserted": None,  # Decomposed to 3 CXs by BasisTranslator
                    "final_emitted_swaps": swap_c,
                    "circuit_depth": int(tqc.depth()),
                    "semantic_status": sem_status,
                    "max_amplitude_error": comp_res["max_amplitude_error"],
                    "ancilla_zero_ok": comp_res["ancilla_zero_ok"],
                    "compiler_options": dict(qiskit_config_observed),
                }
                trial_validator.validate(record)
                trials.append(record)
                qiskit_case_trials.append(record)

            # Aggregate strictly across semantically PASS trials
            qcc_valid = [r for r in qcc_case_trials if r["semantic_status"] == "PASS"]
            qiskit_valid = [r for r in qiskit_case_trials if r["semantic_status"] == "PASS"]

            def agg_side(valid_rows: list[dict[str, Any]]) -> dict[str, Any] | None:
                if not valid_rows:
                    return None
                out = {
                    "verified_trials": len(valid_rows),
                    "compile_wall_time_ms": _metric_stats(
                        [r["compile_wall_time_ms"] for r in valid_rows]
                    ),
                    "single_qubit_gates": _metric_stats(
                        [r["single_qubit_gates"] for r in valid_rows]
                    ),
                    "multi_qubit_gates": _metric_stats(
                        [r["multi_qubit_gates"] for r in valid_rows]
                    ),
                    "total_gates": _metric_stats([r["total_gates"] for r in valid_rows]),
                    "cx_count": _metric_stats([r["cx_count"] for r in valid_rows]),
                    "final_emitted_swaps": _metric_stats(
                        [r["final_emitted_swaps"] for r in valid_rows]
                    ),
                    "circuit_depth": _metric_stats([r["circuit_depth"] for r in valid_rows]),
                }
                if all(r["router_swaps_inserted"] is not None for r in valid_rows):
                    out["router_swaps_inserted"] = _metric_stats(
                        [r["router_swaps_inserted"] for r in valid_rows]
                    )
                else:
                    out["router_swaps_inserted"] = None
                return out

            qcc_agg = agg_side(qcc_valid)
            qiskit_agg = agg_side(qiskit_valid)
            if qcc_agg is not None and qiskit_agg is not None:
                qcc_cx = qcc_agg["cx_count"]["median"]
                qk_cx = qiskit_agg["cx_count"]["median"]
                if qcc_cx < qk_cx:
                    cx_wins_qcc += 1
                elif qcc_cx > qk_cx:
                    cx_wins_qiskit += 1
                else:
                    cx_ties += 1

                qcc_d = qcc_agg["circuit_depth"]["median"]
                qk_d = qiskit_agg["circuit_depth"]["median"]
                if qcc_d < qk_d:
                    depth_wins_qcc += 1
                elif qcc_d > qk_d:
                    depth_wins_qiskit += 1
                else:
                    depth_ties += 1

            case_aggregates.append(
                {
                    "case_id": case.case_id,
                    "family": family,
                    "target": case.target,
                    "logical_qubits": num_logical,
                    "physical_qubits": num_physical,
                    "verification_mode": case.verification_mode,
                    "logical_swaps": logical_swaps_count,
                    "qcc": qcc_agg,
                    "qiskit": qiskit_agg,
                }
            )

    if qcc_version_observed:
        qcc_config_observed["version"] = qcc_version_observed

    config_for_hash = {
        "schema_version": "1.0.0",
        "declared_trials_per_compiler": trials_per_compiler,
        "seed_schedule": seed_schedule,
        "case_ids": [c.case_id for c in cases],
        "qcc_config": qcc_config_observed,
        "qiskit_config": qiskit_config_observed,
    }
    config_sha = sha256_bytes(canonical_json_bytes(config_for_hash))

    aggregate: dict[str, Any] = {
        "schema_id": "qcc.benchmark_aggregate",
        "schema_version": "1.0.0",
        "benchmark_config_sha256": config_sha,
        "declared_trials_per_compiler": trials_per_compiler,
        "seed_schedule": seed_schedule,
        "qcc_config": qcc_config_observed,
        "qiskit_config": qiskit_config_observed,
        "fairness_disclosures": {
            "qcc_is_deterministic_single_config": True if effective_candidates <= 1 else False,
            "qiskit_internal_sabre_layout_trials": qiskit_sabre_meta["sabre_layout_trials"],
            "qiskit_internal_sabre_swap_trials": qiskit_sabre_meta["sabre_swap_trials"],
            "qiskit_internal_sabre_max_iterations": qiskit_sabre_meta[
                "sabre_layout_max_iterations"
            ],
            "qiskit_internal_routing_trials": qiskit_sabre_meta["sabre_routing_trials"],
            "equal_internal_compute_budget": bool(effective_candidates == qiskit_sabre_meta["sabre_layout_trials"]),
            "timing_boundary_note": (
                "QCC wall-clock time measures black-box CLI subprocess execution (including process "
                "startup and file I/O) of one deterministic configuration; Qiskit time measures "
                "in-process Python PassManager.run() excluding circuit construction and oracle checks."
            ),
            "swap_metric_note": (
                "Both compilers emit into basis {rz, sx, cx} so final_emitted_swaps == 0. QCC exposes "
                "router_swaps_inserted directly via CompilationManifest, whereas Qiskit transpile() "
                "decomposes routed SWAPs into 3 CX gates before returning the final circuit (`null`)."
            ),
        },
        "unavailable_comparisons": unavailable_comparisons,
        "case_aggregates": case_aggregates,
        "overall_summary": {
            "total_workloads": comparable_workloads_count,
            "total_qcc_trials": len([t for t in trials if t["compiler"] == "qcc"]),
            "total_qiskit_trials": len([t for t in trials if t["compiler"] == "qiskit"]),
            "qcc_verified_pass_trials": qcc_verified_pass_trials,
            "qiskit_verified_pass_trials": qiskit_verified_pass_trials,
            "cx_wins_qcc": cx_wins_qcc,
            "cx_ties": cx_ties,
            "cx_wins_qiskit": cx_wins_qiskit,
            "depth_wins_qcc": depth_wins_qcc,
            "depth_ties": depth_ties,
            "depth_wins_qiskit": depth_wins_qiskit,
        },
    }

    jsonschema.validate(instance=aggregate, schema=agg_schema)

    if trials_out_path is not None:
        trials_out_path.parent.mkdir(parents=True, exist_ok=True)
        trials_out_path.write_text(
            "\n".join(json.dumps(t, sort_keys=True) for t in trials) + "\n",
            encoding="utf-8",
        )
    if aggregate_out_path is not None:
        aggregate_out_path.parent.mkdir(parents=True, exist_ok=True)
        aggregate_out_path.write_text(
            json.dumps(aggregate, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    return trials, aggregate


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run fair QCC-vs-Qiskit benchmark baseline.")
    parser.add_argument("--trials", type=int, default=DEFAULT_NUM_TRIALS)
    parser.add_argument("--seed-base", type=int, default=DEFAULT_SEED_BASE)
    parser.add_argument("--trials-out", type=pathlib.Path, default=DEFAULT_TRIALS_PATH)
    parser.add_argument("--aggregate-out", type=pathlib.Path, default=DEFAULT_AGGREGATE_PATH)
    parser.add_argument("--qcc-routing-candidates", type=int, default=None)
    parser.add_argument("--qcc-routing-objective", type=str, default=None)
    parser.add_argument("--qcc-elide-swaps", action="store_true", default=False)
    args = parser.parse_args(argv)

    check_pinned_environment()
    _, agg = run_benchmark_baseline(
        trials_per_compiler=args.trials,
        seed_base=args.seed_base,
        trials_out_path=args.trials_out,
        aggregate_out_path=args.aggregate_out,
        qcc_routing_candidates=args.qcc_routing_candidates,
        qcc_routing_objective=args.qcc_routing_objective,
        qcc_elide_swaps=args.qcc_elide_swaps,
    )
    os_sum = agg["overall_summary"]
    fd = agg["fairness_disclosures"]
    print(
        f"Benchmark baseline complete: {os_sum['total_workloads']} comparable workloads x {args.trials} trials "
        f"(qcc PASS={os_sum['qcc_verified_pass_trials']}/{os_sum['total_qcc_trials']}, "
        f"qiskit PASS={os_sum['qiskit_verified_pass_trials']}/{os_sum['total_qiskit_trials']})"
    )
    print(
        f"  Median CX W/T/L (QCC vs Qiskit): {os_sum['cx_wins_qcc']}/{os_sum['cx_ties']}/{os_sum['cx_wins_qiskit']} | "
        f"Median Depth W/T/L: {os_sum['depth_wins_qcc']}/{os_sum['depth_ties']}/{os_sum['depth_wins_qiskit']}"
    )
    print(
        f"  Disclosures: qcc_deterministic={fd['qcc_is_deterministic_single_config']}, "
        f"qiskit_sabre_layout_trials={fd['qiskit_internal_sabre_layout_trials']}, "
        f"equal_internal_compute_budget={fd['equal_internal_compute_budget']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
