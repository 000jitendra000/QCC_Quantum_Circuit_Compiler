# QccIR Architecture and Specification

## 1. Overview

**QccIR** is the canonical intermediate representation (IR) of the **QCC (Quantum Circuit Compiler)**.

QccIR models a quantum program as a collection of typed registers, an explicit circuit-level global phase factor, and **one dependency Directed Acyclic Graph (DAG) per basic block**.

> [!IMPORTANT]
> **Naming & Ecosystem Separation**:
> QccIR is an internal, native Rust data structure owned by QCC. It is **never** referred to as "Microsoft QIR". Microsoft's QIR is an LLVM-based quantum IR; interoperability with it is strictly an external emission/export target in later phases.

---

## 2. Structural Model

A [`Circuit`](../src/ir/circuit.rs) is the primary root entity in QccIR:

```
+-----------------------------------------------------------------------+
| Circuit: name, global_phase (ParameterExpr), metadata                 |
|                                                                       |
|  +--------------------------------+  +------------------------------+ |
|  | Quantum Registers              |  | Classical Registers          | |
|  | - "q": [0..n)                  |  | - "c": [0..m)                | |
|  | - "ancilla": [n..n+k)          |  | - "meas": [m..m+l)           | |
|  +--------------------------------+  +------------------------------+ |
|                                                                       |
|  +------------------------------------------------------------------+ |
|  | CircuitGraph (Dependency DAG)                                    | |
|  | - Nodes: OperationId -> NodeData(sequence_number, Operation)     | |
|  | - Edges: WireDependency(wire, kind)                              | |
|  | - Hazard Tracking: WireAccessState per wire                      | |
|  +------------------------------------------------------------------+ |
+-----------------------------------------------------------------------+
```

### 2.1 Typed Registers and Wires

QccIR segregates quantum and classical state into distinct typed identifiers:

- **`QubitId(usize)`**: Represents a logical (or routed physical) qubit index in the circuit's flat qubit index space.
- **`ClassicalBitId(usize)`**: Represents a classical bit index in the circuit's flat classical bit index space.
- **`WireId`**: Disjoint enum (`Quantum(QubitId)` or `Classical(ClassicalBitId)`).
- **`QuantumRegister`**: Named slice of contiguous `QubitId`s.
- **`ClassicalRegister`**: Named slice of contiguous `ClassicalBitId`s.

Register validation enforces:
1. Unique register names across the circuit.
2. Contiguous, non-overlapping index allocation.
3. Operation operands strictly bounded within declared register capacity.

---

## 3. Strict Decoupling from Graph Library Internals

Internally, [`CircuitGraph`](../src/ir/graph.rs) encapsulates `petgraph::stable_graph::StableDiGraph`.

To prevent leaky abstractions and maintain ABI stability across compiler passes:
- **`petgraph::graph::NodeIndex` and `EdgeIndex` are strictly private.**
- Every operation is assigned a permanent semantic [`OperationId(u64)`](../src/ir/node.rs).
- All graph query methods (`get_operation`, `predecessors`, `successors`, `all_edges`) accept and return `OperationId` and `WireDependency`.
- Even if nodes are added or removed during graph transformation passes, `OperationId` remains constant.

---

## 4. Per-Wire Data Hazard Modeling

In quantum computing and hybrid classical-quantum control, operations on wires can induce hazardous data races if not properly sequenced.

A naive single-pointer representation (tracking only the "last operation" on a wire) is **strictly insufficient** because multiple concurrent operations can read a classical bit before a subsequent write.

QccIR introduces [`WireAccessState`](../src/ir/wire.rs) per wire:

```rust
pub struct WireAccessState {
    pub last_writer: Option<OperationId>,
    pub outstanding_readers: Vec<OperationId>,
}
```

### 4.1 Hazard Classifications & Edge Induction

When an operation executes:

1. **Quantum Updates** (`record_quantum_update`):
   - Quantum operations mutate the unitary statevector of the qubit (both reading and writing).
   - An edge of kind `QuantumOrder` (or `BarrierBoundary`) is created from `last_writer` and from **all** `outstanding_readers` to the new operation.
   - The new operation becomes the new `last_writer`.

2. **Classical Reads** (`record_read`):
   - **Read-After-Write (RAW)**: A dependency edge of kind `ClassicalRaw` is created from `last_writer` to the reader.
   - The reader is appended to `outstanding_readers`.
   - `last_writer` is retained.

3. **Classical Writes** (`record_write`):
   - **Write-After-Write (WAW)**: A dependency edge of kind `ClassicalWaw` is created from `last_writer` to the new writer.
   - **Write-After-Read (WAR)**: A dependency edge of kind `ClassicalWar` is created from **every outstanding reader** to the new writer.
   - `outstanding_readers` is cleared, and the new operation becomes `last_writer`.

4. **Synchronization Barriers** (`Barrier`):
   - Creates a `BarrierBoundary` dependency on all declared qubits, preventing optimization passes from reordering operations across the barrier.

---

## 5. Circuit-Level Global Phase

In accordance with project invariant [`.agents/rules/qcc-project-invariants.md`](../.agents/rules/qcc-project-invariants.md), circuit global phase is explicitly preserved:

$$U_{\text{circuit}} = e^{i\phi} U_{\text{operations}}$$

In QccIR:
- Global phase is tracked on [`Circuit::global_phase`](../src/ir/circuit.rs) using [`ParameterExpr`](../src/ir/parameter.rs).
- `ParameterExpr` supports both numeric constant angles and symbolic algebraic expressions (`Symbolic`, `Add`, `Sub`, `Mul`, `Div`, `Neg`).
- When decomposing gates or applying phase gadgets, phase factors are accumulated into `Circuit::global_phase` and never silently discarded.

---

## 6. Deterministic Topological Ordering

Standard graph topological sorting algorithms often introduce non-deterministic tie-breaking based on internal hash table iteration or pointer addresses.

QccIR guarantees **100% deterministic, platform-independent topological traversal**:
- Implemented via Kahn's algorithm.
- Ready nodes (in-degree zero) are queued into a `std::collections::BTreeSet<OperationId>`.
- Nodes are dequeued strictly in increasing order of `OperationId` (which corresponds to insertion order).
- This guarantees identical serialization output and identical compiler pass transformations across runs.

---

## 7. Deterministic Serialization & Schema

QccIR includes a deterministic JSON serialization format in [`src/ir/serialize.rs`](../src/ir/serialize.rs):

- Operations are emitted in deterministic topological order.
- Dependencies are emitted sorted by `(from_id, to_id, dependency)`.
- Global phase, registers, metadata, and full parameter expressions are preserved.
- Deserialization automatically validates all structural and semantic invariants before returning a `Circuit`.
