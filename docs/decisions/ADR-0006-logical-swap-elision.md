# ADR-0006: Explicit Logical SWAP Elision into Circuit Layout Permutation (--elide-swaps)

## Status
Accepted (14 November 2026)

## Context
In the Phase 70 matched-basis baseline against Qiskit O2 (18 representative canonical workloads on native basis `{rz, sx, cx}`), QCC emitted 378 physical CX gates compared to Qiskit's 261 physical CX gates (+117 excess CX, +44.83%).

Work Package WP0 conducted a strict four-bucket physical CX gate decomposition:
- **Bucket A (Logical Two-Qubit CX/CZ)**: 159 CX (42.06%)
- **Bucket B (Logical SWAPs Synthesized to 3 CX)**: 66 CX (17.46%; 22 logical SWAPs × 3)
- **Bucket C (Router SWAPs Synthesized to 3 CX)**: 153 CX (40.48%; 51 router SWAPs × 3)
- **Bucket D (Other / Direction / Residual CX)**: 0 CX (0.00%)

The decomposition identity $A + B + C + D = 378$ held with zero unaccounted gates across all 18 workloads.

Crucially, **56.4% of QCC's aggregate excess CX (+66 CX)** originated directly from QCC decomposing 22 explicit logical `swap` operations into $22 \times 3 = 66$ physical CX gates across 7 workloads (`04_logical_swap`, `07_qft4_showcase_linear`, `08_qft4_showcase_ring`, `c0011_qft_like_nonadjacent_4q_ring`, `c0036_logical_swap_stress_6q_linear`, `c0068_logical_swap_stress_4q_linear`, and `c0076_logical_swap_stress_5q_ring`). In contrast, Qiskit O2 absorbs logical SWAP operations into initial/final qubit layout permutations (virtual wire tracking), emitting zero physical CX gates for terminal/permutation swaps.

In ADR-0005, SWAP elision was considered but deferred because:
> *"Modifying logical circuit output permutations breaks OptimizerPipeline's strict unitary equivalence contract (`check_equivalence(initial, circuit)`). SWAP elision is a layout/routing boundary capability, not a logical optimizer DAG rewrite."*

To capture this +66 CX reduction while maintaining 100% mathematical rigor and zero regression risk, QCC requires a dedicated, layout-aware logical SWAP elision transformation at the compilation boundary.

---

## Decision

QCC implements **Explicit Logical SWAP Elision** via an opt-in CLI flag `--elide-swaps` and compiler configuration `CompilerConfig.elide_swaps: bool` (default: `false` pending post-WP2 benchmark evidence).

### 1. Execution Placement in the Pipeline
Logical SWAP elision executes at the compilation boundary between **Stage 1 (Logical Optimization)** and **Stage 2 (Target Basis Translation)**:
1. Stage 1 (`OptimizerPipeline`) optimizes the circuit as an unconstrained logical circuit (preserving identity elimination, rotation merging, Euler decomposition, and commutative cancellation).
2. If `elide_swaps` is enabled, `elide_logical_swaps(&opt_circuit)` is invoked:
   - Removes all explicit `Gate::Swap` operations from the circuit DAG.
   - Maintains a virtual wire mapping $\pi_{\text{current}}: Q_{\text{log}} \to \text{Wire}$.
   - Rewrites the operands of all subsequent operations (`Gate`, `Measure`, `Reset`, `Barrier`) to their active virtual wires.
   - Yields the elided circuit and the net logical wire permutation $\pi_{\text{elided}}: Q_{\text{log}} \to \text{Wire}$.
3. Stage 2 (`BasisTranslationEngine`) translates the elided circuit into the target's native basis. Because `Gate::Swap` operations have been removed, **zero CX gates are synthesized for logical swaps**.
4. Stage 3 (Routing) places and routes the elided circuit onto the target coupling graph, starting with initial placement $L_{\text{placement}}$ and ending with $L_{\text{router\_final}}$.
5. The full compilation layout is formed by composing the router's layout with the elided logical permutation:
   $$L_{\text{initial}}(q) = L_{\text{placement}}(q) \quad \forall q \in Q_{\text{log}}$$
   $$L_{\text{final}}(q) = L_{\text{router\_final}}(\pi_{\text{elided}}(q)) \quad \forall q \in Q_{\text{log}}$$
6. Unused physical ancillas for both initial and final layouts are preserved identically from the router:
   $$\text{unused\_physical\_initial} = \text{router\_unused\_initial}$$
   $$\text{unused\_physical\_final} = \text{router\_unused\_final}$$

---

### 2. Exact Permutation Semantics & Composition Law

Let:
- $Q_{\text{log}} = \{0, 1, \dots, n-1\}$ be the set of logical program qubits.
- $W = \{0, 1, \dots, n-1\}$ be the set of virtual circuit wires.
- $Q_{\text{phys}} = \{0, 1, \dots, m-1\}$ be the set of physical target device qubits ($m \ge n$).

1. **Initial Wire State**:
   $$\pi_0(q) = q \quad \forall q \in Q_{\text{log}}$$
2. **Sequential Wire Update**:
   When processing the DAG in deterministic topological order, encountering an operation $O$:
   - If $O = \text{swap}(q_a, q_b)$:
     $$w_a = \pi_{\text{curr}}(q_a), \quad w_b = \pi_{\text{curr}}(q_b)$$
     $$\pi_{\text{next}}(q_a) = w_b, \quad \pi_{\text{next}}(q_b) = w_a$$
     $$\pi_{\text{next}}(q) = \pi_{\text{curr}}(q) \quad \forall q \notin \{q_a, q_b\}$$
     Do not emit $O$ to the elided circuit. Increment `swaps_elided += 1`.
   - If $O$ is any other operation with quantum operands $(q_1, q_2, \dots)$:
     Emit $O$ with rewritten operands $(\pi_{\text{curr}}(q_1), \pi_{\text{curr}}(q_2), \dots)$.
3. **Terminal Permutation**:
   $$\pi_{\text{elided}}(q) = \pi_{\text{final\_step}}(q)$$
4. **Layout Composition**:
   Given the router's layout maps $L_{\text{place}}: W \to Q_{\text{phys}}$ and $L_{\text{router\_final}}: W \to Q_{\text{phys}}$:
   $$L_{\text{initial}}(q) = L_{\text{place}}(q)$$
   $$L_{\text{final}}(q) = L_{\text{router\_final}}(\pi_{\text{elided}}(q))$$

This composition law guarantees that the physical circuit output qubit holding logical qubit $q$ matches the oracle expectation $\mathcal{E}_{L_{\text{final}}}(U_{\text{log}} |\psi\rangle)$ with zero phase deviation ($\Delta \phi = 0.0$) and zero amplitude error ($\Delta < 10^{-15}$).

---

### 3. Edge-Case Handling & Invariant Preservation

1. **Swap Chains**:
   Consecutive swaps (e.g. `swap q[0], q[1]; swap q[1], q[2];`) compose associatively and deterministically from left to right:
   $$\pi_0 = [0, 1, 2] \xrightarrow{\text{swap}(0,1)} [1, 0, 2] \xrightarrow{\text{swap}(1,2)} [1, 2, 0]$$
   yielding $L_{\text{final}} = \{0: 1, 1: 2, 2: 0\}$.
2. **Mid-Circuit Swaps & Interspersed Gates**:
   Gates preceding a swap operate on the pre-swap wire assignments; gates succeeding a swap operate on the post-swap wire assignments.
3. **Measurement Operations (`measure q[i] -> c[j]`)**:
   Measurement of logical qubit $q_i$ reads the virtual wire currently carrying $q_i$ ($w = \pi_{\text{curr}}(q_i)$), and writes strictly to the declared classical bit $c_j$. The classical register mapping is invariant under quantum wire permutation.
4. **Synchronization Barriers (`barrier q[...]`)**:
   Barrier operands are mapped to their active virtual wires $(\pi_{\text{curr}}(q_k))$, maintaining strict execution barriers across the physical wires carrying those qubits.
5. **State Resets (`reset q[i]`)**:
   Reset applies to wire $\pi_{\text{curr}}(q_i)$, resetting logical qubit $q_i$ to $|0\rangle$.
6. **ADR-0001 Clean Ancilla Zero-State Policy**:
   Logical wire permutation operates strictly on the $n$ logical wires ($w < n$). Physical ancilla qubits ($p \in Q_{\text{phys}} \setminus \text{im}(L)$) are unmapped in $W$ and remain strictly in $|0\rangle$ at circuit entry and termination.
7. **Classical Hazards & Graph Invariants**:
   Rebuilding the elided circuit in QccIR constructs a fresh `CircuitGraph` with exact `WireAccessState` (RAW, WAR, WAW) on the virtual wires, guaranteeing no dangling petgraph indices or corrupted dependency edges.
8. **Pass Diagnostics (Milestone M8)**:
   Logical SWAP elision records a `PassDiagnosticReport` (`pass_id: "logical_swap_elision"`) in the manifest / pass report conforming to `qcc.pass_diagnostic v1.0.0`:
   - `status: Success` when swaps are elided.
   - `status: Skipped` when `elide_swaps` is false or the circuit contains 0 logical swaps.
   - `operations_removed: elided_swaps_count`.

---

## Verification & Fault-Injection Plan

1. **Unit Tests (`tests/optimizer/logical_swap_elision.rs`)**:
   - Single terminal swap (`04_logical_swap` equivalent).
   - Multi-swap chains (`swap(0, 1)`, `swap(1, 2)`, `swap(2, 3)`).
   - Mid-circuit interleaved gates ($H - \text{SWAP} - X$).
   - Interleaved measurements, resets, and barriers.
   - Idempotence: running elision on an already-elided circuit yields 0 additional elisions.
2. **Property-Based Testing (`proptest`)**:
   - Random circuits generated with varying widths ($2..6$ qubits), random gate sequences, and dense random logical SWAP gates ($1..10$ swaps).
   - Assert that for 100% of generated circuits:
     $$\text{simulate}(C_{\text{elided}}, L_{\text{final}}) \equiv \text{simulate}(C_{\text{orig}}, L_{\text{init}})$$
3. **Full Oracle Regression**:
   - Re-run independent oracle verification across all 18 Phase 70 workloads with `--elide-swaps` enabled.
   - Verify that all 7 logical SWAP workloads pass oracle verification with 0 mismatches while reducing emitted CX count by exactly **66 CX**.
4. **Mandatory Fault Injection**:
   - Implement an automated negative test where $\pi_{\text{elided}}$ is corrupted or omitted from `final_layout` ($L_{\text{final}}$ forced equal to $L_{\text{initial}}$).
   - Assert that both `LayoutEquivalenceChecker` and `tools/independent_oracle/` fail closed with error $> 1.0$.
