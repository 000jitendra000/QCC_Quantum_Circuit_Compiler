# ADR-0002: OpenQASM 3 Frontend Parser Strategy and Adapter Architecture

- **Status**: **Accepted**
- **Date**: 2026-11-01
- **Deciders**: QCC Compiler Architecture Working Group
- **Consulted**: Phase 15 Parser Evaluation Corpus, Qiskit OpenQASM 3 Parser Empirical Investigation, Python Reference Parser Assessment
- **Primary Source References**:
  - `oq3_lexer v0.7.0`: [`https://crates.io/crates/oq3_lexer/0.7.0`](https://crates.io/crates/oq3_lexer/0.7.0)
  - `oq3_parser v0.7.0`: [`https://crates.io/crates/oq3_parser/0.7.0`](https://crates.io/crates/oq3_parser/0.7.0)
  - `oq3_syntax v0.7.0`: [`https://crates.io/crates/oq3_syntax/0.7.0`](https://crates.io/crates/oq3_syntax/0.7.0)
  - `oq3_semantics v0.7.0`: [`https://crates.io/crates/oq3_semantics/0.7.0`](https://crates.io/crates/oq3_semantics/0.7.0)
  - `rowan v0.15.15`: [`https://crates.io/crates/rowan/0.15.15`](https://crates.io/crates/rowan/0.15.15)
  - `Qiskit/openqasm3_parser` GitHub: [`https://github.com/Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser) (Documented reduced maintenance mode)
  - `openqasm3[parser] v1.0.1`: [`https://pypi.org/project/openqasm3/1.0.1/`](https://pypi.org/project/openqasm3/1.0.1/)
  - OpenQASM 3 Live Specification: [`https://openqasm.com/versions/3.0/index.html`](https://openqasm.com/versions/3.0/index.html) (Section 3.1, Section 4.3, Appendix Reserved Words)

---

## 1. Context and Problem Statement

To ingest quantum circuits from external quantum software environments, the **Quantum Circuit Compiler (QCC)** requires an OpenQASM 3 frontend (Milestone M2). The frontend must parse OpenQASM 3 source files into an Intermediate Representation (QccIR) basic-block DAG with high fidelity, deterministic behavior, and precise diagnostic source-span tracking.

The compiler team evaluated three competing architectural approaches in Phase 15:
1. **Third-Party Rust Crates (`oq3_*`)**: Ingesting `oq3_lexer`, `oq3_parser`, `oq3_syntax`, and `oq3_semantics` from the `Qiskit/openqasm3_parser` workspace.
2. **Dedicated Handwritten Restricted Parser**: Implementing an in-tree lexer, AST, and recursive-descent parser tailored to QCC's supported quantum and classical register subset (Phases 16–20).
3. **Python Runtime Bridge**: Invoking the official reference parser (`openqasm3[parser]==1.0.1`) out-of-process via subprocess or embedded Python.

The architectural challenge is choosing a parser strategy that maintains QCC's non-negotiable invariants:
- **Zero Foreign AST Leakage**: No parser-specific types (e.g. `rowan::SyntaxNode`, `oq3_syntax::ast::*`, or Python objects) may leak into QccIR or public compiler-core APIs.
- **Stable Rust & Minimal Dependencies**: Zero nightly features, pure stable Rust (MSRV 1.80.0+), and freedom from unmaintained or legacy transitive dependencies.
- **Fail-Closed Diagnostics**: Unsupported OpenQASM 3 features (such as pulse `defcal` blocks, arbitrary classical loops, or timing delays) must be rejected with informative, 1-indexed `SourceSpan` compiler diagnostics.
- **Self-Contained Executable**: QCC must compile and run as a standalone, high-performance static Rust binary without external runtime requirements.
- **Exact Reconciliation with QccIR Capabilities**: The supported frontend subset must accurately reflect QccIR's actual gate and parameter capabilities without claiming three-qubit gates or derived operations that have no demonstrated mapping.

---

## 2. Decision

QCC formally adopts **Strategy 2: Dedicated Handwritten Restricted OpenQASM 3 Parser**, strictly encapsulated behind an architectural `Frontend` adapter boundary:

1. **Parser Implementation Strategy**:
   - QCC will implement an in-tree, lightweight, recursive-descent parser (Phases 17, 18, and 19) designed specifically for QCC's supported OpenQASM 3 subset.
   - The lexer and parser will directly generate QCC's typed AST and map source locations directly to QCC's native `SourceSpan` (1-indexed line and column).
   - The parser rejects unsupported syntax early with rich, human-readable compiler diagnostics.

2. **Strict Frontend Boundary Encapsulation (Phase 16)**:
   - All parsing logic resides in an isolated frontend crate (`crates/qcc-frontend-qasm3`).
   - The compiler core (`qcc`) interacts exclusively through a generic `Frontend` trait that consumes a `SourceBuffer` and produces a validated `Circuit`.
   - QccIR remains 100% agnostic to the underlying parser implementation.

3. **Role of Third-Party Reference Tools**:
   - **`openqasm3[parser]==1.0.1` (Python)**: Maintained strictly as an **independent acceptance verification oracle in CI**. It is used to prove that circuits *emitted* by QCC comply with the official specification. It will never be a compile-time or runtime dependency of the QCC compiler.
   - **`oq3_*` (Rust)**: Retained as an optional reference/fallback design. Not adopted into the production core.

---

## 3. Supported OpenQASM 3 Subset Reconciled with QccIR

QCC adopts a **fail-closed** policy for its OpenQASM 3 frontend. Supported constructs are parsed and adapted into QccIR; any construct outside this specification is explicitly rejected with a structured `DiagnosticLevel::Error`.

### 3.1 Supported Quantum Gates (Exact Match with QccIR `Gate` Enum)
- **1-Qubit Fixed Gates**: `x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg` (maps to `Gate::X`, `Gate::Y`, `Gate::Z`, `Gate::H`, `Gate::S`, `Gate::Sdg`, `Gate::T`, `Gate::Tdg`).
- **1-Qubit Parameterized Rotations**: `rx(theta)`, `ry(theta)`, `rz(theta)` (maps to `Gate::Rx(Angle)`, `Gate::Ry(Angle)`, `Gate::Rz(Angle)`).
- **2-Qubit Entangling Gates**:
  - `cx`: Controlled-NOT (Operand 0 is control, Operand 1 is target; maps to `Gate::CX`).
  - `cz`: Controlled-Z (symmetric; maps to `Gate::CZ`).
  - `swap`: State exchange (symmetric; maps to `Gate::SWAP`).

### 3.2 Supported Operations & Declarations
1. **Header & Directives**:
   - Version declaration: `OPENQASM 3.0;`
   - Standard gate inclusion: `include "stdgates.inc";`
2. **Register Declarations**:
   - Sized 1D quantum registers: `qubit[n] q;`
   - Single-qubit declarations: `qubit q;` (normalized to register of size 1).
   - Sized 1D classical bit registers: `bit[n] c;`
   - Single-bit declarations: `bit c;`
3. **Non-Unitary Operations**:
   - Projective measurement (assignment syntax): `c[i] = measure q[j];`
   - Projective measurement (arrow syntax): `measure q[j] -> c[i];`
   - Active state reset: `reset q[j];`
   - Synchronization barriers: global `barrier;` and selective `barrier q[i], q[j];`.
4. **Parameter Expressions & Global Phase**:
   - Global phase directives: `gphase(theta);` (maps to `Circuit::global_phase`).
   - Unitary parameter expressions: constants (`f64`), symbolic identifiers, standard mathematical constants (`pi`, `π`), and binary arithmetic (`+`, `-`, `*`, `/`) mapping to `ParameterExpr`.

### 3.3 Explicitly Unsupported Constructs (Rejected with Compiler Diagnostic)
- **Three-Qubit Gates**: `ccx` (Toffoli) and `cswap` (Fredkin) are **NOT supported** natively in QccIR at this milestone. They do not exist in `Gate` and have no kernel implementation in `src/simulator/kernels/`. They must not be claimed as supported without an explicit decomposition pass (scheduled for Milestone M4).
- **Derived/Custom Gates**: `u1`, `u2`, `u3`, `p`, `sx`, `sxdg`, and user-defined `gate` blocks (until macro expansion or decomposition passes are introduced in Milestone M4).
- **Pulse and Calibration**: `defcal`, `defcalgrammar`, arbitrary pulse schedules.
- **Classical Control Flow**: `while`, `for`, `switch`, nested classical procedures.
- **Classical Memory Types**: `int`, `float`, `angle`, `bool`, `extern`.
- **Timing and Delays**: `delay[t]`, `duration`.

---

## 4. Rationale & Rejection of Alternatives

### 4.1 Why the `oq3_*` Stack Was Rejected for Core
1. **Upstream Maintenance Status**: As documented in [`Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser), the repository operates in **reduced maintenance mode**. Relying on an upstream project that only accepts critical bug fixes creates an unacceptable operational risk for QCC.
2. **Heavy Dependency Footprint**: Compiling `oq3_syntax` and `oq3_semantics` pulled in **32 transitive dependencies** (`rowan`, `text-size`, `cov-mark`, `drop_bomb`, `countme`, `smol_str`, `triomphe`, `ra_ap_limit`, `xshell`, `ariadne`, `ahash`, `boolenum`, etc.).
3. **Rowan Pinning (`rowan <= 0.15.15`)**: `oq3_syntax` is bound to an older version of `rowan`, risking dependency conflicts with modern Rust tooling.
4. **Untyped AST Impedance**: In `oq3_syntax`, gate calls (`h q[0];`, `cx q[0], q[1];`) are parsed as generic untyped `EXPR_STMT` wrapping `GateCallExpr`, requiring complex CST navigation to unpack gate names and operands.
5. **Inflexible Syntax**: `oq3_syntax` rejected standard arrow measurement syntax (`measure q[1] -> c[1];`), which QCC's emitter supports.
6. **Diagnostic Span Friction**: Rowan uses byte-offset `TextRange`s, whereas QCC natively requires 1-indexed line and column `SourceSpan`s.

### 4.2 Why the Python Bridge Was Rejected for Core
1. **Unacceptable Runtime Dependency**: Requiring Python 3.9+, `pip`, `openqasm3`, and `antlr4-python3-runtime` violates QCC's core design charter as a self-contained, high-performance static Rust binary.
2. **Subprocess/IPC Overhead**: Invoking Python per compilation unit introduces orders-of-magnitude latency overhead.
3. **Poor Diagnostic Fidelity**: The ANTLR runtime in `openqasm3` throws `QASM3ParsingError` with empty error strings and `None` line/columns, completely obscuring user syntax errors.

### 4.3 Why the Handwritten Restricted Parser Was Selected
1. **Complete Control**: Owned, maintained, and rigorously tested inside the QCC codebase.
2. **Zero Dependencies**: Pure standard library Rust or lightweight parser utilities (`winnow`), maintaining an MSRV of 1.80.0+.
3. **Perfect Error Diagnostics**: Syntax errors produce exact, human-readable compiler diagnostics with 1-indexed line and column numbers matching QCC's diagnostic standards.
4. **Early, Clean Rejection**: Constructs outside QCC's supported subset are rejected at the parsing boundary.
5. **Architectural Alignment**: The QCC Engineering Roadmap v2 explicitly budgeted Phases 17–20 for this purpose (`source buffer`, `lexer`, `AST`, `recursive-descent parser`, `semantic analysis`).

---

## 5. Licensing and MSRV Implications

- **License**: The handwritten parser is authored under QCC's dual license (`Apache-2.0 OR MIT`). There are zero viral copyleft or proprietary licensing encumbrances.
- **MSRV**: Conforms strictly to QCC's project-wide MSRV of **Rust 1.80.0** on the stable toolchain.
- **Dependency Footprint**: Zero external runtime dependencies added to the production compiler.

---

## 6. Fallback Strategy and Reconsideration Triggers

### 6.1 Fallback Architecture
Because Phase 16 establishes a strictly encapsulated `Frontend` trait:
```rust
pub trait Frontend {
    fn parse(&self, buffer: &SourceBuffer) -> Result<Circuit, FrontendError>;
}
```
If the handwritten parser proves insufficient for expanded future requirements (e.g. if Milestone M8 mandates parsing arbitrary dynamic OpenQASM 3 classical programs), an alternative parser adapter (such as a wrapper around an updated `oq3_syntax` or tree-sitter grammar) can be implemented behind the `Frontend` trait without modifying a single line of QccIR, optimizer passes, or backend code.

### 6.2 Conditions That Would Trigger Reconsideration
1. The OpenQASM 3 working group establishes an official, actively maintained, pure-Rust reference parser crate with zero legacy dependencies and first-class diagnostic spans.
2. QCC's charter expands to support arbitrary classical computation, complex pulse programming, or multi-file compilation units that exceed the scope of a domain-specific circuit compiler frontend.

---

## 7. Implementation Roadmap & Next Steps

1. **Phase 16**: Implement `Frontend` trait, `SourceBuffer`, and diagnostic mapping boundary in `crates/qcc-frontend-qasm3`.
2. **Phase 17**: Implement OpenQASM 3 source buffer and lexer with 1-indexed `SourceSpan` tracking.
3. **Phase 18**: Define typed OpenQASM AST model representing the supported subset.
4. **Phase 19**: Implement recursive-descent parser.
5. **Phase 20**: Implement semantic analysis, symbol tables, and QccIR conversion.
