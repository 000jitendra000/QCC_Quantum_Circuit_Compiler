# OpenQASM 3 Parser Technology Evaluation and Architectural Analysis

- **Document Identifier**: `QCC-RESEARCH-0002`
- **Phase**: Phase 15 — OpenQASM parser dependency evaluation
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Author**: Antigravity Assistant (Google DeepMind)
- **Evaluation Date**: 2026-11-01
- **Status**: Complete / Authoritative

---

## 1. Executive Summary

As part of Phase 15 of the QCC compiler roadmap, this document evaluates candidate parsing technologies for the OpenQASM 3 frontend of the **Quantum Circuit Compiler (QCC)**.

The objective is to establish an evidence-based parser selection that guarantees:
1. Complete isolation behind a frontend adapter boundary (no foreign AST leaking into QccIR).
2. Strict adherence to stable Rust and QCC's MSRV (Rust 1.80+).
3. Exact source-span tracking integrated with QCC's `Diagnostic` and `SourceSpan` infrastructure.
4. Clean diagnostic error rejection of unsupported OpenQASM 3 constructs (pulse calibrations, arbitrary classical control flow).
5. Zero risk of compiler bloat or upstream project abandonment.

Three primary strategies were evaluated:
1. **The Rust `oq3_*` Parser Stack** (`oq3_lexer`, `oq3_parser`, `oq3_syntax`, `oq3_semantics` from `Qiskit/openqasm3_parser`).
2. **A Dedicated Handwritten Restricted Parser** (Recursive descent / combinator tailored to QCC's supported subset).
3. **A Python Subprocess / C-FFI Bridge** (Using the reference `openqasm3[parser]==1.0.1` package).

**Final Recommendation**:
QCC selects **Strategy 2 (Handwritten Restricted Recursive-Descent Parser)** for the production compiler core, supported by a formal `Frontend` trait and `SourceBuffer` abstraction (Phases 16–19). The Python parser (`openqasm3[parser]==1.0.1`) remains strictly an out-of-process differential verification oracle in CI (Phase 11 acceptance follow-up), never a runtime dependency. The `oq3_*` stack is rejected for the production core due to upstream reduced maintenance, rigid lossless-CST complexity (`rowan`), and version lock-in.

---

## 2. Evaluation Candidates & Primary Source Evidence

Evaluation conducted on **2026-11-01** against primary documentation, official crates.io registries, and GitHub repositories.

| Crate / Tool | Evaluated Version | Upstream Repository | License | MSRV | crates.io Status |
|---|:---:|---|:---:|:---:|:---:|
| `oq3_lexer` | `0.7.0` | [`Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser) | `Apache-2.0` | 1.70.0 | Published (late 2024) |
| `oq3_parser` | `0.7.0` | [`Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser) | `Apache-2.0` | 1.70.0 | Published (late 2024) |
| `oq3_syntax` | `0.7.0` | [`Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser) | `Apache-2.0` | 1.70.0 | Published (late 2024) |
| `oq3_semantics` | `0.7.0` | [`Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser) | `Apache-2.0` | 1.70.0 | Published (late 2024) |
| `openqasm3[parser]` | `1.0.1` | [`openqasm/openqasm`](https://github.com/openqasm/openqasm) | `Apache-2.0` | Python 3.9+ | PyPI Reference Implementation |
| Handwritten Restricted Parser | Custom (M2) | `QCC` internal (`crates/qcc-frontend-qasm3`) | `Apache-2.0 OR MIT` | 1.80.0 | In-tree (Phases 16–19) |

---

## 3. Individual Investigation of Rust `oq3_*` Crates

### 3.1 `oq3_lexer`
- **Architecture**: Fork and adaptation of the `rustc_lexer` crate for tokenizing OpenQASM 3 source code.
- **Dependencies**: `unicode-properties` (v0.1.0+), `unicode-xid` (v0.2.0+).
- **Strengths**: High tokenization throughput; negligible external dependencies; stable Rust compliant.
- **Limitations**: Emits flat tokens without AST or CST awareness. Cannot detect syntax errors on its own.
- **Independent Viability**: High as a tokenizer, but does not provide grammatical validation.

### 3.2 `oq3_parser`
- **Architecture**: Recursive-descent Pratt parser that consumes tokens from `oq3_lexer` and emits an event-based stream to construct a Concrete Syntax Tree (CST).
- **Dependencies**: `oq3_lexer`.
- **Strengths**: Parses standard OpenQASM 3 grammar constructs; resilient parsing (attempts recovery on syntax errors).
- **Limitations**: Emits untyped syntax events. Consuming this crate directly requires rebuilding the CST manually or depending on `oq3_syntax`.
- **Independent Viability**: Low; designed as an internal engine for `oq3_syntax`.

### 3.3 `oq3_syntax`
- **Architecture**: Built on the `rowan` lossless syntax-tree library (the same green/red tree model utilized in `rust-analyzer`). Ingests `oq3_parser` CST events and exposes typed AST nodes (`ast::SourceFile`, `ast::Stmt`, `ast::GateCallStmt`, `ast::QuantumDeclarationStmt`).
- **Dependencies**: `oq3_lexer`, `oq3_parser`, `rowan` (`<= 0.15.15`).
- **Strengths**: Comment- and whitespace-preserving lossless syntax tree; rich AST node getters.
- **Critical Findings & Drawbacks**:
  1. **Strict Dependency Constraint**: Pinned to `rowan <= 0.15.15`. Rowan has since evolved, and locking QCC's dependency tree to legacy Rowan versions creates risk of diamond dependency conflicts with modern Rust tooling.
  2. **Upstream Maintenance Status**: The official repository (`Qiskit/openqasm3_parser`) explicitly states that it is operating in **"reduced maintenance mode"**. Only critical bugs are addressed; new features, enhancements, and modern dependency updates are deprioritized.
  3. **Complex Lossless Tree Overhead**: Rowan trees allocate reference-counted green nodes (`GreenNode`) and traverse untyped syntax nodes (`SyntaxNode`) to dynamically wrap them in typed facades (`ast::AstNode`). For a compiler whose target is QccIR DAGs, this imposes unnecessary allocation overhead and cumbersome tree-traversal boilerplate.
  4. **Diagnostic Span Mapping**: Rowan uses relative `TextRange` byte offsets. Converting these to 1-indexed line and column numbers required by QCC's `SourceSpan` necessitates maintaining an external line-index map.

### 3.4 `oq3_semantics`
- **Architecture**: Multi-pass semantic analyzer that transforms `oq3_syntax` ASTs into an Abstract Semantic Graph (ASG) with symbol tables, scoping, and type resolution.
- **Dependencies**: `oq3_syntax`, `oq3_source_file`, `rowan`, `hashbrown`, `boolenum`.
- **Strengths**: Attempts full symbol resolution, type casting, and semantic validation.
- **Critical Findings & Drawbacks**:
  1. **Rigid Scope & Feature Coupling**: Enforces full OpenQASM 3 semantic rules (including classical registers, casts, functions, and timing), but contains documented gaps in calibration (`defcal`) and pulse blocks.
  2. **Heavyweight Abstraction**: Introduces its own AST/ASG representation that directly competes with QCC's native `Circuit`, `Operation`, and `WireAccessState`.
  3. **Error Reporting Structure**: Binds diagnostics to internal diagnostic types that do not cleanly align with QCC's `Diagnostic` and `DiagnosticLevel` architecture.
  4. **Redundant Validation**: QCC already provides semantic validation in `validate_circuit()`, `WireAccessState`, and `CircuitBuilder`. Re-running `oq3_semantics` duplicates checks while introducing foreign errors for constructs QCC intentionally supports or decomposes.

---

## 4. Parser Evaluation Corpus & Empirical Results

A representative test corpus of 10 OpenQASM 3 programs was created in `experiments/oq3-evaluation/corpus/`. The corpus was evaluated against the reference parser `openqasm3[parser]==1.0.1` and analyzed for QCC AST adaptation feasibility.

### 4.1 Corpus Test Matrix

| Corpus Test File | Description / Constructs Tested | Statements | Python `openqasm3` Result | QCC Subset Compatibility | AST Adaptation Complexity |
|---|---|:---:|:---:|:---:|:---:|
| `bell.qasm` | Bell state preparation ($H$, $CX$) & terminal measurement | 7 | **PASS** | Supported | **Low**: 1-to-1 mapping to QccIR |
| `registers.qasm` | Register declarations: `qubit[5]`, `qubit single_q`, `bit[5]`, `bit` | 8 | **REJECTED** (syntax) | Supported | **Low**: Maps to `QuantumRegister`, `ClassicalRegister` |
| `standard_gates.qasm` | 1Q/2Q gates: $X, Y, Z, H, S, S^\dagger, T, T^\dagger, CX, CZ, SWAP$ | 13 | **PASS** | Supported | **Low**: Maps directly to canonical `Gate` |
| `parameterized_gates.qasm` | $Rx, Ry, Rz, P, gphase$ with arithmetic expressions ($\pi/2, 2\pi-0.5$) | 7 | **PASS** | Supported | **Medium**: Maps to `ParameterExpr` trees |
| `measurement.qasm` | Assignment syntax (`c[0] = measure q[0];`) & legacy arrow (`->`) | 5 | **PASS** | Supported | **Low**: Maps to `Operation::Measure` |
| `reset.qasm` | Quantum active reset (`reset q[0];`) | 5 | **PASS** | Supported | **Low**: Maps to `Operation::Reset` |
| `barrier.qasm` | Global (`barrier;`) and operand-selective (`barrier q[0], q[1];`) | 7 | **PASS** | Supported | **Low**: Maps to `Operation::Barrier` |
| `invalid_syntax.qasm` | Missing semicolons, missing brackets, corrupted keywords | 0 | **REJECTED** (syntax) | Rejected | **N/A**: Must produce early `Diagnostic` |
| `malformed_operands.qasm` | Duplicate operands on $CX(q[0], q[0])$, out-of-bounds indices | 4 | **PASS** (syntax) | Rejected | **Medium**: Pure parsers accept; semantic pass must reject |
| `unsupported_features.qasm` | Dynamic `while` loop, `defcal` pulse block, `extern` function | 0 | **REJECTED** (syntax) | Rejected | **N/A**: Must reject cleanly with diagnostic |

### 4.2 Critical Findings from Empirical Evaluation

1. **Syntactic vs Semantic Error Distinction**:
   - `malformed_operands.qasm` parsed **successfully** in `openqasm3` without error. The ANTLR grammar allows arbitrary qubit identifiers and index expressions.
   - Detecting duplicate operands on two-qubit gates ($CX(q[0], q[0])$) or out-of-bounds indices requires semantic validation after parsing.
   - This proves that **no parser alone** can guarantee circuit validity; QCC's `validate_circuit()` and semantic analysis (Phase 20) are indispensable.

2. **Error Quality in Reference Implementation**:
   - When encountering syntax errors (`invalid_syntax.qasm`, `registers.qasm`), `openqasm3` raises `openqasm3.parser.QASM3ParsingError` with an **empty error message (`""`)** and `line=None, column=None` due to ANTLR's default cancellation strategy.
   - For a production compiler, uninformative parse failures are completely unacceptable. A compiler must report:
     ```text
     error: expected ';', found 'h'
       --> input.qasm:5:1
        |
      4 | qubit[2] q
        |           ^ expected ';' after declaration
      5 | h q[0];
     ```

3. **Single Qubit Declarations**:
   - In OpenQASM 3 grammar, single-qubit declarations without brackets (e.g. `qubit q;`) require specific syntactic productions that `openqasm3` v1.0.1 handles inconsistently compared to array registers (`qubit[1] q;`). A custom lexer/parser handles both uniformly without ambiguity.

4. **AST Adaptation Feasibility into QccIR**:
   - Mapping standard OpenQASM 3 AST nodes to QccIR is straightforward:
     - `QubitDeclaration` $\to$ `CircuitBuilder::add_quantum_register`
     - `ClassicalDeclaration` $\to$ `CircuitBuilder::add_classical_register`
     - `QuantumGate` $\to$ `Operation::Gate` with resolved `QubitId`s and evaluated `ParameterExpr`s
     - `QuantumMeasurementStatement` $\to$ `Operation::Measure`
     - `QuantumReset` $\to$ `Operation::Reset`
     - `QuantumBarrier` $\to$ `Operation::Barrier`
     - `QuantumPhase` $\to$ `circuit.global_phase` accumulator
   - Preserving exact source spans from AST nodes directly into `Operation` metadata enables end-to-end provenance and precise error reporting during downstream verification or simulation.

---

## 5. Architectural Strategy Comparison

| Evaluation Criterion | Strategy 1: `oq3_*` Stack | Strategy 2: Handwritten Parser (Recommended) | Strategy 3: Python Parser Bridge |
|---|---|---|---|
| **Stable Rust Compatibility** | Yes (MSRV 1.70.0) | **Native 100% Stable Rust (MSRV 1.80.0)** | No (Requires Python runtime) |
| **Runtime & Dependency Footprint** | Heavy (`rowan <= 0.15.15`, `unicode-*`, `hashbrown`) | **Minimal (0 external deps, or cached `winnow`)** | Unacceptable (Python 3.9+, ANTLR4 runtime) |
| **Maintenance & Project Health** | **Warning**: Upstream in reduced maintenance | **Directly maintained in QCC repository** | Standard OpenQASM project |
| **QCC `SourceSpan` Integration** | Complex: relative `TextRange` conversion | **Direct 1-indexed line/column tracking** | Incomplete: ANTLR drops error spans |
| **Error Quality & Diagnostics** | Moderate (Rowan syntax error nodes) | **High (Custom descriptive diagnostics)** | Poor (`QASM3ParsingError: ""` on errors) |
| **Unsupported Feature Rejection** | Late rejection (parses full grammar, rejects in AST) | **Early, precise rejection at parse boundary** | Uncontrolled (rejects or creates opaque AST) |
| **Compilation Speed & Overhead** | Slower (Rowan CST green/red allocations) | **Fastest (Direct token-to-AST translation)** | Extremely slow (IPC/Subprocess spawn) |
| **Distribution Simplicity** | Standard Rust crate dependency | **Single standalone static Rust binary** | Complex: requires pip, venv, Python environment |
| **CI & Testing Portability** | Pure `cargo test` | **Pure `cargo test`** | Requires Python setup steps in every runner |

---

## 6. Risks, Trade-offs, and Fallback Strategy

### 6.1 Risks of Strategy 1 (`oq3_*`)
- **Upstream Abandonment**: Because `Qiskit/openqasm3_parser` is officially under reduced maintenance, any grammar ambiguities, bugs, or compiler panics must be patched via vendored forks.
- **Dependency Drift**: Locking QCC to `rowan <= 0.15.15` blocks upgrading other potential syntax-tree tools in the future.

### 6.2 Risks of Strategy 2 (Handwritten Restricted Parser)
- **Grammar Coverage**: A restricted parser will only implement the subset required by QCC (Milestone M2 scope). Users attempting to compile complex classical algorithms or advanced pulse sequences will receive rejection errors.
- **Maintenance Burden**: The QCC engineering team owns the lexer, parser, and AST definitions.
- **Mitigation**: The roadmap explicitly allocates **Phases 16, 17, 18, 19, and 20** to implement this exact subsystem with high rigor. Designing the parser to reject unsupported constructs cleanly satisfies QCC's fail-closed correctness philosophy.

### 6.3 Fallback Architecture
If the handwritten restricted parser encounters unanticipated grammar complexity or if broader OpenQASM 3 compliance is mandated in future milestones:
1. The **`Frontend` trait boundary** (Phase 16) completely abstracts the parser implementation.
2. The compiler core interacts solely with validated `SourceBuffer` and `Circuit` representations.
3. An alternative adapter implementing `Frontend` using `oq3_syntax` can be plugged in behind a feature flag (`--features oq3-experimental`) with zero changes to compiler-core passes, optimizer, or simulator.

---

## 7. Final Recommendation & Action Plan

1. **Adopt Strategy 2 (Handwritten Restricted Parser)**: Implement a clean, fast, recursive-descent parser for QCC's canonical OpenQASM 3 subset in Phases 16–19.
2. **Preserve Python Bridge for CI Verification Only**: Keep `openqasm3[parser]==1.0.1` in CI strictly to validate that QCC's *emitted* QASM is accepted by the official reference parser.
3. **Execute Phase 16**: Establish the `Frontend` trait, `SourceBuffer`, and diagnostic boundary in `crates/qcc-frontend-qasm3`.
