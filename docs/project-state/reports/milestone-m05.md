# Milestone M5 Implementation Report — Logical-to-Physical Mapping & Routing (Phases 44–53)

```text
Milestone: Milestone M5 (Logical-to-Physical Mapping & Routing)
Phases Covered: Phases 44–53
Status: Completed & Accepted
Historical Checkpoint: Milestone M5 Baseline
Artifacts: src/routing/, tests/routing.rs, docs/decisions/ADR-0001-ancilla-zero-state-policy.md
```

---

## 1. Executive Summary

Milestone M5 delivered QCC's qubit mapping, placement heuristics, and circuit routing engine, enabling the compiler to route logical quantum circuits over physically constrained coupling graphs (such as linear chains, rings, grids, and heavy-hex topologies) by inserting minimal physical $SWAP$ gates. This milestone established the bidirectional SABRE algorithm and the foundational [ADR-0001](../../decisions/ADR-0001-ancilla-zero-state-policy.md) clean-ancilla verification contract.

---

## 2. Sub-Phase Architecture & Deliverables

| Phase | Title | Core Deliverables & Algorithmic Design |
|:---:|---|---|
| **44** | Injective Layout Map | Typed representations for logical qubits ($Q$) and physical qubits ($P$), explicitly tracking initial layout ($L_{\text{initial}}$), active permutations, and final layout ($L_{\text{final}}$). |
| **45** | Initial Placement Heuristics | Placement algorithms: `TrivialPlacement` ($q_i \to p_i$) and `WeightedPlacement` (minimizing graph distance weighted by two-qubit interaction frequencies). |
| **46** | Shortest-Path Distance Planning | Breadth-first search (BFS) pathfinding computing all-pairs shortest paths over target coupling graphs. |
| **47** | Transactional SWAP Insertion | Safe insertion of physical $SWAP$ operations updating layout maps and reconnecting DAG dependencies. |
| **48** | Greedy Baseline Router | Forward greedy router inserting SWAPs whenever multi-qubit operands are non-adjacent. |
| **49** | Baseline Benchmark Corpus | 8-fixture benchmark corpus establishing baseline routing performance and SWAP counts across topologies. |
| **50** | Forward SABRE Router | Implementation of the SABRE algorithm (Li et al., 2019): front-layer scoring, lookahead window ($\mathcal{E}$), and dynamic decay factors ($D$). |
| **51** | Bidirectional SABRE Refinement | Bidirectional layout refinement alternating forward and backward routing passes to discover optimized initial placements. |
| **52** | Layout-Aware Verification & ADR-0001 | Verification engine comparing routed physical circuits against logical references under layout permutations; established [ADR-0001](../../decisions/ADR-0001-ancilla-zero-state-policy.md) strict clean-ancilla policy. |
| **53** | Routing Acceptance | End-to-end integration and acceptance gate verifying that SABRE routing strictly outperforms the greedy baseline while maintaining 100% semantic equivalence. |

---

## 3. Architecture Decision Record ADR-0001 (Clean Ancilla Policy)

- **The Ancilla Problem**: When routing an $n$-qubit logical circuit on an $m$-qubit physical device ($m > n$), $m - n$ physical ancillas are allocated.
- **Contract**: Every physical qubit outside the logical image starts in $|0\rangle$ and **must return strictly to $|0\rangle$ with zero residual entanglement at circuit completion**.
- **Full Statevector Check**: Equivalence verification compares the full $2^m$-dimensional physical state against $|\psi_{\text{logical}}\rangle \otimes |0\rangle^{\otimes (m-n)}$. Silently tracing out or ignoring unused physical qubits is prohibited.

---

## 4. Verification Evidence & Exit Gate

- **Comparative Results**: SABRE achieved substantial SWAP reductions over the greedy baseline across 10 benchmark workloads (e.g. `dense_random_6q` reduced from 4 to 1 SWAP; `parallel_cx_layers` reduced from 7 to 3 SWAPs).
- **Equivalence Verification**: 100% ADR-0001 clean-ancilla compliance and statevector equivalence verified across all routed circuits.
- **Exit Gate**: All 10 phases completed, all tests passed with zero warnings. Milestone M5 accepted.
