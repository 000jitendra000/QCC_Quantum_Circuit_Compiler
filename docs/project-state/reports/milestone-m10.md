# Milestone M10 Implementation Report — Evidence-Selected Optimizer Improvements (Phases 78–81)

```text
Milestone: Milestone M10 (Evidence-Selected Optimizer Improvements)
Phases Covered: Phases 78–81
Status: Completed & Accepted
Roadmap: QCC Engineering Roadmap v3.1
Artifacts: src/optimizer/passes/commutative_cancellation.rs, tools/independent_oracle/benchmarks/optimizer_validation.py, ADR-0005
```

---

## 1. Executive Summary

Milestone M10 delivered QCC's evidence-selected optimizer improvement, addressing the two-qubit gate count deficit identified in the Phase 70 Qiskit baseline. Based on empirical analysis of the 1,080-circuit verified corpus, this milestone selected, formally specified, and implemented the `CommutativeTwoQubitCancellationPass`, evaluated it across a 130-circuit comparative benchmark, and established the authoritative policy to retain the pass as an opt-in capability (`--enable-commutative-cancellation`).

---

## 2. Sub-Phase Architecture & Deliverables

| Phase | Title | Core Deliverables & Empirical Methodology |
|:---:|---|---|
| **78** | Benchmark-Gap Analysis & Selection | Audited 1,080 corpus circuits via read-only scanner [`audit_two_qubit_cancellation.py`](../../../tools/independent_oracle/benchmarks/audit_two_qubit_cancellation.py). Discovered 165 logical cancellation pairs across 82 cases (163 CX pairs, 2 CZ pairs). Documented [ADR-0005](../../decisions/ADR-0005-evidence-selected-optimizer-priority.md) selecting Commutative Two-Qubit Cancellation. |
| **79** | Specification & Proof Obligations | Formulated mathematical specification in [`docs/optimizer/commutative-cancellation-spec.md`](../../optimizer/commutative-cancellation-spec.md). Proved sufficient-condition cancellation theorem: $(\forall k, [G, O_k] = 0) \implies G \cdot (\prod O_k) \cdot G = \prod O_k$. Implemented 24 automated algebraic rule tests establishing executable proof obligations. |
| **80** | Pass Implementation & Integration | Implemented `CommutativeTwoQubitCancellationPass` in `src/optimizer/passes/commutative_cancellation.rs`. Forward topological reachability, greedy non-overlapping pair consumption, ordered CX matching, symmetric CZ matching, and fail-closed barrier blocking. Added 32 integration tests; gated behind `--enable-commutative-cancellation` (`default: false`). |
| **81** | Independent Validation Benchmark | Independent comparative benchmark across 130 circuits $\times$ 3 repeated trials (390 runs). Verified 100% semantic equivalence (260/260 PASS: 130 Treatment B + 130 Control A). Documented empirical qualifications and decided to keep opt-in. |

---

## 3. Empirical Benchmark Results (130 Circuits × 3 Trials)

| Metric | Control A (Default) | Treatment B (`--enable-commutative-cancellation`) | Net Delta | Verification |
|---|:---:|:---:|:---:|:---:|
| **Total Physical Gates** | 16,842 | 16,304 | **-538 gates** | 260 / 260 PASS |
| **Two-Qubit Gates** | 5,420 | 5,260 | **-160 gates** | 260 / 260 PASS |
| **Router SWAPs** | 412 | 398 | **-14 SWAPs** | 260 / 260 PASS |
| **Circuit Depth** | 8,914 | 8,628 | **-286 depth** | 260 / 260 PASS |
| **Pass Duration (mean)** | — | 0.15 ms | +0.15 ms | — |
| **Overall Multiplier** | 1.00x | 1.00x | ~0.0% overhead | — |

### Mandatory Empirical Qualifications
1. **Stage Attribution**: For the 378 non-two-qubit eliminated gates: *"Observed aggregate reduction; exact stage-level causal attribution unavailable."* (160 gates are directly eliminated 2Q gates).
2. **Wall-Clock Timing**: Timing overhead is preliminary and modest (mean pass duration: 0.15 ms).
3. **Routing Interactions**: In 3 dense ring-topology cases, commutative reordering altered the front-layer DAG and slightly increased router SWAPs (+1 to +4 SWAPs).
4. **Release Decision**: In strict accordance with evidence-based governance, `enable_commutative_cancellation` remains **opt-in** (default `false`).

---

## 4. Verification Evidence & Exit Gate

- **Oracle Pass Rate**: 260 / 260 independent Qiskit oracle passes with 100% ADR-0001 clean-ancilla compliance.
- **Rule Tests**: 24 automated algebraic proof-obligation tests and 32 integration tests passed.
- **Exit Gate**: All 4 phases completed, all tests passed with zero warnings. Milestone M10 accepted.
