# QCC Architecture Overview

## 1. System Architecture Diagram

The high-level architecture of the Quantum Circuit Compiler is partitioned into decoupled subsystems communicating through strictly versioned contracts:

```mermaid
flowchart TD
    subgraph Frontends["Frontend Layer"]
        Builder["Rust CircuitBuilder API\n(Phase 7)"]
        QASM["OpenQASM 3 Frontend\n(Phases 15–27)"]
    end

    subgraph CoreIR["Intermediate Representation (QccIR)"]
        DAG["Dependency DAG per Basic Block\n- Quantum Wires & Access States\n- Classical Wires & RAW/WAR/WAW\n- Stable Semantic Operation IDs\n- Circuit Global Phase"]
        Rewriter["Transactional DAG Rewriter\n(Phase 13)"]
    end

    subgraph Optimization["Optimizer (qcc-opt)"]
        PassMgr["Pass Manager & Invalidation Ledger\n(Phase 12)"]
        Algebra["Gate Algebra & Identities (Phase 14)"]
        OptPasses["Optimization Passes:\n- Identity & Cancellation (Phases 28–29)\n- Rotation Merge & Euler (Phases 30–31)\n- Gate Fusion (Phase 32)\n- DAG Commutation (Phases 33–34)"]
    end

    subgraph Verification["Correctness & Verification"]
        UnitaryEq["Unitary Equivalence Checker\n(Global-Phase Aware, Phase 10)"]
        LayoutEq["Layout-Aware Verifier\n(ADR-0001 Ancilla Policy, Phase 45)"]
        DiffHarness["Differential Test Harness\n(Qiskit Aer / Cirq / Stim, Phases 68–71)"]
    end

    subgraph TargetRouting["Target Translation & Routing"]
        TargetModel["Generic Target Model & Legality\n(Phases 37, 43)"]
        BasisTrans["Basis Translation Engine\n(Phases 39–42)"]
        Placement["Initial Placement (Phase 46)"]
        Router["Shortest-Path Baseline (Phase 47)\nBidirectional SABRE (Phases 50–52)"]
    end

    subgraph SchedulingLayer["Scheduling (qcc-scheduler)"]
        Sched["Post-Routing Scheduler:\n- ASAP / ALAP (Phases 55–56)\n- Resource Constraints (Phase 57)"]
    end

    subgraph BackendsSim["Execution & Export"]
        Sim["Statevector Engine: O(2^n)\n- Terminal Sampling (Phase 5)\n- Trajectories & Branches (Phases 60–62)"]
        QASMEmit["OpenQASM 3 Emitter (Phase 9)"]
        BackendAPI["Provider-Neutral Backend API\n(Phases 72–82)"]
    end

    Frontends --> CoreIR
    CoreIR --> Optimization
    Optimization <--> Verification
    Optimization --> TargetRouting
    TargetRouting <--> Verification
    TargetRouting --> SchedulingLayer
    SchedulingLayer --> BackendsSim
    CoreIR -.-> Sim
    CoreIR -.-> QASMEmit
```

---

## 2. Workspace Evolution Plan

To avoid premature abstraction and build friction while stabilizing core APIs, the codebase follows a two-stage packaging strategy:

### Stage 1: Single Consolidated Package (Milestones M1–M2)
Initial implementation begins as a single Cargo package with well-defined internal modules:
```text
qcc/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── errors.rs
│   ├── diagnostics/
│   ├── quantum/
│   ├── simulator/
│   ├── ir/
│   ├── builder/
│   ├── backend/
│   └── verification/
├── examples/
│   └── bell.rs
├── tests/
│   ├── smoke.rs
│   └── quantum_semantics.rs
└── docs/
```

### Stage 2: Multi-Crate Workspace (Milestones M3+)
Once public interfaces and data contracts are stabilized through exit gates, the package is partitioned into focused, independently testable crates:

| Crate | Responsibility |
|---|---|
| `qcc-core` | Fundamental quantum semantics, gates, registers, parameters, angle algebra. |
| `qcc-ir` | QccIR dependency DAG, nodes, edges, wire access state, transactional rewrites. |
| `qcc-frontend-qasm3` | OpenQASM 3 lexer, parser, AST, semantic checks, and lowering. |
| `qcc-opt` | Pass manager, algebraic identities, optimization passes, commutation rules. |
| `qcc-target` | Target machine descriptions, capability queries, cost models, basis translation. |
| `qcc-routing` | Layout mappings, initial placement heuristics, baseline router, SABRE router. |
| `qcc-scheduler` | Timing models, resource calendars, ASAP/ALAP, resource-constrained scheduling. |
| `qcc-sim-statevector`| In-place statevector simulation kernels, terminal sampling, dynamic trajectories. |
| `qcc-sim-density` | Density matrix reference engine, partial trace, Kraus noise channels. |
| `qcc-backend-api` | Provider-neutral submission, job lifecycle, credential handling, results. |
| `qcc-backend-openqasm`| OpenQASM 3 emission and execution artifact bundling. |
| `qcc-testkit` | Random circuit generators, property-test strategies, differential test harnesses. |
| `qcc-cli` | Command-line interface, inspection commands, ASCII/SVG visualization. |

---

## 3. Dependency Policy

Every dependency introduced into QCC must serve a specific architectural purpose and must strictly avoid polluting core types:

| Dependency | Purpose | Architectural Decision |
|---|---|---|
| `petgraph` | Low-level directed graph data structures. | Use `StableDiGraph` internally. **Never expose raw petgraph `NodeIndex` or `EdgeIndex` outside QccIR**. Maintain QCC-owned stable `OperationId`s and deterministic topological sorting. |
| `num-complex` | Complex numbers (`Complex64`). | Standard representation for statevector amplitudes and unitary matrix elements. |
| `rand` | Seeded pseudo-random number generation. | Required for terminal measurement sampling and property testing. All simulation must accept an explicit seed for 100% deterministic reproducibility. |
| `thiserror` | Structured error derive macros. | Used across crates for domain-specific, typed error enums with clear diagnostic messages. |
| `serde` | Data serialization / deserialization. | Used for machine-readable manifests, target profiles, and schedule outputs. Not used as a replacement for semantic IR types. |
| `proptest` | Property-based testing framework. | Adopted from Phase 10 onwards for generating random unitary circuits, layout permutations, and statevectors. |
| `clap` | Command-line argument parsing. | Used for CLI command-line parsing. |
| `criterion` | Statistical benchmarking framework. | Introduced for performance regressions in simulator kernels, DAG rewrites, and router passes. |
| `rug` / `double-double` | High-precision arithmetic. | **Optional only**. Used exclusively if analytical reference proofs demonstrate $f64$ numerical instability. |
| `oq3_*` | OpenQASM 3 parsing crates. | Evaluated behind an adapter in Phase 15. If experimental crate stability fails, fall back to a handwritten parser for the supported subset. |
| External Simulators (Qiskit, Cirq, Stim) | Differential validation oracles. | **Strictly external reference tools**. Invoked via CLI or Python bridges in `tests/differential/`. Never hard dependencies of `qcc-core`. |
| `mqt-qmap` | Router benchmark reference. | External reference oracle only, not a Rust dependency. |

