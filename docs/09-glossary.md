# QCC Engineering Glossary

### Ancilla Qubit
An auxiliary physical qubit allocated on a quantum hardware target that is not part of the initial logical circuit. Under **ADR-0001**, all ancilla qubits must be initialized to $|0\rangle$ and must return to $|0\rangle$ at circuit completion without residual entanglement.

### ALAP (As Late As Possible)
A scheduling strategy that positions operations as late as possible along the circuit timeline without increasing the total makespan. ALAP is used in conjunction with ASAP to compute operation slack.

### ASAP (As Soon As Possible)
A scheduling strategy that positions operations at the earliest instant all predecessor wire dependencies and hardware resources become available.

### Basic Block (Quantum)
A maximal straight-line sequence of quantum and classical operations containing no interior branches or loop headers. In QCC, each basic block is represented internally as a dependency DAG.

### Bidirectional SABRE
An advanced layout and routing technique that first routes a circuit in reverse chronological order to determine an optimized initial layout $L_{\text{initial}}$, followed by forward SABRE routing to minimize final $SWAP$ overhead.

### CircuitBuilder
The primary Rust API used to construct quantum registers, allocate classical bits, append quantum gates, and configure circuit-level metadata prior to textual parsing.

### Classical Wire Dependency
An explicit causal ordering constraint enforced between operations accessing classical bits:
- **RAW (Read-After-Write)**: A classical read depends on a prior write.
- **WAR (Write-After-Read)**: A classical write depends on all preceding reads.
- **WAW (Write-After-Write)**: Successive writes maintain sequential order.

### Decay Factor (SABRE)
A penalty score applied to recently swapped physical qubits in the SABRE router to discourage oscillation (repeatedly swapping the same pair of qubits back and forth).

### Density Matrix ($\rho$)
An operator in $\mathbb{C}^{2^n \times 2^n}$ representing a pure or mixed quantum state: $\rho = \sum_i p_i |\psi_i\rangle\langle\psi_i|$. Memory scales as $\mathcal{O}(4^n)$.

### Euler Decomposition
The mathematical factorization of any single-qubit unitary $U \in U(2)$ into a scalar global phase and three elementary rotations around orthogonal axes (typically $Z\text{-}Y\text{-}Z$ or $Z\text{-}X\text{-}Z$).

### Exact Branch Enumeration
A deterministic simulation mode for dynamic circuits that recursively tracks all $2^m$ possible measurement collapse branches, weighting each path by its Born-rule probability.

### Front Layer ($F$)
In DAG-based circuit routing, the set of operations whose predecessor dependencies in the DAG are completely satisfied and are immediately ready to be scheduled or routed.

### Global Phase
A scalar complex factor $e^{i\phi}$ ($\phi \in \mathbb{R}$) multiplying an entire circuit unitary operator: $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$. While physically unobservable in an isolated system, global phase becomes observable relative phase when a subcircuit is conditioned or controlled.

### Injective Layout
A one-to-one mapping from logical qubits ($Q$) to physical device qubits ($P$). For $m$ logical qubits and $n \ge m$ physical qubits, each logical qubit maps to exactly one physical qubit.

### KAK Decomposition
The Lie-algebraic canonical decomposition of an arbitrary two-qubit unitary $U \in SU(4)$ into local single-qubit pre- and post-rotations and an entangling core:
$$U = (A_1 \otimes A_2) \cdot \exp\left(i \sum_{k=1}^3 c_k (\sigma_k \otimes \sigma_k)\right) \cdot (B_1 \otimes B_2)$$

### Kraus Operators
A set of matrices $\{K_k\}$ satisfying $\sum_k K_k^\dagger K_k = I$ that describe a completely positive trace-preserving (CPTP) quantum noise channel on a density matrix: $\rho' = \sum_k K_k \rho K_k^\dagger$.

### Little-Endian Statevector Indexing
A mathematical convention where logical qubit $q_0$ corresponds to the least-significant bit (LSB) of the statevector integer index: $i = \sum_{k=0}^{n-1} q_k \cdot 2^k$. Basis state $|q_{n-1} \dots q_0\rangle$ has $q_0$ at the rightmost position.

### Makespan
The total elapsed time from the start of the first physical operation to the completion of the last physical operation in a scheduled quantum circuit.

### Matrix Product State (MPS)
A tensor-network representation that factorizes an $n$-qubit statevector into a chain of $n$ rank-3 tensors with bounded bond dimension $\chi$, enabling efficient simulation of circuits with low entanglement.

### Microsoft QIR
Microsoft's LLVM-based Quantum Intermediate Representation bitcode standard. QCC is completely distinct from Microsoft QIR; QCC uses its own Rust DAG representation called **QccIR**.

### OperationId
A 64-bit integer (`u64`) uniquely identifying an operation within QccIR. Decoupled from graph-library (`petgraph`) internal node indices to guarantee stability across graph rewrites.

### OpenQASM 3
The standardized open textual quantum circuit description language used by QCC for external circuit interchange.

### QccIR
Quantum Circuit Compiler Intermediate Representation. A typed, dependency DAG per basic block that captures quantum and classical wire hazards, stable operation IDs, parameters, and circuit global phase.

### SABRE (SWAP-Based Bidirectional Heuristic Router)
The state-of-the-art heuristic circuit routing algorithm introduced by Li, Ding, and Xie (ASPLOS 2019) combining front-layer tracking, extended lookahead, decay penalties, and bidirectional refinement.

### Slack
The difference between an operation's latest allowable start time (from ALAP) and its earliest possible start time (from ASAP): $\text{slack} = t_{\text{ALAP}} - t_{\text{ASAP}}$. Operations with zero slack lie on the circuit's critical path.

### StateVector
An in-memory array of $2^n$ double-precision complex numbers (`Complex64`) representing the pure state of an $n$-qubit quantum system.

### StableDiGraph
The underlying directed graph structure from the `petgraph` crate used internally by QccIR to store DAG nodes and dependency edges without index invalidation upon node removal.

### Terminal Sampling
An optimized simulation mode for circuits where measurements occur solely at circuit completion. The unitary state is evolved exactly once, and all requested shots are drawn from the final probability distribution using seeded cumulative distribution sampling.

### Topology-Valid
A guarantee that every two-qubit gate in a routed circuit operates strictly between physical qubits connected by an allowed edge in the target coupling graph, matching required hardware directionality. It does **not** imply noise optimization or execution fidelity.

### Trajectory Simulation
A Monte Carlo quantum simulation technique that executes stochastic quantum circuits shot-by-shot, explicitly collapsing the statevector at mid-circuit measurements according to Born probabilities.

### Transactional DAG Rewriter
A centralized compiler infrastructure component (Phase 13) that performs graph mutations (insertion, removal, replacement, fusion) with automated dependency repair, cycle detection, and rollback on invariant failure.

