"""Deterministic 1,000+ difficult canonical circuit corpus generator (M7 Phase 68).

Separate from corpus execution (`run_corpus.py`). Generates deterministic
canonical cases covering:
- Circuit families: single_qubit_rotations, cx_asymmetric_pairs, ghz_entanglement,
  qft_like_nonadjacent, logical_swap_stress, random_layered_dense,
  global_and_relative_phase, parameterized_bound.
- Topologies: linear, ring, linear-generic, ring-generic.
- Logical widths 1..7 (physical widths 2..7), dense/medium/sparse routing pressure,
  both exhaustive_unitary (m <= 5) and sampled_statevector (m >= 6 plus explicit sampled slice).
- Detects duplicate logical circuit hashes (`distinct_logical_hashes`, `duplicate_logical_hashes`).
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
import pathlib
import random
import statistics
import sys
from typing import Any

import jsonschema

ORACLE_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(ORACLE_DIR) not in sys.path:
    sys.path.insert(0, str(ORACLE_DIR))

from cases_loader import (  # noqa: E402
    CASE_SCHEMA_PATH,
    canonical_json_bytes,
    sha256_bytes,
)


CORPUS_DIR = pathlib.Path(__file__).resolve().parent
DEFAULT_CORPUS_JSONL = CORPUS_DIR / "corpus_v1.jsonl"
DEFAULT_SUMMARY_JSON = CORPUS_DIR / "corpus_generation_summary.json"
CORPUS_SUMMARY_SCHEMA_PATH = ORACLE_DIR / "schema" / "corpus_summary.schema.json"

GENERATOR_VERSION = "1.0.0"
DEFAULT_MASTER_SEED = 20261112
DEFAULT_CASE_COUNT = 1080
SAMPLING_STRATEGY_NAME = "basis_plus_uniform_plus_haar_gaussian"

# Strictly QCC-frontend-supported unitary gate subset (ADR-0003).
_NON_PARAM_1Q = ("h", "x", "y", "z", "s", "sdg", "t", "tdg")
_ROT_1Q = ("rx", "ry", "rz")
_PARAM_ANGLES = (
    "pi / 8",
    "pi / 4",
    "pi / 3",
    "pi / 2",
    "3 * pi / 4",
    "-pi / 4",
    "-pi / 2",
    "0.37",
    "1.15",
    "2.0943951023931953",
)


def derive_case_seed(master_seed: int, index: int, family: str, target: str) -> int:
    """Derive deterministic 32-bit unsigned seed from (master_seed, index, family, target)."""
    digest = hashlib.sha256(
        f"{GENERATOR_VERSION}:{master_seed}:{index}:{family}:{target}".encode("utf-8")
    ).digest()
    return int.from_bytes(digest[:4], "big")


def detect_duplicate_logical_hashes(cases: list[dict[str, Any]]) -> dict[str, Any]:
    """Detect duplicate logical_qasm digests across generated canonical case objects."""
    seen: dict[str, list[str]] = {}
    for c in cases:
        digest = sha256_bytes(c["logical_qasm"].encode("utf-8"))
        seen.setdefault(digest, []).append(c["case_id"])
    duplicates = {h: cids for h, cids in seen.items() if len(cids) > 1}
    total_dup_entries = sum(len(cids) - 1 for cids in duplicates.values())
    return {
        "total_cases": len(cases),
        "distinct_logical_hashes": len(seen),
        "duplicate_logical_hashes": total_dup_entries,
        "duplicate_groups": duplicates,
    }


def _rand_angle(rng: random.Random) -> str:
    if rng.random() < 0.55:
        return rng.choice(_PARAM_ANGLES)
    val = round(rng.uniform(-math.pi, math.pi), 6)
    if abs(val) < 1e-4:
        val = 0.25
    return repr(val)


def _append_1q(lines: list[str], rng: random.Random, qubit: int) -> None:
    if rng.random() < 0.45:
        lines.append(f"{rng.choice(_NON_PARAM_1Q)} q[{qubit}];")
    else:
        lines.append(f"{rng.choice(_ROT_1Q)}({_rand_angle(rng)}) q[{qubit}];")


def _classify_density(num_qubits: int, two_q_count: int) -> str:
    if num_qubits <= 1 or two_q_count == 0:
        return "single_qubit_only"
    ratio = two_q_count / float(num_qubits)
    if ratio < 1.5:
        return "sparse"
    if ratio < 3.5:
        return "medium"
    return "dense"


def _build_circuit_for_family(
    family: str, num_qubits: int, rng: random.Random, variant_salt: int
) -> tuple[str, dict[str, float], int, int, list[str]]:
    """Build deterministic logical OpenQASM 3 source + metadata for one family."""
    lines = [
        "OPENQASM 3.0;",
        'include "stdgates.inc";',
        "",
        f"qubit[{num_qubits}] q;",
        "",
    ]
    bindings: dict[str, float] = {}
    notes: list[str] = []
    two_q_count = 0
    depth_layers = 0

    if family == "single_qubit_rotations":
        layers = 4 + (variant_salt % 9)
        depth_layers = layers
        if rng.random() < 0.35:
            lines.append(f"gphase({_rand_angle(rng)});")
        for _ in range(layers):
            for q in range(num_qubits):
                _append_1q(lines, rng, q)
        if num_qubits == 1:
            notes.append("Compiled onto minimum 2-qubit target with 1 spare physical ancilla.")

    elif family == "cx_asymmetric_pairs":
        layers = 4 + (variant_salt % 7)
        depth_layers = layers * 2
        for layer in range(layers):
            for q in range(num_qubits):
                _append_1q(lines, rng, q)
            # Deliberately alternate both CX orientations & non-adjacent span
            span = 1 + (layer % max(1, num_qubits - 1))
            for q in range(num_qubits):
                targ = (q + span) % num_qubits
                if targ == q:
                    continue
                if (layer + q) % 2 == 0:
                    lines.append(f"cx q[{q}], q[{targ}];")
                else:
                    lines.append(f"cx q[{targ}], q[{q}];")
                two_q_count += 1

    elif family == "ghz_entanglement":
        mode = variant_salt % 3
        lines.append("h q[0];")
        # Add single-qubit twist on non-trivial variants so GHZ family covers diverse hashes
        lines.append(f"rz({_rand_angle(rng)}) q[0];")
        if mode == 0:
            # Nearest-neighbor chain + optional trailing interactions
            for q in range(num_qubits - 1):
                lines.append(f"cx q[{q}], q[{q + 1}];")
                two_q_count += 1
        elif mode == 1:
            # Hub-and-spoke from q[num_qubits - 1] (forces long-range routing on linear)
            hub = num_qubits - 1
            for q in range(num_qubits - 1):
                lines.append(f"cx q[{hub}], q[{q}];")
                two_q_count += 1
        else:
            # Bidirectional tree + phase twist
            for q in range(1, num_qubits):
                parent = (q - 1) // 2
                lines.append(f"cx q[{parent}], q[{q}];")
                two_q_count += 1
        for q in range(num_qubits):
            if rng.random() < 0.6:
                _append_1q(lines, rng, q)
        depth_layers = num_qubits + 3

    elif family == "qft_like_nonadjacent":
        # QFT-style controlled-phase rotations decomposed to {h, rz, cx} + bit-reversal SWAPs
        for j in range(num_qubits):
            lines.append(f"h q[{j}];")
            for k in range(j + 1, num_qubits):
                angle = repr(math.pi / (2 ** (k - j + 1)))
                lines.append(f"rz({angle}) q[{k}];")
                lines.append(f"rz({angle}) q[{j}];")
                lines.append(f"cx q[{k}], q[{j}];")
                lines.append(f"rz(-{angle}) q[{j}];")
                lines.append(f"cx q[{k}], q[{j}];")
                two_q_count += 2
        # Bit-reversal SWAPs + optional salt layer so variants remain unique
        for i in range(num_qubits // 2):
            j = num_qubits - 1 - i
            lines.append(f"swap q[{i}], q[{j}];")
            two_q_count += 1
        _append_1q(lines, rng, variant_salt % num_qubits)
        depth_layers = num_qubits * 4

    elif family == "logical_swap_stress":
        layers = 3 + (variant_salt % 6)
        depth_layers = layers * 3
        for layer in range(layers):
            for q in range(num_qubits):
                _append_1q(lines, rng, q)
            q_a = rng.randrange(num_qubits)
            q_b = (q_a + max(1, num_qubits - 1 - (layer % max(1, num_qubits - 1)))) % num_qubits
            if q_a != q_b:
                lines.append(f"swap q[{q_a}], q[{q_b}];")
                lines.append(f"cx q[{q_b}], q[{q_a}];")
                two_q_count += 2
            # Additional distant CX to force layout permutations
            if num_qubits >= 3:
                lines.append(f"cx q[0], q[{num_qubits - 1}];")
                two_q_count += 1

    elif family == "random_layered_dense":
        layers = 5 + (variant_salt % 9)
        depth_layers = layers * 2
        for layer in range(layers):
            for q in range(num_qubits):
                _append_1q(lines, rng, q)
            # Dense pairing per layer biased toward distant pairs on linear/ring
            pairs = [(i, j) for i in range(num_qubits) for j in range(i + 1, num_qubits)]
            rng.shuffle(pairs)
            num_2q_this_layer = min(len(pairs), max(1, num_qubits // 2 + (layer % 2)))
            for a, b in pairs[:num_2q_this_layer]:
                if rng.random() < 0.5:
                    ctrl, targ = a, b
                else:
                    ctrl, targ = b, a
                if rng.random() < 0.15:
                    lines.append(f"cz q[{ctrl}], q[{targ}];")
                else:
                    lines.append(f"cx q[{ctrl}], q[{targ}];")
                two_q_count += 1

    elif family == "global_and_relative_phase":
        # Nonzero input gphase + Hadamard-conjugation / relative-phase-sensitive structure
        gphase_val = round(rng.uniform(0.15, 5.8), 6)
        lines.append(f"gphase({gphase_val});")
        layers = 3 + (variant_salt % 5)
        depth_layers = layers * 3
        for q in range(num_qubits):
            lines.append(f"h q[{q}];")
        for _ in range(layers):
            for q in range(num_qubits):
                lines.append(f"rz({_rand_angle(rng)}) q[{q}];")
            if num_qubits >= 2:
                a, b = rng.sample(range(num_qubits), 2)
                lines.append(f"cx q[{a}], q[{b}];")
                lines.append(f"rz({_rand_angle(rng)}) q[{b}];")
                lines.append(f"cx q[{a}], q[{b}];")
                two_q_count += 2
        for q in range(num_qubits):
            lines.append(f"h q[{q}];")
        if num_qubits == 1:
            notes.append("Single logical qubit tests spare physical ancilla + nonzero global phase.")

    elif family == "parameterized_bound":
        theta = round(rng.uniform(0.1, 3.0), 6)
        phi = round(rng.uniform(-2.8, 2.8), 6)
        lam = round(rng.uniform(0.2, 2.5), 6)
        bindings = {"lam": lam, "phi": phi, "theta": theta}
        lines.append("rz(theta) q[0];")
        if num_qubits >= 2:
            lines.append("rx(phi) q[1];")
            lines.append(f"cx q[0], q[{num_qubits - 1}];")
            two_q_count += 1
        lines.append(f"ry(lam) q[{variant_salt % num_qubits}];")
        extra_layers = 2 + (variant_salt % 5)
        depth_layers = extra_layers * 2 + 2
        for _ in range(extra_layers):
            for q in range(num_qubits):
                _append_1q(lines, rng, q)
            if num_qubits >= 2:
                a, b = rng.sample(range(num_qubits), 2)
                lines.append(f"cx q[{a}], q[{b}];")
                two_q_count += 1
        lines.append("rz(-theta) q[0];")
    else:
        raise ValueError(f"Unknown circuit family: {family}")

    qasm_text = "\n".join(lines).rstrip() + "\n"
    return qasm_text, bindings, depth_layers, two_q_count, notes


def generate_corpus(
    *,
    count: int = DEFAULT_CASE_COUNT,
    master_seed: int = DEFAULT_MASTER_SEED,
    output_jsonl: pathlib.Path = DEFAULT_CORPUS_JSONL,
    output_summary: pathlib.Path = DEFAULT_SUMMARY_JSON,
) -> dict[str, Any]:
    """Generate `count` distinct deterministic canonical cases + schema-validated summary."""
    if count < 1000:
        raise ValueError(f"Session 4 corpus requires at least 1,000 cases, got {count}")

    case_schema = json.loads(CASE_SCHEMA_PATH.read_text(encoding="utf-8"))
    case_validator = jsonschema.Draft202012Validator(case_schema)

    families = (
        "single_qubit_rotations",
        "cx_asymmetric_pairs",
        "ghz_entanglement",
        "qft_like_nonadjacent",
        "logical_swap_stress",
        "random_layered_dense",
        "global_and_relative_phase",
        "parameterized_bound",
    )
    targets = ("linear", "ring", "linear-generic", "ring-generic")

    seen_hashes_by_target: set[tuple[str, str]] = set()
    logical_qasm_hashes: list[str] = []
    cases_raw: list[dict[str, Any]] = []

    by_topology: Counter[str] = Counter()
    by_logical_qubits: Counter[str] = Counter()
    by_family: Counter[str] = Counter()
    by_density: Counter[str] = Counter()
    by_mode: Counter[str] = Counter()
    depths: list[int] = []

    idx = 0
    salt_offset = 0
    while len(cases_raw) < count:
        family = families[idx % len(families)]
        target = targets[(idx // len(families)) % len(targets)]

        if family == "single_qubit_rotations":
            num_qubits = 1 if (idx % 16) < 8 else 2
        elif family == "global_and_relative_phase":
            num_qubits = [1, 2, 3, 4, 5][((idx // 8) % 5)]
        elif family == "qft_like_nonadjacent":
            num_qubits = [3, 4, 5, 6][((idx // 8) % 4)]
        elif family == "parameterized_bound":
            num_qubits = [2, 3, 4, 5][((idx // 8) % 4)]
        else:
            num_qubits = [2, 3, 4, 5, 6, 7][((idx // 8) % 6)]

        derived_seed = derive_case_seed(master_seed, idx + salt_offset, family, target)
        rng = random.Random(derived_seed)

        qasm_text, bindings, depth_target, two_q_count, extra_notes = _build_circuit_for_family(
            family, num_qubits, rng, idx + salt_offset
        )
        qasm_sha = sha256_bytes(qasm_text.encode("utf-8"))
        # Guarantee zero accidental duplicate (logical_qasm, target) pairs cleanly
        if (qasm_sha, target) in seen_hashes_by_target:
            salt_offset += 1
            continue
        seen_hashes_by_target.add((qasm_sha, target))
        logical_qasm_hashes.append(qasm_sha)

        phys_qubits = max(num_qubits, 2)
        # Verification mode policy:
        # - Exhaustive full 2^m unitary check for tractable widths m <= 5
        # - Explicit sampled statevector for m >= 6, plus explicit sampled slice at m=4,5
        if phys_qubits >= 6 or (phys_qubits >= 4 and (idx % 11 == 0)):
            verif_mode = "sampled_statevector"
            sampled_count = 16
            sampling_strategy: str | None = SAMPLING_STRATEGY_NAME
        else:
            verif_mode = "exhaustive_unitary"
            sampled_count = 1
            sampling_strategy = None

        density = _classify_density(num_qubits, two_q_count)
        case_id = f"c{idx:04d}_{family}_{num_qubits}q_{target.replace('-', '_')}"

        limitations = list(extra_notes)
        if num_qubits <= 2 and "ring" in target:
            limitations.append(
                "For <=2 qubits, QCC ring targets share identical single-edge (0,1) connectivity with linear targets."
            )
        if verif_mode == "sampled_statevector":
            limitations.append(
                "Verified via deterministic basis + seeded random complex statevectors (non-exhaustive)."
            )

        case_obj: dict[str, Any] = {
            "schema_id": "qcc.oracle_case",
            "schema_version": "1.0.0",
            "case_id": case_id,
            "description": (
                f"Corpus {family} case on {target} ({num_qubits} logical qubits, "
                f"density={density}, mode={verif_mode})"
            ),
            "logical_qubits": num_qubits,
            "logical_qasm": qasm_text,
            "parameter_bindings": bindings,
            "compiler_config": {
                "target": target,
                "global_phase_policy": "auto",
                "seed": derived_seed,
            },
            "verification_mode": verif_mode,
            "expected_oracle_status": "PASS",
            "limitations": limitations,
            "generator_metadata": {
                "generator_version": GENERATOR_VERSION,
                "master_seed": master_seed,
                "case_index": idx,
                "derived_seed": derived_seed,
                "family": family,
                "depth_target": depth_target,
                "two_qubit_gate_count": two_q_count,
                "interaction_density": density,
                "sampling_strategy": sampling_strategy,
            },
        }
        if verif_mode == "sampled_statevector":
            case_obj["sampled_state_count"] = sampled_count

        case_validator.validate(case_obj)
        cases_raw.append(case_obj)

        by_topology[target] += 1
        by_logical_qubits[str(num_qubits)] += 1
        by_family[family] += 1
        by_density[density] += 1
        by_mode[verif_mode] += 1
        depths.append(depth_target)
        idx += 1

    output_jsonl.parent.mkdir(parents=True, exist_ok=True)
    jsonl_bytes = ("\n".join(json.dumps(c, sort_keys=True) for c in cases_raw) + "\n").encode(
        "utf-8"
    )
    output_jsonl.write_bytes(jsonl_bytes)
    corpus_sha = sha256_bytes(jsonl_bytes)

    distinct_hashes = len(set(logical_qasm_hashes))
    duplicate_hashes = len(logical_qasm_hashes) - distinct_hashes

    hist = Counter()
    for d in depths:
        if d <= 5:
            hist["1-5"] += 1
        elif d <= 10:
            hist["6-10"] += 1
        elif d <= 20:
            hist["11-20"] += 1
        else:
            hist["21+"] += 1

    summary: dict[str, Any] = {
        "schema_id": "qcc.corpus_summary",
        "schema_version": "1.0.0",
        "generator_version": GENERATOR_VERSION,
        "master_seed": master_seed,
        "total_cases": len(cases_raw),
        "distinct_case_ids": len({c["case_id"] for c in cases_raw}),
        "distinct_logical_hashes": distinct_hashes,
        "duplicate_logical_hashes": duplicate_hashes,
        "corpus_file_sha256": corpus_sha,
        "by_topology": dict(sorted(by_topology.items())),
        "by_logical_qubits": dict(sorted(by_logical_qubits.items())),
        "by_family": dict(sorted(by_family.items())),
        "by_interaction_density": dict(sorted(by_density.items())),
        "by_verification_mode": dict(sorted(by_mode.items())),
        "depth_summary": {
            "min": int(min(depths)),
            "median": float(statistics.median(depths)),
            "max": int(max(depths)),
            "histogram_buckets": dict(sorted(hist.items())),
        },
        "execution_summary": None,
    }

    summary_schema = json.loads(CORPUS_SUMMARY_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.validate(instance=summary, schema=summary_schema)

    output_summary.parent.mkdir(parents=True, exist_ok=True)
    output_summary.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate QCC deterministic 1,000+ corpus.")
    parser.add_argument("--count", type=int, default=DEFAULT_CASE_COUNT)
    parser.add_argument("--seed", type=int, default=DEFAULT_MASTER_SEED)
    parser.add_argument("--output-jsonl", type=pathlib.Path, default=DEFAULT_CORPUS_JSONL)
    parser.add_argument("--output-summary", type=pathlib.Path, default=DEFAULT_SUMMARY_JSON)
    args = parser.parse_args(argv)

    summary = generate_corpus(
        count=args.count,
        master_seed=args.seed,
        output_jsonl=args.output_jsonl,
        output_summary=args.output_summary,
    )
    print(
        f"Generated {summary['total_cases']} deterministic cases "
        f"(distinct_ids={summary['distinct_case_ids']}, "
        f"distinct_logical_hashes={summary['distinct_logical_hashes']}, "
        f"duplicate_logical_hashes={summary['duplicate_logical_hashes']}, "
        f"sha256={summary['corpus_file_sha256'][:16]}...)"
    )
    print(f"  by_verification_mode: {summary['by_verification_mode']}")
    print(f"  by_topology:          {summary['by_topology']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
