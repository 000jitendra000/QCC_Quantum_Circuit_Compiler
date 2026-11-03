# Phase 35 Implementation Plan — Optimizer Integration and Reports

## 1. Executive Overview

Phase 35 implements the integrated compiler optimizer pipeline (`OptimizerPipeline`), optimization level configurations (`OptimizationLevel`, `OptimizerConfig`), circuit metrics calculation (`CircuitMetrics`), and structured audit reports (`OptimizerReport`, `PassExecutionReport`).

This phase composes the previously verified individual optimizer passes (Phases 28–34) into a deterministic, verifiable, bounded optimization pipeline with per-pass QccIR graph validation, optional debug-mode unitary equivalence verification, analysis invalidation, and comprehensive auditability.

---

## 2. Architecture & Invariants

1. **Optimization Levels (`src/optimizer/config.rs`)**:
   - `OptimizationLevel::O0`: No optimizations; validation and pass-through.
   - `OptimizationLevel::O1`: Basic peephole reductions (`IdentityEliminationPass` + `InverseCancellationPass`). Single iteration.
   - `OptimizationLevel::O2` (Default): Standard logical optimization loop (`IdentityEliminationPass` $\to$ `InverseCancellationPass` $\to$ `RotationMergePass` $\to$ `OneQubitFusionPass` $\to$ `IdentityEliminationPass` $\to$ `InverseCancellationPass`), running to fixed-point convergence or `max_iterations`.
   - `OptimizationLevel::O3`: Aggressive optimization loop including DAG commutation reordering (`CommutationReorderPass` with `ExposeOpportunities`), running to fixed-point convergence or `max_iterations`.

2. **Deterministic Bounded Termination**:
   - Fixed-point loop terminates immediately with `PipelineTermination::Converged` if an iteration produces zero modifications across all enabled passes.
   - Bounded by `config.max_iterations` (default 10), terminating with `PipelineTermination::IterationLimitReached` to strictly prevent infinite cycling.

3. **Circuit Metrics & Critical Path Depth (`src/optimizer/report.rs`)**:
   - `CircuitMetrics`: Gate count, single-qubit gates, multi-qubit gates, non-unitary operation count, gate histogram (`BTreeMap<String, usize>`), explicit global phase, and critical path depth.
   - Critical path depth is defined rigorously as the longest topological path in the dependency DAG where each operation step takes 1 time unit ($depth[v] = 1 + \max_{u \in pred(v)} depth[u]$).

4. **Structured Audit Reports (`src/optimizer/report.rs`)**:
   - `PassExecutionReport`: Detailed per-pass audit entry capturing iteration, pass ID, modified flag, initial/final metrics, summary, duration (telemetry), and equivalence check result.
   - `OptimizerReport`: Comprehensive audit report with deterministic summary formatting (`display_summary`, `to_deterministic_string`). Nondeterministic wall-clock elapsed time is stored separately to allow bit-exact repeatability tests.

5. **QccIR Graph & Equivalence Verification**:
   - Graph validation (`circuit.validate()`) runs after every pass execution.
   - When `verify_equivalence` is enabled:
     - Pure unitary circuits are verified against the pre-pass/initial state using Phase 10 `check_equivalence` under a common global phase.
     - Non-unitary circuits (containing measurement, reset, barrier, or classical operations) are cleanly detected and skipped from unitary equivalence, setting `unitary_equivalence_verified = None`.

---

## 3. Ordered Implementation Steps

1. **Configuration & Levels (`src/optimizer/config.rs`)**:
   - Implement `OptimizationLevel` and `OptimizerConfig` with builder methods.

2. **Metrics & Reports (`src/optimizer/report.rs`)**:
   - Implement `CircuitMetrics`, `MetricDelta`, `PipelineTermination`, `PassExecutionReport`, and `OptimizerReport`.
   - Implement critical path depth computation on `Circuit`.

3. **Pipeline Orchestration (`src/optimizer/pipeline.rs`)**:
   - Implement `OptimizerPipeline`, pass construction per level, iteration loop, analysis invalidation, graph verification, equivalence verification, and report synthesis.

4. **Module Registration & Re-exports (`src/optimizer/mod.rs` & `src/lib.rs`)**:
   - Expose new modules and types in `qcc::optimizer` and `qcc`.

5. **Integration & Regression Testing (`tests/optimizer/pipeline.rs`)**:
   - Test repeatability, per-pass equivalence, no-op behavior, metrics consistency, pass ordering, graph validation, iteration limits, regression corpus, and non-unitary circuits.

6. **Verification & Ledger Updates**:
   - Run full test suite, format, lint, build, doc, examples, and OpenQASM 3 validation.
   - Update `docs/project-state/reports/phase-035.md` and `docs/project-state/progress.md`.

