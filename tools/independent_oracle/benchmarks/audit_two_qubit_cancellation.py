#!/usr/bin/env python3
"""
audit_two_qubit_cancellation.py

Read-only evidence audit script for Milestone M10 / Phase 78.
Scans the 1,080-case canonical verified corpus (tools/independent_oracle/corpus/corpus_v1.jsonl)
under mathematically verified commutation rules to determine:
- Raw detected pairs
- Commutation-valid pairs
- Mutually compatible non-overlapping selected pairs
- Realizable cancellation count
- Number of unique gates eliminated
- Per-family and per-circuit distributions
"""

import json
import re
from collections import defaultdict
from typing import Dict, List, Optional, Set, Tuple

def parse_operations(qasm: str) -> List[Tuple[str, Tuple[int, ...], Optional[str]]]:
    """Parses OpenQASM 3.0 lines into structured (name, qubits, parameter) tuples."""
    ops = []
    for line in qasm.splitlines():
        line = line.strip().rstrip(";")
        if not line or line.startswith(("//", "OPENQASM", "include", "qubit", "gphase")):
            continue
        
        # 1Q parameterized rotation: e.g. rz(0.123) q[0]
        m_rot = re.match(r"^([a-z0-9_]+)\(([^)]+)\)\s+q\[(\d+)\]", line)
        if m_rot:
            ops.append((m_rot.group(1), (int(m_rot.group(3)),), m_rot.group(2)))
            continue
            
        # 2Q gate: e.g. cx q[0], q[1] or cz q[0], q[1] or swap q[0], q[1]
        m_2q = re.match(r"^([a-z0-9_]+)\s+q\[(\d+)\],\s*q\[(\d+)\]", line)
        if m_2q:
            ops.append((m_2q.group(1), (int(m_2q.group(2)), int(m_2q.group(3))), None))
            continue
            
        # 1Q standard gate: e.g. x q[0], h q[1]
        m_1q = re.match(r"^([a-z0-9_]+)\s+q\[(\d+)\]", line)
        if m_1q:
            ops.append((m_1q.group(1), (int(m_1q.group(2)),), None))
            continue
            
        # Barriers or measurements
        if line.startswith("barrier"):
            ops.append(("barrier", (), None))
        elif line.startswith("measure"):
            ops.append(("measure", (), None))
        elif line.startswith("reset"):
            ops.append(("reset", (), None))
        else:
            ops.append(("unsupported", (), None))
            
    return ops

def is_valid_cx_commutation(
    ops: List[Tuple[str, Tuple[int, ...], Optional[str]]],
    i: int,
    j: int,
    ctrl: int,
    targ: int
) -> bool:
    """Checks if all operations between i and j strictly commute with CX(ctrl, targ)."""
    for k in range(i + 1, j):
        op, qargs, _ = ops[k]
        
        # Non-unitary or barrier strictly blocks
        if op in ("barrier", "measure", "reset", "unsupported"):
            return False
            
        touches_ctrl = ctrl in qargs
        touches_targ = targ in qargs
        
        if not touches_ctrl and not touches_targ:
            # Disjoint wires: strictly commutes
            continue
            
        if touches_ctrl and touches_targ:
            # Intervening operation touches BOTH control and target
            if op == "cx" and qargs == (ctrl, targ):
                # Identical CX(ctrl, targ) commutes with itself
                continue
            # Reversed CX(targ, ctrl), CZ, SWAP, etc. do NOT commute
            return False
            
        if touches_ctrl:
            # Operation on control wire must commute with Z-axis
            if op in ("z", "s", "sdg", "t", "tdg", "rz", "p"):
                continue
            if op == "cz":
                # CZ is diagonal on ctrl
                continue
            if op == "cx" and qargs[0] == ctrl:
                # CX sharing control commutes
                continue
            return False
            
        if touches_targ:
            # Operation on target wire must commute with X-axis
            if op in ("x", "rx"):
                continue
            if op == "cx" and qargs[1] == targ:
                # CX sharing target commutes
                continue
            return False
            
    return True

def is_valid_cz_commutation(
    ops: List[Tuple[str, Tuple[int, ...], Optional[str]]],
    i: int,
    j: int,
    q0: int,
    q1: int
) -> bool:
    """Checks if all operations between i and j strictly commute with CZ(q0, q1)."""
    cz_qubits = {q0, q1}
    for k in range(i + 1, j):
        op, qargs, _ = ops[k]
        
        if op in ("barrier", "measure", "reset", "unsupported"):
            return False
            
        for q in cz_qubits:
            if q in qargs:
                # Must commute with Z-axis on wire q
                if op in ("z", "s", "sdg", "t", "tdg", "rz", "p", "cz"):
                    continue
                if op == "cx" and qargs[0] == q:
                    # CX where wire is control commutes with CZ
                    continue
                return False
                
    return True

def audit_corpus(corpus_path: str):
    with open(corpus_path, "r", encoding="utf-8") as f:
        cases = [json.loads(line) for line in f]

    print(f"Total corpus cases loaded: {len(cases)}")

    raw_cx_pairs = 0
    raw_cz_pairs = 0
    cases_with_raw_pairs = set()

    total_realizable_cx_pairs = 0
    total_realizable_cz_pairs = 0
    cases_with_realizable_pairs = set()

    family_stats = defaultdict(lambda: {
        "total_cases": 0,
        "cases_with_cancels": 0,
        "raw_cx": 0,
        "raw_cz": 0,
        "realizable_cx": 0,
        "realizable_cz": 0,
        "gates_eliminated": 0
    })

    circuits_with_multiple_pairs = 0
    overlapping_incidents = 0
    per_circuit_records = []

    for c in cases:
        cid = c["case_id"]
        # Extract family from case_id (e.g. c0001_arithmetic_... -> arithmetic)
        fam = "unknown"
        parts = cid.split("_")
        if len(parts) > 1:
            fam = parts[1]
        family_stats[fam]["total_cases"] += 1

        ops = parse_operations(c["logical_qasm"])

        # Detect raw pairs
        case_raw_cx = []
        case_raw_cz = []

        for i, (op1, qargs1, _) in enumerate(ops):
            if op1 == "cx":
                ctrl, targ = qargs1
                for j in range(i + 1, len(ops)):
                    op2, qargs2, _ = ops[j]
                    if op2 == "cx" and qargs2 == (ctrl, targ):
                        if j > i + 1:
                            if is_valid_cx_commutation(ops, i, j, ctrl, targ):
                                case_raw_cx.append((i, j))
                        break # Stop at first matching partner
            elif op1 == "cz":
                q0, q1 = qargs1
                target_set = {q0, q1}
                for j in range(i + 1, len(ops)):
                    op2, qargs2, _ = ops[j]
                    if op2 == "cz" and set(qargs2) == target_set:
                        if j > i + 1:
                            if is_valid_cz_commutation(ops, i, j, q0, q1):
                                case_raw_cz.append((i, j))
                        break

        total_raw = len(case_raw_cx) + len(case_raw_cz)
        if total_raw > 0:
            cases_with_raw_pairs.add(cid)
            raw_cx_pairs += len(case_raw_cx)
            raw_cz_pairs += len(case_raw_cz)
            family_stats[fam]["raw_cx"] += len(case_raw_cx)
            family_stats[fam]["raw_cz"] += len(case_raw_cz)

        if total_raw > 1:
            circuits_with_multiple_pairs += 1

        # Check for overlapping gates (a gate participating in more than one pair)
        gate_usage = defaultdict(int)
        for i, j in case_raw_cx:
            gate_usage[i] += 1
            gate_usage[j] += 1
        for i, j in case_raw_cz:
            gate_usage[i] += 1
            gate_usage[j] += 1

        for g, count in gate_usage.items():
            if count > 1:
                overlapping_incidents += 1

        # Deterministic greedy selection
        selected_cx = []
        selected_cz = []
        consumed_gates = set()

        # Prioritize in topological order of first gate
        all_candidate_pairs = [("cx", p) for p in case_raw_cx] + [("cz", p) for p in case_raw_cz]
        all_candidate_pairs.sort(key=lambda item: (item[1][0], item[1][1]))

        for kind, (i, j) in all_candidate_pairs:
            if i not in consumed_gates and j not in consumed_gates:
                consumed_gates.add(i)
                consumed_gates.add(j)
                if kind == "cx":
                    selected_cx.append((i, j))
                else:
                    selected_cz.append((i, j))

        realizable_count = len(selected_cx) + len(selected_cz)
        if realizable_count > 0:
            cases_with_realizable_pairs.add(cid)
            total_realizable_cx_pairs += len(selected_cx)
            total_realizable_cz_pairs += len(selected_cz)
            family_stats[fam]["cases_with_cancels"] += 1
            family_stats[fam]["realizable_cx"] += len(selected_cx)
            family_stats[fam]["realizable_cz"] += len(selected_cz)
            family_stats[fam]["gates_eliminated"] += realizable_count * 2

            per_circuit_records.append({
                "case_id": cid,
                "family": fam,
                "total_ops": len(ops),
                "raw_pairs": total_raw,
                "realizable_pairs": realizable_count,
                "gates_eliminated": realizable_count * 2,
                "cx_pairs": selected_cx,
                "cz_pairs": selected_cz
            })

    print("\n=======================================================")
    print("      PHASE 78 TWO-QUBIT CANCELLATION AUDIT REPORT     ")
    print("=======================================================")
    print(f"Total Corpus Cases Evaluated:           {len(cases):>6}")
    print(f"Circuits with Cancellation Potential:   {len(cases_with_realizable_pairs):>6} ({len(cases_with_realizable_pairs)/len(cases)*100:.1f}%)")
    print(f"Circuits with Multiple Pairs:           {circuits_with_multiple_pairs:>6}")
    print(f"Overlapping Gate Incidents:             {overlapping_incidents:>6}")
    print("-------------------------------------------------------")
    print(f"Raw Commutation-Valid CX Pairs:         {raw_cx_pairs:>6}")
    print(f"Raw Commutation-Valid CZ Pairs:         {raw_cz_pairs:>6}")
    print(f"Total Raw Valid Pairs:                  {raw_cx_pairs + raw_cz_pairs:>6}")
    print("-------------------------------------------------------")
    print(f"Deterministic Selected CX Pairs:        {total_realizable_cx_pairs:>6}")
    print(f"Deterministic Selected CZ Pairs:        {total_realizable_cz_pairs:>6}")
    print(f"Total Realizable Pair Cancellations:    {total_realizable_cx_pairs + total_realizable_cz_pairs:>6}")
    print(f"Total Unique Physical 2Q Gates Saved:   {(total_realizable_cx_pairs + total_realizable_cz_pairs) * 2:>6}")
    print("=======================================================\n")

    print("PER-FAMILY BREAKDOWN:")
    print(f"{'Family':<16} | {'Cases':>5} | {'Raw 2Q':>6} | {'Realizable':>10} | {'Gates Saved':>11}")
    print("-" * 58)
    for fam, s in sorted(family_stats.items()):
        if s["cases_with_cancels"] > 0:
            print(f"{fam:<16} | {s['cases_with_cancels']:>5} | {s['raw_cx']+s['raw_cz']:>6} | {s['realizable_cx']+s['realizable_cz']:>10} | {s['gates_eliminated']:>11}")
    print("-" * 58)
    total_saved = (total_realizable_cx_pairs + total_realizable_cz_pairs) * 2
    print(f"{'TOTAL':<16} | {len(cases_with_realizable_pairs):>5} | {raw_cx_pairs+raw_cz_pairs:>6} | {total_realizable_cx_pairs+total_realizable_cz_pairs:>10} | {total_saved:>11}\n")

    return {
        "total_cases": len(cases),
        "affected_cases": len(cases_with_realizable_pairs),
        "raw_cx_pairs": raw_cx_pairs,
        "raw_cz_pairs": raw_cz_pairs,
        "selected_cx_pairs": total_realizable_cx_pairs,
        "selected_cz_pairs": total_realizable_cz_pairs,
        "total_cancellations": total_realizable_cx_pairs + total_realizable_cz_pairs,
        "unique_gates_eliminated": total_saved,
        "family_stats": dict(family_stats)
    }

if __name__ == "__main__":
    audit_corpus("tools/independent_oracle/corpus/corpus_v1.jsonl")

