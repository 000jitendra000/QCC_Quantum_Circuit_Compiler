# OpenQASM 3 Supported Language Subset & Grammar Specification

This document provides the authoritative, contract-level specification of the accepted OpenQASM 3 subset in QCC, together with the fail-closed rejection contracts for deferred language features.

---

## 1. Architectural Principles

1. **Fail-Closed Language Boundary**:
   The compiler frontend rejects any unsupported construct deterministically with source-located diagnostics (`DiagnosticLevel::Error`). No unsupported statement is silently ignored, skipped without diagnostic recording, or partially accepted into executable QccIR.
2. **AST Statement Non-Omission**:
   When error-recovering parsing is invoked, unsupported constructs are captured in the AST as `AstStatement::Unsupported(AstUnsupported)` nodes rather than being discarded. This enables language servers, linters, and verification oracles to analyze complete source documents without syntactic holes.
3. **Pure Frontend Independence**:
   The frontend parsing layer is fully decoupled from QccIR and physical quantum execution semantics. Gate signatures, parameter expressions, and register declarations are verified syntactically and semantically before any lowering begins.

---

## 2. Supported Grammar Specification

QCC supports a rigorous, static subset of OpenQASM 3.0 matching quantum circuit execution semantics.

### 2.1 File Directives & Modifiers

| Construct | Syntax | Notes |
|---|---|---|
| Version Declaration | `OPENQASM 3.0;` or `OPENQASM 3;` | Optional; must appear first if present. Version 3.x accepted. |
| Include Directive | `include "stdgates.inc";` | Quoted include path. Checked and preserved in AST. |

### 2.2 Register Declarations

Quantum and classical registers use typed 0-indexed integer identifiers. All register names must be unique within a program scope.

| Construct | Syntax | Semantic Contract |
|---|---|---|
| Scalar Qubit | `qubit q;` | Single logical qubit ($size = 1$). |
| Qubit Array | `qubit[n] q;` | Linear quantum register of $n$ qubits ($n \ge 1$). |
| Scalar Bit | `bit c;` | Single classical bit ($size = 1$). |
| Bit Array | `bit[n] c;` | Linear classical register of $n$ bits ($n \ge 1$). |

### 2.3 Supported Quantum Gate Set (MVP Basis)

All gate names in OpenQASM are strictly case-sensitive. Operands are strictly preserved in source order (for `cx`, operand 0 is control and operand 1 is target).

| Gate Name | Arity ($Q$) | Params ($P$) | Parameter Domain | Matrix / Unitary Semantics |
|---|---|---|---|---|
| `x` | 1 | 0 | N/A | Pauli-X ($NOT$) |
| `y` | 1 | 0 | N/A | Pauli-Y |
| `z` | 1 | 0 | N/A | Pauli-Z |
| `h` | 1 | 0 | N/A | Hadamard ($H = \frac{X+Z}{\sqrt{2}}$) |
| `s` | 1 | 0 | N/A | Phase gate ($S = \sqrt{Z}$) |
| `sdg` | 1 | 0 | N/A | Conjugate transpose of $S$ ($S^\dagger$) |
| `t` | 1 | 0 | N/A | $\pi/8$ gate ($T = \sqrt{S}$) |
| `tdg` | 1 | 0 | N/A | Conjugate transpose of $T$ ($T^\dagger$) |
| `rx(theta)` | 1 | 1 | $\theta \in \mathbb{R}$ | $R_x(\theta) = \exp(-i \frac{\theta}{2} X)$ |
| `ry(theta)` | 1 | 1 | $\theta \in \mathbb{R}$ | $R_y(\theta) = \exp(-i \frac{\theta}{2} Y)$ |
| `rz(phi)` | 1 | 1 | $\phi \in \mathbb{R}$ | $R_z(\phi) = \exp(-i \frac{\phi}{2} Z)$ |
| `cx` | 2 | 0 | N/A | Controlled-NOT ($CX(q_c, q_t)$) |
| `cz` | 2 | 0 | N/A | Controlled-Z ($CZ(q_0, q_1)$) |
| `swap` | 2 | 0 | N/A | Qubit state swap ($SWAP(q_0, q_1)$) |

### 2.4 Parameter Expressions

Parameter expressions are parsed into symbolic expression trees without eager lossy floating-point evaluation.

| Category | Supported Syntax | Semantics |
|---|---|---|
| Numeric Literals | `42`, `3.14159`, `1e-5`, `2.5E+2` | Decimal and scientific notation ($f64$). |
| Mathematical Constants | `pi`, `π` | Exactly resolved to $\pi \approx 3.141592653589793$. |
| Free / Bound Symbols | `theta`, `alpha`, `beta` | Preserved symbolically; bound at analysis or synthesis time. |
| Unary Operators | `+expr`, `-expr` | Unary positive and negation. |
| Binary Operators | `+`, `-`, `*`, `/` | Standard arithmetic precedence: unary $> \{*, /\} > \{+, -\}$. |
| Grouping | `(expr)` | Explicit precedence override. |

### 2.5 Non-Unitary and Ordering Operations

| Construct | Syntax Form | Semantics |
|---|---|---|
| Arrow Measurement | `measure q[i] -> c[j];` | Projective Z-basis measurement with classical assignment. |
| Assignment Measurement | `c[j] = measure q[i];` | Canonical OpenQASM 3 assignment measurement form. |
| Whole-Register Measure | `c = measure q;` | Measures each $q[k]$ into $c[k]$ ($size(q) == size(c)$ required). |
| Active State Reset | `reset q[i];` or `reset q;` | Resets target qubit(s) to computational ground state $\|0\rangle$. |
| Barrier (Targeted) | `barrier q[0], q[1];` | Scheduling and optimization barrier across designated qubits. |
| Barrier (Global) | `barrier;` | Full-circuit synchronization boundary across all qubits. |
| Circuit Global Phase | `gphase(angle);` | Explicit global phase factor $e^{i \cdot angle}$ applied to circuit unitary. |

---

## 3. Unsupported Constructs & Rejection Contracts

Every construct listed below is deterministically rejected at frontend analysis time with a source-located diagnostic and designated error code.

### 3.1 Summary of Error Codes

| Code | Standard Category | Triggering Language Features |
|---|---|---|
| `E0001` | Syntax Error | Unterminated tokens, malformed literals, missing semicolons. |
| `E0002` | Reserved / Legacy Keyword | OpenQASM 2 declarations (`qreg`, `creg`). |
| `E0003` | Unsupported 3-Qubit Gate | Multi-qubit gates requiring synthesis (`ccx`, `cswap`). |
| `E0004` | Unsupported Standard Gate | Non-MVP standard gates requiring basis decomposition (`u1`, `u2`, `u3`, `u`, `p`, `sx`, `sxdg`, etc.). |
| `E0005` | Unsupported Language Feature | Control flow (`if`, `else`, `while`, `for`), subroutines (`def`, `extern`), calibrations (`defcal`, `cal`), classical scalar types (`int`, `float`), timing (`delay`). |

### 3.2 Granular Feature Rejection Matrix

| Feature Category | Trigger Tokens / Keywords | Diagnostic Code | Primary Diagnostic Message | Roadmap Milestone |
|---|---|---|---|---|
| **Classical Branching** | `if`, `else` | `E0005` | `classical conditional branching ('if'/'else') is not supported in QccIR at this milestone` | Phase 32+ (Dynamic Circuits) |
| **Classical Loops** | `while`, `for`, `break`, `continue` | `E0005` | `classical control flow loops ('while'/'for') are not supported in QccIR at this milestone` | Milestone M6 (Control Flow) |
| **Subroutines & Linkage** | `def`, `return`, `extern`, `end` | `E0005` | `subroutines ('def') / extern symbol declarations are not supported in QccIR at this milestone` | Milestone M6 (Linkage) |
| **Custom Gate Definitions** | `gate` | `E0005` | `custom gate definitions ('gate') are not supported in QccIR at this milestone without macro expansion` | Phase 26 (Gate Expansion) |
| **Pulse Calibrations** | `defcal`, `defcalgrammar`, `cal` | `E0005` | `pulse calibration definitions ('defcal') are not supported in QccIR at this milestone` | Backend Microcode Phases |
| **Classical Types** | `int`, `uint`, `float`, `angle`, `bool`, `complex` | `E0005` | `classical scalar memory types ('type') are not supported in QccIR at this milestone` | Milestone M6 |
| **Modifiers & Directives** | `const`, `readonly`, `pragma` | `E0005` | `modifier or compiler directive ('const') is not supported in QccIR at this milestone` | Milestone M6 |
| **Timing & Delays** | `delay`, `duration`, `stretch` | `E0005` | `timing construct ('delay') is not supported in QccIR at this milestone` | Milestone M4 (Scheduling) |
| **Legacy Declarations** | `qreg`, `creg` | `E0002` | `OpenQASM 2 keyword 'qreg' is not supported in OpenQASM 3; use 'qubit[size] name;' instead` | Deprecated (OpenQASM 2) |
| **Three-Qubit Gates** | `ccx`, `cswap` | `E0003` | `three-qubit gate 'ccx' is not supported in QccIR at this milestone; decomposition into 1- and 2-qubit operations is required` | Milestone M4 (Synthesis) |
| **Decomposition-Required Gates** | `u1`, `u2`, `u3`, `u`, `p`, `sx`, `sxdg`, `cy`, `ch`, `crx`, `cry`, `crz`, `cu`, `rxx`, `ryy`, `rzz` | `E0004` | `gate 'u3' is not supported in QccIR at this milestone without decomposition` | Milestone M3 (Basis Translation) |

---

## 4. AST Representation and Non-Omission Contract

When parsing in error-recovery mode via `OpenQasmFrontend::parse_recovering`:

1. Any statement matching an unsupported keyword is converted to an `AstStatement::Unsupported(AstUnsupported)` node.
2. The `kind` field contains a standardized category (`"conditional"`, `"loop"`, `"subroutine"`, `"gate_definition"`, `"calibration"`, `"classical_type"`, `"modifier_or_directive"`, `"timing"`, `"legacy_keyword"`, `"three_qubit_gate"`, `"unsupported_gate"`).
3. The `raw` field contains the verbatim source slice of the construct (e.g. `"while (c == 0) { ... }"` or `"if (x == 1) { ... } else { ... }"`).
4. The `span` covers the entire statement boundary from the opening keyword to the closing brace or semicolon.
5. All subsequent valid statements following the unsupported construct are successfully parsed and preserved in the AST statement vector.
6. Downstream lowering stages check diagnostics or statement variants and fail closed, preventing partially lowered or unsound circuits from entering optimization pipelines.
