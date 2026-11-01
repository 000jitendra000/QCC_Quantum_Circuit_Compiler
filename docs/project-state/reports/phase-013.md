# Phase Completion Report: Phase 13 — Transactional DAG Rewrite Infrastructure

## 1. Summary
- **Phase Number**: Phase 13
- **Exact Phase Title**: Transactional DAG rewrite infrastructure
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-01
- **Engineer / Agent Session**: Antigravity Assistant (Gemini 3.8 Flash)

---

## 2. Deliverables & Files Changed

### New Files Created:
- `src/ir/rewrite/mod.rs`: Rewrite subsystem root defining `RewriteError`, `RewriteResult`, and `CircuitTransaction` with atomic commit/rollback.
- `src/ir/rewrite/insert.rs`: Insertion operations (`insert_after`, `insert_before`, `insert_at_start`, `insert_at_end`) with automatic dependency induction.
- `src/ir/rewrite/remove.rs`: Deletion operations (`remove_operation`) with automated wire reconnection.
- `src/ir/rewrite/replace.rs`: Replacement operations (`replace_operation`) with provenance tracking.
- `src/ir/rewrite/expand.rs`: 1-to-N gate decompositions (`expand_operation`) with provenance preservation.
- `src/ir/rewrite/fuse.rs`: N-to-1 gate combinations (`fuse_operations`) with DAG convexity verification (`check_convexity`).
- `tests/ir_rewrites.rs`: 17 comprehensive unit, integration, and property tests covering all 5 operations, full transactional rollback, classical hazards, barrier ordering, and randomized rewrite sequences.
- `docs/project-state/plans/phase-013.md`: Implementation plan for Phase 13.
- `docs/project-state/reports/phase-013.md`: This completion report.

### Modified Files:
- `src/ir/node.rs`: Added `provenance: Vec<OperationId>` and `NodeData::with_provenance()` to track operation lineage.
- `src/ir/graph.rs`: Added `allocate_operation_id()`, `ordered_nodes()`, and `rebuild_from_ordered_nodes()` using canonical `WireAccessState` hazard tracking.
- `src/ir/circuit.rs`: Added `check_operation_wire_bounds()` and `rebuild_graph()`.
- `src/ir/serialize.rs`: Added `provenance` to `SerializedOperation` for backwards-compatible schema serialization.
- `src/ir/mod.rs`: Exposed `pub mod rewrite;` and re-exported core rewrite functions.
- `src/errors.rs`: Added `QccError::Rewrite(RewriteError)` enum variant.
- `src/lib.rs`: Re-exported rewrite types and functions.
- `docs/project-state/progress.md`: Updated progress ledger marking Phase 13 as complete.

---

## 3. Architecture & Design Decisions

### 3.1 Transactional Candidate Model & Full Rollback
- Every mutation staged in `CircuitTransaction` operates on an isolated candidate clone of `Circuit`.
- Mutations update the topological sequence and rebuild the dependency DAG using `WireAccessState`.
- If any stage encounters an error (out-of-bounds wires, duplicate operands, cycle detection, non-convex fusion, target not found) or if the transaction is dropped before calling `commit()`, the target circuit remains completely unmodified (0% mutated).
- On `commit()`, the candidate is verified against acyclicity and complete semantic invariants via `validate_circuit()`. Only upon successful validation is the candidate committed to the underlying circuit.

### 3.2 Canonical Dependency Reconstruction via `WireAccessState`
- Wire dependencies are reconstructed through the same `WireAccessState` engine used in initial circuit building.
- Rebuilding topologically guarantees:
  - Quantum order preservation without spurious dependencies between disjoint wires.
  - Classical Read-After-Write (RAW), Write-After-Read (WAR), and Write-After-Write (WAW) hazards are accurately computed.
  - Multi-qubit and barrier boundary dependencies are maintained.
  - Monotonic `sequence_number`s are automatically re-indexed, satisfying QccIR validation.

### 3.3 DAG Convexity Checking for Multi-Node Fusion
- Fusing an arbitrary set of nodes $S \subseteq V$ requires that $S$ is convex in the DAG.
- If an intermediate node $w \notin S$ lies on a directed path between two targets ($u \to^+ w \to^+ v$ with $u, v \in S$), fusing $S$ would violate causality and induce a directed cycle.
- `check_convexity()` computes the intersection of external ancestors and external descendants: $(\text{Descendants}(S) \cap \text{Ancestors}(S)) \setminus S$. If non-empty, the fusion is rejected with `RewriteError::InvalidFusion`.
- Valid fused nodes are positioned safely between external ancestors and external descendants.

### 3.4 Stable Semantic IDs & Provenance Tracking
- Nodes surviving a rewrite retain their original `OperationId`s.
- Newly created nodes receive freshly allocated `OperationId`s via `CircuitGraph::allocate_operation_id()`, preventing collisions.
- New nodes carry `provenance: Vec<OperationId>` pointing to the original operation IDs they replace, expand, or fuse.

---

## 4. Exit Gate Verification Evidence

- **Exit Gate Statement**: `Invalid rewrites are rejected without leaving a corrupted graph.`
- **Verification Commands Executed**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-targets
  cargo build
  cargo doc --no-deps
  cargo run --example bell
  cargo run --example qasm
  ```
- **Execution Evidence**:
  - `cargo fmt --check`: Passed with 0 formatting discrepancies.
  - `cargo clippy --all-targets --all-features -- -D warnings`: Passed with 0 errors and 0 warnings.
  - `cargo test --all-targets`: All 230+ tests passed across 14 test suites, including 17 new tests in `tests/ir_rewrites.rs`.
  - Rollback verified: Rejected operations (out of bounds, duplicate operands, non-convex fusion, missing target) leave the original circuit bit-for-bit identical (`assert_eq!(circuit, original)`).
  - Randomized sequence property test `prop_random_rewrite_sequence_always_validates` passed.
  - Examples `bell` and `qasm` ran and validated successfully.

---

## 5. Next Steps

- Proceed to **Phase 14: Gate Algebra and Inverse Registry** upon explicit user request.
- Do NOT begin Phase 14 until requested.
