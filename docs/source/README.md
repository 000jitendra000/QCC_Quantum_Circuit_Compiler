# Primary Engineering Specification Source (Historical)

> [!NOTE]
> **Historical Artifact Notice**: This directory preserves the original engineering roadmap specification (`original-roadmap.docx`). Phases 1–59 were implemented and verified against this specification. The remaining phases (60–105) have been **retired**, and development is currently paused after the integrated compilation pipeline checkpoint (Phase 59). Future development priorities and the next roadmap will be determined separately (TBD).

This directory preserves the authoritative primary engineering specification for the QCC (Quantum Circuit Compiler) project.

## Preserved Artifacts

- **File**: [`original-roadmap.docx`](./original-roadmap.docx)
- **Original Filename**: `QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`
- **Document Title**: *QCC — Quantum Circuit Compiler: Detailed Engineering Roadmap, Architecture Contracts and Implementation Specification*
- **Document Date**: 01 November 2026
- **Document Size**: 55 pages, 17,643 words, 141,602 characters
- **Scope**: 105 granular implementation phases, 9 core milestones, 1 advanced research track
- **Sha256**: Preserved byte-for-byte from the user upload.

## Document Structure

The specification consists of seven primary parts:

1. **Part I — Executive Architecture Decisions**: Product thesis, compilation pipeline, QccIR vs CFG design, scope boundaries (v0.1 vs v1.0 vs research).
2. **Part II — Formal Semantics and Interface Contracts**: Little-endian qubit indexing ($q_0$ as LSB), column-vector matrix conventions, circuit multiplication order ($B \cdot A$), explicit global phase tracking, wire effect models, QccIR data structures, Target/Layout models, Scheduler/Simulator contracts, and Pass Manager contracts.
3. **Part III — 105-Phase Implementation Roadmap**: Complete implementation units for Phases 1 through 105 with objectives, concepts, prerequisites, ordered tasks, files/modules, tests, definition of done, failure modes, relative effort, and explicit exit gates.
4. **Part IV — Execution Order, Milestones and First Sessions**: Critical path analysis, milestone boundaries, detailed plan for the first five coding sessions, workspace evolution plan, and dependency policy.
5. **Part V — Algorithm Notes and Verification Policy**: DAG construction algorithm, unitary equivalence verification up to a common global phase, layout-aware routed circuit equivalence, fixed gate templates (SWAP, CX-CZ, Toffoli), SABRE algorithm breakdown, in-place statevector kernel mathematics and complexity, numerical tolerances, and pass verification checklist.
6. **Part VI — Risks, Exclusions and Review Decisions**: Risk register, deferred features and revisit triggers, accepted review changes, and scope boundaries.
7. **Part VII — Landscape and Source References**: Upstream OpenQASM specifications, petgraph, SABRE literature (Li et al. ASPLOS 2019), transpiler prior art, differential simulator references (Qiskit Aer, Cirq, Stim), and final acceptance checklist.

## Role in Engineering Governance

This original document is preserved as the foundational historical source of truth for architectural contracts, mathematical conventions, and design specifications implemented across Phases 1–59. The roadmap beyond Phase 59 has been retired; future development priorities and the next roadmap will be determined separately.

