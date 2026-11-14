# Milestone M4 Implementation Report — Generic Target & Basis Translation (Phases 37–43)

```text
Milestone: Milestone M4 (Generic Target & Basis Translation)
Phases Covered: Phases 37–43
Status: Completed & Accepted
Historical Checkpoint: Milestone M4 Baseline
Artifacts: src/target/, src/synthesis/, tests/target.rs, tests/synthesis.rs
```

---

## 1. Executive Summary

Milestone M4 engineered QCC's target-aware hardware abstraction and recursive basis translation subsystem, allowing the compiler to ingest circuits written with arbitrary standard gates and synthesize them into native physical gate sets (such as `{rz, sx, cx}` or `{rx, ry, cz}`) over directed hardware coupling graphs, concluding with strict fail-closed target legality validation.

---

## 2. Sub-Phase Architecture & Deliverables

| Phase | Title | Core Deliverables & Architectural Contracts |
|:---:|---|---|
| **37** | Target Architecture Model | Strongly typed hardware model capturing physical qubit counts, directed/undirected coupling topologies, native gate sets, and basis configurations. |
| **38** | Target Capability Queries | High-performance query engine checking whether a target natively supports a given gate operation on specific physical qubit operands. |
| **39** | 1Q Synthesis Templates | Exact mathematical decomposition templates expressing non-native single-qubit gates ($H$, $X$, $Y$, $Z$, $S$, $T$) in terms of native rotations ($R_z$, $\sqrt{X}$) with exact scalar global phase tracking. |
| **40** | 2Q Synthesis & KAK | Two-qubit synthesis templates and decomposition rules synthesizing $SWAP \to 3 \times CX$, $CZ \to H \cdot CX \cdot H$, and reversed $CX$ direction corrections. |
| **41** | Recursive Basis Translation | Pass recursively expanding non-native DAG operations through synthesis templates until all operations in the circuit strictly belong to the target's native basis. |
| **42** | Target Legality Validation | Strict validator failing closed with structured errors if any non-native gate or unsupported interaction edge remains in the circuit. |
| **43** | Basis Translation Acceptance | Comprehensive test suite validating synthesis correctness, phase preservation, and target legality across diverse simulated hardware architectures. |

---

## 3. Mathematical Foundations & Directionality

- **Native Basis Decomposition**: For transmon architectures with native basis $\{R_z(\theta), \sqrt{X}, CX\}$:
  $$H = R_z\left(\frac{\pi}{2}\right) \cdot \sqrt{X} \cdot R_z\left(\frac{\pi}{2}\right), \quad \phi = \frac{\pi}{4}$$
- **Reversed CX Synthesis**: Over directed coupling edges $(p_1 \to p_2)$ where $CX(p_2, p_1)$ is requested:
  $$CX(p_2, p_1) = (H \otimes H) \cdot CX(p_1, p_2) \cdot (H \otimes H)$$
- **Fail-Closed Principle**: Target legality runs after basis translation and after routing, guaranteeing illegal gates never reach backend emission.

---

## 4. Verification Evidence & Exit Gate

- **Unitary Equivalence**: All synthesized circuits verified against original multi-gate unitaries using double-precision matrix multiplication and statevector simulation.
- **Directionality Enforcement**: Verified that reversed edges are correctly conjugated with Hadamard decompositions.
- **Exit Gate**: All 7 phases completed, all tests passed with zero warnings. Milestone M4 accepted.
