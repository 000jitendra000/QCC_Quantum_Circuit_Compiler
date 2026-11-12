# QCC Engineering Roadmap (Phases 1–85)

## Overview

This directory contains the phase-by-phase implementation specifications for **QCC (Quantum Circuit Compiler)**:

- **Phases 1–59 (Milestones M1–M6 — Preserved Historical Baseline)**: Derived from the original engineering specification ([`docs/source/original-roadmap.docx`](../source/original-roadmap.docx)), completed and preserved at the **Phase 59 checkpoint**.
- **Phases 60–85 (Milestones M7–M12 — Active v3.1 Continuation)**: Defined in **Roadmap v3.1** (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap_v3.1.docx`, 12 November 2026). The previous 105-phase roadmap beyond Phase 59 is **superseded**.

The active priority is **Milestone M7 — Independent Verification Oracle (v0.2)**, executed in **five practical implementation sessions**, starting with **M7 Session 1 (Contract and manifest-first Rust work)**.

---

## Roadmap Structure & Milestone Map

| Milestone | Code | Phases | Status | Core Deliverable | Documentation File |
|---|:---:|:---:|:---:|---|---|
| **Bell Vertical Slice** | `M1` | 1–11 | **Complete (Baseline)** | Rust builder → QccIR DAG → statevector simulator → valid OpenQASM 3 emitter → independent parse verification. | [phases-001-011.md](./phases-001-011.md) |
| **Compiler Foundations & QASM Frontend** | `M2` | 12–27 | **Complete (Baseline)** | Pass manager, transactional DAG rewrite infrastructure, gate algebra, OpenQASM 3 parser/lexer/AST, lowering, and round-trip suite. | [phases-012-027.md](./phases-012-027.md) |
| **Verified Logical Optimizer** | `M3` | 28–36 | **Complete (Baseline)** | Identity elimination, inverse cancellation, rotation merge, Euler decomposition, one-qubit fusion, commutation analysis and DAG rewriting. | [phases-028-036.md](./phases-028-036.md) |
| **Generic Target & Basis Translation** | `M4` | 37–43 | **Complete (Baseline)** | Target model, capability/cost queries, fixed decomposition templates, native single-qubit synthesis, basis translation, direction correction, target legality. | [phases-037-043.md](./phases-037-043.md) |
| **Logical-to-Physical Mapping & Routing** | `M5` | 44–53 | **Complete (Baseline)** | Injective layout model, layout-aware equivalence, initial placement heuristics, baseline shortest-path router, physical SWAPs, and full bidirectional SABRE. | [phases-044-053.md](./phases-044-053.md) |
| **Scheduling & Integrated Compilation** | `M6` | 54–59 | **Complete (Checkpoint)** | Target timing/resources, ASAP and ALAP schedulers, resource-constrained scheduling, schedule serialization/visualization, integrated pipeline. | [phases-054-059.md](./phases-054-059.md) |
| **Independent Verification Oracle (v0.2)** | `M7` | 60–70 | **Active Next (Session 1)** | Versioned compilation manifest, pinned Qiskit oracle in `tools/independent_oracle/`, real QASM compatibility, layout/phase/ancilla checks, 1,000+ corpus, CI gates, fair baseline. | [phases-060-085.md](./phases-060-085.md) |
| **Compiler Introspection & Diagnostics** | `M8` | 71–73 | **Planned (v3.1)** | Per-pass diagnostic schema, pass-manager hooks, human-readable & JSON reports distinguishing `passed`/`failed`/`not_run`/`unsupported`. | [phases-060-085.md](./phases-060-085.md) |
| **Multi-Candidate Routing Improvements** | `M9` | 74–77 | **Planned (v3.1)** | Bounded multi-seed/layout SABRE candidate search, deterministic ranking/selection, comparative corpus benchmark. | [phases-060-085.md](./phases-060-085.md) |
| **Evidence-Selected Optimizer Improvements** | `M10` | 78–81 | **Planned (v3.1)** | Benchmark-gap analysis, formal pass specification, one isolated rewrite pass, independent oracle validation. | [phases-060-085.md](./phases-060-085.md) |
| **Bounded Hardware Profiles (Optional)** | `M11` | 82–84 | **Optional (Go/No-Go after M10)** | Capability gap audit, typed per-qubit properties/durations, profile-aware legality & scheduling. | [phases-060-085.md](./phases-060-085.md) |
| **Integrated Upgrade Acceptance** | `M12` | 85 | **Planned (v3.1)** | Full Rust + Qiskit oracle CI acceptance, reproducible benchmarks, updated README scope & limitations. | [phases-060-085.md](./phases-060-085.md) |

For a quick searchable index of Phases 1–85, see [phase-index.md](./phase-index.md).

> [!NOTE]
> **Deferred Scope**: Noise simulation, density-matrix simulation, dynamic CFG execution (mid-circuit measurement collapse/trajectories), KAK, MPS, and remote provider adapters are outside active Phases 60–85 unless explicitly reopened by the user.

---

## Phase Specification Schema

Every phase specification in this directory adheres to the structured contract:

1. **Phase Number**: Canonical integer sequence (`1` to `85`; Phases 60–70 execute via M7 Sessions 1–5).
2. **Exact Phase Title**: Verbatim title from the authoritative specification.
3. **Milestone**: Milestone identifier and name.
4. **Relative Engineering Effort**: Complexity estimate (`XS`, `S`, `M`, `L`, `XL`), not calendar time.
5. **Objective**: Crisp statement of what the phase delivers.
6. **Required Concepts & Background**: Theoretical and engineering principles required.
7. **Prerequisites & Dependencies**: Explicit prerequisite phases/sessions that must pass their exit gates.
8. **Ordered Implementation Tasks**: Step-by-step implementation sequence.
9. **Expected Files and Modules**: File paths and Rust/Python module locations.
10. **Required Tests**: Mandatory unit, property, golden, fault-injection, or integration tests.
11. **Definition of Done (DoD)**: Concrete functional conditions for completion.
12. **Failure Modes & Debugging Considerations**: Common pitfalls, anti-patterns, and debugging advice.
13. **Explicit Exit Gate**: The non-negotiable verification hurdle before downstream work may proceed.

---

## Engineering Conventions & Ground Rules

1. **Effort Labels**: Relative complexity estimates (`XS` = trivial/wrapper, `S` = small bounded pass, `M` = medium subsystem, `L` = substantial engineering, `XL` = high-complexity mathematical or algorithmic subsystem). Effort is **not** a calendar promise.
2. **Exit Gates**: An exit gate is a hard barrier. Downstream sessions/phases must **never** be started until all prerequisite exit gates have been verified and recorded with evidence in [`docs/project-state/progress.md`](../project-state/progress.md).
3. **QccIR Distinction**: QCC's internal representation is called **QccIR**. Never confuse or conflate QccIR with Microsoft's LLVM-based Quantum Intermediate Representation (QIR); Microsoft QIR remains a distinct future export target outside Phases 60–85.
4. **Topology-Valid vs Hardware-Ready**: "Topology-valid" means that 2-qubit gates obey the hardware coupling graph and orientation constraints. It does **not** mean the circuit is noise-optimized, calibrated, or guaranteed to execute on quantum hardware.
5. **External Toolchains**: Qiskit and Python packages live strictly in `tools/independent_oracle/` as optional dev/CI verification and baseline comparison tools. They are never core runtime dependencies of `qcc-core` or normal Rust compilation.
