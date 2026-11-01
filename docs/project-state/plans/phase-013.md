# Phase 13 Implementation Plan: Transactional DAG Rewrite Infrastructure

## Objective
Establish centralized, safe, transactional graph mutation infrastructure for QccIR dependency DAGs.
All future compiler optimization passes (such as identity elimination, gate cancellation, rotation merging, Euler decomposition, gate fusion, and commutation in Milestone M3) will perform graph mutations exclusively through this transactional rewrite API rather than direct, unchecked graph mutations.

---

## 1. Scope & Modules

### Files to Create:
- `src/ir/rewrite/mod.rs`: Transactional engine root, `CircuitTransaction`, `RewriteResult`, `RewriteError`.
- `src/ir/rewrite/insert.rs`: `insert_after`, `insert_before`, and position-based insertion.
- `src/ir/rewrite/remove.rs`: `remove_operation` with automatic wire reconnection.
- `src/ir/rewrite/replace.rs`: `replace_operation` with provenance tracking.
- `src/ir/rewrite/expand.rs`: `expand_operation` for 1-to-N gate decompositions.
- `src/ir/rewrite/fuse.rs`: `fuse_operations` for N-to-1 gate combinations with DAG convexity verification.
- `tests/ir_rewrites.rs`: Integration and property test suite covering all 5 operations, full transactional rollback, classical hazards, cycle rejection, and randomized sequences.

### Files to Modify:
- `src/ir/node.rs`: Add `provenance: Vec<OperationId>` and `NodeData::with_provenance()`.
- `src/ir/graph.rs`: Add `rebuild_from_ordered_nodes()`, `allocate_operation_id()`, and helper methods.
- `src/ir/mod.rs`: Expose `pub mod rewrite;` and re-export rewrite functions.
- `src/errors.rs`: Add `QccError::Rewrite(RewriteError)`.
- `src/lib.rs`: Re-export rewrite types and functions.
- `docs/project-state/reports/phase-013.md`: Completion report.
- `docs/project-state/progress.md`: Update progress ledger.

---

## 2. Invariants & Transactional Contracts

1. **Transactional Atomicity & Complete Rollback**:
   - Every rewrite operates on a candidate clone staged inside `CircuitTransaction`.
   - The candidate graph is mutated, dependency-rebuilt, and validated against all QccIR invariants (`circuit.validate()`, acyclicity, wire consistency, register bounds).
   - If any step fails, the transaction is discarded without mutating the target circuit.
   - Rollback covers the entire circuit state: graph nodes, edges, operation IDs, metadata, quantum registers, classical registers, and global phase.
2. **Deterministic Wire-Access Dependency Reconstruction**:
   - Rebuilding uses `WireAccessState` over topological execution sequences, guaranteeing exact preservation of:
     - Quantum wire execution orders.
     - Classical RAW, WAR, and WAW hazard edges.
     - Multi-qubit gate dependencies.
     - Measurement and reset ordering.
     - Barrier boundaries.
   - Independent operations on disjoint wires remain completely independent without spurious dependency edges.
3. **Stable Semantic Operation IDs & Provenance**:
   - Operations that survive a rewrite retain their original `OperationId` and sequence order.
   - New operations receive freshly allocated, collision-free `OperationId`s via `CircuitGraph::allocate_operation_id()`.
   - Provenance vectors link new operations back to the original operation IDs they replace, expand, or fuse.
   - `petgraph::NodeIndex` is never exposed as a semantic ID.
4. **Convexity Check in Fusion**:
   - Multi-node fusion checks that the target set is convex in the DAG: no intermediate non-target node depends on a target and precedes another target.
   - Non-convex fusions are rejected before graph corruption can occur.

---

## 3. Step-by-Step Implementation Sequence

1. Update `src/ir/node.rs` with `provenance: Vec<OperationId>` on `NodeData`.
2. Update `src/ir/graph.rs` with `rebuild_from_ordered_nodes` and `allocate_operation_id`.
3. Create `src/ir/rewrite/mod.rs` with `RewriteError`, `RewriteResult`, and `CircuitTransaction`.
4. Create `src/ir/rewrite/insert.rs`.
5. Create `src/ir/rewrite/remove.rs`.
6. Create `src/ir/rewrite/replace.rs`.
7. Create `src/ir/rewrite/expand.rs`.
8. Create `src/ir/rewrite/fuse.rs`.
9. Expose `rewrite` in `src/ir/mod.rs`, `src/errors.rs`, and `src/lib.rs`.
10. Write `tests/ir_rewrites.rs` covering all required test cases and proptest properties.
11. Run complete verification suite (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`, examples).
12. Write `docs/project-state/reports/phase-013.md` and update `docs/project-state/progress.md`.
13. Commit with `feat(phase-13): add transactional DAG rewrite infrastructure`.
