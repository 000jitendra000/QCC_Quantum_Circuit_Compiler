# Phase Plan: Phase 44 — Logical/Physical Layout Model

## 1. Metadata
- **Phase Number**: Phase 44
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: **M**
- **Prerequisites Satisfied**: Phases 37–43 verified and completed (Milestone M4 100% complete).
- **Source Document Page**: Page 24 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Represent logical qubit identity separately from physical locations, with validated injective initial, current, and final layouts, bidirectional lookup, support for partially occupied devices (unused ancillae per ADR-0001), and deterministic serialization.
- **Explicit Inclusions**:
  - Module `src/routing/` containing `layout_map.rs`, `layout.rs`, and `mod.rs`.
  - Distinct strongly typed IDs: reuse existing logical `QubitId` (`crate::ir::wire::QubitId`) and physical `PhysicalQubitId` (`crate::target::operation::PhysicalQubitId`), with type alias `LogicalQubitId`.
  - `LayoutMap` (aliased as `Layout`): Injective bidirectional mapping (`BTreeMap` backed for deterministic ordering) between logical qubits and physical qubits.
  - Forward (`logical_to_physical`) and reverse (`physical_to_logical`) lookups with strict consistency validation.
  - Partial mapping support: explicit tracking of assigned vs unused/idle physical qubits (`unused_physical_qubits`).
  - Physical and logical SWAP mutators: `swap_physical(p1, p2)` updating layout states upon routing SWAP insertions.
  - `CircuitLayout`: Encapsulates `initial` ($L_{\text{initial}}$), `current` ($L_{\text{current}}$), and `final_layout` ($L_{\text{final}}$) across the routing lifecycle.
  - Structured error types (`LayoutError`) for out-of-bounds, duplicate assignments, non-injective mappings, and capacity mismatches.
  - Deterministic JSON serialization and validating deserialization ensuring invalid serialized structures fail closed.
  - Re-exports in `src/routing/mod.rs` and `src/lib.rs`.
- **Explicit Exclusions**:
  - Do NOT implement layout-aware unitary equivalence checking (deferred to Phase 45).
  - Do NOT implement placement algorithms (deferred to Phase 46).
  - Do NOT implement Dijkstra/SABRE routers (deferred to Phases 47–52).

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero nightly features).
- [x] Distinct typed IDs for logical and physical qubits (cannot be interchanged).
- [x] ADR-0001 strict ancilla policy: explicitly tracks unused physical qubits for zero-state verification in Phase 45.
- [x] Deterministic ordering: `BTreeMap` used for all internal collections and serialization.
- [x] Little-endian qubit indexing preserved ($q_0$ / $p_0$ as LSB).
- [x] No petgraph NodeIndex exposure; strictly uses stable typed identifiers.
- [x] Comprehensive automated unit and integration tests.

## 4. Step-by-Step Implementation Sequence
1. **Module Scaffolding**: Create `src/routing/` and declare it in `src/lib.rs`.
2. **Layout Map Core (`src/routing/layout_map.rs`)**:
   - Define `LayoutError` enum.
   - Define `LayoutMap` struct (`num_logical`, `num_physical`, `logical_to_physical`, `physical_to_logical`).
   - Implement constructors (`new`, `identity`, `from_mappings`, `from_slice`, `from_permutation`, `identity_for_target`).
   - Implement queries (`get_physical`, `get_logical`, `assigned_logical_qubits`, `assigned_physical_qubits`, `unused_physical_qubits`, `is_physical_idle`, etc.).
   - Implement mutators (`assign`, `unassign`, `swap_physical`, `swap_logical`).
   - Implement validation (`validate`) and validating serde deserializer.
3. **Circuit Layout Triple (`src/routing/layout.rs`)**:
   - Define `CircuitLayout` struct (`initial`, `current`, `final_layout`).
   - Implement constructors (`new`, `with_layouts`, `identity_for_circuit`).
   - Implement tracking and mutation (`apply_physical_swap`, `commit_final`, `validate`).
   - Implement serialization and deserialization.
4. **Integration & Re-exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Expose `LayoutMap`, `Layout`, `CircuitLayout`, `LayoutError`, `LogicalQubitId`.
5. **Testing Suite (`tests/routing/layout.rs`, `tests/routing.rs`)**:
   - Identity and nontrivial permutations.
   - Partial mappings on larger devices with idle physical ancillae.
   - Forward and reverse lookup round-trips.
   - Validation failure on duplicate physical/logical assignments.
   - Serialization round-trip and rejection of corrupt/inconsistent serialized data.
   - Capacity mismatch and distinct ID type non-interchangeability.
6. **Full Verification Suite**:
   - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build`, `cargo doc`, examples, independent parser check.
7. **Report & Commit**:
   - Create completion report `docs/project-state/reports/phase-044.md`, update progress ledger, git commit.

## 5. Expected Files and Modules
- New files:
  - `src/routing/mod.rs`
  - `src/routing/layout_map.rs`
  - `src/routing/layout.rs`
  - `tests/routing/mod.rs` (or `tests/routing/layout.rs`)
  - `tests/routing.rs`
  - `docs/project-state/plans/phase-044.md`
  - `docs/project-state/reports/phase-044.md`
- Existing files to modify:
  - `src/lib.rs`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- Unit tests in `src/routing/layout_map.rs` and `src/routing/layout.rs`.
- Integration tests in `tests/routing.rs` verifying all 7 test cases specified in prompt.
- Workspace test regression with all 34 existing test binaries (1,064+ tests).

## 7. Exit Gate Verification Target
- Invariants validated: injectivity, forward-reverse consistency, unused qubit tracking, deterministic serde round-trip.
- Zero warnings across all targets.
