# OpenQASM 3 Language Support & Round-Trip Architecture

This document defines QCC's OpenQASM 3 support, compilation pipeline, canonical emitter behavior, and round-trip verification guarantees established in Milestone M2 (Phases 15–26).

---

## 1. Overview & Architectural Principles

The QCC OpenQASM 3 frontend and backend provide an end-to-end import/export pipeline between human-readable OpenQASM 3 source code and QCC's internal representation (**QccIR**).

### Core Design Principles

1. **Independent Verification Oracle**:
   QCC never relies solely on its own parser to validate its emitted OpenQASM output. All emitted programs are validated against the independent reference parser (`openqasm3[parser]==1.0.1`) to ensure strict language conformance.
2. **Semantic Equivalence Over Textual Equality**:
   Round-trip fidelity is defined semantically, not byte-for-byte. Formatting, comments, whitespace, register indexing, and measurement syntax forms may normalize to canonical representations without altering circuit semantics.
3. **Fail-Closed Boundary**:
   Every construct outside the accepted static subset is rejected with stable, source-located diagnostics (`E0001`–`E0005`). No unsupported syntax is silently ignored or partially ingested.
4. **Strict Invariant Preservation**:
   - **Little-Endian Indexing**: $q[0]$ is the least significant bit (LSB).
   - **Dirac Left-Multiplication**: If operation $A$ precedes $B$, the composite operator is $B \cdot A$.
   - **CX Operand Order**: Operand 0 is control; operand 1 is target.
   - **Explicit Global Phase**: $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$ is preserved across AST, QccIR, and emission.
   - **Wire Hazards**: Classical wires strictly track RAW, WAR, and WAW hazards via `WireAccessState`.

---

## 2. Supported Language Matrix

QCC supports a static, fully deterministic OpenQASM 3.0 language subset suitable for logical compilation and simulation.

### 2.1 Program Structure & Header Directives

| Statement | Syntax | Semantics & Emitter Behavior |
|---|---|---|
| Version Declaration | `OPENQASM 3.0;` or `OPENQASM 3;` | Emitted as `OPENQASM 3.0;\n` at the top of every generated file. |
| Standard Library Include | `include "stdgates.inc";` | Emitted by default when standard quantum gates are used. |

### 2.2 Register Declarations

Registers are zero-indexed and allocate contiguous flat ID ranges in QccIR.

| Declaration | Syntax | QccIR Lowering Representation | Emitter Output |
|---|---|---|---|
| Scalar Qubit | `qubit q;` | `QuantumRegister { name: "q", size: 1, start_qubit: i }` | `qubit[1] q;` |
| Qubit Array | `qubit[n] q;` | `QuantumRegister { name: "q", size: n, start_qubit: i }` | `qubit[n] q;` |
| Scalar Bit | `bit c;` | `ClassicalRegister { name: "c", size: 1, start_bit: j }` | `bit[1] c;` |
| Bit Array | `bit[m] c;` | `ClassicalRegister { name: "c", size: m, start_bit: j }` | `bit[m] c;` |

*Note*: Scalar declarations in source are lowered to size-1 registers in QccIR and canonicalized to `qubit[1] name;` upon emission.

### 2.3 Supported Quantum Gate Set (14 Canonical Gates)

All gate names are case-sensitive and must match standard lowercase mnemonics.

| Gate | Arity | Parameters | Mathematical Definition | Canonical Emission Form |
|---|:---:|:---:|---|---|
| `x` | 1 | 0 | Pauli-X ($NOT$) | `x q[i];` |
| `y` | 1 | 0 | Pauli-Y | `y q[i];` |
| `z` | 1 | 0 | Pauli-Z | `z q[i];` |
| `h` | 1 | 0 | Hadamard | `h q[i];` |
| `s` | 1 | 0 | Phase gate ($\sqrt{Z}$) | `s q[i];` |
| `sdg` | 1 | 0 | Conjugate transpose of S | `sdg q[i];` |
| `t` | 1 | 0 | $\pi/8$ gate ($\sqrt{S}$) | `t q[i];` |
| `tdg` | 1 | 0 | Conjugate transpose of T | `tdg q[i];` |
| `id` | 1 | 0 | Single-qubit identity ($I$) | `id q[i];` |
| `rx(θ)` | 1 | 1 | $\exp(-i \frac{\theta}{2} X)$ | `rx(<expr>) q[i];` |
| `ry(θ)` | 1 | 1 | $\exp(-i \frac{\theta}{2} Y)$ | `ry(<expr>) q[i];` |
| `rz(φ)` | 1 | 1 | $\exp(-i \frac{\phi}{2} Z)$ | `rz(<expr>) q[i];` |
| `cx` | 2 | 0 | Controlled-NOT ($CX(q_{\text{ctrl}}, q_{\text{targ}})$) | `cx q[c], q[t];` |
| `cz` | 2 | 0 | Controlled-Z | `cz q[a], q[b];` |
| `swap` | 2 | 0 | Qubit swap | `swap q[a], q[b];` |

### 2.4 Parameter Expressions

Parameter expressions are parsed into symbolic abstract syntax trees using a Pratt precedence parser. Constants are folded without loss of mathematical precision.

- **Literals**: Integers (`42`), standard decimals (`3.14159`), scientific notation (`1e-5`, `2.5E+2`).
- **Mathematical Constants**: `pi` and `π` (resolved to IEEE-754 $f64$ $\approx 3.141592653589793$).
- **Operators**: Unary `+`, `-`; Binary `+`, `-`, `*`, `/`.
- **Precedence**: Atoms $>$ Unary Negation $>$ Multiplicative (`*`, `/`) $>$ Additive (`+`, `-`).
- **Division by Zero**: Literal and bound division by zero is rejected with `E0030`.
- **Non-Finite Quantities**: NaN and $\pm\infty$ are rejected with `E0032`.

### 2.5 Non-Unitary and Ordering Operations

| Construct | Source Forms | QccIR Lowering | Canonical Emission Form |
|---|---|---|---|
| Single-Bit Measure | `c[j] = measure q[i];`<br>`measure q[i] -> c[j];` | `Operation::Measure { qubit: QubitId(i), destination: ClassicalBitId(j) }` | `c[j] = measure q[i];` (default)<br>`measure q[i] -> c[j];` (arrow mode) |
| Whole-Register Measure | `c = measure q;` | Expanded to $N$ sequential `Operation::Measure` operations ($0 \le k < N$). | `c[0] = measure q[0];`<br>$\dots$<br>`c[N-1] = measure q[N-1];` |
| Active Reset | `reset q[i];` | `Operation::Reset { qubit: QubitId(i) }` | `reset q[i];` |
| Whole-Register Reset | `reset q;` | Expanded to $N$ sequential `Operation::Reset` operations. | `reset q[0];`<br>$\dots$<br>`reset q[N-1];` |
| Targeted Barrier | `barrier q[0], r[1];` | `Operation::Barrier { qubits: vec![QubitId(0), QubitId(3)] }` | `barrier q[0], r[1];` |
| Global Barrier | `barrier;` | `Operation::Barrier { qubits: (0..num_qubits).collect() }` | `barrier;` |
| Whole-Register Barrier | `barrier q;` | Expanded to all qubits in register `q`. | `barrier q[0], q[1], ...;` |
| Circuit Global Phase | `gphase(angle);` | Accumulated into `circuit.global_phase` via `circuit.add_global_phase(expr)`. | `gphase(<expr>);` |

---

## 3. Canonical OpenQASM 3 Emitter Architecture

The emitter module ([`src/backend/openqasm/`](../src/backend/openqasm/)) exports validated QccIR circuits deterministically.

### 3.1 Emitter Options (`EmitterOptions`)

```rust
pub struct EmitterOptions {
    /// Whether to emit `include "stdgates.inc";`.
    /// `None` (default): automatically included when gates are used.
    pub include_stdgates: Option<bool>,
    /// Global phase export policy (EmitGPhase, RejectNonZero, AlwaysEmit).
    pub global_phase_policy: GlobalPhasePolicy,
    /// Measurement syntax style (Assignment vs Arrow).
    pub measurement_syntax: MeasurementSyntax,
    /// Whether to allow custom gates without rejection.
    pub allow_custom_gates: bool,
}
```

### 3.2 Canonical Emission Formatting Rules

1. **Header Block**: `OPENQASM 3.0;\n` followed by `include "stdgates.inc";\n\n`.
2. **Register Block**: Quantum registers sorted by allocation offset, followed by classical registers.
3. **Global Phase**: Emitted immediately after register declarations if non-zero (`gphase(<expr>);\n\n`).
4. **Operation Block**: Operations emitted in deterministic topological order.
5. **No Synthetic Noise**: Zero trailing whitespace, zero comments, exactly one terminating newline.

---

## 4. Semantic Round-Trip Fidelity & Verification Guarantees

Round-trip testing validates the pipeline:

$$\text{Source } S_0 \xrightarrow{\text{lower}} \text{Circuit } C_1 \xrightarrow{\text{emit}} \text{QASM } S_1 \xrightarrow{\text{validate}} \text{openqasm3} \xrightarrow{\text{lower}} \text{Circuit } C_2 \xrightarrow{\text{emit}} \text{QASM } S_2$$

### 4.1 Semantic Equivalence Contract

Two circuits $C_1$ and $C_2$ are semantically equivalent ($C_1 \equiv C_2$) if:
1. **Dimensions**: $n_{\text{qubits}}(C_1) == n_{\text{qubits}}(C_2)$ and $n_{\text{bits}}(C_1) == n_{\text{bits}}(C_2)$.
2. **Registers**: Quantum and classical registers match in name, size, and index offsets.
3. **Operation Graph**: Deterministic topological traversal yields identical operation sequences:
   - Gate names, parameter values ($|\Delta| < 10^{-12}$), and ordered qubit operands match.
   - Measurement source qubits and destination bits match.
   - Reset and barrier targets match.
4. **Global Phase**: Circuit-level global phases match ($|\Delta\phi| < 10^{-12}$).
5. **Wire Dependencies**: All causal dependency edges in the DAG (`DependencyKind::QuantumOrder`, `DependencyKind::BarrierBoundary`, classical RAW/WAR/WAW) match identically.

### 4.2 Equivalence Verification Methods

- **Pure Unitary Circuits**: Verified via [`check_equivalence`](../src/verification/unitary.rs) against all $2^n$ basis states ($|\text{diff}| < 10^{-10}$).
- **Non-Unitary Measurement Circuits**: Verified behaviorally via [`sample_circuit`](../src/simulator/executor.rs) using seeded RNG ([`SplitMix64`](../src/simulator/rng.rs)), asserting bit-for-bit identical outcome histograms.
- **Safety**: Non-unitary circuits are rejected by the unitary equivalence checker with `VerificationError::NonUnitaryOperation`.

### 4.3 Double Round-Trip Idempotency

Once normalized through QccIR, re-emission is strictly idempotent:

$$S_1 == S_2$$

Re-parsing $S_1$ and emitting it again produces byte-identical source text.

### 4.4 Formatting Insensitivity

The compiler frontend accepts arbitrary formatting without affecting semantic output:
- Varied whitespace, indentations, and blank lines.
- Semicolon placement and trailing comments (`// ...`).
- Multi-line parameter expressions (e.g. nested arithmetic across lines).
- Measurement syntax styles (`c = measure q;` vs `measure q -> c;`).

All normalized circuits produce canonical OpenQASM 3 on emission.

---

## 5. Golden Fixture Test Corpus

The test suite in [`tests/qasm3/roundtrip.rs`](../tests/qasm3/roundtrip.rs) exercises round trips on representative golden fixtures in [`tests/fixtures/qasm3/`](../tests/fixtures/qasm3/):

| Fixture | Features Tested | Verification Method |
|---|---|---|
| `bell.qasm` | Bell state preparation, H, CX, single-bit measurement | Topological match + Seeded sampling (`SplitMix64(42)`) |
| `rotations.qasm` | Parameterized `rx`, `ry`, `rz` with arithmetic and $\pi$ | Unitary equivalence ($|\text{diff}| < 10^{-10}$) |
| `non_unitary.qasm` | Mixed H, active reset, selective barrier, CX, measurement | Structural equality + active reset rejection validation |
| `multi_register.qasm` | Multiple quantum and classical registers, whole-register barrier | Multi-register offset matching + whole-register expansion |
| `global_phase.qasm` | Multiple `gphase` statements, phase accumulation | Global phase evaluation ($|\Delta\phi| < 10^{-12}$) + unitary check |
| `teleportation.qasm` | EPR pair creation, Bell basis measurement, multi-qubit barriers | Topological equality + Seeded sampling (`SplitMix64(777)`) |

---

## 6. Independent Parser Verification

Every emitted QASM program in the round-trip suite is validated with the pinned independent reference parser:

- **Library**: `openqasm3[parser]==1.0.1` (Python ANTLR-based parser).
- **Execution**: Invoked via `python3` subprocess with QASM fed through `stdin`.
- **Assertion**: AST parses successfully with `ast.version == '3.0'` and expected statement count.

---

## 7. Deferred Language Features & Rejection Catalog

Features deferred to later milestones produce deterministic, source-located diagnostics:

| Code | Diagnostic Category | Triggering Constructs | Target Milestone |
|---|---|---|:---:|
| `E0001` | Syntax Error | Unterminated tokens, malformed numbers, syntax errors | M2 |
| `E0002` | Legacy OpenQASM 2 | `qreg`, `creg` | M2 |
| `E0003` | Unsupported 3-Qubit Gate | `ccx`, `cswap` | M4 (Basis Synthesis) |
| `E0004` | Non-MVP Standard Gate | `u1`, `u2`, `u3`, `u`, `p`, `sx`, `sxdg`, `cy`, `ch`, etc. | M4 (Basis Translation) |
| `E0005` | Control Flow & Advanced | `if`, `else`, `while`, `for`, `def`, `extern`, `defcal`, `cal`, `int`, `float`, `delay` | M7 (Dynamic Simulation) / Research |
| `E0010`–`E0014` | Register Errors | Duplicate declarations, zero-sized registers, undeclared/out-of-bounds references | M2 |
| `E0021`–`E0025` | Gate & Operand Errors | Parameter/qubit count mismatches, duplicate operands, domain violations | M2 |
| `E0030`–`E0032` | Expression Errors | Division by zero, unresolved symbols, non-finite parameters | M2 |
| `E0040` | Lowering Error | Graph construction failure, invalid AST-to-QccIR conversion | M2 |

