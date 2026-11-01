# Phase 12 Implementation Plan: Pass Manager Interface

## Objective
Establish the deterministic compiler-pass execution contract for QCC at the beginning of Milestone M2 (Compiler Foundations & OpenQASM Frontend).
The pass manager orchestrates the execution of compilation passes over QccIR dependency DAGs, with strict contracts for analysis dependencies, preservation and invalidation, semantic domains, post-pass debug verification, and telemetry statistics.

---

## 1. Architectural Strategy & Module Locations

Per [`docs/01-architecture-overview.md`](../../01-architecture-overview.md#2-workspace-evolution-plan), QCC operates as a **single consolidated package during Milestones M1 and M2 (Phases 1–27)** before transitioning to a multi-crate workspace (`qcc-core`, `qcc-ir`, `qcc-opt`, etc.) in Milestone M3+.

To maintain architecture coherence without circular crate dependencies or premature workspace partitioning:
- Intended modules are implemented in `src/opt/`:
  - `src/opt/pass.rs`: `CompilerPass` trait, `PassId`, `PassResult`, `PassMutation`, and `SemanticDomain`.
  - `src/opt/context.rs`: `PassContext`, `AnalysisId`, `PreservedAnalyses`, and `AnalysisStore`.
  - `src/opt/pass_manager.rs`: `PassManager`, execution pipeline, debug verification, and runner.
  - `src/opt/statistics.rs`: `PassStatistics`, `PassExecutionTrace`, and `PassManagerReport`.
  - `src/opt/errors.rs`: `PassError` and `PassManagerError`.
  - `src/opt/mod.rs`: Module exports and facade.
- Re-exported via `src/lib.rs` and `src/errors.rs`.
- Integration tests in `tests/pass_manager.rs`.

---

## 2. Core Concepts & Contracts

### 2.1 Pass Identity & Ordering
- **Stable Pass Identity**: `PassId(&'static str)` provides a deterministic, human-readable name.
- **Explicit Registration Order**: Passes execute in the exact vector order registered via `PassManager::add_pass`. Never reordered by hash maps or heuristics.
- **Deterministic Execution Trace**: `PassExecutionTrace` records step index, pass identity, execution outcome, and analysis state transitions. Durations are kept strictly in telemetry statistics to ensure trace equality across identical runs.

### 2.2 Analysis Dependency & Invalidation
- **Analysis Identity**: `AnalysisId(&'static str)` identifies cached analysis data (e.g. gate counts, interaction graph, topological order).
- **Required Analyses**: Passes declare `required_analyses() -> Vec<AnalysisId>`. The manager verifies presence and validity prior to execution; missing analyses cause structured aborts.
- **Preserved Analyses**: `PreservedAnalyses` represents which analyses survive pass mutation (`PreservedAnalyses::all()`, `PreservedAnalyses::none()`, or explicit subsets).
- **Pessimistic Invalidation on Failure**: If a pass fails or mutates without preserving an analysis, all non-preserved analyses are immediately invalidated. Subsequent passes cannot silently consume stale data.

### 2.3 Semantic Domains
- `SemanticDomain::UnitaryOnly`: Pass promises to operate on and produce pure unitary DAGs. Precondition rejects non-unitary operations (`Measure`, `Reset`, `Classical`).
- `SemanticDomain::NonUnitary`: Pass handles or produces measurement, reset, or feedback operations.
- `SemanticDomain::GeneralStructural`: Canonicalization or graph properties applicable across all circuits.
- `SemanticDomain::AnalysisOnly`: Reads DAG properties without mutating the circuit. Automatically preserves all analyses by default.

### 2.4 Debug Verification
- Configurable via `PassManager::with_debug_verification(bool)`.
- When enabled, runs after every individual pass:
  1. Full QccIR structural validation (`circuit.validate()`).
  2. Pass-declared postconditions (`pass.check_postconditions()`).
  3. Semantic domain invariant verification.
- Any violation immediately halts the pipeline and pinpoints the responsible pass.

### 2.5 Phase 13 Mutation Boundary
- Phase 12 establishes the pass mutation contract (`PassMutation` declaration, `&mut Circuit` in `run()`).
- Full transactional DAG rewriting (insert, delete, replace, fuse, dependency repair) will be implemented in Phase 13.
- In Phase 12, passes that mutate the circuit directly must report their effects and conservative invalidation. If a pass fails mid-execution, all cached analyses are purged because partial rollback is not yet guaranteed until Phase 13.

---

## 3. Implementation Steps

1. Create `src/opt/errors.rs` with `PassError` and `PassManagerError`.
2. Create `src/opt/context.rs` with `AnalysisId`, `PreservedAnalyses`, and `PassContext`.
3. Create `src/opt/pass.rs` with `CompilerPass`, `PassId`, `PassResult`, and `SemanticDomain`.
4. Create `src/opt/statistics.rs` with `PassStatistics`, `PassExecutionTrace`, and `PassManagerReport`.
5. Create `src/opt/pass_manager.rs` with `PassManager` pipeline orchestrator and debug verifier.
6. Create `src/opt/mod.rs` re-exporting all items.
7. Integrate into `src/lib.rs` and `src/errors.rs`.
8. Write comprehensive tests in `tests/pass_manager.rs` covering all 14 required scenarios.
9. Verify formatting (`cargo fmt`), linting (`cargo clippy`), and tests (`cargo test`).
10. Update documentation and progress ledger in `docs/project-state/progress.md`.
11. Commit with message: `feat(phase-12): introduce compiler pass manager`.
