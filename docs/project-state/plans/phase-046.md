# Phase Plan: Phase 46 — Initial Placement Algorithms

## 1. Metadata
- **Phase Number**: Phase 46
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 37 (Target representation), Phase 38 (Target capability queries/costs), and Phase 44 (Logical/physical layout model) completed.
- **Source Document Page**: Page 25 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Choose a valid, deterministic starting physical layout ($L_{\text{initial}}$) for logical circuit qubits before routing, based on circuit interaction patterns and target physical connectivity.
- **Explicit Inclusions**:
  - Logical interaction graph extraction (`InteractionGraph`) tracking weighted two-qubit interaction frequencies.
  - Target physical distance matrix (`TopologyDistances`) computing all-pairs shortest paths and physical degrees.
  - Identity placement (`IdentityPlacer`).
  - Random-seeded placement (`RandomPlacer`) with strict reproducibility from an explicit `u64` seed.
  - Degree-based placement (`DegreePlacer`) matching high-degree logical nodes to high-degree physical nodes.
  - Interaction-weighted placement (`WeightedPlacer`) optimizing placement of heavily interacting qubit pairs to minimize physical routing distances.
  - Explicit, deterministic placement scoring (`PlacementScore`) calculating total weighted distance, unweighted distance, max distance, and direct adjacency fraction.
  - Topologies: Line, ring, grid, star, and irregular physical topologies.
  - Capacity validation: explicit reporting when $m > n$ (insufficient physical qubits).
  - Integration into `src/routing/placement/` and re-exports in `src/routing/mod.rs` and `src/lib.rs`.
- **Explicit Exclusions**:
  - Do NOT implement SWAP routing or greedy/SABRE routers (Phases 47–52).
  - Do NOT mutate circuits or insert SWAP gates (placement selects the initial layout only).
  - Do NOT introduce unseeded randomness or non-deterministic ordering.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (zero nightly flags).
- [x] Logical (`QubitId`) and physical (`PhysicalQubitId`) types strictly separated; no conflation.
- [x] Strict determinism: all graph traversals and tie-breakers use ordered structures (`BTreeMap`, `BTreeSet`).
- [x] Random placement strictly requires explicit seed; no unseeded PRNG.
- [x] Interaction frequency is explicitly incorporated, not just unweighted degree.
- [x] ADR-0001 ancilla zero-state policy: initial layout clearly identifies unassigned physical qubits as idle ancillae.
- [x] Injective mapping validation: every assigned logical qubit maps to a distinct physical qubit.

## 4. Step-by-Step Implementation Sequence
1. **Target Topology Enhancements (`src/target/connectivity.rs`)**:
   - Add `CouplingGraph::ring(num_qubits, directionality)`.
   - Add `CouplingGraph::star(num_qubits, center, directionality)`.
2. **Interaction Graph (`src/routing/placement/interaction_graph.rs`)**:
   - Build interaction graph from circuit's multi-qubit operations with deterministic edge weights.
   - Implement degree, weighted degree, and neighbor queries.
3. **Topology Distances (`src/routing/placement/distance.rs`)**:
   - Compute all-pairs shortest paths on `CouplingGraph` via BFS.
4. **Scoring Metric (`src/routing/placement/score.rs`)**:
   - Implement `PlacementScore` evaluating layout quality against interaction graph and topology distances.
5. **Placement Algorithms**:
   - `trivial.rs`: `IdentityPlacer` and `RandomPlacer` (seeded PRNG).
   - `degree.rs`: `DegreePlacer` matching logical degree to physical degree.
   - `weighted.rs`: `WeightedPlacer` greedy affinity-based constructive placement.
6. **Unified API & Error Types (`src/routing/placement/mod.rs`)**:
   - Define `PlacementStrategy`, `PlacementResult`, `PlacementError`.
   - Implement `place_circuit` and `place_circuit_with_coupling`.
7. **Re-exports (`src/routing/mod.rs`, `src/lib.rs`)**:
   - Export placement types and functions.
8. **Integration Tests (`tests/routing/placement.rs`)**:
   - Topologies: Line, ring, grid, star, irregular.
   - Strategies: Identity, random-seeded, degree-based, interaction-weighted.
   - Reproducibility across identical seeds.
   - Failure modes: insufficient capacity, zero qubits, disconnected target.
   - Layout injectivity and scoring validation.
9. **Full Project Verification**:
   - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo doc --no-deps`.
10. **Report & Commit**:
    - Write `docs/project-state/reports/phase-046.md`, update progress ledger, commit `feat(phase-46): implement initial placement algorithms`.

## 5. Expected Files and Modules
- `src/routing/placement/mod.rs`
- `src/routing/placement/interaction_graph.rs`
- `src/routing/placement/distance.rs`
- `src/routing/placement/trivial.rs`
- `src/routing/placement/degree.rs`
- `src/routing/placement/weighted.rs`
- `src/routing/placement/score.rs`
- `tests/routing/placement.rs`
- `docs/project-state/plans/phase-046.md`
- `docs/project-state/reports/phase-046.md`

## 6. Exit Gate
Placement fixtures must yield reproducible, valid mappings or explicitly report impossible capacity.

