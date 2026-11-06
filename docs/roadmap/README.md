# QCC Engineering Roadmap (Phases 1–59)

## Overview

This directory preserves the phase-by-phase implementation specifications for **QCC (Quantum Circuit Compiler)**, derived from `QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx` (preserved in [`docs/source/original-roadmap.docx`](../source/original-roadmap.docx)).

The compiler pipeline comprises **six core milestones (M1–M6)** spanning Phases 1 through 59.

---

## Roadmap Structure & Milestone Map

| Milestone | Code | Phases | Status | Core Deliverable | Documentation File |
|---|:---:|:---:|:---:|---|---|
| **Bell Vertical Slice** | `M1` | 1–11 | **Complete** | Rust builder → QccIR DAG → statevector simulator → valid OpenQASM 3 emitter → independent parse verification. | [phases-001-011.md](./phases-001-011.md) |
| **Compiler Foundations & QASM Frontend** | `M2` | 12–27 | **Complete** | Pass manager, transactional DAG rewrite infrastructure, gate algebra, OpenQASM 3 parser/lexer/AST, lowering, and round-trip suite. | [phases-012-027.md](./phases-012-027.md) |
| **Verified Logical Optimizer** | `M3` | 28–36 | **Complete** | Identity elimination, inverse cancellation, rotation merge, Euler decomposition, one-qubit fusion, commutation analysis and DAG rewriting. | [phases-028-036.md](./phases-028-036.md) |
| **Generic Target & Basis Translation** | `M4` | 37–43 | **Complete** | Target model, capability/cost queries, fixed decomposition templates, native single-qubit synthesis, basis translation, direction correction, target legality. | [phases-037-043.md](./phases-037-043.md) |
| **Logical-to-Physical Mapping & Routing** | `M5` | 44–53 | **Complete** | Injective layout model, layout-aware equivalence, initial placement heuristics, baseline shortest-path router, physical SWAPs, and full bidirectional SABRE. | [phases-044-053.md](./phases-044-053.md) |
| **Scheduling & Integrated Compilation** | `M6` | 54–59 | **Complete** | Target timing/resources, ASAP and ALAP schedulers, resource-constrained scheduling, schedule serialization/visualization, integrated pipeline. | [phases-054-059.md](./phases-054-059.md) |

For a quick searchable index of Phases 1–59, see [phase-index.md](./phase-index.md).

---

## Phase Specification Schema

Every phase specification in this directory adheres strictly to the following 13-field contract:

1. **Phase Number**: Canonical integer sequence (`1` to `59`).
2. **Exact Phase Title**: Verbatim title from the authoritative specification.
3. **Milestone**: Milestone identifier and name.
4. **Relative Engineering Effort**: Complexity estimate (`XS`, `S`, `M`, `L`, `XL`), not calendar time.
5. **Source Document Page**: Page reference in the original 55-page specification document.
6. **Objective**: Crisp statement of what the phase delivers.
7. **Required Concepts & Background**: Theoretical and engineering principles required.
8. **Prerequisites & Dependencies**: Explicit prerequisite phases that must pass their exit gates.
9. **Ordered Implementation Tasks**: Step-by-step implementation sequence.
10. **Expected Files and Modules**: File paths and Rust module locations.
11. **Required Tests**: Mandatory unit, property, golden, or integration tests.
12. **Definition of Done (DoD)**: Concrete functional conditions for phase completion.
13. **Failure Modes & Debugging Considerations**: Common pitfalls, anti-patterns, and debugging advice.
14. **Explicit Exit Gate**: The non-negotiable verification hurdle before downstream phases may proceed.

---

## Engineering Conventions & Ground Rules

1. **Effort Labels**: Relative complexity estimates (`XS` = trivial/wrapper, `S` = small bounded pass, `M` = medium subsystem, `L` = substantial engineering, `XL` = high-complexity mathematical or algorithmic subsystem). Effort is **not** a calendar promise.
2. **Exit Gates**: An exit gate is a hard barrier. Downstream phases must **never** be started until all prerequisite exit gates have been verified and recorded with evidence in [`docs/project-state/progress.md`](../project-state/progress.md).
3. **QccIR Distinction**: QCC's internal representation is called **QccIR**. Never confuse or conflate QccIR with Microsoft's LLVM-based Quantum Intermediate Representation (QIR). QIR interoperability is an export adapter target (Phase 81), not our IR.
4. **Topology-Valid vs Hardware-Ready**: "Topology-valid" means that 2-qubit gates obey the hardware coupling graph and orientation constraints. It does **not** mean the circuit is noise-optimized, calibrated, or guaranteed to execute on quantum hardware.
5. **External Toolchains**: External libraries (Qiskit Aer, Cirq, Stim, MQT QMAP, PyZX) are **optional reference and test oracles only**. They are never core runtime dependencies of `qcc-core`.

