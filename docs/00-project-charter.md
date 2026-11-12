# QCC Project Charter

## 1. Project Thesis

**QCC (Quantum Circuit Compiler)** is a Rust-first quantum circuit compiler engineered around:
- A dependency DAG-based intermediate representation (**QccIR**).
- An in-place reference statevector simulator operating on double-precision complex numbers (`Complex64`).
- Semantics-preserving, verified logical optimization passes.
- Target-aware basis translation.
- Logical-to-physical qubit mapping and circuit routing.
- Duration- and resource-constrained scheduling.
- OpenQASM 3 interoperability (input parsing and output emission).
- Explainable compilation reports and execution inspection.

QCC is both a high-rigor compiler-engineering initiative and a quantum-computing foundations project. We do not invent a bespoke quantum programming language; rather, we build a robust compiler that ingests quantum circuits, transforms them under target constraints, and preserves quantum semantics with mathematical provability.

---

## 2. Core Architectural Philosophy

1. **Rust-First & Stable Toolchain**: Written entirely in idiomatic, stable Rust. No nightly features, no unstable compiler flags, no unverified unsafe blocks.
2. **Semantics Before Optimization**: A compiler transformation is useless if it alters quantum state amplitudes. Equivalence checking up to a common global phase must exist before any optimization pass is written.
3. **Builder Before Parser**: Core compiler data structures and execution engines are developed and validated using a native Rust builder API before exposing a textual parser frontend.
4. **DAG Per Basic Block**: Operation ordering is governed by explicit quantum and classical wire dependencies in a directed acyclic graph, not by a fragile linear array of instructions.
5. **Fail-Closed Target Legality**: Circuits targeting physical hardware must strictly adhere to native gate sets, coupling topologies, and operand orientations. Illegal gates cannot silently slip into emission.
6. **Explicit Unknowns**: If hardware calibration or timing metrics are absent, they are explicitly modeled as `Unknown`, never assumed to be zero.

---

## 3. Product Release Horizons & Development Status

> [!NOTE]
> **Active Engineering Plan (Roadmap v3.1 — 12 November 2026)**:
> Phases 1–59 (Milestones M1–M6) are preserved at the **Phase 59 integrated compilation pipeline checkpoint**.
> The previous 105-phase roadmap (old Phases 60–105) has been **superseded** by **Roadmap v3.1** (**Phases 60–85**, Milestones **M7–M12**).
> The first active priority is **M7 — Independent Verification Oracle v0.2** (executed in 5 sessions, starting with **M7 Session 1**).

### 3.1 Completed Historical Baseline (Phases 1–59)
- **v0.1 Deliverable — Bell Vertical Slice (Phases 1–11)**: Complete end-to-end vertical slice from builder to QccIR, simulation, OpenQASM 3 emission, and unitary verification.
- **Compiler Foundations & QASM Frontend (Phases 12–27)**: OpenQASM 3 lexer, parser, AST, semantic analysis, lowering, and roundtrip verification.
- **Verified Logical Optimizer (Phases 28–36)**: Verified peephole rewrites, cancellation, rotation merging, Euler decomposition, and commutation rewriting.
- **Target Description & Basis Translation (Phases 37–43)**: Dynamic target capability queries, fixed decomposition templates, native synthesis, recursive basis translation, and target legality.
- **Logical-to-Physical Mapping & Routing (Phases 44–53)**: Injective layouts, baseline router, and bidirectional SABRE with ADR-0001 strict ancilla zero-state verification.
- **Scheduling & Integrated Compilation (Phases 54–59)**: Target timing/resource models, ASAP/ALAP schedulers, resource-constrained scheduling, timeline visualization, and end-to-end compilation pipeline.

### 3.2 Active Continuation Horizons (Roadmap v3.1 — Phases 60–85)
- **M7 — Independent Verification Oracle (v0.2, Phases 60–70, Sessions 1–5)**: Versioned compilation manifest, pinned optional Qiskit oracle in `tools/independent_oracle/`, real QCC OpenQASM 3 compatibility checks, layout/endianness/clean-ancilla/common-phase verification plus informational strict phase accounting, fault injection, 1,000+ difficult corpus, CI integration, and fair matched-$N$ Qiskit comparison.
- **M8 — Compiler Introspection & Diagnostics (Phases 71–73)**: Truthful per-pass before/after metrics and verification status (`passed` / `failed` / `not_run` / `unsupported`) in human-readable and JSON reports.
- **M9 — Multi-Candidate Routing Improvements (Phases 74–77)**: Bounded multi-seed/layout SABRE search with deterministic selection and comparative validation.
- **M10 — Evidence-Selected Optimizer Improvements (Phases 78–81)**: Benchmark-gap-selected single bounded optimizer pass with independent oracle validation.
- **Optional M11 — Bounded Hardware Profiles (Phases 82–84)**: Conditional on an explicit post-M10 go/no-go review; per-qubit capability and duration profiles preserving explicit unknowns.
- **M12 — Integrated Upgrade Acceptance (Phase 85)**: Full clean-checkout Rust + Qiskit oracle CI acceptance and evidence-backed README closeout.

---

## 4. Explicit Scope Boundaries and Non-Goals

To maintain strict engineering focus, the following items remain explicitly **out of scope / deferred** unless the project direction is explicitly revised:

1. **No Bespoke DSL**: QCC will not introduce a custom `.qcc` programming language. OpenQASM 3 is the standard textual interchange format.
2. **QccIR is Not Microsoft QIR**: QccIR is an in-memory dependency DAG implemented in Rust. Microsoft QIR is an LLVM-based bitcode representation; QIR export interoperability remains deferred outside Phases 60–85.
3. **No Dynamic Execution, Noise, or Density-Matrix Simulation**: Mid-circuit measurement collapse, trajectory/branch simulation, dynamic classical CFG execution, noise models, and density-matrix simulation remain deferred outside active Phases 60–85.
4. **No Remote Provider Adapters or Credential Management**: Provider-specific execution adapters remain outside active Phases 60–85.
5. **No Arbitrary KAK / MPS / Speculative Optimizers**: KAK two-qubit synthesis and MPS simulation remain deferred; M10 selects only one evidence-justified optimizer improvement.
6. **No Pulse-Level Control or QEC**: OpenPulse scheduling and quantum error correction are outside scope.
7. **No GPU or Distributed Simulator**: The primary simulator is an in-place CPU statevector engine optimized for exactness and transparency.
8. **No Unsupported Hardware Execution Claims**: Passing "topology validity" means satisfying graph connectivity and gate directions; it does not guarantee calibration, noise optimality, or execution fidelity on physical hardware. QCC remains a personal, learning-oriented compiler with a bounded feature set.

