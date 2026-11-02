# OpenQASM 3 Conformance Specification

This document defines the conformance test suite and verification matrix for the **QCC (Quantum Circuit Compiler)** OpenQASM 3 frontend, established in **Phase 27**.

---

## 1. Conformance Corpus Overview

The conformance suite is located at [`tests/qasm3/conformance/`](../tests/qasm3/conformance/) and is executed on every `cargo test` run via [`tests/qasm3.rs`](../tests/qasm3.rs).

The suite is partitioned into three rigorous categories:
1. **Supported Conformance Suite** (`fixtures/supported/`): Positive conformance cases covering the complete accepted language subset.
2. **Unsupported Conformance Suite** (`fixtures/unsupported/`): Fail-closed negative conformance cases verifying that deferred language features produce deterministic diagnostics and are never silently accepted or skipped.
3. **Invalid Conformance Suite** (`fixtures/invalid/`): Semantic and syntactic violation cases verifying source-located diagnostic precision and resource bounds.

---

## 2. Supported Language Subset Matrix

All supported fixtures must parse, semantically validate, lower to a verified [`Circuit`], emit canonical OpenQASM 3, and validate against the pinned reference parser (`openqasm3[parser]==1.0.1`).

| Fixture File | Language Constructs Tested | Validation Criteria |
|---|---|---|
| `conformance_bell.qasm` | `qubit[2]`, `bit[2]`, `h`, `cx`, scalar `measure` assignment | Valid QccIR, Bell state amplitudes, correct MSB/LSB order |
| `conformance_rotations.qasm` | Parameterized gates `rx`, `ry`, `rz`, arithmetic expressions (`pi / 4`, `3 * pi / 4`, `-pi / 2`) | Constant folding, machine-precision float evaluation, unitary equivalence |
| `conformance_all_canonical_gates.qasm` | All 14 canonical gates: `x, y, z, h, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap`, `barrier` | 15 AST operations, exact gate signatures, unitary equivalence |
| `conformance_multi_register.qasm` | Multiple quantum (`q[2]`, `r[3]`) and classical (`c[1]`, `d[2]`) registers, cross-register gates, whole-register barrier | Non-adjacent qubit wire allocation, register offset tables, barrier expansion |
| `conformance_non_unitary.qasm` | `reset`, `barrier`, assignment measurement, arrow measurement (`measure q -> c`) | Non-unitary DAG dependency tracking (RAW, WAR, WAW), active reset simulator rejection |
| `conformance_global_phase.qasm` | Multiple `gphase` directives and unitary gates | Explicit phase accumulation ($e^{i(\phi_1 + \phi_2)}$), canonical `gphase` emission |
| `conformance_teleportation.qasm` | Quantum state preparation, Bell pair distribution, Bell basis measurement | Valid QccIR, no classical control-flow dependencies, behavioral sampling matches |

---

## 3. Unsupported Language Constructs (Fail-Closed Matrix)

In accordance with Phase 24, all unsupported OpenQASM 3 constructs are rejected deterministically before code generation, while retaining an [`AstStatement::Unsupported`] node in the recovery AST.

| Fixture File | Construct Tested | Established Diagnostic Code | Compiler Behavior |
|---|---|:---:|---|
| `unsupported_if_else.qasm` | `if` / `else` classical conditionals | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_for_loop.qasm` | `for` loops | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_while_loop.qasm` | `while` loops | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_subroutine.qasm` | `def` subroutine declarations | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_gate_def.qasm` | Custom `gate` definitions | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_defcal.qasm` | `defcal` pulse calibration definitions | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_cal_block.qasm` | Inline `cal` pulse blocks | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_three_qubit_ccx.qasm` | Standard 3-qubit `ccx` (Toffoli) gate | `E0003` | Fail-closed (requires 1Q/2Q decomposition) |
| `unsupported_three_qubit_cswap.qasm` | Standard 3-qubit `cswap` (Fredkin) gate | `E0003` | Fail-closed (requires 1Q/2Q decomposition) |
| `unsupported_standard_u3.qasm` | Standard gates requiring decomposition (`u3`, `cy`, etc.) | `E0004` | Fail-closed (deferred to decomposition passes) |
| `unsupported_standard_p.qasm` | Standard phase gate requiring basis decomposition (`p`) | `E0004` | Fail-closed (deferred to decomposition passes) |
| `unsupported_types.qasm` | Classical types `int[32]`, `float[64]`, modifiers, timing delays | `E0005` | Fail-closed, retained in recovery AST |
| `unsupported_legacy_qasm2.qasm` | Legacy OpenQASM 2 keywords (`qreg`, `creg`) | `E0002` | Fail-closed with modern migration suggestion |

---

## 4. Invalid Program Diagnostics & Resource Bounds

| Fixture File | Violation Condition | Diagnostic Code | Enforcement Mechanism |
|---|---|:---:|---|
| `invalid_out_of_bounds.qasm` | Register index out of bounds (`q[5]` on `qubit[2]`) | `E0014` | Semantic symbol table bounds check |
| `invalid_undeclared_reg.qasm` | Reference to undeclared register | `E0012` | Semantic symbol table lookup |
| `invalid_duplicate_reg.qasm` | Duplicate register identifier declaration | `E0010` | Declaration scope validation |
| `invalid_zero_sized_reg.qasm` | Declaration with size 0 (`qubit[0]`) | `E0011` | Syntactic parser validation |
| `invalid_oversized_reg.qasm` | Declaration exceeding limit (`> 1,000,000`) | `E0011` | Resource limit check (`MAX_REGISTER_SIZE`) |
| `invalid_division_by_zero.qasm` | Parameter expression division by zero (`1.0 / 0.0`) | `E0030` | Syntactic and semantic expression evaluator |
| `invalid_unresolved_param.qasm` | Unbound symbolic identifier at lowering | `E0031` | Semantic environment resolution |
| `invalid_duplicate_operands.qasm` | Duplicate qubit operands in gate (`cx q[0], q[0]`) | `E0023` | Multi-qubit operand distinctness check |
| `invalid_unknown_gate.qasm` | Unknown or unrecognized quantum gate identifier (`foo`) | `E0020` | Parser and symbol table gate lookup |
| `invalid_param_arity.qasm` | Incorrect parameter count for gate | `E0021` | Gate signature validator |
| `invalid_qubit_arity.qasm` | Incorrect qubit count for gate | `E0022` | Gate signature validator |
| `invalid_deep_nesting.qasm` | Expression nesting exceeding limit (`> 128`) | `E0001` | Recursion depth bound (`MAX_EXPRESSION_DEPTH`) |
| `invalid_type_mismatch.qasm` | Register kind mismatch (e.g. quantum destination in measurement) | `E0013` | Semantic register type validator |
| `invalid_measurement_size.qasm` | Measurement register size mismatch (`size(q) != size(c)`) | `E0025` | Semantic register size validator |

