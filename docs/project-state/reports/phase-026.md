# Phase Completion Report: Phase 26 — QASM Import/Export Round-Trip

## 1. Summary
- **Phase Number**: Phase 26
- **Exact Phase Title**: QASM import/export round-trip
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity (Advanced Agentic Coding)
- **Status**: **Completed & Audited**
- **Exit Gate Satisfied**: **Yes** — Independent parse and semantic round-trip suite passes (`169/169` tests in `tests/qasm3.rs`, `553/553` workspace tests across all 21 test targets).

---

## 2. Deliverables & Files Changed

### New Files Created
- `tests/fixtures/qasm3/bell.qasm`: Bell state preparation and measurement fixture.
- `tests/fixtures/qasm3/rotations.qasm`: Parameterized rotation gates (`rx`, `ry`, `rz` with arithmetic and $\pi$ constants).
- `tests/fixtures/qasm3/non_unitary.qasm`: Mixed unitary gates, active reset, selective barrier, and measurement.
- `tests/fixtures/qasm3/multi_register.qasm`: Multiple quantum and classical registers, non-adjacent qubit operands, and whole-register barrier.
- `tests/fixtures/qasm3/global_phase.qasm`: Explicit `gphase` directives and gate sequences.
- `tests/fixtures/qasm3/teleportation.qasm`: Standard quantum teleportation protocol in supported OpenQASM 3 subset.
- `tests/qasm3/roundtrip.rs`: Comprehensive integration test suite (16 tests) implementing:
  - Golden fixture round trips ($S_0 \to C_1 \to S_1 \to C_2 \to S_2$)
  - Independent reference parser validation (`openqasm3[parser]==1.0.1`)
  - Double round-trip re-emission idempotency ($S_1 == S_2$)
  - Formatting insensitivity (whitespace, comments, multi-line expressions, arrow vs assignment syntax)
  - Parameter, global phase, and wire dependency preservation
  - Pseudo-randomized supported-circuit property round trips (unitary and non-unitary)
  - Parameterized symbolic runtime bindings round trip
  - Direct automated reference parser conformance verification
- `docs/qasm3_support.md`: Comprehensive specification of QCC's OpenQASM 3 language support, grammar rules, canonical emitter conventions, semantic round-trip contracts, and fail-closed diagnostics.
- `docs/project-state/plans/phase-026.md`: Phase 26 implementation plan.

### Existing Files Modified
- `tests/qasm3.rs`: Registered `roundtrip` submodule in the test runner.
- `docs/project-state/progress.md`: Updated milestone progress, phase ledger, and completed phase evidence log.

---

## 3. Exit Gate Verification Evidence

> **EXIT GATE**: Independent parse and semantic round-trip suite passes.

### Verification Commands Executed
```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --all-targets
cargo doc --no-deps
cargo run --example bell
cargo run --example qasm
python3 -c "
import sys
sys.path.insert(0, '/tmp/py_packages')
import openqasm3
for f in ['bell', 'rotations', 'non_unitary', 'multi_register', 'global_phase', 'teleportation']:
    openqasm3.parse(open(f'tests/fixtures/qasm3/{f}.qasm').read())
"
```

All commands exited with status `0`.

### Execution Log / Output Summary
- `cargo test --test qasm3`: **169 passed**, 0 failed, 0 ignored.
- `cargo test --all`: **553 passed across all 21 test targets**, 0 failed, 0 ignored.
- Independent reference parser (`openqasm3` v1.0.1) parsed all 6 golden fixtures and all dynamically emitted round-trip programs with 0 errors.
- Unitary equivalence (`check_equivalence`) verified across all pure-unitary round trips ($|\text{diff}| < 10^{-10}$).
- Behavioral sampling equivalence (`sample_circuit` with seeded `SplitMix64`) verified across all measurement round trips with bit-for-bit identical outcome histograms.
- Double round-trip re-emission proven strictly idempotent ($S_1 == S_2$) across all supported programs.

---

## 4. Test Suite Summary & Inventory

### Test Targets Before vs. After Phase 26

| # | Test Target | Phase 25 Count (`c019c06`) | Phase 26 Initial (`f0446ac`) | Phase 26 Audited (HEAD) | Delta & Notes |
|:---:|---|:---:|:---:|:---:|---|
| 1 | `src/lib.rs` (unit tests) | 71 | 71 | 71 | Unchanged |
| 2 | `src/main.rs` (unit tests) | 0 | 0 | 0 | Unchanged |
| 3 | `tests/ast.rs` | 30 | 30 | 30 | Unchanged |
| 4 | `tests/builder.rs` | 10 | 10 | 10 | Unchanged |
| 5 | `tests/end_to_end.rs` | 9 | 9 | 9 | Unchanged |
| 6 | `tests/equivalence.rs` | 23 | 23 | 23 | Unchanged |
| 7 | `tests/frontend_adapter.rs` | 14 | 14 | 14 | Unchanged |
| 8 | `tests/gate_identities.rs` | 23 | 23 | 23 | Unchanged |
| 9 | `tests/ir.rs` | 19 | 19 | 19 | Unchanged |
| 10 | `tests/ir_rewrites.rs` | 21 | 21 | 21 | Unchanged |
| 11 | `tests/lexer.rs` | 29 | 29 | 29 | Unchanged |
| 12 | `tests/measurement.rs` | 11 | 11 | 11 | Unchanged |
| 13 | `tests/openqasm_emitter.rs` | 21 | 21 | 21 | Unchanged |
| 14 | `tests/pass_manager.rs` | 14 | 14 | 14 | Unchanged |
| 15 | `tests/qasm3.rs` | 153 | 166 | 169 | +16 (`roundtrip.rs` integration tests: +13 initial, +3 audit) |
| 16 | `tests/quantum_semantics.rs` | 23 | 23 | 23 | Unchanged |
| 17 | `tests/sampling.rs` | 9 | 9 | 9 | Unchanged |
| 18 | `tests/simulator_dag.rs` | 15 | 15 | 15 | Unchanged |
| 19 | `tests/simulator_single_qubit.rs` | 19 | 19 | 19 | Unchanged |
| 20 | `tests/simulator_two_qubit.rs` | 18 | 18 | 18 | Unchanged |
| 21 | `tests/smoke.rs` | 5 | 5 | 5 | Unchanged |
| **Total** | **All 21 Test Targets** | **537** | **550** | **553** | **+16 net new passing tests (0 removed, 0 modified)** |

*Arithmetic Verification*:
$71 + 0 + 30 + 10 + 9 + 23 + 14 + 23 + 19 + 21 + 29 + 11 + 21 + 14 + 169 + 23 + 9 + 15 + 19 + 18 + 5 = \mathbf{553}$.

### Full List of Integration Tests in `tests/qasm3/roundtrip.rs` (16 tests)
1. `test_golden_fixture_bell_roundtrip`: Verifies round-trip, independent `openqasm3` parsing, idempotency, and sampling equivalence on `bell.qasm`.
2. `test_golden_fixture_rotations_roundtrip`: Verifies round-trip, independent `openqasm3` parsing, idempotency, and unitary equivalence on `rotations.qasm`.
3. `test_golden_fixture_non_unitary_roundtrip`: Verifies round-trip, independent `openqasm3` parsing, idempotency, and active reset rejection on `non_unitary.qasm`.
4. `test_golden_fixture_multi_register_roundtrip`: Verifies multi-register allocation, non-adjacent operands, and whole-register barrier expansion on `multi_register.qasm`.
5. `test_golden_fixture_global_phase_roundtrip`: Verifies global phase accumulation, canonical `gphase` emission, and unitary equivalence on `global_phase.qasm`.
6. `test_golden_fixture_teleportation_roundtrip`: Verifies full quantum teleportation round-trip and sampling equivalence on `teleportation.qasm`.
7. `test_measurement_syntax_variants_roundtrip`: Verifies source arrow syntax round-trips to canonical assignment by default, and arrow emission option preserves arrow syntax.
8. `test_whole_register_measurement_roundtrip`: Verifies whole-register measurement expands to $N$ scalar measurements and preserves sampling distribution.
9. `test_formatting_insensitivity_roundtrip`: Verifies erratic whitespace, comments, blank lines, and multi-line expressions normalize to clean canonical OpenQASM without semantic change.
10. `test_parameter_expressions_and_constant_folding_roundtrip`: Verifies parameterized rotation gates with arithmetic expressions round-trip with machine precision.
11. `test_global_phase_policies_roundtrip`: Verifies `EmitGPhase`, `AlwaysEmit`, and zero-phase omission policies across round trips.
12. `test_all_canonical_gates_roundtrip`: Verifies round-trip across all 14 canonical gates (`x, y, z, h, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap`).
13. `test_randomized_circuit_roundtrips`: Property test generating 15 pseudo-random supported non-unitary circuits across all 14 canonical gates with varied rotation angles, verifying round-trip semantic equality, idempotency, independent parser validation, and behavioral sampling.
14. `test_randomized_unitary_circuit_roundtrips` *(added during audit)*: Property test generating 15 pseudo-random pure-unitary circuits across 2 to 4 qubits and all 14 gates, verifying exact unitary equivalence (`check_equivalence`), statevector amplitude equality (`execute_circuit`), re-emission idempotency ($S_1 == S_2$), and independent parser acceptance.
15. `test_parameterized_symbolic_bindings_roundtrip` *(added during audit)*: Verifies round-tripping of parameterized circuits with runtime symbol bindings (`theta`, `phi`, `alpha`), checking concrete value emission, unitary equivalence, and statevector equality.
16. `test_independent_parser_presence_and_golden_fixtures_conformance` *(added during audit)*: Automated assertion that pinned Python reference parser (`openqasm3==1.0.1`) is available and parses all 6 golden fixtures and their emitted round-trip circuits directly.

---

## 5. Phase 26 Exit-Gate Audit Findings

### 1. Randomized Round-Trip Coverage
- **Generator**: Uses deterministic PRNG `SplitMix64` (seed `0xDEAD_BEEF_CAFE` for non-unitary, `0xBEEF_CAFE_FEED` for pure-unitary).
- **Supported Operations**: Covers all 14 canonical gates (`h, x, y, z, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap`), global phase, barriers, and measurements. During audit, `sdg` and `tdg` were restored to the randomized generator and rotation angles were varied across a realistic domain (`pi/8, pi/4, pi/2, 3*pi/4, -pi/4, -pi/2, 0.35, 1.2`).
- **Qubit & Gate Counts**: 2 to 4 qubits, 5 to 17 gates per circuit, across 15 iterations per test (30 pseudo-random cases total).
- **Failures & Reproducibility**: Seeded PRNG ensures any failure is strictly deterministic and trivially reproducible by seed index.
- **Coverage Scope**:
  - *Establishes*: Topological DAG order, register table preservation, wire dependency DAG edges, re-emission idempotency ($S_1 == S_2$), independent parser acceptance, identical behavioral sampling distributions for non-unitary circuits, and machine-precision unitary equivalence ($|\text{diff}| < 10^{-10}$) and statevector amplitude equality for pure-unitary circuits.
  - *Does Not Establish*: Dynamic branching or runtime control-flow (which are explicitly rejected by M2 frontend).

### 2. Non-Unitary Semantics
- **Measurement Semantic Checks**: In `assert_circuits_semantically_equal`, measurements are compared for source qubit index, classical destination bit index, quantum/classical register tables, and DAG wire dependency edges (RAW, WAR, WAW).
- **Behavioral Sampling**: Applied responsibly to measurement circuits (`bell.qasm`, `teleportation.qasm`, `test_formatting_insensitivity_roundtrip`, `test_randomized_circuit_roundtrips`) with seeded PRNG (`SplitMix64`) and MSB-first bitstring histogram verification.
- **Active Reset**: In `non_unitary.qasm`, active reset `reset q[1];` is tested through lowering and emission. The test verifies that both $C_1$ and $C_2$ fail terminal-only simulation with `ExecutionError::UnsupportedOperation { operation: "reset" }`, accurately confirming simulator rejection without falsely claiming mid-circuit state reset in M2.
- **Barrier Synchronization**: Operands and ordered wire dependencies are strictly asserted across all round trips. Whole-register barrier expansion (`barrier q, r;` $\to$ `barrier q[0], q[1], r[0], r[1], r[2];`) preserves all dependency constraints.
- **Unitary Equivalence Isolation**: Non-unitary circuits containing `Measure` or `Reset` are strictly excluded from `check_equivalence`, satisfying the contract that `check_equivalence` only operates on unitary operators.

### 3. Operation, Register, Parameter, and Phase Comparison
- **Register & Operation Checks**: Topological traversal compares gate names (case-insensitive), ordered qubit operands, parameter values, measurement source/destination, reset qubits, and barrier qubit lists.
- **Parameter Comparison**: Evaluates parameters with tolerance `1e-12`. Enhanced during audit to accept optional environment bindings (`bindings: Option<&HashMap<String, f64>>`) with graceful fallback to structural expression equivalence (`p_a == p_b`) if symbols remain unbound.
- **Global Phase Comparison**: Circuit-level global phase is verified using `shortest_angle_difference(phase1, phase2).abs() < 1e-12`, preserving phase accumulated across multiple `gphase` directives.
- **Symbolic Bindings**: Tested via `test_parameterized_symbolic_bindings_roundtrip`, proving circuits with symbolic parameters lower with bindings, emit evaluated constants, re-import, and preserve unitary equivalence.

### 4. Independent Parser and Fixture Audit
- **Pinned Version**: Python `openqasm3` v1.0.1 installed at `/tmp/py_packages` (`openqasm3.__version__ == '1.0.1'`).
- **Execution Guarantee**: Verified through `test_independent_parser_presence_and_golden_fixtures_conformance`, which fails if Python or `openqasm3` is unavailable or fails to parse any fixture.
- **Six Golden Fixtures**:
  - `bell.qasm`: 7 statements (Bell state prep and measurement)
  - `rotations.qasm`: 8 statements ($R_x, R_y, R_z$ with arithmetic and $\pi$)
  - `non_unitary.qasm`: 9 statements (Reset, barrier, CX, measurements)
  - `multi_register.qasm`: 12 statements (Multiple quantum/classical registers, whole-register barrier)
  - `global_phase.qasm`: 6 statements (Multiple `gphase` directives accumulating to $\pi/2$)
  - `teleportation.qasm`: 12 statements (State preparation, EPR pair, Bell basis measurement)
- **Control Flow Conformance**: `teleportation.qasm` intentionally avoids unsupported classical control flow (`if`), performing state preparation, entanglement distribution, Bell-basis change, and classical recording strictly within the supported Phase 24 subset.

---

## 6. Architectural Invariants Verified

- [x] **Independent Verification**: Every emitted program validated against independent `openqasm3` v1.0.1; never self-validating only.
- [x] **Semantic Equivalence Contract**: Equivalence defined by topological operation matching, wire hazards, unitary matrices, and sampling distributions—not byte-for-byte formatting.
- [x] **Double Round-Trip Idempotency**: Normalized circuits re-emit to identical source strings ($S_1 == S_2$).
- [x] **Formatting Insensitivity**: Frontend accepts comments, whitespace, and syntax variants, producing normalized canonical output.
- [x] **Little-Endian Endianness**: Logical qubit $q[0]$ is LSB in statevector index and basis state representations.
- [x] **Gate Operand Ordering**: $CX(q_{\text{ctrl}}, q_{\text{targ}})$ operand 0 is control, operand 1 is target throughout round trips.
- [x] **Circuit Global Phase**: Preserved explicitly in `circuit.global_phase` ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] **Wire Hazards & Dependencies**: Maintained through `Circuit::add_operation`, accurately inducing RAW/WAR/WAW dependencies on classical wires and quantum ordering on qubits.
- [x] **Stable IDs**: Nodes identified by stable `OperationId`, registers by typed `QubitId` and `ClassicalBitId`. Petgraph indices remain strictly unexposed.
- [x] **QccIR Graph Validation**: All lowered and round-tripped circuits pass `circuit.validate()` with zero errors.

---

## 7. Known Limitations & Technical Debt

- **Phase Boundary**: Phase 27 will add parser/lexer fuzzing, conformance expansion, and frontend CLI commands (`qcc compile`, `qcc inspect`).
- **Dynamic Control Flow**: Classical conditionals and loops remain intentionally deferred to later milestones in accordance with the fail-closed subset policy.

---

## 8. Approval & Ledger Update

- [x] Exit gate satisfied.
- [x] Audit complete and verified.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
- [x] Next Phase: **Phase 27 — QASM fuzzing, conformance and frontend CLI**.
