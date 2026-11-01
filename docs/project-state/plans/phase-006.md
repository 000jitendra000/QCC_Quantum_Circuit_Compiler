# Implementation Plan: Phase 6 — DAG-based QccIR

## 1. Context & Objectives
- **Phase Number**: Phase 6
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Primary Goal**: Create QccIR as the canonical operation representation:
  - Dependency DAG per basic block using `petgraph::stable_graph::StableDiGraph` internally.
  - Strict abstraction: petgraph node and edge indices are NEVER exposed outside QccIR.
  - Typed semantic identities: `QubitId`, `ClassicalBitId`, `RegisterId`, `OperationId`.
  - Operations: Gates (referencing Phase 2 catalog), Measure, Reset, Barrier, Classical.
  - Typed dependencies: `QuantumOrder`, `ClassicalRaw`, `ClassicalWar`, `ClassicalWaw`, `BarrierBoundary`.
  - Per-wire access state (`WireAccessState`) maintaining `last_writer` and `outstanding_readers` for accurate RAW/WAR/WAW data hazard modeling.
  - Circuit metadata and explicit global phase preservation (`ParameterExpr`).
  - Executable validation returning structured errors (acyclicity, register bounds, gate arity/parameters, wire consistency).
  - Deterministic topological serialization (Kahn's algorithm with tie-breaking) and round-trip verification.

## 2. Invariants & Conventions
1. **Stable Semantic IDs**: QccIR operations own stable sequence IDs (`OperationId(u64)`).
2. **Never Call QccIR "Microsoft QIR"**: QccIR is QCC's internal typed dependency DAG.
3. **Classical Data Hazards**: A single last-operation pointer is strictly prohibited. `WireAccessState` tracks `last_writer` and `outstanding_readers`.
4. **Deterministic Serialization**: Topological traversal uses Kahn's algorithm with deterministic priority queue tie-breaking.
5. **Phase & Gate Compatibility**: Reuses Phase 2 gate definitions, matrices, and `Angle` / `GlobalPhase`.

## 3. Module Architecture
- `src/ir/mod.rs`: IR module facade and re-exports.
- `src/ir/error.rs`: Typed `IrError` and `IrValidationError`.
- `src/ir/wire.rs`: `WireId` (`Quantum`, `Classical`), `WireAccessState`.
- `src/ir/parameter.rs`: `ParameterExpr` (constant, symbolic, algebraic expressions).
- `src/ir/operation.rs`: `Operation` (`Gate`, `Measure`, `Reset`, `Barrier`, `Classical`), wire effect queries.
- `src/ir/edge.rs`: `WireDependency`, `DependencyKind`.
- `src/ir/node.rs`: `NodeData`, `OperationId`.
- `src/ir/graph.rs`: `CircuitGraph` (encapsulating `StableDiGraph`, deterministic Kahn's toposort, dependency construction).
- `src/ir/circuit.rs`: `Circuit`, `QuantumRegister`, `ClassicalRegister`, global phase.
- `src/ir/validation.rs`: Comprehensive executable validation checks.
- `src/ir/serialize.rs`: Deterministic JSON serialization and deserialization.
- `docs/qccir.md`: Full architectural specification of QccIR.
- `tests/ir/`: Exhaustive unit and integration tests.

## 4. Execution Steps
1. Add `Ir(#[from] IrError)` to [`src/errors.rs`].
2. Implement core IR data types (`wire.rs`, `parameter.rs`, `node.rs`, `edge.rs`, `operation.rs`).
3. Implement `graph.rs` with `WireAccessState` dependency construction and deterministic topological sorting.
4. Implement `circuit.rs` with register management and circuit-level global phase.
5. Implement `validation.rs` with complete executable invariant checks.
6. Implement `serialize.rs` with deterministic serialization and deserialization.
7. Write `docs/qccir.md`.
8. Write comprehensive test suite in `tests/ir/`.
9. Run full verification: `cargo fmt`, `cargo clippy`, `cargo test`, `cargo doc`.
10. Update progress ledger and create Git checkpoint commit.
