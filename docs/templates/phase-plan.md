# Phase Implementation Plan Template

> Use this template to create a bounded implementation plan before modifying any code for a given roadmap phase.

---

# Phase Plan: Phase [NUM] — [TITLE]

## 1. Metadata
- **Phase / Session**: [Phase NUM / M7 Session X]
- **Milestone**: Milestone [ID]: [NAME]
- **Relative Effort**: [XS / S / M / L / XL]
- **Prerequisites Satisfied**: [List prerequisite phases/sessions and link to their completion reports in progress ledger]
- **Roadmap Reference**: [Roadmap v3.1 (`docs/roadmap/phases-060-085.md`) or baseline spec]

## 2. Objective & Scope Boundaries
- **Objective**: [Crisp summary of what will be implemented]
- **Explicit Inclusions**: [Specific functions, types, traits to be created]
- **Explicit Exclusions**: [What will NOT be implemented in this phase]

## 3. Architecture & Semantic Invariants Checklist
- [ ] Rust toolchain remains strictly stable.
- [ ] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [ ] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [ ] Explicit circuit global phase tracked and preserved.
- [ ] Quantum and classical wire hazards properly modeled.
- [ ] No direct petgraph index exposure (use stable semantic `OperationId`).
- [ ] Target legality fail-closed.
- [ ] ADR-0001 ancilla zero-state policy preserved (if routing/layout).

## 4. Step-by-Step Implementation Sequence
1. [Step 1: Core data structures / traits]
2. [Step 2: Core algorithms / modules]
3. [Step 3: Public API exposure]
4. [Step 4: Tests and golden assertions]

## 5. Expected Files and Modules
- New files to create:
  - `path/to/file.rs`
- Existing files to modify:
  - `path/to/mod.rs`
- Tests to add:
  - `tests/phase_xxx_test.rs`

## 6. Verification & Test Plan
- Unit tests: [Describe specific tests]
- Property tests: [Describe proptest strategies if applicable]
- Regressions: [Confirm existing test suites to run]

## 7. Exit Gate Verification Target
> **EXIT GATE CRITERIA**: [Exact text from roadmap specification]
> - Command to execute: `cargo test ...`
> - Expected outcome: [Clean pass, zero warnings]

