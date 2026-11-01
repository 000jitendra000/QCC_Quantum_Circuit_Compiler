# Phase 9 Implementation Plan: OpenQASM 3 Emitter

## Objective
Implement a deterministic QccIR-to-OpenQASM 3 exporter so the Bell vertical slice and circuit models can be exported as valid OpenQASM 3.0 source code immediately, without waiting for the full parser frontend.

---

## 1. Scope & Modules

### Files to Create:
- `src/backend/mod.rs`: Backend module container re-exporting `openqasm`.
- `src/backend/openqasm/mod.rs`: OpenQASM 3 backend module root exposing emitter, options, formatting, and errors.
- `src/backend/openqasm/errors.rs`: Structured `OpenQasmError` enum.
- `src/backend/openqasm/format.rs`: Deterministic formatting for identifiers, register references, floats, parameter expressions, and precedence.
- `src/backend/openqasm/emitter.rs`: `OpenQasmEmitter`, `EmitterOptions`, `GlobalPhasePolicy`, `MeasurementSyntax`, and `emit_openqasm(&Circuit)`.
- `examples/qasm/main.rs`: Example building, validating, exporting, and saving Bell state OpenQASM 3 output.
- `tests/openqasm_emitter.rs`: Comprehensive test suite verifying all required syntax, formatting, ordering, error cases, and golden outputs.
- `docs/project-state/reports/phase-009.md`: Phase completion report.

### Files to Modify:
- `src/lib.rs`: Expose `backend` module and top-level OpenQASM export types/functions.
- `src/errors.rs`: Add `QccError::OpenQasm(#[from] OpenQasmError)`.
- `docs/project-state/progress.md`: Update ledger upon verified completion.

---

## 2. Architecture & Design

### Output Grammar & Formatting
1. Version declaration: `OPENQASM 3.0;\n`
2. Include directive: `include "stdgates.inc";\n` (auto-included if standard gates are present).
3. Register declarations:
   - `qubit[n] q;\n`
   - `bit[n] c;\n`
4. Global phase:
   - When 0.0: omitted by default (semantic identity $e^{i \cdot 0} = 1$).
   - When non-zero: `gphase(<formatted_expr>);\n`.
   - Supported via `GlobalPhasePolicy` (`EmitGPhase`, `RejectNonZero`, `AlwaysEmit`).
5. Operations:
   - Emitted in strictly deterministic topological order from Kahn's algorithm.
   - Standard gates in lowercase: `h q[0];`, `cx q[0], q[1];`.
   - Parameter-first syntax: `rx(theta) q[0];`, `rz(1.5707963267948966) q[1];`.
   - Measurements: `c[0] = measure q[0];` (default assignment) or `measure q[0] -> c[0];` (optional arrow).
   - Resets: `reset q[0];`.
   - Barriers: `barrier q[0], q[1];` or `barrier;`.

### Parameter Expression Precedence
- Atomic (Constant, Symbol): highest precedence (Level 4).
- Negation (`-`): Level 3.
- Multiplication, Division (`*`, `/`): Level 2.
- Addition, Subtraction (`+`, `-`): Level 1.
- Operands parenthesized when precedence demands, preserving exact mathematical semantics without unnecessary clutter.
- Constants validated: non-finite (`NaN`, `Inf`) rejected with `OpenQasmError::NonFiniteParameter`.

### Pre-export Validation
- Validate circuit graph before export (`circuit.validate()?`). Never emit partial or invalid circuits.
- Unsupported operations (e.g. `Operation::Classical`) produce structured `OpenQasmError::UnsupportedOperation`.
- Unsupported gates produce `OpenQasmError::UnsupportedGate`.

---

## 3. Step-by-Step Execution Plan

1. Create `src/backend/openqasm/errors.rs`.
2. Create `src/backend/openqasm/format.rs`.
3. Create `src/backend/openqasm/emitter.rs`.
4. Create `src/backend/openqasm/mod.rs` and `src/backend/mod.rs`.
5. Update `src/errors.rs` and `src/lib.rs`.
6. Implement `examples/qasm/main.rs`.
7. Implement `tests/openqasm_emitter.rs`.
8. Run full verification suite (`fmt`, `clippy`, `test`, `build`, `doc`, `examples`).
9. Draft completion report in `docs/project-state/reports/phase-009.md`.
10. Update progress ledger in `docs/project-state/progress.md`.
11. Commit changes and present results.
