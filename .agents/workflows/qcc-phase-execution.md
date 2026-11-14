---
description: Step-by-step workflow for implementing and verifying QCC Roadmap v3.1 sessions and phases
---

# QCC Phase & Session Execution Workflow

> [!NOTE]
> **Active Engineering Plan (Roadmap v3.1)**:
> Phases 1–59 (Milestones M1–M6) are preserved at the **Phase 59 checkpoint**. The previous 105-phase roadmap (old Phases 60–105) is **superseded**.
> Active continuation follows **Roadmap v3.1 (Phases 60–85, Milestones M7–M12)**, starting with **M7 — Independent Verification Oracle v0.2** executed in **five implementation sessions**.
>
> **Default Next Task**: **M7 Session 1 — Contract and manifest-first Rust work**.

This workflow governs the execution of any implementation session or phase in the **QCC (Quantum Circuit Compiler)** project. Future agent sessions must execute these steps sequentially and adhere strictly to milestone, session, and phase boundaries.

---

## M7 Five-Session Execution Model (Phases 60–70)

For **Milestone M7 (Independent Verification Oracle v0.2)**, Phase IDs `60`–`70` are retained for traceability and acceptance checklists, but implementation and review proceed as **five practical sessions** (not 11 isolated commits):

1. **Session 1 — Contract and manifest-first Rust work** *(Phases 60 & 63 manifest deliverable)*:
   - Inspect actual Phase 59 compiler result types, CLI, layout types, targets, and CI.
   - Finalize the oracle ADR and versioned manifest schema (explicitly defining logical-to-physical mapping direction).
   - Implement versioned machine-readable compilation manifest emission as the first Rust code change, generated directly from actual compiler result data (`CompilationResult`) — never parsed from terminal output.
2. **Session 2 — Pinned environment and real-QCC-QASM compatibility** *(Phases 61 & 63 QASM-import validation)*:
   - Pin Python, `qiskit`, and `qiskit-qasm3-import` in `tools/independent_oracle/` (keeping `qcc-core` and normal Rust execution 100% Python-free).
   - Import and simulate actual QCC-emitted physical OpenQASM 3 circuits (`rz`, `sx`, `cx`, `gphase`, parameter expressions). Document exact compatibility; use documented semantics-preserving fallbacks or classify unsupported — never silently drop phase or operations.
3. **Session 3 — Canonical cases, independent runners and semantic comparator** *(Phases 62–66)*:
   - Build neutral canonical cases and separate independent Qiskit/QCC adapters.
   - Implement black-box QCC execution, endianness/layout normalization, common-global-phase semantic comparison, full physical clean-ancilla verification (ADR-0001; no tracing out spare qubits), and the separate informational strict phase-accounting diagnostic (`PASS` / `MISMATCH` / `UNVERIFIED`).
4. **Session 4 — Fault injection, difficult corpus and fair Qiskit baseline** *(Phases 67, 68, 70 baseline)*:
   - Validate the oracle against positive fixtures and injected faults (bad layouts, endian mistakes, corrupted manifest phase, wrong physical gates/CX orientation, residual ancilla population/entanglement, unsupported non-unitary features).
   - Scale to 1,000+ deterministic cases (deep circuits, non-adjacent interactions, linear/ring topologies, dense high-SWAP workloads, multiple initial layouts, reversed CX directions, logical SWAPs, spare physical qubits).
   - Run aligned multi-trial QCC vs. Qiskit comparisons with equal declared trial counts $N$, recording seeds, configs, best/median/worst, spread, wall-clock compile time, and Qiskit internal randomized trial disclosures.
5. **Session 5 — CI integration, reproducible results and v0.2 acceptance** *(Phases 69 & 70 acceptance)*:
   - Integrate deterministic PR smoke/regression checks and broader seeded main/release/scheduled CI runs with failure artifact uploads.
   - Publish reproducible machine-readable and README results (including losses honestly), complete the v0.2 acceptance report, and stop before M8 until v0.2 acceptance is signed off.

---

## Step 1: Identify Active Unit & Verify Governance Boundaries

1. Confirm the active unit requested by the user. If the user asks to continue QCC without specifying a unit, check [`docs/project-state/progress.md`](../../docs/project-state/progress.md); at the Phase 59 checkpoint, the default next task is **M7 Session 1**.
2. Locate the specification in `docs/roadmap/`:
   - Active continuation (Phases 60–85 / M7–M12): [`docs/roadmap/phases-060-085.md`](../../docs/roadmap/phases-060-085.md)
   - Preserved historical baseline (Phases 1–59 / M1–M6):
     - Phases 1–11: [`docs/roadmap/phases-001-011.md`](../../docs/roadmap/phases-001-011.md)
     - Phases 12–27: [`docs/roadmap/phases-012-027.md`](../../docs/roadmap/phases-012-027.md)
     - Phases 28–36: [`docs/roadmap/phases-028-036.md`](../../docs/roadmap/phases-028-036.md)
     - Phases 37–43: [`docs/roadmap/phases-037-043.md`](../../docs/roadmap/phases-037-043.md)
     - Phases 44–53: [`docs/roadmap/phases-044-053.md`](../../docs/roadmap/phases-044-053.md)
     - Phases 54–59: [`docs/roadmap/phases-054-059.md`](../../docs/roadmap/phases-054-059.md)
3. Enforce roadmap sequencing guards:
   - Never use the superseded 105-phase roadmap (`docs/source/original-roadmap.docx` Phases 60–105).
   - Do not begin **M8** until **M7/v0.2** has passed its acceptance gate.
   - Do not begin **M10** optimizer improvements before M7/M9 benchmark evidence identifies a concrete gap.
   - Do not begin **Optional M11** without an explicit post-M10 go/no-go review.
   - Never introduce deferred features (noise/density-matrix simulation, dynamic CFG execution, KAK, MPS, provider adapters) unless explicitly requested by the user.
4. Read every field of the relevant phase/session specification: Objective, Prerequisites, Ordered Tasks, Files, Tests, Definition of Done, Failure Modes, and Exit Gate.

---

## Step 2: Validate Prerequisites & Exit Gates

1. Open [`docs/project-state/progress.md`](../../docs/project-state/progress.md).
2. Verify that all prerequisite phases and prior sessions are marked **Completed** with verified exit-gate evidence.
3. **Hard Stop Condition**: If any prerequisite has not passed its exit gate, **halt execution immediately** and report the dependency blocker to the user.

---

## Step 3: Inspect Workspace State & Existing Interfaces

1. Inspect the existing source tree (`src/`) and tests (`tests/`) directly before designing changes — never assume a CLI flag, manifest option, or API already exists.
2. Check existing `tools/` and `.github/workflows/` if applicable.
3. Verify that the working directory is clean (`git status --short`).

---

## Step 4: Prepare Bounded Implementation Plan

1. Using [`docs/templates/phase-plan.md`](../../docs/templates/phase-plan.md), outline the concrete tasks for the single active session or phase.
2. Define exact files to create/modify and tests to write.
3. Verify the plan against [`.agents/rules/qcc-project-invariants.md`](../rules/qcc-project-invariants.md) and [`AGENTS.md`](../../AGENTS.md).
4. Keep scope strictly bounded to the active session or phase.

---

## Step 5: Execute Implementation Tasks

1. Follow the ordered tasks sequentially.
2. Stop immediately if a correctness-critical issue or unresolved external dependency (such as Qiskit OpenQASM 3 importer incompatibility) is discovered; resolve or document/classify it transparently rather than hiding semantic loss.
3. Use stable Rust only (`#![feature(...)]` is prohibited).
4. Respect all mathematical, QccIR, Target/Layout/ADR-0001, Simulator, and M7 Oracle invariants.

---

## Step 6: Implement Required Tests

1. Add all unit, integration, property, and (in M7+) fault-injection / corpus tests required by the session or phase.
2. Assert exact mathematical semantics (full statevectors up to one common global phase, layout permutations, clean ancillas), never histogram-only checks.

---

## Step 7: Run Mandatory Verification Pipeline

Execute the standard Rust verification checks (and oracle checks once `tools/independent_oracle/` exists):

```bash
# 1. Formatting check
cargo fmt --check

# 2. Linter / Clippy with zero warnings
cargo clippy --all-targets -- -D warnings

# 3. Test execution
cargo test

# 4. Documentation build
cargo doc --no-deps
```

All commands must exit cleanly with code `0`. Note that Rust-only builds and `cargo test` must continue to pass without requiring Python or Qiskit.

---

## Step 8: Regression Verification

1. Confirm all previously implemented suites pass with zero regressions.
2. If oracle tooling is active, verify no unexplained semantic equivalence failures exist.

---

## Step 9: Verify Exit Gate

1. Compare actual command and test outputs directly against the **Explicit Exit Gate** of the session/phase.
2. If criteria are not 100% satisfied:
   - The session/phase is **NOT COMPLETE**.
   - Do not mark it completed or advance to the next unit.
   - Debug and resolve the failure (or stop and report blockers candidly).

---

## Step 10: Report, Update Progress Ledger & Checkpoint

1. Draft a completion report using [`docs/templates/phase-completion-report.md`](../../docs/templates/phase-completion-report.md) and save it to `docs/project-state/reports/` (e.g., `milestone-mXX.md`).
2. Update [`docs/project-state/progress.md`](../../docs/project-state/progress.md) only with verified command/test evidence.
3. Create a meaningful local Git checkpoint after the session/phase exit gate passes (do not manufacture commits for tiny sub-items; **never push unless explicitly requested by the user**).
4. Report completion honestly to the user with exact test results, exit-gate confirmation, and the next authorized unit.
