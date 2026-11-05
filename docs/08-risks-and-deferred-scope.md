# QCC Risk Register, Deferred Scope, and Accepted Decisions

## 1. Project Risk Register

> [!NOTE]
> **Status Note**: QCC development is currently paused after the integrated compilation pipeline checkpoint (Phase 59). The original 105-phase roadmap has been retired; future development priorities and risks will be evaluated separately.

The following register identifies the highest-severity failure modes in the QCC compiler architecture along with mandatory mitigation strategies:

| Risk Category | Severity | Potential Failure Mode | Mandatory Mitigation Strategy |
|---|:---:|---|---|
| **Endian & Operand Drift** | **Very High** | Inverting little-endian convention ($q_0$ as LSB) or swapping control/target operands creates silent state inversion. | Freeze conventions in Phase 2. Enforce Dirac notation $|q_{n-1} \dots q_0\rangle$ in all tests. Cross-check against Qiskit Aer and OpenQASM emitter golden files. |
| **DAG Dependency Corruption** | **Very High** | Missing classical RAW/WAR/WAW hazards or barrier ordering edges allows invalid parallel execution. | Typed `WireDependency` edges. Per-wire `WireAccessState` (last writer + outstanding readers). Run automated invariant validator after every pass. |
| **Global Phase Loss** | **Very High** | Discarding global phase during decomposition causes catastrophic relative phase errors if subcircuits are later controlled. | Explicit circuit-level global phase. All decomposition/fusion routines must return $(\text{gates}, \Delta\phi)$. Equivalence checker requires a single common phase. |
| **Incorrect Optimizer Rule** | **Very High** | Applying an algebraic identity that is invalid under non-zero phase or non-commuting parameters alters program semantics. | Every identity rule requires mathematical proof, precondition guards, matrix checks, and randomized `proptest` equivalence testing. |
| **OpenQASM Scope Explosion** | **High** | Attempting to parse the complete OpenQASM 3 language (including classical functions and complex types) delays core compiler development. | Restrict frontend to a strictly bounded static circuit subset. Explicitly detect and reject unsupported grammar with actionable diagnostics. |
| **Basis Translation Cycles** | **Very High** | Recursive decomposition without cycle detection or depth bounding causes compiler stack overflow. | Directed translation rule graph with cycle detection, explicit recursion depth budgets, and fallback to verified fixed templates. |
| **Routing Permutation Bug** | **Very High** | Mismatch between physical $SWAP$ insertions and logical-to-physical layout mappings produces corrupted physical outputs. | Injective `Layout` tracking. Every route modification must pass layout-aware equivalence with ADR-0001 ancilla zero-state enforcement. |
| **SABRE Implementation Drift** | **High** | Implementing a naive greedy router and incorrectly claiming it is SABRE leads to sub-optimal compilation and false architectural claims. | Separate baseline shortest-path router (Phase 47) from SABRE (Phases 50–52). SABRE must include front layer, lookahead, decay, and bidirectional refinement. |
| **Statevector Memory Exhaustion** | **High** | Attempting to allocate $>30$ qubits on desktop hardware causes catastrophic out-of-memory OS aborts. | Mandatory resource estimation ($16 \cdot 2^n$ bytes). Check user limits before allocation; return structured `SimulatorError::ResourceLimitExceeded`. |
| **Stale / Unknown Calibration** | **High** | Treating missing hardware error rates or durations as zero produces severely distorted scheduling and routing decisions. | The "Explicit Unknown" rule: missing data is modeled as `None` (`Unknown`), never defaulted to zero. |
| **Provider API Volatility** | **Medium–High**| Hardware vendor cloud APIs change rapidly, breaking integrated remote backends. | All remote adapters reside in optional downstream crates (`qcc-backend-*`). Core compilation remains 100% functional offline without cloud credentials. |
| **Scope Expansion & Burnout** | **Very High** | Attempting premature research tracks without clear prioritization causes project stagnation. | Strict phase governance. The original 105-phase roadmap has been retired; future work will be scoped and prioritized separately. |

---

## 2. Deferred Features and Revisit Triggers

To prevent scope creep, advanced capabilities are explicitly deferred until concrete technical prerequisites are satisfied:

| Deferred Capability | Architectural Rationale for Deferral | Explicit Revisit Trigger |
|---|---|---|
| **Custom `.qcc` DSL** | OpenQASM 3 and the native Rust `CircuitBuilder` API fully cover circuit specification needs. | A concrete ergonomic or quantum domain limitation is demonstrated that OpenQASM 3 cannot express. |
| **Full Classical Dynamic CFG** | General loops, branches, and integer expressions require a complex classical type system and control-flow graph. | Milestone M6 completes and static compilation with classical wire hazards is 100% verified. |
| **Arbitrary KAK $SU(4)$ Synthesis** | Demands extensive numerical linear algebra, canonical angle extraction, and numerical degeneracy handling. | Fixed decomposition templates and single-qubit Euler synthesis are verified, and an application requires arbitrary $4 \times 4$ unitaries. |
| **Quantum Error Correction (QEC)** | QEC involves logical surface codes, syndrome extraction, and decoder integration, which is a distinct architectural layer. | A dedicated logical-qubit / fault-tolerant research objective is established. |
| **Pulse-Level Hardware Control** | Requires vendor-specific waveform generators, port calibrations, and analog hardware descriptors. | A partner quantum hardware provider exposes documented, open pulse-level API access. |
| **Analog Hamiltonian Simulation** | Relies on continuous-time differential equation solvers rather than discrete gate application. | A concrete analog quantum computing research goal is formulated. |
| **GPU / Distributed Simulation** | Substantially increases build complexity, hardware dependencies, and unsafe FFI bindings. | Statevector CPU bottlenecks are measured on real benchmarks and justify dedicated CUDA/OpenCL kernels. |

---

## 3. Accepted Architectural Review Decisions

The following architectural decisions have been formally approved and incorporated into the permanent specification:

1. **Primary IR is a DAG Per Basic Block**: QccIR represents operations as nodes in a dependency DAG per basic block. A CFG is introduced only as a later supervisory layer for structured control flow.
2. **Classical Wires from Day One**: Classical wires, measurement, reset, barriers, parameters, and global phase exist in QccIR from the very first data model.
3. **Rust Builder Before Textual Parser**: Core circuit construction, QccIR, and the simulator are proven using the Rust builder before writing an OpenQASM 3 grammar parser.
4. **Early OpenQASM 3 Emitter**: The emitter is built in Phase 9 (Milestone M1) to enable external parser validation of the Bell vertical slice immediately.
5. **Equivalence Checking Precedes Optimization**: Automated correctness verification (Phase 10) must exist before any optimizer pass is developed.
6. **In-Place Kernels Over Dense Matrices**: All single- and two-qubit operations execute in $\mathcal{O}(2^n)$ time directly on amplitude vectors; dense $2^n \times 2^n$ operators are strictly prohibited.
7. **Terminal Sampling vs Dynamic Trajectories**: Terminal measurements evolve the state once and sample from the resulting probability distribution. Mid-circuit measurements use stochastic trajectories or exact branch enumeration.
8. **Target-Aware Basis Translation Precedes Routing**: Circuits are decomposed into hardware-native entanglers before routing along physical coupling edges.
9. **Scheduling Consumes Final Routed Physical DAG**: Scheduling requires physical operation durations and resources; it must occur strictly after routing.
10. **SABRE Requires Lookahead, Decay, and Bidirectional Refinement**: Greedy shortest-path routing is a distinct baseline and must never be labeled as SABRE.
11. **ADR-0001 Ancilla Zero-State Policy**: All unused physical qubits start in $|0\rangle$ and must return to $|0\rangle$ at circuit exit, verified without silently tracing them out.
12. **QccIR is Not Microsoft QIR**: QccIR is an internal Rust DAG. Microsoft QIR is an optional export profile.
13. **Topology-Valid Means Couplings Satisfied**: Topology validity does not imply calibrated or noise-optimized execution.
14. **Stable Rust Toolchain Only**: All builds, tests, and CI runs strictly require stable Rust with zero nightly dependencies.

