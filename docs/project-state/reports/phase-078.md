# QCC Implementation Report — Phase 78: Benchmark-Gap Analysis and Optimizer Work Selection

```text
Phase: Phase 78
Milestone: Milestone M10 — Evidence-Selected Optimizer Improvements
Status: Completed & Accepted
Date: 2026-11-13
Roadmap Plan: QCC Engineering Roadmap v3.1 (Phases 60–85)
Artifacts Generated:
  - docs/research/optimizer-gap-analysis.md
  - docs/decisions/ADR-0005-evidence-selected-optimizer-priority.md
  - docs/project-state/reports/phase-078.md
```

---

## 1. Executive Overview

Phase 78 initiates **Milestone M10 (Evidence-Selected Optimizer Improvements)** by conducting a structured investigation of QCC's current optimizer quality across the canonical 1,080-case verified corpus (`corpus_v1.jsonl`) and the 18-workload fair Qiskit baseline (`benchmark_aggregate.json`).

In strict compliance with Roadmap v3.1 governance rules:
- **No optimizer behavior was modified and no new optimization passes were implemented in this phase.**
- The selection is grounded entirely in measured empirical evidence rather than algorithmic intuition.
- Pipeline effects (logical optimization vs basis translation vs routing vs scheduling) were systematically disentangled.
- Qiskit served strictly as an external, independent comparison reference, not as a runtime dependency.

### Key Investigation Results
1. **The Primary Competitive Deficit is Two-Qubit (CX) Count**:
   In the independent Qiskit baseline comparison (Phase 70; 18 workloads, 90 paired trials under identical bases, topologies, and trial counts):
   - **CX Count**: QCC won **0**, Qiskit won **13**, with **5 ties**.
   - **Critical-Path Depth**: QCC won **1**, Qiskit won **15**, with **2 ties**.
   - **Total Gate Count**: QCC won **2**, Qiskit won **15**, with **1 tie**.
2. **Current Optimizer Pipeline Ignores Commutative Multi-Qubit Redundancies**:
   QCC's standard logical optimizer (`OptimizationLevel::O2`) includes `IdentityEliminationPass`, `InverseCancellationPass`, `RotationMergePass`, and `OneQubitFusionPass`.
   `InverseCancellationPass` strictly requires candidate gate pairs to be **wire-adjacent on all shared quantum wires simultaneously**. If two identical, self-inverse gates ($CX(c, t) \dots CX(c, t)$) are separated by operations that mathematically commute with control or target, `O2` fails to cancel them.
3. **Topological Commutation Reordering (`O3`) is Ineffective and Unstable**:
   The existing `CommutationReorderPass` in `O3` attempts speculative adjacent node swaps in topological order. In testing on dense circuits (e.g. `c0037`):
   - It incurred a **$6.83\times$ compile-time penalty** (18.1 ms $\to$ 123.6 ms).
   - It caused loop oscillation, executing all 10 allowed iterations and 70 passes.
   - It **worsened** circuit gate count (57 $\to$ 58) and depth (19 $\to$ 21) by disrupting contiguous single-qubit fusion windows.
4. **Quantified Opportunity Sizing Across 1,080 Corpus Cases**:
   - **Commutative Two-Qubit (CX/CZ) Cancellation**: **278 pairs across 154 cases** ($14.3\%$ of the corpus), representing up to **556 physical CX gates** and **$83.4\,\mu\text{s}$** of physical pulse execution time ($150\text{ ns/CX}$).
   - **Commutative Same-Axis Rotation Merging**: **2,179 pairs across 527 cases** ($48.8\%$ of the corpus), but yields **$0.0\text{ ns}$ hardware duration improvement** on native superconducting targets where $R_z$ gates execute as zero-duration virtual frame changes.
5. **Formal Selection & ADR**:
   Formally adopted **ADR-0005**, selecting **Commutative Two-Qubit (CX/CZ) Cancellation** as the single bounded optimizer improvement for Phases 79–81.

---

## 2. Deliverables Summary

| Deliverable | Path | Description |
|---|---|---|
| **Research Gap Analysis** | `docs/research/optimizer-gap-analysis.md` | In-depth technical report analyzing optimizer architecture, pipeline stage attribution, level progression across 104 stratified cases, failure mode analysis, opportunity sizing, and candidate evaluations. |
| **Architecture Decision Record** | `docs/decisions/ADR-0005-evidence-selected-optimizer-priority.md` | Formal ADR documenting the selection of Commutative Two-Qubit Cancellation, documenting rejected alternatives, implementation bounds, and proof obligations. |
| **Phase Completion Report** | `docs/project-state/reports/phase-078.md` | Official completion report verifying exit gates, deliverables, and Phase 79–81 execution roadmap. |
| **Progress Ledger Update** | `docs/project-state/progress.md` | Reconciled progress ledger, recording Phase 78 completion and activating Phase 79 without marking future phases complete. |

---

## 3. Evidence Table of Evaluated Optimization Opportunities

| Candidate | Optimization Rule | Measured Opportunity in Corpus | Hardware Value | Semantic & Global Phase Risk | Implementation Complexity | Disposition |
|---|---|:---:|---|:---:|:---:|:---:|
| **Candidate 1: Commutative Two-Qubit Cancellation** | Cancel $CX(c, t) \dots CX(c, t)$ and $CZ(a, b) \dots CZ(a, b)$ across commuting gates along both wires. | **278 pairs (154 cases)** | **Critical**: CX duration is $150\text{ ns}$, infidelity $\sim 1\%$. Saves up to **$83.4\,\mu\text{s}$** makespan and **556 physical CX gates**. | **Zero**: Exact unitary inverse ($CX^2 = I$). Net global phase change $\Delta \phi = 0$. No floating-point angle algebra. | Moderate (DAG reachability scan with `CommutationRegistry`). | **SELECTED (Strongest Candidate)** |
| **Candidate 2: Commutative Rotation Merging** | Merge $R_z(q, \theta_1) \dots R_z(q, \theta_2)$ across commuting operations on wire $q$ ($CX$ control, $CZ$). | **2,179 pairs (527 cases)** | **Zero**: On native superconducting target, $R_z$ is a virtual frame change ($0.0\text{ ns}$ duration). | **Moderate**: Requires tracking $4\pi$ periodicity and $+\pi$ global phase adjustments on $2\pi$ reduction. Floating-point angle sum tolerances. | Moderate. | Deferred (High frequency, but zero hardware makespan benefit). |
| **Candidate 3: Topological Commutation Reorder Tuning** | Tune lookahead window and strategy in `CommutationReorderPass`. | N/A (Algorithmic) | Variable / Negative. Proven to cause depth regressions and $6.8\times$ compile slowdown. | Low. | High (Heuristic bubble-sort across topological orderings). | **REJECTED**: Causes loop oscillation and depth regressions. |
| **Candidate 4: Virtual / Terminal SWAP Elision** | Absorb terminal SWAP gates into output layout permutation ($L_{\text{final}}$). | 270 cases | High (saves 1 to 3 SWAPs, 3 to 9 CXs per QFT circuit). | **High Architectural Hazard**: Mutating logical circuit unitary breaks `OptimizerPipeline` logical equivalence verification without layout tracking. | High (Requires refactoring optimizer-routing interface boundary). | **REJECTED for M10**: Belongs to routing/layout boundary, not logical optimizer DAG rewrites. |
| **Candidate 5: Two-Qubit Block Optimization (KAK)** | $SU(4)$ Cartan decomposition for arbitrary 2-qubit blocks. | Seen in `c0014` (14 CX $\to$ 2 CX). | Extreme. | High (Numerical phase ambiguity). | High. | **PROHIBITED**: Explicitly deferred by project charter and invariant rule 10. |
| **Candidate 6: Symmetric Gate Operand Normalization** | Cancel $CZ(a, b) \cdot CZ(b, a)$ and $SWAP(a, b) \cdot SWAP(b, a)$. | **0 cases** | 0 gates saved on current corpus. | Zero. | Low. | **REJECTED**: Zero empirical occurrence in corpus. |

---

## 4. Bounded Implementation and Verification Plan (Phases 79–81)

### 4.1 Phase 79: Selected Optimizer Rule Specification & Proof Obligations
- **Mathematical Specification**: Formalize exact commutation conditions:
  - $CX(c, t)$ with control operations: $\mathcal{C}(c) = \{R_z(\theta), Z, S, S^\dagger, T, T^\dagger, CX(c, t_2), CZ(c, q)\}$.
  - $CX(c, t)$ with target operations: $\mathcal{T}(t) = \{R_x(\theta), X, CX(c_2, t)\}$.
  - $CZ(a, b)$ with symmetric operations: $\mathcal{Z}(a, b) = \{R_z(\theta), Z, S, S^\dagger, T, T^\dagger, CZ(a, q), CZ(b, q)\}$.
- **Preconditions and Wire Safety**: Define forward traversal rules. Crossing non-unitary operations (`Measure`, `Reset`), synchronization `Barrier`s, classical hazard boundaries (RAW/WAR/WAW), or non-commuting operations strictly terminates traversal.
- **Negative Proof Fixtures**: Unitary matrix tests proving non-cancellation when non-commuting operations intervene (e.g. $CX \cdot R_y \cdot CX \neq R_y$).
- **Deliverables**: `docs/optimizer/commutative-cancellation-spec.md`, rule-level matrix tests.

### 4.2 Phase 80: Pass Implementation
- **Implementation**: Implement `CommutativeCancellationPass` under `src/optimizer/passes/commutative_cancellation.rs`.
- **Transactional Rewriting**: Execute atomically via `CircuitTransaction::remove_operation`, updating wire dependencies and validating DAG acyclicity.
- **Pass Manager Integration**: Add opt-in configuration toggle (`OptimizerConfig.enable_commutative_cancellation`) and record detailed telemetry (pairs examined, pairs cancelled, CX vs CZ breakdown) in Phase 71 `PassDiagnosticReport`.
- **Deliverables**: Pass implementation, comprehensive unit/property tests in `tests/optimizer/commutative_cancellation.rs`.

### 4.3 Phase 81: Independent Validation & Release Decision
- **Oracle Verification**: Validate all 1,080 cases in `corpus_v1.jsonl` with independent Qiskit statevector oracle (target: 100% PASS, 0 semantic mismatches, 0 clean-ancilla violations).
- **Comparative Baseline Update**: Re-run the Phase 70 benchmark to quantify CX count reductions, depth improvements, and wall-clock compile times.
- **Release Decision**: Make evidence-based decision to enable commutative cancellation by default in `OptimizationLevel::O2`.
- **Deliverables**: Comparative benchmark report, schema-compliant JSON artifact, release decision record.

---

## 5. Verification Evidence & Quality Gates Proof

To verify that this investigation has strictly preserved the repository and introduced zero regressions:

1. **`cargo fmt --check`**: **Passed** (clean codebase formatting).
2. **`cargo clippy --all-targets -- -D warnings`**: **Passed** (zero warnings across all crates and test targets).
3. **`cargo doc --no-deps`**: **Passed** (clean rustdoc generation).
4. **`cargo test --test routing`**: **Passed** (**213/213 passed**, 0 failed).
5. **`cargo test --test cli`**: **Passed** (**51/51 passed**, 0 failed).
6. **`PATH=".venv/bin:$PATH" cargo test --no-fail-fast`**: **Passed** (**1,523 workspace tests passed** across all 41 test targets).
7. **`.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`**: **Passed** (**61/61 Python oracle tests passed**).
8. **No Code Mutations**: Verified that no optimizer code, compiler passes, or compiler defaults were altered in Phase 78.

---

## 6. Milestone M10 Status and Next Action

Phase 78 exit gate is fully satisfied.
- **Current Milestone**: Milestone M10 (Evidence-Selected Optimizer Improvements).
- **Active Next Task**: **Phase 79 — Selected optimizer rule specification and local proof obligations**.

