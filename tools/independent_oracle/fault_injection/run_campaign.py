"""Systematic fault-injection campaign validating oracle sensitivity (M7 Phase 67).

Injects controlled defects across all 17 required mutation families against clean
copies of compiled canonical outputs or canonical logical inputs — NEVER modifying
original canonical cases or committed Session 2/3 artifacts.

Verifies:
- Semantic defects -> MISMATCH
- Contract / schema / hash / width / syntax defects -> INVALID_INPUT
- Non-unitary / unsupported constructs -> UNSUPPORTED
- Manifest global_phase metadata corruption -> semantic PASS + informational
  strict_phase_accounting = MISMATCH
- Zero undetected semantic defects (fails closed if any occurs).
"""

from __future__ import annotations

import argparse
from collections import Counter
import copy
import json
import pathlib
import re
import sys
import tempfile
from typing import Any, Callable

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    DEFAULT_CASES_DIR,
    canonical_json_bytes,
    load_canonical_case,
    load_canonical_case_from_dict,
    sha256_bytes,
)
from equivalence.comparator import compare_logical_vs_physical  # noqa: E402
from qasm3_compat import check_pinned_environment  # noqa: E402
from runners.qcc_runner import load_and_simulate_physical, run_qcc_compile  # noqa: E402
from runners.qiskit_reference import run_logical_reference  # noqa: E402


CAMPAIGN_VERSION = "1.0.0"
DEFAULT_CAMPAIGN_SEED = 20261113
FAULT_SUMMARY_SCHEMA_PATH = ORACLE_DIR / "schema" / "fault_campaign_summary.schema.json"
DEFAULT_REPORT_PATH = ORACLE_DIR / "reports" / "fault_campaign_summary.json"


def _update_phys_and_manifest(
    work_dir: pathlib.Path,
    new_qasm: str,
    orig_manifest: dict[str, Any],
    *,
    update_sha256: bool = True,
    manifest_mutator: Callable[[dict[str, Any]], None] | None = None,
) -> tuple[pathlib.Path, pathlib.Path]:
    qasm_path = work_dir / "mutated.physical.qasm"
    manifest_path = work_dir / "mutated.manifest.json"
    raw_bytes = new_qasm.encode("utf-8")
    qasm_path.write_bytes(raw_bytes)
    manifest = copy.deepcopy(orig_manifest)
    manifest["emitted_circuit"]["path"] = qasm_path.name
    if update_sha256:
        manifest["emitted_circuit"]["sha256"] = sha256_bytes(raw_bytes)
        manifest["emitted_circuit"]["byte_length"] = len(raw_bytes)
    if manifest_mutator is not None:
        manifest_mutator(manifest)
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8")
    return manifest_path, qasm_path


def _mutate_first_rz_angle(qasm_text: str, delta: float) -> str:
    pattern = re.compile(r"rz\(([-0-9.eE+]+)\)")
    match = pattern.search(qasm_text)
    if match is None:
        raise ValueError("No numeric rz(...) gate found to mutate")
    old_val = float(match.group(1))
    new_val = old_val + delta
    return qasm_text[: match.start(1)] + repr(new_val) + qasm_text[match.end(1) :]


def _drop_first_gate_line(qasm_text: str) -> str:
    lines = qasm_text.splitlines()
    for idx, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith(("rz(", "sx ", "cx ", "rx(", "ry(", "h ", "x ", "swap ")):
            # Drop only non-identity gate
            del lines[idx]
            return "\n".join(lines) + "\n"
    raise ValueError("No gate line found to drop")


def _swap_first_cx(qasm_text: str) -> str:
    pattern = re.compile(r"cx\s+(p\[\d+\]),\s*(p\[\d+\]);")
    match = pattern.search(qasm_text)
    if match is None:
        raise ValueError("No physical cx gate found to reverse")
    return (
        qasm_text[: match.start()]
        + f"cx {match.group(2)}, {match.group(1)};"
        + qasm_text[match.end() :]
    )


def _replace_first_sx_with_x(qasm_text: str) -> str:
    pattern = re.compile(r"(?m)^sx\s+(p\[\d+\]);$")
    match = pattern.search(qasm_text)
    if match is None:
        # Fallback: replace first rz(...) line with x p[0];
        return _drop_first_gate_line(qasm_text) + "x p[0];\n"
    return qasm_text[: match.start()] + f"x {match.group(1)};" + qasm_text[match.end() :]


def _corrupt_swap_decomposition(qasm_text: str) -> str:
    # Remove one cx out of a 3-CX swap sequence or turn swap into single cx
    lines = qasm_text.splitlines()
    for idx in range(len(lines) - 2):
        if (
            lines[idx].strip().startswith("cx ")
            and lines[idx + 1].strip().startswith("cx ")
            and lines[idx + 2].strip().startswith("cx ")
        ):
            del lines[idx + 2]
            return "\n".join(lines) + "\n"
    # Fallback for single cx removal if physical SWAP was already absorbed cleanly
    return _swap_first_cx(qasm_text)


def _swap_layout_keys(layout_map: dict[str, int]) -> None:
    keys = sorted(layout_map.keys(), key=int)
    if len(keys) < 2:
        raise ValueError("Need at least 2 logical qubits to swap layout entries")
    k0, k1 = keys[0], keys[1]
    layout_map[k0], layout_map[k1] = layout_map[k1], layout_map[k0]


def _bump_qubit_decl(qasm_text: str) -> str:
    m = re.search(r"qubit\[(\d+)\]\s+p;", qasm_text)
    if not m:
        raise ValueError("No `qubit[N] p;` declaration found")
    n = int(m.group(1))
    return qasm_text[: m.start(1)] + str(n + 1) + qasm_text[m.end(1) :]


def run_fault_campaign(
    *,
    seed: int = DEFAULT_CAMPAIGN_SEED,
    output_summary_path: pathlib.Path | None = DEFAULT_REPORT_PATH,
    failure_artifacts_root: pathlib.Path | None = None,
    qcc_binary: pathlib.Path | None = None,
) -> dict[str, Any]:
    """Execute systematic fault injection across all 17 mutation families."""
    # Precompile reusable clean baseline copies in an isolated temp dir (read-only originals!)
    base_case_files = [
        "01_single_qubit_rotations.json",
        "02_cx_both_directions.json",
        "03_bell_pair.json",
        "04_logical_swap.json",
        "05_nontrivial_routing.json",
        "06_nonzero_global_phase_spare_ancilla.json",
        "07_qft4_showcase_linear.json",
        "09_parameterized_bound.json",
    ]
    base_cases = {
        fname.replace(".json", ""): load_canonical_case(DEFAULT_CASES_DIR / fname)
        for fname in base_case_files
    }

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = pathlib.Path(tmp)
        compiled_cache: dict[str, tuple[Any, str, dict[str, Any]]] = {}
        for cid, case in base_cases.items():
            lref = run_logical_reference(case)
            if not lref.ok:
                raise RuntimeError(f"Base case {cid} logical ref failed: {lref.error_detail}")
            cexec = run_qcc_compile(case, tmp_path / "base" / cid, qcc_binary=qcc_binary)
            if not cexec.ok:
                raise RuntimeError(f"Base case {cid} QCC compile failed: {cexec.stderr}")
            phys_txt = cexec.physical_qasm_path.read_text(encoding="utf-8")
            man_obj = json.loads(cexec.manifest_path.read_text(encoding="utf-8"))
            compiled_cache[cid] = (lref, phys_txt, man_obj)

        # Define deterministic suite covering all 17 required families cleanly (37 targeted trials)
        mutations: list[dict[str, Any]] = []

        def add_phys_mut(
            mut_id: str,
            family: str,
            base_cid: str,
            expected_status: str,
            desc: str,
            qasm_fn: Callable[[str], str],
            *,
            update_sha256: bool = True,
            manifest_fn: Callable[[dict[str, Any]], None] | None = None,
            expected_strict_phase: str | None = None,
        ) -> None:
            mutations.append(
                {
                    "kind": "physical",
                    "mutation_id": mut_id,
                    "family": family,
                    "base_cid": base_cid,
                    "expected_status": expected_status,
                    "expected_strict_phase": expected_strict_phase,
                    "description": desc,
                    "qasm_fn": qasm_fn,
                    "update_sha256": update_sha256,
                    "manifest_fn": manifest_fn,
                }
            )

        def add_log_mut(
            mut_id: str,
            family: str,
            base_cid: str,
            expected_status: str,
            desc: str,
            new_logical_qasm: str,
        ) -> None:
            mutations.append(
                {
                    "kind": "logical",
                    "mutation_id": mut_id,
                    "family": family,
                    "base_cid": base_cid,
                    "expected_status": expected_status,
                    "expected_strict_phase": None,
                    "description": desc,
                    "new_logical_qasm": new_logical_qasm,
                }
            )

        # 1. single_qubit_gate_mutation (3 cases)
        for cid in ("01_single_qubit_rotations", "03_bell_pair", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_1q_{cid}",
                "single_qubit_gate_mutation",
                cid,
                "MISMATCH",
                "Replace physical `sx` gate with `x` gate.",
                _replace_first_sx_with_x,
            )

        # 2. incorrect_rotation_angle (3 cases)
        for cid, delta in (
            ("01_single_qubit_rotations", 0.37),
            ("02_cx_both_directions", -0.52),
            ("09_parameterized_bound", 0.41),
        ):
            add_phys_mut(
                f"mut_rot_{cid}",
                "incorrect_rotation_angle",
                cid,
                "MISMATCH",
                f"Perturb first `rz` angle by {delta:+.2f} rad.",
                lambda q, d=delta: _mutate_first_rz_angle(q, d),
            )

        # 3. missing_gate (3 cases)
        for cid in ("03_bell_pair", "05_nontrivial_routing", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_miss_{cid}",
                "missing_gate",
                cid,
                "MISMATCH",
                "Delete first physical gate from emitted QASM.",
                _drop_first_gate_line,
            )

        # 4. extra_gate (3 cases)
        for cid in ("01_single_qubit_rotations", "03_bell_pair", "09_parameterized_bound"):
            add_phys_mut(
                f"mut_extra_{cid}",
                "extra_gate",
                cid,
                "MISMATCH",
                "Append spurious trailing `sx p[0];`.",
                lambda q: q + "sx p[0];\n",
            )

        # 5. reversed_cx_control_target (3 cases)
        for cid in ("02_cx_both_directions", "03_bell_pair", "05_nontrivial_routing"):
            add_phys_mut(
                f"mut_revcx_{cid}",
                "reversed_cx_control_target",
                cid,
                "MISMATCH",
                "Reverse control and target on first physical `cx`.",
                _swap_first_cx,
            )

        # 6. incorrect_swap_behavior (2 cases)
        for cid in ("04_logical_swap", "05_nontrivial_routing"):
            add_phys_mut(
                f"mut_swap_{cid}",
                "incorrect_swap_behavior",
                cid,
                "MISMATCH",
                "Corrupt physical routed SWAP gate sequence.",
                _corrupt_swap_decomposition,
            )

        # 7. incorrect_initial_layout (2 cases)
        for cid in ("03_bell_pair", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_initlayout_{cid}",
                "incorrect_initial_layout",
                cid,
                "MISMATCH",
                "Swap logical qubits 0 and 1 in `initial_logical_to_physical`.",
                lambda q: q,
                update_sha256=False,
                manifest_fn=lambda m: _swap_layout_keys(m["layout"]["initial_logical_to_physical"]),
            )

        # 8. incorrect_final_layout (2 cases)
        for cid in ("04_logical_swap", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_finallayout_{cid}",
                "incorrect_final_layout",
                cid,
                "MISMATCH",
                "Swap logical qubits 0 and 1 in `final_logical_to_physical`.",
                lambda q: q,
                update_sha256=False,
                manifest_fn=lambda m: _swap_layout_keys(m["layout"]["final_logical_to_physical"]),
            )

        # 9. incorrect_physical_qubit_count (2 cases)
        for cid in ("01_single_qubit_rotations", "03_bell_pair"):
            add_phys_mut(
                f"mut_width_{cid}",
                "incorrect_physical_qubit_count",
                cid,
                "INVALID_INPUT",
                "Inflate `qubit[N] p;` declaration count by 1 relative to manifest.",
                _bump_qubit_decl,
            )

        # 10. corrupted_emitted_qasm (2 cases)
        for cid in ("03_bell_pair", "05_nontrivial_routing"):
            add_phys_mut(
                f"mut_syntax_{cid}",
                "corrupted_emitted_qasm",
                cid,
                "INVALID_INPUT",
                "Inject malformed OpenQASM 3 statement (missing closing paren).",
                lambda q: q + "rz(1.23 p[0];\n",
            )

        # 11. corrupted_manifest_hash (2 cases)
        for cid in ("03_bell_pair", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_hash_{cid}",
                "corrupted_manifest_hash",
                cid,
                "INVALID_INPUT",
                "Modify physical QASM without updating manifest SHA-256.",
                lambda q: q + "// tampered bytes\n",
                update_sha256=False,
            )

        # 12. corrupted_global_phase_metadata (2 cases — must remain semantic PASS while strict_phase=MISMATCH!)
        for cid in ("06_nonzero_global_phase_spare_ancilla", "07_qft4_showcase_linear"):
            add_phys_mut(
                f"mut_gpmeta_{cid}",
                "corrupted_global_phase_metadata",
                cid,
                "PASS",
                "Perturb `manifest.global_phase.radians` by +0.65 while leaving physical circuit unitary intact.",
                lambda q: q,
                update_sha256=False,
                manifest_fn=lambda m: m["global_phase"].__setitem__(
                    "radians", float(m["global_phase"]["radians"]) + 0.65
                ),
                expected_strict_phase="MISMATCH",
            )

        # 13. relative_phase_defect (2 cases — diagonal Z phase leaving basis probabilities unchanged)
        for cid in ("01_single_qubit_rotations", "06_nonzero_global_phase_spare_ancilla"):
            add_phys_mut(
                f"mut_relphase_{cid}",
                "relative_phase_defect",
                cid,
                "MISMATCH",
                "Inject `rz(pi) p[0];` relative-phase error not repairable by single common global phase.",
                lambda q: q + "rz(3.141592653589793) p[0];\n",
            )

        # 14. residual_ancilla_population (2 cases — excite spare physical qubit p[1] to |1>)
        for cid in ("01_single_qubit_rotations", "06_nonzero_global_phase_spare_ancilla"):
            add_phys_mut(
                f"mut_ancpop_{cid}",
                "residual_ancilla_population",
                cid,
                "MISMATCH",
                "Flip spare physical ancilla qubit p[1] into |1> via `sx p[1]; sx p[1];`.",
                lambda q: q + "sx p[1];\nsx p[1];\n",
            )

        # 15. residual_ancilla_entanglement (2 cases — entangle logical qubit with spare p[1])
        for cid in ("01_single_qubit_rotations", "06_nonzero_global_phase_spare_ancilla"):
            add_phys_mut(
                f"mut_ancent_{cid}",
                "residual_ancilla_entanglement",
                cid,
                "MISMATCH",
                "Entangle logical p[0] with spare ancilla p[1] via un-uncomputed `cx p[0], p[1];`.",
                lambda q: q + "cx p[0], p[1];\n",
            )

        # 16. unsupported_non_unitary_operation (2 cases — physical reset & logical measure)
        add_phys_mut(
            "mut_nonunit_phys_reset",
            "unsupported_non_unitary_operation",
            "03_bell_pair",
            "UNSUPPORTED",
            "Inject `reset p[0];` into emitted physical QASM.",
            lambda q: q + "reset p[0];\n",
        )
        add_log_mut(
            "mut_nonunit_log_meas",
            "unsupported_non_unitary_operation",
            "03_bell_pair",
            "UNSUPPORTED",
            "Add `bit[2] c; c[0] = measure q[0];` to canonical logical input.",
            'OPENQASM 3.0;\ninclude "stdgates.inc";\n\nqubit[2] q;\nbit[2] c;\n\nh q[0];\ncx q[0], q[1];\nc[0] = measure q[0];\n',
        )

        # 17. invalid_or_incomplete_layout_metadata (3 cases)
        add_phys_mut(
            "mut_badlayout_noninjective",
            "invalid_or_incomplete_layout_metadata",
            "03_bell_pair",
            "INVALID_INPUT",
            "Corrupt `initial_logical_to_physical` to non-injective map {0:0, 1:0}.",
            lambda q: q,
            update_sha256=False,
            manifest_fn=lambda m: m["layout"].__setitem__(
                "initial_logical_to_physical", {"0": 0, "1": 0}
            ),
        )
        add_phys_mut(
            "mut_badlayout_missing_key",
            "invalid_or_incomplete_layout_metadata",
            "03_bell_pair",
            "INVALID_INPUT",
            "Drop logical qubit '1' from `final_logical_to_physical`.",
            lambda q: q,
            update_sha256=False,
            manifest_fn=lambda m: m["layout"]["final_logical_to_physical"].pop("1", None),
        )
        add_phys_mut(
            "mut_badlayout_null",
            "invalid_or_incomplete_layout_metadata",
            "03_bell_pair",
            "INVALID_INPUT",
            "Set manifest `layout` to null on target_compiled manifest.",
            lambda q: q,
            update_sha256=False,
            manifest_fn=lambda m: m.__setitem__("layout", None),
        )

        by_family: dict[str, dict[str, Any]] = {}
        classification_breakdown: Counter[str] = Counter()
        representative_diagnostics: list[dict[str, Any]] = []

        detected_correctly = 0
        misclassified = 0
        undetected_semantic_defects = 0

        for m in mutations:
            mut_id = m["mutation_id"]
            family = m["family"]
            base_cid = m["base_cid"]
            base_case = base_cases[base_cid]
            mut_work = tmp_path / "runs" / mut_id
            mut_work.mkdir(parents=True, exist_ok=True)
            art_root = (
                failure_artifacts_root if failure_artifacts_root is not None else tmp_path / "artifacts"
            )

            if m["kind"] == "physical":
                lref, orig_qasm, orig_man = compiled_cache[base_cid]
                mut_qasm = m["qasm_fn"](orig_qasm)
                mut_man_path, _ = _update_phys_and_manifest(
                    mut_work,
                    mut_qasm,
                    orig_man,
                    update_sha256=m["update_sha256"],
                    manifest_mutator=m["manifest_fn"],
                )
                mut_case_dict = copy.deepcopy(base_case.raw_json)
                mut_case_dict["case_id"] = mut_id
                mut_case = load_canonical_case_from_dict(mut_case_dict)
                phys_res = load_and_simulate_physical(mut_man_path)
                res = compare_logical_vs_physical(
                    mut_case,
                    lref,
                    phys_res,
                    failure_artifacts_root=art_root,
                )
            else:
                mut_case_dict = copy.deepcopy(base_case.raw_json)
                mut_case_dict["case_id"] = mut_id
                mut_case_dict["logical_qasm"] = m["new_logical_qasm"]
                mut_case_dict.pop("logical_qasm_path", None)
                mut_case = load_canonical_case_from_dict(mut_case_dict)
                lref = run_logical_reference(mut_case)
                from runners.qcc_runner import PhysicalCircuitResult

                res = compare_logical_vs_physical(
                    mut_case,
                    lref,
                    PhysicalCircuitResult(
                        ok=False,
                        status=lref.status,
                        failure_category=lref.failure_category,
                    ),
                    failure_artifacts_root=art_root,
                )

            # Persist explicit mutation description inside failure artifact dir when present
            if res.get("failure_artifact_dir"):
                art_dir = pathlib.Path(res["failure_artifact_dir"])
                (art_dir / "mutation_description.json").write_text(
                    json.dumps(
                        {
                            "mutation_id": mut_id,
                            "family": family,
                            "base_case_id": base_cid,
                            "expected_status": m["expected_status"],
                            "expected_strict_phase_status": m["expected_strict_phase"],
                            "description": m["description"],
                        },
                        indent=2,
                        sort_keys=True,
                    )
                    + "\n",
                    encoding="utf-8",
                )

            actual_status = res["status"]
            actual_strict = res["strict_phase_accounting"]["status"]
            classification_breakdown[actual_status] += 1

            ok_status = actual_status == m["expected_status"]
            ok_strict = (
                True
                if m["expected_strict_phase"] is None
                else actual_strict == m["expected_strict_phase"]
            )
            is_ok = ok_status and ok_strict
            if is_ok:
                detected_correctly += 1
            else:
                misclassified += 1
                if m["expected_status"] == "MISMATCH" and actual_status == "PASS":
                    undetected_semantic_defects += 1

            fam_entry = by_family.setdefault(
                family,
                {
                    "attempted": 0,
                    "detected_correctly": 0,
                    "misclassified": 0,
                    "expected_statuses": [],
                    "observed_statuses": [],
                },
            )
            fam_entry["attempted"] += 1
            if is_ok:
                fam_entry["detected_correctly"] += 1
            else:
                fam_entry["misclassified"] += 1
            if m["expected_status"] not in fam_entry["expected_statuses"]:
                fam_entry["expected_statuses"].append(m["expected_status"])
            if actual_status not in fam_entry["observed_statuses"]:
                fam_entry["observed_statuses"].append(actual_status)

            # Keep one representative diagnostic per family (plus any misclassified)
            if (fam_entry["attempted"] == 1) or (not is_ok):
                representative_diagnostics.append(
                    {
                        "mutation_id": mut_id,
                        "family": family,
                        "base_case_id": base_cid,
                        "expected_status": m["expected_status"],
                        "actual_status": actual_status,
                        "expected_strict_phase_status": m["expected_strict_phase"],
                        "actual_strict_phase_status": actual_strict,
                        "failure_category": res["failure_category"],
                        "max_amplitude_error": res["max_amplitude_error"],
                        "max_residual_ancilla_population": res["max_residual_ancilla_population"],
                        "detected_correctly": is_ok,
                        "description": m["description"],
                    }
                )

    summary_core: dict[str, Any] = {
        "schema_id": "qcc.fault_campaign_summary",
        "schema_version": "1.0.0",
        "campaign_version": CAMPAIGN_VERSION,
        "seed": seed,
        "attempted": len(mutations),
        "detected_correctly": detected_correctly,
        "misclassified": misclassified,
        "undetected_semantic_defects": undetected_semantic_defects,
        "detection_rate": detected_correctly / float(len(mutations)),
        "by_mutation_family": dict(sorted(by_family.items())),
        "classification_breakdown": dict(sorted(classification_breakdown.items())),
        "representative_diagnostics": representative_diagnostics,
    }
    summary_core["results_sha256"] = sha256_bytes(canonical_json_bytes(summary_core))

    schema = json.loads(FAULT_SUMMARY_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=summary_core, schema=schema)

    if output_summary_path is not None:
        output_summary_path.parent.mkdir(parents=True, exist_ok=True)
        output_summary_path.write_text(
            json.dumps(summary_core, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    return summary_core


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run systematic QCC oracle fault-injection campaign.")
    parser.add_argument("--seed", type=int, default=DEFAULT_CAMPAIGN_SEED)
    parser.add_argument("--output-report", type=pathlib.Path, default=DEFAULT_REPORT_PATH)
    parser.add_argument("--failure-dir", type=pathlib.Path, default=None)
    args = parser.parse_args(argv)

    check_pinned_environment()
    summary = run_fault_campaign(
        seed=args.seed,
        output_summary_path=args.output_report,
        failure_artifacts_root=args.failure_dir,
    )
    print(
        f"Fault campaign completed: attempted={summary['attempted']}, "
        f"detected_correctly={summary['detected_correctly']}, "
        f"misclassified={summary['misclassified']}, "
        f"undetected_semantic_defects={summary['undetected_semantic_defects']}, "
        f"rate={summary['detection_rate']:.1%}"
    )
    print(f"  classification_breakdown: {summary['classification_breakdown']}")
    return 0 if summary["misclassified"] == 0 and summary["undetected_semantic_defects"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
