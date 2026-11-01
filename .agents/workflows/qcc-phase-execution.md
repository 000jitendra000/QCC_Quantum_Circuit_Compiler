---
description: Step-by-step workflow for implementing and verifying a single QCC roadmap phase
---

# QCC Phase Execution Workflow

This workflow governs the execution of any implementation phase (Phases 1 through 105) in the **QCC (Quantum Circuit Compiler)** project.

Future agent sessions must execute these steps sequentially and adhere strictly to phase boundaries.

---

## Step 1: Identify Phase & Ingest Specification

1. Confirm the exact phase number requested by the user.
2. Locate the phase specification in `docs/roadmap/`:
   - Phases 1–11: [`docs/roadmap/phases-001-011.md`](../../docs/roadmap/phases-001-011.md)
   - Phases 12–27: [`docs/roadmap/phases-012-027.md`](../../docs/roadmap/phases-012-027.md)
   - Phases 28–36: [`docs/roadmap/phases-028-036.md`](../../docs/roadmap/phases-028-036.md)
   - Phases 37–43: [`docs/roadmap/phases-037-043.md`](../../docs/roadmap/phases-037-043.md)
   - Phases 44–53: [`docs/roadmap/phases-044-053.md`](../../docs/roadmap/phases-044-053.md)
   - Phases 54–59: [`docs/roadmap/phases-054-059.md`](../../docs/roadmap/phases-054-059.md)
   - Phases 60–67: [`docs/roadmap/phases-060-067.md`](../../docs/roadmap/phases-060-067.md)
   - Phases 68–82: [`docs/roadmap/phases-068-082.md`](../../docs/roadmap/phases-068-082.md)
   - Phases 83–89: [`docs/roadmap/phases-083-089.md`](../../docs/roadmap/phases-083-089.md)
   - Phases 90–105: [`docs/roadmap/phases-090-105.md`](../../docs/roadmap/phases-090-105.md)
3. Thoroughly read all 13 fields of the phase specification: Objective, Background, Prerequisites, Ordered Tasks, Files, Tests, DoD, Failure Modes, Effort, and Exit Gate.

---

## Step 2: Validate Prerequisites & Exit Gates

1. Open [`docs/project-state/progress.md`](../../docs/project-state/progress.md).
2. Check every prerequisite phase listed in Field 3 of the phase specification.
3. Verify that all prerequisites are marked **Completed** with verified exit gate evidence in the ledger.
4. **Hard Stop Condition**: If any prerequisite phase has not passed its exit gate, **halt execution immediately**. Report the dependency blocker to the user. Do not proceed with an unblocked dependent phase.

---

## Step 3: Inspect Workspace State & Source Tree

1. Inspect the existing source directory (`src/` or workspace crates).
2. Review existing unit and integration tests.
3. Verify that the working directory is clean (`git status --short`).

---

## Step 4: Prepare Bounded Implementation Plan

1. Using [`docs/templates/phase-plan.md`](../../docs/templates/phase-plan.md), outline the concrete implementation tasks.
2. Define the exact files to create, files to modify, and tests to write.
3. Review the plan against [`.agents/rules/qcc-project-invariants.md`](../rules/qcc-project-invariants.md) to ensure no architectural conventions or mathematical semantics are breached.
4. Keep scope strictly bounded: implement **only** what the current phase requires.

---

## Step 5: Execute Implementation Tasks

1. Follow the ordered implementation tasks sequentially.
2. Implement typed error variants in `errors.rs` rather than panicking or using generic strings.
3. Use stable Rust only. Do not enable `#![feature(...)]` flags.
4. Respect all mathematical invariants:
   - Little-endian indexing ($q_0$ is LSB).
   - Column-vector multiplication order ($B \cdot A$).
   - Explicit circuit global phase preservation.
   - RAW, WAR, WAW dependencies on classical wires.
   - Ancilla zero-state policy (ADR-0001).

---

## Step 6: Implement Required Tests

1. Add all unit tests specified in Field 6 of the phase specification.
2. Add negative tests, edge case tests, and property-based tests as required.
3. Ensure tests assert exact mathematical semantics (e.g., statevector amplitudes or verified unitary matrices), not just histogram probabilities.

---

## Step 7: Run Mandatory Verification Pipeline

Execute the standard Rust quality and verification checks:

```bash
# 1. Formatting check
cargo fmt --check

# 2. Linter / Clippy with zero warnings
cargo clippy --all-targets -- -D warnings

# 3. Test execution
cargo test

# 4. Check documentation builds
cargo doc --no-deps
```

All commands must exit cleanly with code `0`.

---

## Step 8: Regression Verification

1. Run the entire test suite across all previously implemented modules.
2. Confirm that zero regressions were introduced.

---

## Step 9: Verify Exit Gate

1. Compare test output directly against Field 9 (**Explicit Exit Gate**) of the phase specification.
2. If the exit gate criteria are not 100% satisfied:
   - The phase is **NOT COMPLETE**.
   - Do not mark the phase completed.
   - Debug and resolve the failure.

---

## Step 10: Produce Completion Report & Update Progress Ledger

1. Draft a completion report using [`docs/templates/phase-completion-report.md`](../../docs/templates/phase-completion-report.md) and save it to `docs/project-state/reports/phase-XXX.md`.
2. Update [`docs/project-state/progress.md`](../../docs/project-state/progress.md):
   - Update the top summary checkpoint.
   - Mark Phase [NUM] as **Completed** with date and exit gate proof.
   - Append the verified entry to the Completed Phase Evidence Log.
3. Report the completion to the user with a concise summary of tests passed, exit gate satisfied, and next phase in line.

