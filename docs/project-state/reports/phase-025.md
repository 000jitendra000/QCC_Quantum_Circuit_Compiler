# Phase Completion Report: Phase 25 — AST-to-QccIR Lowering

## 1. Summary
- **Phase Number**: Phase 25
- **Exact Phase Title**: AST-to-QccIR lowering
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity (Advanced Agentic Coding)
- **Status**: **Completed**
- **Exit Gate Satisfied**: **Yes** — Lowering semantic equivalence passes (`147/147` tests in `tests/qasm3.rs`, `531/531` workspace tests across all 21 test targets).

---

## 2. Deliverables & Files Changed

### New Files Created
- `src/frontend/lowering/registers.rs`: Register mapping and allocation into QccIR quantum and classical registers, resolving single-bit/qubit references and whole-register expansions into typed QccIR `QubitId` and `ClassicalBitId` identities.
- `src/frontend/lowering/operations.rs`: Lowers fixed gates (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`), parameterized rotation gates (`rx`, `ry`, `rz`), 2-qubit entanglers (`cx`, `cz`, `swap`), measurement statements (single-bit and whole-register), active reset statements, synchronization barriers (global and selective), circuit global phase (`gphase`), and parameter expressions.
- `src/frontend/lowering/mod.rs`: `CircuitLowerer` driver orchestrating semantic validation, register allocation, statement lowering, QccIR graph invariant validation, source-provenance preservation in `circuit.metadata`, and structured diagnostic generation.
- `tests/qasm3/lowering.rs`: Comprehensive integration test suite (17 tests) covering builder-vs-QASM equivalence, unitary equivalence checking, register allocation, operand ordering, measurement/reset/barrier lowering, global phase accumulation, graph invariants, and fail-closed negative rejection.

### Existing Files Modified
- `src/diagnostics/qasm.rs`: Added stable lowering error code `QASM_LOWERING_ERROR` (`E0040`) and constructor helper `lowering_error`.
- `src/frontend/adapter.rs`: Added `lower` and `lower_with_name` methods to `OpenQasmFrontend`. Fixed broken rustdoc intra-doc links.
- `src/frontend/mod.rs`: Added `pub mod lowering;` and re-exported lowering driver and helper functions.
- `src/lib.rs`: Re-exported `CircuitLowerer`, `lower_qasm_str`, and lowering functions.
- `tests/qasm3.rs`: Registered `lowering` submodule in test runner.
- `docs/project-state/plans/phase-025.md`: Implementation plan.
- `docs/project-state/progress.md`: Updated roadmap progress ledger.

---

## 3. Exit Gate Verification Evidence

> **EXIT GATE**: Lowering semantic equivalence passes.

### Verification Commands Executed
```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --all-targets
cargo doc --no-deps
cargo run --example bell
cargo run --example qasm
python3 -c "import sys; sys.path.insert(0, '/tmp/py_packages'); import openqasm3; openqasm3.parse(open('examples/qasm/bell.qasm').read())"
```

All commands exited with status `0`.

### Execution Log / Output
- `cargo test --test qasm3`: **147 passed**, 0 failed, 0 ignored.
- `cargo test --all`: **531 passed across 21 test targets**, 0 failed, 0 ignored.
- `UnitaryEquivalenceChecker::check_circuits` / `check_equivalence`:
  - Bell circuit lowering matches `CircuitBuilder` Bell state ($| \text{diff} | < 10^{-10}$).
  - Parameterized rotation circuits (`rx`, `ry`, `rz`) with constant expressions (`pi / 4`, `pi / 2`, `3 * pi / 4`) match `CircuitBuilder` rotations ($| \text{diff} | < 10^{-10}$).
  - Parameterized rotation circuits with bound symbols (`theta = 1.2345`, `phi = 0.6789`) match `CircuitBuilder` rotations ($| \text{diff} | < 10^{-10}$).
  - Global phase accumulation (`gphase(pi/3)` + `gphase(pi/6)` = $\pi/2$) matches `CircuitBuilder` with `set_global_phase(PI / 2.0)` ($| \text{diff} | < 10^{-10}$).
- Independent Python `openqasm3` reference parser successfully parsed `examples/qasm/bell.qasm` (7 top-level statements).

---

## 4. Test Suite Summary & Inventory

### Test Targets Before vs. After Phase 25

| # | Test Target | Phase 24 Count (`884498e`) | Phase 25 Count (HEAD) | Delta & Notes |
|:---:|---|:---:|:---:|---|
| 1 | `src/lib.rs` (unit tests) | 71 | 71 | Unchanged |
| 2 | `src/main.rs` (unit tests) | 0 | 0 | Unchanged |
| 3 | `tests/ast.rs` | 30 | 30 | Unchanged |
| 4 | `tests/builder.rs` | 10 | 10 | Unchanged |
| 5 | `tests/end_to_end.rs` | 9 | 9 | Unchanged |
| 6 | `tests/equivalence.rs` | 23 | 23 | Unchanged |
| 7 | `tests/frontend_adapter.rs` | 14 | 14 | Unchanged |
| 8 | `tests/gate_identities.rs` | 23 | 23 | Unchanged |
| 9 | `tests/ir.rs` | 19 | 19 | Unchanged |
| 10 | `tests/ir_rewrites.rs` | 21 | 21 | Unchanged |
| 11 | `tests/lexer.rs` | 29 | 29 | Unchanged |
| 12 | `tests/measurement.rs` | 11 | 11 | Unchanged |
| 13 | `tests/openqasm_emitter.rs` | 21 | 21 | Unchanged |
| 14 | `tests/pass_manager.rs` | 14 | 14 | Unchanged |
| 15 | `tests/qasm3.rs` | 130 | 147 | +17 (`lowering.rs` integration tests) |
| 16 | `tests/quantum_semantics.rs` | 23 | 23 | Unchanged |
| 17 | `tests/sampling.rs` | 9 | 9 | Unchanged |
| 18 | `tests/simulator_dag.rs` | 15 | 15 | Unchanged |
| 19 | `tests/simulator_single_qubit.rs` | 19 | 19 | Unchanged |
| 20 | `tests/simulator_two_qubit.rs` | 18 | 18 | Unchanged |
| 21 | `tests/smoke.rs` | 5 | 5 | Unchanged |
| **Total** | **All 21 Test Targets** | **514** | **531** | **+17 net new passing tests (0 removed, 0 modified)** |

*Arithmetic Verification*:
$71 + 0 + 30 + 10 + 9 + 23 + 14 + 23 + 19 + 21 + 29 + 11 + 21 + 14 + 147 + 23 + 9 + 15 + 19 + 18 + 5 = \mathbf{531}$.

### New Integration Tests in `tests/qasm3/lowering.rs`
1. `test_bell_circuit_lowering_and_builder_equivalence`: Validates structural, topological, and unitary equivalence between QASM and builder Bell circuits.
2. `test_parameterized_rotations_constant_and_pi_expressions`: Validates unitary equivalence for `rx`, `ry`, `rz` with arithmetic and $\pi$ constants.
3. `test_parameterized_rotations_with_runtime_symbol_bindings`: Validates unitary equivalence for bound symbolic variables against builder circuits.
4. `test_single_bit_measurement_assignment_and_arrow_lowering`: Verifies lowering of assignment and arrow measurement forms to QccIR `Operation::Measure`.
5. `test_whole_register_measurement_lowering`: Verifies whole-register measurement expands to $N$ sequential measurements.
6. `test_scalar_measurement_lowering`: Verifies scalar `c = measure q;` lowers to single measurement.
7. `test_multi_register_allocation_and_cx_operand_ordering`: Verifies multi-register index arithmetic and CX control/target operand order.
8. `test_reset_statement_lowering`: Verifies single and whole-register reset lowering.
9. `test_global_and_selective_barrier_lowering`: Verifies global and selective barrier lowering to `Operation::Barrier`.
10. `test_global_phase_accumulation_and_equivalence`: Verifies `gphase` accumulation and unitary equivalence up to global phase.
11. `test_source_span_preservation_in_metadata`: Verifies program and per-operation source span coordinates in `circuit.metadata`.
12. `test_lowered_circuit_satisfies_all_qccir_invariants`: Verifies comprehensive `circuit.validate()` graph invariant checks on lowered circuit with all supported operations.
13. `test_negative_unsupported_syntax_rejected_fail_closed`: Verifies fail-closed rejection and diagnostic codes for unsupported constructs (`E0002`–`E0005`).
14. `test_negative_semantic_errors_fail_closed`: Verifies fail-closed rejection for undeclared registers, arity mismatches, duplicate CX operands, division by zero, unresolved symbols, and size mismatches.
15. `test_negative_duplicate_register_declaration`: Verifies duplicate register declarations fail lowering (`E0010`).
16. `test_negative_zero_size_register`: Verifies zero-sized register declarations fail lowering (`E0011`).
17. `test_openqasm_frontend_adapter_lower_method`: Verifies `OpenQasmFrontend::lower` and `lower_with_name` API methods.

---

## 5. Architectural Invariants Verified

- [x] **Stable Rust Toolchain**: Clean compilation with stable toolchain (zero nightly features, zero compiler warnings).
- [x] **Little-Endian Endianness**: $q_0$ is LSB; computational basis indices correspond to bit integers $i = \sum q_k 2^k$.
- [x] **Gate Operand Ordering**: For $CX(q_{\text{ctrl}}, q_{\text{targ}})$, operand 0 is control and operand 1 is target.
- [x] **Circuit Global Phase**: Preserved explicitly in `circuit.global_phase` ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] **Wire Hazards & Dependencies**: Maintained through `Circuit::add_operation`, accurately inducing RAW/WAR/WAW dependencies on classical wires and quantum ordering on qubits.
- [x] **Stable IDs**: Nodes identified by stable `OperationId`, registers by typed `QubitId` and `ClassicalBitId`. Petgraph indices remain strictly unexposed.
- [x] **QccIR Graph Validation**: All lowered circuits pass `circuit.validate()` with zero errors.
- [x] **Fail-Closed Boundary**: Unsupported syntax and semantic errors halt lowering immediately; `output` is strictly `None` upon error.

---

## 6. Known Limitations & Technical Debt

- **Phase Boundary Boundary**: Phase 26 will implement bidirectional import/export round-trip validation and canonical re-emission of lowered circuits.
- **Dynamic Control Flow**: Conditionals and loops are strictly rejected with `E0005` in accordance with the Phase 24 fail-closed policy; dynamic CFG lowering is reserved for future research milestones (`M7`/`Research`).

---

## 7. Approval & Ledger Update

- [x] Exit gate satisfied.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
- [x] Next Phase: **Phase 26 — QASM import/export round-trip**.

