# QCC Formal Semantics and Data Contracts

## 1. Qubit Ordering and Basis Convention

QCC adopts a **little-endian statevector indexing convention** to align with standard computational-basis tensor indexing (and direct compatibility with Qiskit/OpenQASM 3 ecosystem conventions):

### 1.1 Mathematical Definition
For an $n$-qubit register:
- **Logical $q[0]$ is the least-significant bit (LSB)** in the statevector integer index.
- **Logical $q[n-1]$ is the most-significant bit (MSB)**.
- Computational basis states are displayed in Dirac notation as:
  $$|q_{n-1} \, q_{n-2} \, \dots \, q_1 \, q_0\rangle$$
  The rightmost displayed bit corresponds to $q[0]$.
- The corresponding statevector index $i \in \{0, 1, \dots, 2^n - 1\}$ is given by:
  $$i = \sum_{k=0}^{n-1} q_k \cdot 2^k$$

### 1.2 Two-Qubit Computational Basis Mapping

| Qubit Basis Label $|q_1 q_0\rangle$ | Statevector Index ($i$) | Amplitude Vector Position | Bit Assignments |
|:---:|:---:|:---:|---|
| $|00\rangle$ | 0 | Index 0 | $q_1 = 0, \; q_0 = 0$ |
| $|01\rangle$ | 1 | Index 1 | $q_1 = 0, \; q_0 = 1$ |
| $|10\rangle$ | 2 | Index 2 | $q_1 = 1, \; q_0 = 0$ |
| $|11\rangle$ | 3 | Index 3 | $q_1 = 1, \; q_0 = 1$ |

> [!WARNING]
> **Classical Bitstring Output Convention**:
> Measurement result bitstrings (e.g., `"01"`) must adhere to an explicitly documented register formatting policy. Classical bit formatting must never be casually inferred from quantum statevector index ordering.

---

## 2. Matrix, Tensor, and Gate Conventions

### 2.1 Statevectors and Multiplication Order
- **Column Vectors**: Quantum statevectors are represented as column vectors in $\mathbb{C}^{2^n}$:
  $$|\psi\rangle = \sum_{i=0}^{2^n - 1} \alpha_i |i\rangle$$
- **Left Multiplication**: Gates act on statevectors by left multiplication in circuit chronological order:
  $$|\psi_{\text{final}}\rangle = U_k \dots U_2 U_1 |\psi_{\text{initial}}\rangle$$
- If operation $A$ executes before operation $B$, their composite unitary operator is:
  $$U_{\text{composite}} = B \cdot A$$

### 2.2 Gate Operand Order
- In multi-qubit operations, operands are ordered explicitly according to their declared semantic roles.
- For the standard controlled-NOT gate $CX(q_{\text{ctrl}}, q_{\text{targ}})$:
  - **Operand 0**: Control qubit ($q_{\text{ctrl}}$).
  - **Operand 1**: Target qubit ($q_{\text{targ}}$).
- For $CX(q_0, q_1)$, $q_0$ controls $q_1$. In matrix form on basis $\{|00\rangle, |01\rangle, |10\rangle, |11\rangle\}$:
  $$CX(q_0 = \text{ctrl}, q_1 = \text{targ}) = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \end{pmatrix}$$
  Whereas with $q_1$ as control and $q_0$ as target:
  $$CX(q_1 = \text{ctrl}, q_0 = \text{targ}) = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \end{pmatrix}$$

---

## 3. Global Phase Tracking

Every circuit carries an explicit circuit-level global phase:
$$U_{\text{circuit}} = e^{i \phi} \cdot U_{\text{operations}}$$

### 3.1 Non-Negotiable Phase Rules
1. **Never Discard Intermediate Phase**: A phase factor that is global to an uncontrolled subcircuit becomes physically observable relative phase when that subcircuit is placed under coherent control.
2. **Phase-Aware Decomposition**: All gate synthesis and decomposition APIs (such as single-qubit Euler decomposition and two-qubit KAK synthesis) must return both the synthesized gate sequence and the extracted scalar global phase contribution:
   $$\text{decompose}(U) \to (\text{operations}, \Delta \phi)$$
3. **Single Global Phase for Equivalence**: When comparing two uncontrolled unitary circuits, equivalence holds if and only if there exists a **single common scalar phase** $\phi \in [0, 2\pi)$ such that:
   $$U_{\text{candidate}} = e^{i\phi} \cdot U_{\text{reference}}$$
   **Selecting a separate phase per basis state or per test input is strictly prohibited**, as it masks catastrophic relative-phase bugs.

---

## 4. Operation Semantics and Wire Effects

QccIR classifies all circuit operations into distinct semantic categories with rigorous wire-effect rules:

| Operation | Quantum Wire Effects | Classical Wire Effects | Semantic Class |
|---|---|---|---|
| $H(q_0)$, $RZ(q_0, \theta)$ | Read/Write $q_0$ | None | Unitary |
| $CX(q_0, q_1)$ | Read/Write $q_0$ and $q_1$ | None | Unitary |
| `measure q0 -> c0` | Read/Collapse $q_0$ | Write $c_0$ | Non-unitary (Stochastic) |
| `reset q0` | Read/Write $q_0$ | None | Non-unitary (Channel) |
| `barrier(q0, q1)` | Ordering boundary on $q_0, q_1$ | None | Ordering / Control |
| Classical Condition | None (unless gate touches qubits) | Read condition bits | Classical Control |

### 4.1 Dependency Maintenance on Classical Wires
A simplistic "pointer to last operation" fails to capture general classical dependencies. QccIR models all data hazards on both quantum and classical wires:
- **Read-After-Write (RAW)**: A classical operation or conditioned gate reading bit $c_k$ must depend on the prior measurement or operation that wrote $c_k$.
- **Write-After-Read (WAR)**: A measurement writing $c_k$ must depend on all preceding classical operations that read the prior value of $c_k$.
- **Write-After-Write (WAW)**: Successive writes to the same wire must preserve sequential execution order.

**Implementation Requirement**: The graph builder must maintain a per-wire `WireAccessState`:
```rust
pub struct WireAccessState {
    pub last_writer: Option<OperationId>,
    pub outstanding_readers: Vec<OperationId>,
}
```

---

## 5. QccIR Structural Data Contract

```rust
use std::collections::HashMap;

/// Canonical circuit intermediate representation.
pub struct QccCircuit {
    pub quantum_registers: Vec<QuantumRegister>,
    pub classical_registers: Vec<ClassicalRegister>,
    pub graph: CircuitDag,
    pub global_phase: ParameterExpr,
    pub metadata: CircuitMetadata,
}

/// Identifiers for physical and logical wires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WireId {
    Quantum(QubitId),
    Classical(BitId),
}

/// Directed dependency edge carrying the inducing wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireDependency {
    pub wire: WireId,
    pub kind: DependencyKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    QuantumOrder,
    ClassicalRaw,
    ClassicalWar,
    ClassicalWaw,
    BarrierBoundary,
}

/// Node operations within a QccIR basic-block DAG.
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Gate {
        gate: GateId,
        qubits: Vec<QubitId>,
        parameters: Vec<ParameterExpr>,
    },
    Measure {
        qubit: QubitId,
        destination: BitId,
    },
    Reset {
        qubit: QubitId,
    },
    Barrier {
        qubits: Vec<QubitId>,
    },
    Classical {
        operation: ClassicalOperation,
    },
}

/// Stable semantic identifier for operations, decoupled from petgraph indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(pub u64);
```

### 5.1 Enforceable Graph Invariants
1. **Acyclicity**: The dependency graph must contain zero directed cycles (`petgraph::algo::is_cyclic_directed(&dag) == false`).
2. **Wire Consistency**: Every edge must reference a wire legitimately touched by both source and destination operations.
3. **Arity and Parameter Signatures**: The number of operand qubits and parameters in an `Operation::Gate` must match the gate's definition in the canonical catalog.
4. **Stable Semantic IDs**: Graph mutation (insertion, deletion, fusion) must preserve stable semantic `OperationId`s for unchanged operations.
5. **Deterministic Serialization**: Topological traversal for serialization or execution must produce identical, deterministic node sequences across executions and platforms.

