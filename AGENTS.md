# QCC Engineering Agent Instructions

Welcome to the **QCC (Quantum Circuit Compiler)** codebase.

QCC is a long-term, high-rigor Rust compiler engineering project designed across **105 granular implementation phases**.

Every future agent session must treat this document and the associated documentation in `docs/` as the immutable, authoritative source of truth.

---

## 12 Mandatory Agent Rules

Every agent session operating in this repository must strictly adhere to these 12 rules:

1. **Read Contracts Before Implementation**: Before touching code, read the relevant architecture specifications in [`docs/`](./docs/):
   - [`00-project-charter.md`](./docs/00-project-charter.md) — Scope boundaries, release horizons, and non-goals.
   - [`01-architecture-overview.md`](./docs/01-architecture-overview.md) — Subsystems, crate evolution, and dependency policy.
   - [`02-formal-semantics-and-contracts.md`](./docs/02-formal-semantics-and-contracts.md) — Little-endian qubit indexing, Dirac notation, matrix conventions, global phase, and wire effects.
   - [`03-compilation-pipeline.md`](./docs/03-compilation-pipeline.md) — Pipeline order and pass manager contracts.
   - [`04-correctness-and-verification.md`](./docs/04-correctness-and-verification.md) — Unitary equivalence with one common global phase.
   - [`05-target-routing-and-layout.md`](./docs/05-target-routing-and-layout.md) — Target models, layouts, baseline router vs SABRE, and ADR-0001.
   - [`06-simulator-design.md`](./docs/06-simulator-design.md) — In-place $\mathcal{O}(2^n)$ kernels; no dense $2^n \times 2^n$ operators.
2. **Consult the Roadmap for Current Phase**: Locate the exact requested phase in [`docs/roadmap/phase-index.md`](./docs/roadmap/phase-index.md) and read its full 13-field specification in the corresponding milestone file.
3. **Check the Progress Ledger**: Inspect [`docs/project-state/progress.md`](./docs/project-state/progress.md) to understand current repository state.
4. **Never Bypass Prerequisites**: Never start a phase before all of its explicit prerequisites have been verified and recorded as completed.
5. **Preserve All Non-Negotiable Invariants**: Always follow [`.agents/rules/qcc-project-invariants.md`](./.agents/rules/qcc-project-invariants.md):
   - Stable Rust only (zero nightly flags).
   - Little-endian qubit indexing ($q_0$ is LSB).
   - Column vectors and left multiplication ($B \cdot A$).
   - CX: operand 0 = control, operand 1 = target.
   - Explicit circuit global phase preservation.
   - Classical wire hazards (RAW, WAR, WAW) via `WireAccessState`.
   - ADR-0001 strict ancilla zero-state policy.
   - Never call QccIR "Microsoft QIR".
6. **Implement Only the Explicitly Requested Phase**: Do not jump ahead, do not create speculative future abstractions, and do not work on unrequested phases.
7. **Avoid Unapproved Architectural Changes**: If an architectural detail is ambiguous or requires change, raise it with the user or draft an ADR. Never invent ad-hoc conventions.
8. **Write Tests and Documentation**: Every phase requires automated tests (unit, property, golden) and rustdoc comments on public items.
9. **Run Verification Commands**: Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`. All must pass with zero errors and zero warnings.
10. **Update Progress Ledger Only With Evidence**: Update [`docs/project-state/progress.md`](./docs/project-state/progress.md) only after test output proves the phase's exit gate is met.
11. **Report Incomplete Work Honestly**: If an exit gate fails or a dependency is missing, report it candidly. Never claim a phase is complete when it is partial.
12. **Follow the Standard Execution Workflow**: Always follow the step-by-step workflow in [`.agents/workflows/qcc-phase-execution.md`](./.agents/workflows/qcc-phase-execution.md).

---

## Directory Quick Reference

- **Authoritative Original Spec**: [`docs/source/original-roadmap.docx`](./docs/source/original-roadmap.docx) (55 pages, 105 phases).
- **Core Architecture Docs**: [`docs/00-project-charter.md`](./docs/00-project-charter.md) through [`docs/09-glossary.md`](./docs/09-glossary.md).
- **105-Phase Roadmap**: [`docs/roadmap/phase-index.md`](./docs/roadmap/phase-index.md) and [`docs/roadmap/`](./docs/roadmap/).
- **Architecture Decision Records**: [`docs/decisions/ADR-0001-ancilla-zero-state-policy.md`](./docs/decisions/ADR-0001-ancilla-zero-state-policy.md).
- **Implementation Progress Ledger**: [`docs/project-state/progress.md`](./docs/project-state/progress.md).
- **Workspace Invariants Rule**: [`.agents/rules/qcc-project-invariants.md`](./.agents/rules/qcc-project-invariants.md).
- **Phase Execution Workflow**: [`.agents/workflows/qcc-phase-execution.md`](./.agents/workflows/qcc-phase-execution.md).

