---
description: Non-negotiable architectural, mathematical, and implementation invariants for QCC (Quantum Circuit Compiler)
trigger: always_on
---

# QCC Non-Negotiable Project Invariants

Every agent session working on the **QCC (Quantum Circuit Compiler)** codebase must strictly enforce the following rules and invariants without exception.

---

## 1. Mathematical Conventions & Semantics

1. **Little-Endian Qubit Indexing**:
   - Logical qubit $q[0]$ is the **least-significant bit (LSB)** in the statevector integer index.
   - Computational basis states are displayed as $|q_{n-1} \, q_{n-2} \dots q_1 \, q_0\rangle$ (rightmost bit is $q[0]$).
   - Statevector index $i = \sum_{k=0}^{n-1} q_k \cdot 2^k$.
2. **Column Vectors & Execution Order**:
   - Statevectors are column vectors. Operations act by left multiplication.
   - If operation $A$ executes before operation $B$, their combined operator is $B \cdot A$.
3. **Gate Operand Order**:
   - For $CX(q_{\text{ctrl}}, q_{\text{targ}})$: **Operand 0 is control**, **Operand 1 is target**.
4. **Circuit-Level Global Phase**:
   - Preserve explicit circuit global phase: $U_{\text{circuit}} = e^{i\phi} U_{\text{operations}}$.
   - Never discard global phase during decomposition or synthesis.
   - For equivalence checking, verify against **one single common global phase** across all test states. **Never choose an independent phase per test state or basis column.**
5. **No Measurement-Distribution Equivalence**:
   - Equal measurement outcome probabilities do NOT establish unitary equivalence. Unitary equivalence requires statevector amplitude comparison up to a common global phase.

---

## 2. QccIR & Dependency Invariants

1. **QccIR Identity**:
   - QCC's internal representation is **QccIR** (a typed dependency DAG per basic block).
   - **Never refer to QccIR as Microsoft QIR**. QIR is Microsoft's LLVM-based quantum IR; interoperability with it is strictly an export target.
2. **Classical & Quantum Wire Hazards**:
   - Orderings on classical wires must model **Read-After-Write (RAW)**, **Write-After-Read (WAR)**, and **Write-After-Write (WAW)**.
   - Maintain a per-wire `WireAccessState` (last writer + outstanding readers). A single previous-node pointer is strictly insufficient.
3. **Stable Semantic Operation IDs**:
   - Use QCC-owned stable `OperationId` (e.g., `u64`).
   - **Never expose petgraph `NodeIndex` or `EdgeIndex` as permanent semantic IDs**.
4. **Transactional Rewriting**:
   - Graph transformations must use centralized transactional rewrite helpers with automatic dependency reconnection, cycle detection, and rollback. Direct petgraph mutation across passes is prohibited.
5. **Deterministic Serialization**:
   - Graph traversals, serialization, and compilation artifacts must follow deterministic topological order.

---

## 3. Target, Layout, & Ancilla Invariants (ADR-0001)

1. **Hardware-Independent Target Model**:
   - Target operations, coupling graphs, and directionalities must be dynamically resolved from `Target`. Never hardcode vendor-specific bases in compiler passes.
2. **The "Explicit Unknown" Principle**:
   - Missing hardware timing or calibration data is modeled as `None` (`Unknown`), **never assumed to be zero**.
3. **Injective Layout Tracking**:
   - Logical qubits ($Q$) and physical qubits ($P$) use distinct typed IDs.
   - The compiler must explicitly track both initial layout ($L_{\text{initial}}$) and final layout ($L_{\text{final}}$) across all $SWAP$ insertions.
4. **Strict Ancilla Zero-State Policy (ADR-0001)**:
   - Every physical qubit not occupied by a logical qubit starts in $|0\rangle$ and **must return to $|0\rangle$ at circuit termination**.
   - The layout-aware verifier verifies the complete $2^n$-dimensional physical state. Silently tracing out or ignoring unused physical qubits is strictly prohibited.
5. **Final Target Legality Check**:
   - All physical circuits must pass the final target legality validator before scheduling or backend export.

---

## 4. Simulator Invariants

1. **In-Place $\mathcal{O}(2^n)$ Kernels**:
   - Normal simulation uses `num_complex::Complex64`.
   - Single-qubit gates act on amplitude pairs ($\mathcal{O}(2^n)$).
   - Two-qubit gates act on amplitude quartets ($\mathcal{O}(2^n)$).
   - **Never allocate or construct dense $2^n \times 2^n$ global matrices during ordinary simulation.**
2. **Deterministic Sampling**:
   - Terminal measurements sample from the final probability distribution using a seeded RNG without re-simulating the circuit per shot.
3. **Resource Limits**:
   - Check state memory ($16 \cdot 2^n$ bytes) against resource limits before allocation. Refuse unsafe allocations cleanly with structured errors.

---

## 5. Engineering Governance & Execution Boundary

1. **Stable Rust Toolchain**:
   - Implement strictly in stable Rust. No nightly features, no unverified unsafe code.
2. **Strict Task Bounding**:
   - Implement **only the explicitly requested task**.
   - Note that QCC development is currently paused after Phase 59 and the original 105-phase roadmap has been retired. Do not implement retired phases (60–105).
   - Do not jump ahead, do not write speculative code, and do not introduce unapproved architectural changes.
3. **Exit Gates Are Hard Gates**:
   - A phase is not complete until its explicit exit gate passes with automated test proof.
   - Update `docs/project-state/progress.md` only with verified test evidence.

