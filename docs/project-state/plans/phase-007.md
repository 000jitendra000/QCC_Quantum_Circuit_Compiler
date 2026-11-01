# Implementation Plan: Phase 7 — Rust Circuit Builder API

## 1. Context & Objectives
- **Phase Number**: Phase 7
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Primary Goal**: Provide a safe, ergonomic, strongly typed public Rust circuit-construction API (`CircuitBuilder`) on top of Phase 6 QccIR:
  - Strongly typed quantum (`QubitHandle`) and classical (`ClassicalBitHandle`) handles.
  - Safe register creation (`QuantumRegisterHandle`, `ClassicalRegisterHandle`) with bounds and uniqueness validation.
  - Cross-builder handle rejection (`ForeignHandle` error if a handle from another builder instance is used).
  - Single-qubit fixed gates (`h`, `x`, `y`, `z`, `s`, `sdg`, `t`, `tdg`).
  - Single-qubit parameterized rotations (`rx`, `ry`, `rz`) accepting numeric, `Angle`, and symbolic `ParameterExpr`.
  - Multi-qubit entanglers (`cx`, `cz`, `swap`) with explicit operand role validation (operand 0 = control, operand 1 = target for CX; distinct operands enforced).
  - Non-unitary operations (`measure`, `reset`, `barrier`).
  - Circuit-level global phase manipulation (`set_global_phase`, `add_global_phase`).
  - Public immutable circuit view (`CircuitView` and `Circuit::view()`).
  - Consuming `finish()` method performing complete semantic validation on the constructed `Circuit`.
  - Complete Bell circuit example in `examples/bell.rs`.
  - Comprehensive test suite in `tests/builder.rs`.

## 2. Invariants & Conventions
1. **Never Expose Graph Internals**: The builder wraps `Circuit` and `CircuitGraph`; petgraph internals and raw graph mutation are never exposed.
2. **Handle Provenance Validation**: Every handle stores `builder_id: u64`. Operations reject foreign handles with `BuilderError::ForeignHandle`.
3. **Consuming Finish**: `finish(self)` consumes the builder by value, preventing double-finishing or inconsistent mutation after completion.
4. **Distinct Operands**: Multi-qubit gates and barriers reject duplicate operand qubits.
5. **Phase & Gate Compatibility**: Gate operations correctly instantiate Phase 2 `Gate` definitions and maintain Phase 6 `WireAccessState` hazard tracking.

## 3. Module Architecture
- `src/builder/mod.rs`: Builder module facade and re-exports.
- `src/builder/errors.rs`: Typed `BuilderError` enum with structured variants.
- `src/builder/handles.rs`: `QubitHandle`, `ClassicalBitHandle`, `QuantumRegisterHandle`, `ClassicalRegisterHandle`.
- `src/builder/view.rs`: `CircuitView` providing a read-only query and summary interface.
- `src/builder/circuit_builder.rs`: `CircuitBuilder` implementation with all gate, measurement, barrier, and phase methods.
- `examples/bell.rs`: Bell circuit construction demonstration.
- `tests/builder.rs`: Integration tests for builder functionality and failure modes.

## 4. Execution Steps
1. Create `src/builder/errors.rs` with typed error variants.
2. Add `ParameterExpr` conversion helpers (`From<f64>`, `From<Angle>`, `From<&str>`, `From<String>`) in `src/ir/parameter.rs`.
3. Create `src/builder/handles.rs` with builder ID tracking, indexing, and register slice handles.
4. Create `src/builder/view.rs` with `CircuitView`.
5. Create `src/builder/circuit_builder.rs` with builder logic and consuming `finish()`.
6. Create `src/builder/mod.rs` and re-export builder types.
7. Update `src/errors.rs` to include `Builder(#[from] BuilderError)`.
8. Update `src/lib.rs` to expose `pub mod builder;` and re-export top-level builder items.
9. Implement `examples/bell.rs`.
10. Write comprehensive test suite in `tests/builder.rs`.
11. Run full verification suite (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`).
12. Create completion report and update progress ledger.
