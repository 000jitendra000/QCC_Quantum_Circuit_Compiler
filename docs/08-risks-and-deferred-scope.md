# QCC Risk Register, Deferred Scope, and Accepted Decisions (Roadmap v3.1)

## 1. Project Risk Register

> [!NOTE]
> **Active Roadmap Status (v3.1 — 12 November 2026)**:
> Phases 1–59 (Milestones M1–M6) are preserved at the Phase 59 checkpoint. The previous 105-phase roadmap (old Phases 60–105) is superseded by **Roadmap v3.1 (Phases 60–85, Milestones M7–M12)**, beginning with **M7 Session 1**.

The following register identifies the highest-severity failure modes across QCC's compiler core and active v3.1 continuation, together with mandatory mitigations:

| Risk Category | Severity | Potential Failure Mode | Mandatory Mitigation Strategy |
|---|:---:|---|---|
| **Oracle Shares QCC Logic or Bugs** | **Very High** | Deriving Qiskit references from QCC output or calling QCC's equivalence checker hides shared compiler/simulator defects. | Build Qiskit references independently from canonical fixtures; never call QCC verification code from the oracle; validate with known-good and injected-bad fault suites. |
| **Endian & Operand-Order Drift** | **Very High** | Inverting little-endian convention ($q_0$ as LSB) or swapping control/target operands creates silent state inversion. | Keep $q_0$-LSB contract; implement explicit tested adapter conversions; test basis-state permutations and asymmetric $CX$ cases. |
| **Incorrect Initial/Final Layout Interpretation** | **Very High** | Confusing logical-to-physical vs physical-to-logical direction or assuming $L_{\text{initial}} = L_{\text{final}}$ breaks routed equivalence. | Emit explicit directional initial and final layouts in the versioned compilation manifest from `CompilationResult`; verify with nontrivial routing permutations. |
| **Global-Phase or Relative-Phase Loss** | **Very High** | Choosing per-input phases or dropping intermediate synthesis phase masks relative-phase bugs. | Estimate one common phase for the complete semantic comparison; never fit a separate phase per input state. |
| **Manifest Global-Phase Metadata Wrong While Equivalence Passes** | **High** | Semantic equivalence ignores common global phase, potentially masking corrupted `gphase` metadata or accounting errors. | Keep common-phase semantic equivalence authoritative; add a separate informational strict phase-accounting diagnostic with a fixed convention and independently reconstructed synthesis/decomposition contributions (`PASS` / `MISMATCH` / `UNVERIFIED`). |
| **Ancilla Garbage Hidden by Partial Trace** | **Very High** | Tracing out unused physical qubits hides residual population or entanglement on final spare qubits. | Enforce ADR-0001: compare full $2^{n_{\text{phys}}}$ physical statevectors; all qubits outside the final layout image must return to $|0\rangle$. |
| **Qiskit OpenQASM 3 Importer Rejection / Partial Coverage** | **Very High** | Pinned `qiskit-qasm3-import` (marked exploratory with partial `gphase` support) may reject or misrepresent real QCC-emitted `gphase` or parameter expressions. | Pin `qiskit` and `qiskit-qasm3-import`; test actual QCC-emitted output in **M7 Session 2**; use only documented semantics-preserving fallbacks or classify unsupported — never silently strip phase or expressions. |
| **Qiskit API / Version Drift** | **High** | Unpinned Qiskit/Python dependencies change importer or transpiler behavior silently across environments. | Pin exact dependencies in `tools/independent_oracle/requirements*`; record versions in every artifact; rerun oracle self-tests on updates. |
| **Flaky Random Tests / Irreproducible Failures** | **High** | Unseeded generation or discarded failing circuits prevents debugging and regression creation. | Record seeds and case hashes; persist complete failure artifacts (canonical case, QCC input/output, layouts, phase, versions); maintain deterministic PR suite. |
| **Biased / Misleading QCC-vs-Qiskit Metrics** | **High** | Differing bases/topologies, conflated SWAP metrics, or unequal internal Qiskit trial budgets distort comparisons. | Align basis, coupling, directionality, width, and gate subset; run matched trial count $N$; record individual seeds, configs, best/median/worst, spread, wall-clock time; document Qiskit's internal trials and publish losses honestly. |
| **DAG Dependency Corruption** | **Very High** | Missing classical RAW/WAR/WAW hazards or barrier ordering edges allows invalid parallel execution. | Typed `WireDependency` edges, per-wire `WireAccessState`, and centralized transactional DAG rewriters. |
| **Routing Search Compile-Time Overhead** | **Medium–High** | Multi-seed/layout routing search (M9) grows compile time excessively if unbounded. | Bound seeds/layouts and runtime budgets explicitly; retain single-candidate baseline fallback. |
| **Optimizer Metric Trade-Off or Semantic Regression** | **Very High** | An optimizer pass improves one metric while harming semantics or other circuit metrics. | Select M10 work from oracle/benchmark gaps; require formal spec + negative tests + zero unexplained oracle failures before enabling by default. |
| **Missing Hardware Data Mistaken for Zero** | **High** | Treating absent timing or calibration metadata as `0` distorts scheduling and cost models. | Explicit Unknown rule: model missing data as `None` (`Unknown`) with provenance/timestamps; never fabricate durations or calibration values. |
| **Scope Creep into Deferred Tracks** | **High** | Expanding into dynamic trajectories, noise simulation, or remote provider adapters derails v0.2 and core verification rigor. | Enforce Roadmap v3.1 boundaries: keep noise/density-matrix simulation, dynamic CFG execution, KAK, MPS, and provider adapters outside Phases 60–85 unless explicitly reopened. |

---

## 2. Deferred Features and Revisit Triggers

| Deferred Capability | Rationale for Deferral (Roadmap v3.1) | Explicit Revisit Trigger |
|---|---|---|
| **Mid-Circuit Measurement, Dynamic CFG & Trajectory Simulation** | Active continuation prioritizes static-unitary compilation correctness (M7–M12) and does not expand the simulator domain. | A concrete dynamic-circuit product requirement and a suitable non-unitary behavioral oracle are agreed. |
| **Noise & Density-Matrix Simulation** | Separate simulation domain with $\mathcal{O}(4^n)$ cost; does not verify static compiler correctness. | A separate simulation objective, supported channels, and validated noise data are defined. |
| **Error-Aware Routing / Optimization** | Trustworthy device calibration data and freshness provenance are not currently assumed. | A trusted calibration source, freshness policy, and uncertainty model are available. |
| **Remote Provider Adapters & Credential Lifecycle** | Not required for compiler verification or planned routing/optimizer/profile work. | A specific provider/device target and supported execution workflow are selected. |
| **Standalone Public Verify Tool** | Keeping the oracle in-repository and CI-coupled prevents drift from QCC output and target contracts. | The in-repository oracle is stable and external user demand justifies a standalone tool. |
| **KAK, MPS, GPU/Distributed Simulation & Advanced Research Optimizers** | Large independent research tracks; M10 selects one optimizer improvement from observed benchmark gaps first. | A measured limitation or explicit research question justifies a bounded proposal. |
| **Custom `.qcc` DSL & Full Classical Control Flow** | Existing Rust `CircuitBuilder` and supported OpenQASM 3 subset cover the active validation path. | A concrete frontend limitation or dynamic control-flow goal is accepted. |

---

## 3. Accepted Continuation & Architectural Review Decisions

1. **Preserved Checkpoint & Superseded v1 Tail**: Phases 1–59 are preserved as the QCC implementation baseline; the previous Phases 60–105 plan is superseded by **Roadmap v3.1 (Phases 60–85)**.
2. **In-Repo Optional Oracle & Manifest-First Rust Deliverable**: The independent Qiskit oracle lives in `tools/independent_oracle/` for dev/CI only (never a `qcc-core` or runtime dependency). M7's first Rust deliverable is a versioned compile manifest from actual compiler results containing target, widths, explicit logical-to-physical initial/final layouts, global phase, compiler identity, output circuit identity, and metrics.
3. **Separate Semantic Truth vs Cost Baseline**: The oracle builds Qiskit logical references directly from canonical cases and never calls QCC's own equivalence checker. Qiskit's transpiler output is strictly a separate cost/quality baseline.
4. **Strict Clean-Ancilla Routed Verification (ADR-0001)**: Embed logical input through $L_{\text{initial}}$, simulate full emitted physical output, embed logical reference output through $L_{\text{final}}$, and require every qubit outside $L_{\text{final}}$ image to return to $|0\rangle$ — never trace out spare qubits.
5. **Single Common Global Phase + Informational Strict Phase Accounting**: Semantic equivalence uses one common global phase for the whole comparison. A separate informational strict phase-accounting diagnostic independently reconstructs synthesis/decomposition contributions (`PASS` only with complete evidence, otherwise `MISMATCH` or `UNVERIFIED`).
6. **Tiered Oracle CI & Reproducible Failure Artifacts**: Deterministic smoke/regression gate on pull requests and larger seeded suite on main/release/scheduled runs; failures preserve all inputs, outputs, layouts, target configs, versions, phase, and seeds.
7. **Fair Matched-Trial QCC vs Qiskit Comparisons**: Align native basis, coupling, directionality, physical width, and gate subset. Run matched declared trials $N$ on both sides, record seeds/configs/best/median/worst/spread/compile time, disclose Qiskit's internal randomized trials, separate logical/router-inserted/final SWAP metrics, and publish losses honestly.
8. **Truthful Introspection Reports (M8)**: Per-pass reports distinguish `passed`, `failed`, `not_run`, and `unsupported` verification; never claim equivalence without executed evidence.
9. **Bounded Deterministic Routing Search (M9)**: Search over seeds/layouts is bounded and deterministic under declared objectives; never claim global optimality.
10. **Evidence-Selected Optimization (M10)**: Select one bounded optimizer improvement only after oracle/routing benchmarks expose a concrete gap.
11. **Optional Hardware Profiles (M11)**: Reuse existing target, directionality, connectivity, and timing contracts; keep unknowns explicit (`None`); execute only after a post-M10 go/no-go decision (not a v0.2 blocker).
12. **Honest Scope & Release Claims**: QCC remains a personal, learning-oriented compiler with a bounded feature set. Passing the oracle validates only the tested semantic domain, circuits, targets, versions, and tolerances — it is not formal proof, production certification, or hardware calibration readiness.
13. **Core Architectural Invariants Retained**: DAG per basic block (`QccIR`, distinct from Microsoft QIR), RAW/WAR/WAW classical wire hazards (`WireAccessState`), `CircuitBuilder` + OpenQASM 3 subset, in-place $\mathcal{O}(2^n)$ statevector kernels (no dense $2^n \times 2^n$ operators), terminal sampling, and stable Rust toolchain only.
