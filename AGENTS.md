# QCC Engineering Agent Instructions

Welcome to the **QCC (Quantum Circuit Compiler)** codebase.

QCC is a high-rigor Rust compiler engineering project. **Phases 1 through 59 (Milestones M1–M6)** are completed historical baseline work preserved at the **Phase 59 integrated compilation pipeline checkpoint**. The previous 105-phase roadmap (old Phases 60–105) is **superseded** and must not be used to select future work.

The active engineering plan is **Roadmap v3.1** (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap_v3.1.docx`, 12 November 2026), covering **Phases 60–85 (Milestones M7–M12)**:
- **M7 — Independent Verification Oracle v0.2 (Phases 60–70)** *(First priority; executed in 5 implementation sessions)*
- **M8 — Compiler Introspection and Diagnostics (Phases 71–73)**
- **M9 — Multi-Candidate Routing Improvements (Phases 74–77)**
- **M10 — Evidence-Selected Optimizer Improvements (Phases 78–81)**
- **Optional M11 — Bounded Hardware Profiles (Phases 82–84; optional, requires explicit go/no-go review after M10)**
- **M12 — Integrated Upgrade Acceptance (Phase 85)**

**Default Next Task**: When asked to continue QCC, the default next task is **M7 Session 1 — Contract and manifest-first Rust work** (not Phase 60 interpreted in isolation, and never a continuation of the superseded 105-phase roadmap).

Every future agent session must treat this document and the associated documentation in `docs/` and `.agents/` as the authoritative source of truth.

---

## Mandatory Roadmap Governance Rules (Prevent Roadmap Drift)

1. **v3.1 Is Authoritative**: The v3.1 roadmap is the authoritative engineering plan for all work beyond Phase 59.
2. **Old 105-Phase Roadmap Is Superseded**: The old 105-phase roadmap (old Phases 60–105) is superseded and must not be used to select future tasks.
3. **No Invented Scope**: Do not invent phases, milestones, features, or parallel project directions.
4. **One Defined Unit at a Time**: Work on one defined task/session at a time (M7 uses **5 sessions** rather than 11 isolated commits; retain Phase IDs 60–70 for traceability).
5. **Check Active Plan Before Acting**: Before proposing or implementing a task, check it against the active v3.1 roadmap (`docs/roadmap/phases-060-085.md`) and the current checkpoint (`docs/project-state/progress.md`).
6. **Never Skip Acceptance Gates**: Do not proceed past a session/phase boundary without verified exit-gate evidence.
7. **M7/v0.2 First**: Do not begin M8 (or routing, optimizer, or hardware profile work) before **M7/v0.2** has passed its acceptance gate.
8. **Evidence-Selected Optimization**: Do not begin M10 optimizer improvements before M7/M9 benchmark evidence identifies a concrete gap.
9. **Optional M11 Go/No-Go**: M11 bounded hardware profiles are optional and require a documented go/no-go decision after M10.
10. **Explicitly Deferred Scope**: Noise simulation, density-matrix simulation, dynamic CFG execution (mid-circuit measurement collapse/trajectories), KAK, MPS, and remote provider adapters remain deferred unless the user explicitly changes the project direction.
11. **Preserve Established Contracts**: Do not modify established architecture contracts or historical decisions (Phases 1–59, ADR-0001, ADR-0002) without explicit review.
12. **No Unrequested Pushes**: Do not push commits unless the user explicitly requests it.

---

## Core Engineering & Verification Rules

1. **Read Contracts Before Implementation**: Before touching code, read the relevant architecture specifications in [`docs/`](./docs/):
   - [`00-project-charter.md`](./docs/00-project-charter.md) — Scope boundaries, v3.1 release horizons, and deferred tracks.
   - [`01-architecture-overview.md`](./docs/01-architecture-overview.md) — Subsystems, tooling layout, and dependency policy.
   - [`02-formal-semantics-and-contracts.md`](./docs/02-formal-semantics-and-contracts.md) — Little-endian qubit indexing, Dirac notation, matrix conventions, global phase, and wire effects.
   - [`03-compilation-pipeline.md`](./docs/03-compilation-pipeline.md) — Pipeline order and pass manager contracts.
   - [`04-correctness-and-verification.md`](./docs/04-correctness-and-verification.md) — Unitary equivalence with one common global phase and independent oracle architecture.
   - [`05-target-routing-and-layout.md`](./docs/05-target-routing-and-layout.md) — Target models, layouts, baseline router vs SABRE, and ADR-0001.
   - [`06-simulator-design.md`](./docs/06-simulator-design.md) — In-place $\mathcal{O}(2^n)$ kernels; no dense $2^n \times 2^n$ operators.
   - [`07-milestones-and-critical-path.md`](./docs/07-milestones-and-critical-path.md) — Active v3.1 milestones, critical path, and M7 5-session plan.
   - [`08-risks-and-deferred-scope.md`](./docs/08-risks-and-deferred-scope.md) — Risk register, deferred scope, and accepted continuation decisions.
2. **Check the Progress Ledger**: Inspect [`docs/project-state/progress.md`](./docs/project-state/progress.md) to verify completed prerequisites and active session status.
3. **Never Bypass Prerequisites**: Never start a session or phase before its explicit prerequisites are verified as completed.
4. **Preserve All Non-Negotiable Invariants**: Always follow [`.agents/rules/qcc-project-invariants.md`](./.agents/rules/qcc-project-invariants.md):
   - Stable Rust only (zero nightly flags).
   - Little-endian qubit indexing ($q_0$ is LSB).
   - Column vectors and left multiplication ($B \cdot A$).
   - CX: operand 0 = control, operand 1 = target.
   - Explicit circuit global phase preservation.
   - Classical wire hazards (RAW, WAR, WAW) via `WireAccessState`.
   - ADR-0001 strict ancilla zero-state policy (full physical statevector check, no tracing out).
   - Never call QccIR "Microsoft QIR".
   - Qiskit/Python belong in `tools/independent_oracle/` as optional dev/CI tooling only; never add Qiskit/Python as a `qcc-core` or normal Rust runtime dependency.
5. **Write Tests and Documentation**: Every session/phase requires automated tests (unit, property, golden, fault-injection where applicable) and rustdoc comments on public items.
6. **Run Verification Commands**: Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` (plus oracle checks once M7 tooling is added). All must pass with zero errors and zero warnings.
7. **Update Progress Ledger Only With Evidence**: Update [`docs/project-state/progress.md`](./docs/project-state/progress.md) only after test output proves the session/phase exit gate is met.
8. **Report Incomplete Work Honestly**: If an exit gate fails, a correctness-critical bug surfaces, or a dependency is unresolved, stop and report it candidly.
9. **Follow the Standard Execution Workflow**: Always follow [`.agents/workflows/qcc-phase-execution.md`](./.agents/workflows/qcc-phase-execution.md).

---

## M7 (v0.2) Mandatory Implementation Decisions

- **A. Manifest-first Rust work**: The first Rust deliverable in M7 (Session 1) is a versioned, machine-readable compilation manifest generated directly from actual compiler result data (`CompilationResult`) — never parsed from stdout — exposing logical/physical qubit counts, explicit logical-to-physical initial and final layouts, target identity/configuration, global phase, compiler/build identity, emitted physical circuit identity/path, gate counts, CX/SWAP distinctions, depth, and comparison metadata. Inspect existing CLI/compiler code first and make an additive change.
- **B. Independent Qiskit oracle**: Lives in `tools/independent_oracle/` (dev/CI only; never a `qcc-core` or normal runtime dependency). Must independently construct the canonical logical reference, run QCC black-box, run independent Qiskit simulation, normalize layout/endianness, validate the full physical state including unused ancillas, and preserve reproducible failure artifacts. Never call QCC's own equivalence checker to establish oracle pass/fail, and keep Qiskit's reference simulation separate from Qiskit's transpiler cost baseline.
- **C. Real QCC OpenQASM compatibility**: In Session 2, test whether the pinned Qiskit OpenQASM 3 importer (`qiskit-qasm3-import`) actually imports and simulates real QCC-emitted physical QASM (`rz`, `sx`, `cx`, `gphase`, parameter expressions). Never assume compatibility; use a documented semantics-preserving fallback or classify unsupported — never silently drop phase or operations.
- **D. Global-phase verification**: Common-global-phase semantic equivalence (one common phase across the whole comparison) remains authoritative. A separate informational strict phase-accounting diagnostic independently reconstructs synthesis/decomposition phase contributions and reports `PASS`, `MISMATCH`, or `UNVERIFIED` without overriding semantic equivalence.
- **E. Clean ancilla policy**: Every physical qubit outside the final layout image must return to $|0\rangle$ (ADR-0001). Compare the full $2^{n_{\text{phys}}}$ physical state; never trace out spare qubits.
- **F. Fault injection**: Validate the oracle against injected bugs (bad initial/final layout, endianness errors, corrupted manifest phase, wrong physical gates/CX orientation, residual ancilla population/entanglement, unsupported features).
- **G. Difficult deterministic corpus**: Scale to 1,000+ cases including deep circuits, long-range/non-adjacent interactions, linear and ring topologies, dense high-SWAP workloads, multiple initial layouts, reversed CX directions, logical SWAPs, and targets with spare physical qubits.
- **H. Fair Qiskit comparison**: Align basis, coupling, directionality, width, and gate subset. Run the same declared trial count $N$ on both sides, record individual seeds, configs, best/median/worst, spread, wall-clock compile time, gates/CX/SWAPs/depth, document Qiskit's internal randomized trials, disclose unequal compute budgets, and publish losses honestly without claiming global optimality.

---

## Directory Quick Reference

- **Active Roadmap Continuation (v3.1, Phases 60–85)**: [`docs/roadmap/phases-060-085.md`](./docs/roadmap/phases-060-085.md) and [`docs/roadmap/phase-index.md`](./docs/roadmap/phase-index.md).
- **Preserved Baseline Roadmap (Phases 1–59)**: [`docs/roadmap/README.md`](./docs/roadmap/README.md) (`phases-001-011.md` through `phases-054-059.md`).
- **Historical v1 Spec (Phases 1–59 Baseline Only)**: [`docs/source/original-roadmap.docx`](./docs/source/original-roadmap.docx) (superseded beyond Phase 59 by Roadmap v3.1).
- **Core Architecture Docs**: [`docs/00-project-charter.md`](./docs/00-project-charter.md) through [`docs/09-glossary.md`](./docs/09-glossary.md).
- **Architecture Decision Records**: [`docs/decisions/ADR-0001-ancilla-zero-state-policy.md`](./docs/decisions/ADR-0001-ancilla-zero-state-policy.md), [`docs/decisions/ADR-0002-qasm-parser.md`](./docs/decisions/ADR-0002-qasm-parser.md).
- **Implementation Progress Ledger**: [`docs/project-state/progress.md`](./docs/project-state/progress.md).
- **Workspace Invariants Rule**: [`.agents/rules/qcc-project-invariants.md`](./.agents/rules/qcc-project-invariants.md).
- **Phase & Session Execution Workflow**: [`.agents/workflows/qcc-phase-execution.md`](./.agents/workflows/qcc-phase-execution.md).
