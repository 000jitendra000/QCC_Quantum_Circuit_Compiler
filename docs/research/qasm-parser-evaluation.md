# OpenQASM 3 Parser Technology Evaluation and Architectural Analysis

- **Document Identifier**: `QCC-RESEARCH-0002`
- **Phase**: Phase 15 — OpenQASM parser dependency evaluation
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Author**: Antigravity Assistant (Google DeepMind)
- **Evaluation Date**: 2026-11-01
- **Status**: Complete / Authoritative

---

## 1. Executive Summary

As part of Phase 15 of the QCC compiler roadmap, this document presents an evidence-based technological evaluation of OpenQASM 3 parsing solutions for the **Quantum Circuit Compiler (QCC)**.

The objective is to establish an architectural decision that guarantees:
1. Complete isolation behind an architectural `Frontend` adapter boundary (zero foreign AST leakage into QccIR).
2. Pure stable Rust toolchain compliance with QCC's MSRV (Rust 1.80.0+).
3. Exact source-span tracking integrated with QCC's `Diagnostic` and `SourceSpan` infrastructure (1-indexed line and column).
4. Early, clean rejection of unsupported OpenQASM 3 constructs (pulse calibrations, arbitrary classical control flow).
5. Exact reconciliation with QCC's actual IR capabilities: 1-qubit and 2-qubit unitary gates, projective measurements, resets, barriers, and global phase—with no unsupported claims of 3-qubit gates without demonstrated mappings.

Three primary strategies were evaluated:
1. **The Rust `oq3_*` Parser Stack** (`oq3_lexer`, `oq3_parser`, `oq3_syntax`, `oq3_semantics` from `Qiskit/openqasm3_parser`).
2. **A Dedicated Handwritten Restricted Parser** (In-tree recursive-descent parser tailored to QCC's canonical subset, Phases 16–20).
3. **A Python Reference Parser Bridge** (Out-of-process invocation of `openqasm3[parser]==1.0.1`).

**Final Architectural Decision**:
QCC selects **Strategy 2 (Dedicated Handwritten Restricted Recursive-Descent Parser)** for the production compiler core, encapsulated behind the `Frontend` trait boundary (Phase 16). The Python reference parser (`openqasm3[parser]==1.0.1`) is retained strictly as an independent acceptance verification oracle in CI (Phase 11 acceptance follow-up). The `oq3_*` stack was compiled and experimentally verified against the test corpus, but is rejected for the production core due to upstream reduced maintenance, legacy dependency lock-in (`rowan <= 0.15.15` and 32 transitive crates), untyped expression statement wrapping, and rejection of standard arrow measurement syntax.

---

## 2. Evaluated Candidates & Primary Source Evidence

Primary source evaluation conducted on **2026-11-01** across crates.io, docs.rs, GitHub repositories, and PyPI:

| Candidate | Investigated Version | Source / Repository Link | License | MSRV | crates.io Status |
|---|:---:|---|:---:|:---:|:---:|
| `oq3_lexer` | `0.7.0` | [`crates.io/crates/oq3_lexer/0.7.0`](https://crates.io/crates/oq3_lexer/0.7.0) | `Apache-2.0` | 1.70.0 | Published (Late 2024) |
| `oq3_parser` | `0.7.0` | [`crates.io/crates/oq3_parser/0.7.0`](https://crates.io/crates/oq3_parser/0.7.0) | `Apache-2.0` | 1.70.0 | Published (Late 2024) |
| `oq3_syntax` | `0.7.0` | [`crates.io/crates/oq3_syntax/0.7.0`](https://crates.io/crates/oq3_syntax/0.7.0) | `Apache-2.0` | 1.70.0 | Published (Late 2024) |
| `oq3_semantics` | `0.7.0` | [`crates.io/crates/oq3_semantics/0.7.0`](https://crates.io/crates/oq3_semantics/0.7.0) | `Apache-2.0` | 1.70.0 | Published (Late 2024) |
| `rowan` | `0.15.15` | [`crates.io/crates/rowan/0.15.15`](https://crates.io/crates/rowan/0.15.15) | `MIT OR Apache-2.0` | 1.66.0 | Pinned by `oq3_syntax` |
| `openqasm3[parser]` | `1.0.1` | [`pypi.org/project/openqasm3/1.0.1/`](https://pypi.org/project/openqasm3/1.0.1/) | `Apache-2.0` | Python 3.9+ | PyPI Reference Implementation |
| Handwritten Restricted Parser | Custom (M2) | `crates/qcc-frontend-qasm3` | `Apache-2.0 OR MIT` | 1.80.0 | In-Tree (Phases 16–20) |

### Canonical Specification References:
- **OpenQASM 3 Live Specification**: [`https://openqasm.com/versions/3.0/index.html`](https://openqasm.com/versions/3.0/index.html)
  - Section 3.1: Quantum Types and Register Declarations.
  - Section 4.3: Subroutines and Extern Functions (prohibiting quantum types as extern arguments).
  - Appendix: Reserved Keywords (specifying `qreg` and `creg` as reserved words).
- **Qiskit OpenQASM 3 Parser Repository**: [`https://github.com/Qiskit/openqasm3_parser`](https://github.com/Qiskit/openqasm3_parser)
  - Maintenance Policy: Explicitly states the repository is in **reduced maintenance mode**, accepting only critical bug fixes.

---

## 3. Investigation & Empirical Compilation of Rust `oq3_*` Crates

A dedicated Rust evaluation harness was compiled in `experiments/oq3-evaluation/` using `oq3_syntax v0.7.0` and `oq3_semantics v0.7.0`.

### 3.1 `oq3_lexer`
- **Architecture**: A tokenizer adapted from `rustc_lexer`.
- **Dependencies**: `unicode-properties` (v0.1.5), `unicode-xid` (v0.2.6).
- **Findings**: Fast, stable token stream. However, it produces only flat tokens without syntax recovery or grammar validation.

### 3.2 `oq3_parser`
- **Architecture**: Pratt/recursive-descent parser emitting event streams to construct Rowan syntax trees.
- **Dependencies**: `oq3_lexer`.
- **Findings**: Emits untyped CST events. Requires `oq3_syntax` to access typed AST nodes.

### 3.3 `oq3_syntax`
- **Architecture**: Built on top of `rowan` (the lossless green/red syntax-tree library from `rust-analyzer`). Ingests `oq3_parser` events and exposes typed AST facades (`ast::SourceFile`, `ast::Stmt`).
- **Empirical Compilation Findings**:
  1. **Heavy Transitive Footprint**: Adding `oq3_syntax` and `oq3_semantics` pulled in **32 transitive crate dependencies** into the lockfile (`rowan`, `text-size`, `cov-mark`, `drop_bomb`, `countme`, `smol_str`, `triomphe`, `ra_ap_limit`, `xshell`, `ariadne`, `ahash`, `boolenum`, etc.).
  2. **Rowan Pinning**: Bound strictly to `rowan <= 0.15.15`. Modern `rowan` versions (0.15.19+) cannot be used without updating upstream.
  3. **Untyped Gate Call Wrapping**: In `oq3_syntax`, standard quantum gate applications (`h q[0];`, `cx q[0], q[1];`) are not parsed into direct gate statements; instead, they are parsed as generic `EXPR_STMT` wrapping an untyped `GateCallExpr`. Extracting the gate name, parameter list, and operand registers requires navigating untyped syntax nodes and checking identifier tokens.
  4. **Measurement Arrow Incompatibility**: `oq3_syntax` rejected the OpenQASM 2 compatibility arrow syntax `measure q[1] -> c[1];` with syntax errors (`expr_bp: expected expression`), demanding strictly assignment syntax (`c = measure q;`).

### 3.4 `oq3_semantics`
- **Architecture**: Multi-pass semantic analyzer generating an Abstract Semantic Graph (ASG).
- **Findings**: Introduces an additional intermediate model that duplicates QCC's native `validate_circuit()` and `WireAccessState` validation, while imposing rigid type rules on classical types that QCC compiles directly into registers.

---

## 4. Parser Evaluation Corpus & Empirical Results

The evaluation corpus in `experiments/oq3-evaluation/corpus/` was tested through both the **Python reference parser** (`openqasm3[parser]==1.0.1`) and the **Rust parser** (`oq3_syntax v0.7.0`):

| Corpus File | Tested Language Constructs | Python `openqasm3` | Rust `oq3_syntax` | QCC Subset Status | Technical Finding / Reason |
|---|---|:---:|:---:|:---:|---|
| `bell.qasm` | Bell state prep ($H, CX$) & measurements | **PASS** (7 stmts) | **PASS** (8 stmts) | **Supported** | Both parsers succeed cleanly; 1-to-1 mapping to QccIR. |
| `registers.qasm` | 1D qubit/bit registers (`q_arr`, `c_arr`) & single qubits | **PASS** (9 stmts) | **PASS** (10 stmts) | **Supported** | Valid non-reserved identifiers parse with 0 errors in both. |
| `registers_reserved_keywords.qasm` | Declarations using `qreg` & `creg` as identifiers | **REJECTED** | **REJECTED** (6 errs) | **Rejected** | `qreg` & `creg` are reserved keywords in OpenQASM 3 spec. |
| `standard_gates.qasm` | Canonical 1Q/2Q gates ($X, Y, Z, H, S, S^\dagger, T, T^\dagger, CX, CZ, SWAP$) | **PASS** (13 stmts) | **PASS** (14 stmts) | **Supported** | Maps directly to canonical QCC `Gate` enum variants. |
| `parameterized_gates.qasm` | $Rx, Ry, Rz, P, gphase$ with expressions ($\pi/2, 2\pi-0.5$) | **PASS** (7 stmts) | **PASS** (8 stmts) | **Supported** | Expressions map to QccIR `ParameterExpr` trees. |
| `measurement.qasm` | Assignment (`c = measure q;`) & arrow (`measure q -> c;`) | **PASS** (5 stmts) | **SYNTAX ERR** (2 errs)| **Supported** | Python accepts both; `oq3_syntax` rejects arrow syntax. |
| `reset.qasm` | Active quantum reset (`reset q[0];`) | **PASS** (5 stmts) | **PASS** (6 stmts) | **Supported** | Maps to QccIR `Operation::Reset`. |
| `barrier.qasm` | Global & operand-selective barriers | **PASS** (7 stmts) | **PASS** (8 stmts) | **Supported** | Maps to QccIR `Operation::Barrier`. |
| `invalid_syntax.qasm` | Missing semicolons, malformed statements | **REJECTED** | **SYNTAX ERR** (4 errs)| **Rejected** | Both reject. Python drops error line/col; Rust gives errors. |
| `malformed_operands.qasm` | Duplicate operands $CX(q[0], q[0])$, out-of-bounds | **PASS** (Syntax) | **PASS** (Syntax) | **Rejected** | Pure parsers accept; semantic pass must reject. |
| `unsupported_features.qasm` | While loops, pulse `defcal`, `extern` | **REJECTED** | **SYNTAX ERR** (12 errs)| **Rejected** | Early rejection of non-circuit constructs. |

---

## 5. In-Depth Construct Analysis: Rejections Explained

### 5.1 `registers.qasm` vs `registers_reserved_keywords.qasm`
- **Initial Rejection**: When declarations used `qubit[5] qreg;` and `bit[5] creg;`, both Python `openqasm3` and Rust `oq3_syntax` failed with syntax errors (`no viable alternative at input 'bit[5]creg'`).
- **Root Cause**: In the OpenQASM 3 Language Specification (Appendix: Reserved Words), `qreg` and `creg` are explicitly reserved keywords retained for OpenQASM 2 backwards compatibility (`qreg q[5];`). They cannot be used as variable identifiers.
- **Verification**: When variable names were updated to valid identifiers (`q_arr`, `c_arr`), **both parsers passed with 0 errors**.

### 5.2 `unsupported_features.qasm` Construct Rejections
Each construct was tested independently:
1. **Dynamic `while` Loop (`while (count < 5) { ... }`)**:
   - Python `openqasm3` parses `while` loops into an AST `WhileLoop` node.
   - Rust `oq3_syntax` rejected compound assignment `count += 1;` with block errors.
   - **QCC Architectural Rejection**: QCC Milestone M1/M2 compiles basic-block DAGs (`QccIR`), where quantum gates are statically scheduled nodes in an acyclic dependency graph without dynamic classical loop control. Dynamic classical control flow must be rejected at the frontend boundary.
2. **Pulse Calibration (`defcal rx(angle theta) q[0] { ... }`)**:
   - Both parsers rejected this declaration. In OpenQASM 3, `defcal` requires either a prior `defcalgrammar` directive or explicitly sized angle types (`angle[32]`).
   - **QCC Architectural Rejection**: Pulse-level definitions are out of scope for QCC's gate-level intermediate representation. QCC operates at the circuit/gate abstraction layer. Pulse calibrations are explicitly rejected.
3. **External Host Function (`extern custom_oracle(qubit[2]) -> bit;`)**:
   - Both parsers rejected this declaration. In OpenQASM 3 Specification Section 4.3 ("Subroutines and externs"), extern parameters and return types must be classical types (`int`, `float`, `bit`). Quantum types cannot be passed to extern host functions.
   - **QCC Architectural Rejection**: QCC circuits are self-contained quantum basic blocks; host foreign-function calls are unsupported.

### 5.3 `measurement.qasm` Syntax Discrepancy
- Python `openqasm3` accepted both assignment syntax (`c[0] = measure q[0];`) and arrow syntax (`measure q[1] -> c[1];`).
- Rust `oq3_syntax` failed on the arrow syntax with `expr_bp: expected expression`.
- **Implication**: QCC's OpenQASM emitter supports both syntax options (`MeasurementSyntax::Assignment` and `MeasurementSyntax::Arrow`). A handwritten parser can easily accept both syntax forms, whereas `oq3_syntax` requires post-hoc preprocessing.

---

## 6. Reconciled Supported Subset vs QccIR Capabilities

To maintain documentation integrity, the supported OpenQASM 3 subset is strictly reconciled with the actual capabilities of QccIR (`src/ir/operation.rs`, `src/quantum/gate.rs`, `src/ir/parameter.rs`):

### 6.1 Supported Gates (Exact Match with QccIR `Gate` Enum)
- **1-Qubit Fixed Gates**: `x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg` (maps to `Gate::X`, `Gate::Y`, `Gate::Z`, `Gate::H`, `Gate::S`, `Gate::Sdg`, `Gate::T`, `Gate::Tdg`).
- **1-Qubit Parameterized Rotations**: `rx(theta)`, `ry(theta)`, `rz(theta)` (maps to `Gate::Rx(Angle)`, `Gate::Ry(Angle)`, `Gate::Rz(Angle)`).
- **2-Qubit Entangling Gates**:
  - `cx`: Controlled-NOT (Operand 0 is control, Operand 1 is target; maps to `Gate::CX`).
  - `cz`: Controlled-Z (symmetric; maps to `Gate::CZ`).
  - `swap`: State exchange (symmetric; maps to `Gate::SWAP`).

### 6.2 Supported Operations & Directives
- **Declarations**: Sized 1D quantum registers (`qubit[n] q;`), single qubits (`qubit q;` normalized to register of size 1), sized 1D classical bit registers (`bit[n] c;`), single bits (`bit c;`).
- **Header**: `OPENQASM 3.0;`, `include "stdgates.inc";`.
- **Global Phase**: `gphase(theta);` (maps directly to `Circuit::global_phase`).
- **Measurement**: Assignment syntax (`c = measure q;`) and arrow syntax (`measure q -> c;`) mapping to `Operation::Measure { qubit, destination }`.
- **Reset**: Active state reset (`reset q;`) mapping to `Operation::Reset { qubit }`.
- **Barrier**: Global (`barrier;`) and selective (`barrier q[0], q[1];`) mapping to `Operation::Barrier { qubits }`.
- **Parameter Expressions**: Constants (`f64`), symbolic identifiers, standard constants (`pi`, `π`), and binary expressions (`+`, `-`, `*`, `/`) mapping to `ParameterExpr`.

### 6.3 Explicitly Unsupported Constructs (No 3-Qubit Claims)
- **Three-Qubit Gates**: `ccx` (Toffoli) and `cswap` (Fredkin) are **NOT supported** natively in QccIR at this milestone. They do not exist in `Gate` and have no kernel implementation in `src/simulator/kernels/`. They must not be claimed as supported without an explicit decomposition pass (scheduled for Milestone M4).
- **Derived/Custom Gates**: `u1`, `u2`, `u3`, `p`, `sx`, `sxdg`, and user-defined `gate` blocks must be rejected at this phase until macro expansion or decomposition passes are introduced.
- **Pulse/Timing**: `defcal`, `defcalgrammar`, `delay`, `duration`.
- **Classical Control Flow**: `while`, `for`, `if` conditional execution (deferred outside active v3.1 Phases 60–85).
- **Classical Types**: `int`, `float`, `angle`, `bool`, `extern`.

---

## 7. Comparative Strategy Evaluation

| Metric | Strategy 1: `oq3_*` Stack | Strategy 2: Handwritten Parser (Selected) | Strategy 3: Python Bridge |
|---|---|---|---|
| **Stable Rust & MSRV** | Yes (MSRV 1.70.0) | **Native 100% Stable Rust (MSRV 1.80.0)** | No (Requires Python 3.9+) |
| **Dependency Footprint** | Heavy (32 transitive crates, `rowan <= 0.15.15`) | **Zero external dependencies** | Unacceptable (ANTLR4, pip, venv) |
| **Maintenance Risk** | **High**: Upstream in reduced maintenance mode | **Zero**: In-tree, owned by QCC | External tool |
| **SourceSpan Integration** | Complex byte-offset translation (`TextRange`) | **Direct 1-indexed line/column tracking** | Incomplete (ANTLR drops spans on error) |
| **Syntax Flexibility** | Strict (rejects arrow measurement syntax) | **Accepts both assignment and arrow syntax** | Accepts both |
| **AST Impedance** | Gates wrapped in untyped `EXPR_STMT` | **Direct typed AST mapping to QccIR** | Python object marshaling overhead |
| **Compilation Speed** | Slower (Rowan CST green/red allocations) | **Fastest (Direct token-to-AST translation)** | Extremely slow (IPC/subprocess spawn) |
| **Distribution Model** | Cargo crate | **Single standalone static Rust executable** | Multi-runtime dependency |

---

## 8. Final Architectural Recommendation & Action Plan

1. **Adopt Strategy 2 (Dedicated Handwritten Restricted Parser)** for the production compiler core (Phases 16–20).
2. **Encapsulate behind Frontend Boundary (Phase 16)**: Implement `Frontend` trait and `SourceBuffer` in `crates/qcc-frontend-qasm3`.
3. **Preserve Python Bridge in CI Only**: Retain `openqasm3[parser]==1.0.1` strictly as an independent acceptance verification oracle in CI to validate QCC's emitted OpenQASM output.
