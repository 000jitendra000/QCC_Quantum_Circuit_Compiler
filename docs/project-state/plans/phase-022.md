# Implementation Plan: Phase 22 — Measurement, Reset and Barrier Parser

## 1. Objective & Scope

Implement Phase 22 of the QCC Engineering Roadmap v2:
- Parse single-bit and whole-register measurement statements (`c[i] = measure q[j];`, `c = measure q;`, `measure q[j] -> c[i];`, `measure q -> c;`).
- Parse active state reset statements (`reset q[i];`, `reset q;`).
- Parse global and operand-selective synchronization barriers (`barrier;`, `barrier q[0], q[1];`, `barrier q;`).
- Validate quantum source vs classical destination types and bounds against the static [`RegisterTable`].
- Preserve statement order, operand ordering, and exact source spans.
- Maintain pure frontend isolation: zero dependencies on `crate::ir` or `crate::quantum`.

## 2. Prerequisites Verification

- Phase 21 follow-up committed and verified as `2587093`.
- Repository working tree is clean on branch `main`.
- All 408 existing tests pass with zero warnings across 21 test targets.

## 3. Ordered Implementation Steps

1. **Register Table Enhancements (`src/frontend/parser/registers.rs`)**:
   - Add `RegisterError::MeasurementSizeMismatch` (diagnostic code `E0025`).
   - Add `validate_qubit_ref(&self, qubit_ref: &AstQubitRef, allow_unindexed_array: bool) -> Result<(), RegisterError>`.
   - Add `validate_bit_ref(&self, bit_ref: &AstBitRef, allow_unindexed_array: bool) -> Result<(), RegisterError>`.
   - Add `validate_measurement(&self, qubit_ref: &AstQubitRef, bit_ref: &AstBitRef) -> Result<(), RegisterError>` enforcing quantum/classical typing, bounds, scalar vs array matching, and whole-register size equality.

2. **Operand Parser Enhancements (`src/frontend/parser/operands.rs`)**:
   - Add `parse_qubit_operand(&mut self, table: Option<&RegisterTable>, allow_unindexed: bool) -> Result<AstQubitRef, Box<Diagnostic>>`.
   - Add `parse_bit_operand(&mut self, table: Option<&RegisterTable>, allow_unindexed: bool) -> Result<AstBitRef, Box<Diagnostic>>`.
   - Add `parse_qubit_operands_allow_unindexed(&mut self, table: Option<&RegisterTable>, allow_unindexed: bool) -> Result<Vec<AstQubitRef>, Box<Diagnostic>>`.
   - Keep existing `parse_qubit_ref`, `parse_bit_ref`, and `parse_qubit_operands` delegating with `allow_unindexed = false`.

3. **Measurement Parser (`src/frontend/parser/measurement.rs`)**:
   - Implement `MeasurementParser<'a>`.
   - Parse assignment measurement: `target = measure source;`.
   - Parse arrow measurement: `measure source -> target;`.
   - Handle whole-register forms: `c = measure q;` and `measure q -> c;`.
   - Error recovery with semicolon skipping and source-located diagnostics (`E0001`, `E0012`, `E0013`, `E0014`, `E0015`, `E0016`, `E0025`).

4. **Reset Parser (`src/frontend/parser/reset.rs`)**:
   - Implement `ResetParser<'a>`.
   - Parse `reset q[i];` and `reset q;`.
   - Validate operand is quantum (`qubit`) and within bounds.
   - Represent as non-unitary `AstStatement::Reset(AstReset)`.

5. **Barrier Parser (`src/frontend/parser/barrier.rs`)**:
   - Implement `BarrierParser<'a>`.
   - Parse global barrier: `barrier;` -> empty `qubits` list.
   - Parse selective barrier: `barrier q[0], q[1];` and register barriers `barrier q;`.
   - Validate operands are quantum (`qubit`), reject classical registers, and preserve operand order.

6. **Adapter Integration (`src/frontend/adapter.rs`)**:
   - Dispatch to `MeasurementParser`, `ResetParser`, and `BarrierParser`.
   - Unambiguously distinguish assignment measurement from gate statements by looking ahead for `=`.
   - Re-export new parsers in `src/frontend/parser/mod.rs`, `src/frontend/mod.rs`, and `src/lib.rs`.

7. **Integration Tests (`tests/qasm3/non_unitary.rs` & `tests/qasm3.rs`)**:
   - Add comprehensive tests covering all required categories.
   - Inspect AST structure (quantum source, classical target, barrier operands, source spans).

8. **Verification & Ledger**:
   - Verify `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo build`, `cargo doc --no-deps`.
   - Run `bell` and `qasm` examples.
   - Validate emitted QASM with Python `openqasm3`.
   - Author `docs/project-state/reports/phase-022.md` and update `docs/project-state/progress.md`.
   - Commit as `feat(phase-22): implement measurement reset and barrier parser`.
