# ADR-0001: Strict Ancilla and Unused Physical-Qubit Zero-State Verification Policy

## Status
**Accepted** (01 November 2026)

## Context
When mapping and routing a logical circuit of $m$ logical qubits onto a physical target device with $n$ physical qubits (where $n > m$), $n - m$ physical qubits are unused by the logical program. These physical qubits act as hardware ancillae and may be utilized by the compiler (e.g., during routing or decomposition) or remain idle.

In quantum compiler literature and in the original phrasing of Phase 45 (Task 4) of `QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`, the policy was described broadly as:
> *"Check unused physical qubits or compare reduced logical state under a defined policy."*

This phrasing left open the possibility of a permissive policy where the verification oracle merely traces out or discards unmapped physical qubits, comparing only the reduced logical state. 

However, in physical quantum execution, leaving unused physical qubits in an arbitrary non-zero state, or worse, leaving residual entanglement between ancillae and logical qubits, is a severe compiler bug. It can corrupt subsequent operations on those physical qubits, violate hardware assumptions, and cause silent information leakage.

## Decision
QCC formally adopts the **Strict Ancilla Zero-State Policy**:

1. **Initial State Requirement**: Every physical qubit not assigned to a logical qubit starts strictly in $|0\rangle$.
2. **Final State Requirement**: Every physical qubit not assigned to a logical qubit **must return to $|0\rangle$ at circuit termination**.
3. **Complete Physical State Verification**: The layout-aware verifier (Phase 45) will **not** silently trace out or discard unused physical qubits. Instead, it must:
   - Embed the logical input state into the full $n$-qubit physical register according to the initial layout $L_{\text{initial}}$, initializing all unmapped physical qubits to $|0\rangle$.
   - Simulate the routed physical circuit.
   - Construct the reference physical output by placing the reference logical output amplitudes according to the final layout $L_{\text{final}}$, embedding all unmapped physical qubits in $|0\rangle$.
   - Compare the complete $2^n$-dimensional physical statevector against the expected physical statevector up to one common scalar global phase:
     $$|\Psi_{\text{out}}^{\text{phys}}\rangle = e^{i\phi} |\Psi_{\text{expected}}^{\text{phys}}\rangle$$
   - Require all ancilla qubits to have returned to $|0\rangle$ with zero residual entanglement with logical qubits.

## Rationale
- **Correctness Guarantee**: Guarantees that routing passes and decomposition templates do not leave dirty ancillae, unreversed control operations, or uncomputed entanglement.
- **Hardware Realism**: On physical quantum hardware, ancillary qubits reused in subsequent algorithmic blocks must be in $|0\rangle$ to prevent crosstalk and noise amplification.
- **Fail-Closed Verification**: Comparing the complete $2^n$ statevector eliminates ambiguity and ensures that layout permutations and ancilla states are verified simultaneously.

## Relationship to Original Document Wording
This decision serves as an **explicit user-approved architecture refinement** to the original Phase 45 wording. The permissive alternative mentioned in Phase 45 (tracing out unused qubits) is formally superseded by this strict zero-state requirement.

## Implementation Consequences
1. `qcc-routing`: Initial placement and routers may utilize spare physical qubits for routing paths or intermediate $SWAP$s, but any operations altering ancilla states must be cleanly inverted before circuit exit.
2. `qcc-verify`: The `layout_equivalence` module must implement full physical state construction ($2^n$) and reject any routed circuit where an ancilla amplitude on basis states with bit value `1` exceeds $\epsilon_{\text{state}}$.
3. `qcc-report`: Compilation reports must explicitly confirm that all $n - m$ ancilla qubits terminated in $|0\rangle$.

## Required Tests
1. **Clean Route Pass**: Test a circuit where logical qubits are routed through ancilla qubits and returned to $|0\rangle$; verification must pass.
2. **Dirty Ancilla Rejection**: Construct a test circuit that leaves an ancilla qubit in $|1\rangle$; verification must fail with an explicit diagnostic indicating ancilla contamination.
3. **Entangled Ancilla Rejection**: Construct a test circuit where an ancilla qubit is left entangled with a logical qubit (e.g., $CX(\text{logical}, \text{ancilla})$ without uncomputing); verification must fail with a non-unitary / entanglement mismatch error.
4. **Layout Permutation Check**: Verify that when ancilla qubits are placed at non-contiguous physical indices (e.g., physical qubits 0 and 3 are logical, 1 and 2 are ancillae), the zero-state verification correctly checks the specific ancilla bit positions.

