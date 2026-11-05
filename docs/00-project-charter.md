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
> **Status & Roadmap Update**:
> QCC development is currently paused after the integrated compilation pipeline checkpoint (Phase 59).
> The previous 105-phase roadmap and its original release horizon projections (v1.0 via Phases 12–89 and Advanced Research via Phases 90–105) have been retired.
> Completed work (Phases 1–59) spans the foundational compiler core: Bell vertical slice, OpenQASM 3 frontend and QccIR, logical optimization, target modeling and basis translation, layout-aware routing, physical scheduling, and end-to-end integrated compilation.
> Future development priorities, release packaging, and the next development roadmap will be determined separately (TBD).

### 3.1 Completed Milestone Horizons (Phases 1–59)
- **v0.1 Deliverable — Bell Vertical Slice (Phases 1–11)**: Complete end-to-end vertical slice from builder to QccIR, simulation, OpenQASM 3 emission, and unitary verification.
- **Compiler Foundations & QASM Frontend (Phases 12–27)**: OpenQASM 3 lexer, parser, AST, semantic analysis, lowering, and roundtrip verification.
- **Verified Logical Optimizer (Phases 28–36)**: Verified peephole rewrites, cancellation, rotation merging, Euler decomposition, and commutation rewriting.
- **Target Description & Basis Translation (Phases 37–43)**: Dynamic target capability queries, fixed decomposition templates, native synthesis, recursive basis translation, and target legality.
- **Logical-to-Physical Mapping & Routing (Phases 44–53)**: Injective layouts, baseline router, and bidirectional SABRE with ADR-0001 strict ancilla zero-state verification.
- **Scheduling & Integrated Compilation (Phases 54–59)**: Target timing/resource models, ASAP/ALAP schedulers, resource-constrained scheduling, timeline visualization, and end-to-end compilation pipeline.

### 3.2 Future Development Horizons
- Development past Phase 59 is currently paused.
- Future milestone definitions, backend integrations, and next-generation simulation features are TBD.

---

## 4. Explicit Scope Boundaries and Non-Goals

To maintain strict engineering focus, the following items are explicitly **out of scope** for v0.x and v1.0:

1. **No Bespoke DSL**: QCC will not introduce a custom `.qcc` programming language. OpenQASM 3 is the standard textual interchange format.
2. **QccIR is Not Microsoft QIR**: QccIR is an in-memory dependency DAG implemented in Rust. Microsoft QIR is an LLVM-based bitcode representation. Interoperability with QIR is strictly an export target (Phase 81).
3. **No Unbounded Dynamic Programs in v1.0**: General loops and conditionals require the post-v1.0 CFG layer. v1.0 handles static quantum circuits with terminal or bounded mid-circuit measurements.
4. **No Pulse-Level Control**: QCC targets gate-level quantum compilation. OpenPulse / Qiskit Pulse scheduling is deferred.
5. **No Quantum Error Correction (QEC)**: Logical-to-physical QEC architectures (surface codes, lattice surgery) are separate disciplines.
6. **No GPU or Distributed Simulator**: The primary simulator is an in-place CPU statevector engine optimized for exactness and transparency.
7. **No Unsupported Hardware Execution Claims**: Passing "topology validity" means satisfying graph connectivity and gate directions; it does not guarantee execution fidelity on physical quantum hardware without live calibration and credentials.

