# Phase Completion Report: Phase 30 — Same-Axis Rotation Merging

## 1. Summary
- **Phase Number**: Phase 30
- **Exact Phase Title**: Same-axis rotation merging
- **Milestone**: Milestone M3: Verified Logical Optimizer (`M3`) — **Third Phase of Milestone M3**
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity (Advanced Agentic Coding)
- **Status**: **Completed & Audited**
- **Exit Gate Satisfied**: **Yes** — Rotation merging passes randomized unitary-equivalence checks, with unresolved symbolic cases left unchanged (`113/113` tests in `tests/optimizer.rs`, `731/731` workspace tests across all test targets, zero warnings, zero errors).

---

## 2. Deliverables & Files Changed

### Files Created
- `src/quantum/angle.rs`:
  - Dedicated continuous rotation angle normalization, periodicity, and global-phase convention module.
  - `normalize_rotation_angle`: Normalizes continuous rotation angles under $4\pi$ operator double-cover periodicity and tracks $2\pi$ global-phase shifts.
  - `RotationNormalizedAngle`: Structured representation holding the normalized angle in radians, scalar global-phase correction (0.0 or $\pi$), and identity classification (`ExactZero`, `PhaseZero`, `NotIdentity`).
  - `RotationNormalizationMode`: `TwoPiWithPhase` (canonical $[0, 2\pi)$ with $+\pi$ phase adjustment on $2\pi$ shift) and `FourPiExact` (exact $[0, 4\pi)$ with $0$ phase for non-identities).
  - Constants: `TWO_PI` ($2\pi$), `FOUR_PI` ($4\pi$).
- `src/optimizer/passes/rotation_merge.rs`:
  - `RotationMergePass`: Transactional compiler optimization pass combining adjacent same-axis single-qubit rotations ($R_x$, $R_y$, $R_z$).
  - `RotationAxis`: Strongly typed enum for spatial rotation axes (`X`, `Y`, `Z`).
  - `RotationMergeConfig`: Pass configuration specifying angular tolerance (default `1e-12`), maximum fixed-point iterations (default `1000`), identity elimination options, normalization mode, symbolic merge policy, and runtime symbol environment.
  - `RotationMergeStats`: Telemetry statistics tracking examined pairs, merged rotations, fused non-zero rotations, exact zero identities eliminated, phase zero identities eliminated, symbolic pairs merged, rejected candidate pairs, removed and added operations, global phase adjustments, and iterations.
  - `SymbolicMergePolicy`: `PreserveUnresolved` (leaves unbound symbolic parameters unchanged as separate gates) and `CombineSymbolic` (combines expressions algebraically into `ParameterExpr::Add`).
  - `extract_rotation`: Inspects operation and extracts axis, qubit ID, and parameter expression.
  - `find_adjacent_wire_partner`: Locates unique direct wire successor along `WireId::Quantum(q)` in the DAG.
  - `are_algebraic_opposites`: Checks for symbolic opposites ($p$ and $-p$, including nested negations) that cancel to zero.
  - `contains_division_by_zero`: Rejects invalid symbolic expressions containing division by zero, failing closed.
- `tests/optimizer/rotation_merge.rs`:
  - Comprehensive integration test suite (34 unit/integration tests + 5 property-based tests = 39 tests, including randomized circuit equivalence proof over 30 trials).
- `docs/project-state/plans/phase-030.md`: Implementation plan for Phase 30.
- `docs/project-state/reports/phase-030.md`: Phase 30 completion and audit report.

### Existing Files Modified
- `src/quantum/mod.rs`:
  - Declared `angle` module and re-exported `normalize_rotation_angle`, `RotationNormalizationMode`, `RotationNormalizedAngle`, `FOUR_PI`, `TWO_PI`.
- `src/ir/operation.rs`:
  - Added `pub fn parameters(&self) -> &[ParameterExpr]` helper method to `Operation`.
- `src/optimizer/passes/mod.rs`:
  - Declared `rotation_merge` module and re-exported `RotationAxis`, `RotationMergeConfig`, `RotationMergePass`, `RotationMergeStats`, `SymbolicMergePolicy`, `DEFAULT_ROTATION_TOLERANCE`.
- `src/lib.rs`:
  - Re-exported all rotation merge types and angle normalization helpers from crate root.
- `tests/optimizer.rs`:
  - Registered `rotation_merge` test module.
- `docs/project-state/progress.md`:
  - Updated Phase 30 to Completed (`30 / 105` phases complete, Milestone M3 progress `3 / 9`).

---

## 3. Mathematical & Architectural Invariant Verification

### 3.1 Same-Axis Rotation Composition & Axis Segregation
- **Composition Rule**:
  - For continuous single-qubit rotations along axis $\hat{n} \in \{X, Y, Z\}$:
    $$R_{\hat{n}}(\theta_2) \cdot R_{\hat{n}}(\theta_1) = R_{\hat{n}}(\theta_1 + \theta_2)$$
  - Validated by tests: `test_numeric_rx_simple_merge`, `test_numeric_ry_simple_merge`, `test_numeric_rz_simple_merge`, `test_chain_of_three_rotations_merges_to_single`.
- **Different Axes Do Not Merge**:
  - Because non-identical Pauli matrices do not commute ($[X, Y] \neq 0$, etc.), rotations along different axes ($R_x$ followed by $R_z$) are rejected from candidate pair formation and left untouched (`test_different_rotation_axes_do_not_merge`).

### 3.2 Periodicity Conventions & Global Phase Tracking
- **$4\pi$ Identity Periodicity**:
  - $R_{\hat{n}}(\theta + 4\pi) = R_{\hat{n}}(\theta)$ is an exact matrix equality.
  - When $\theta_1 + \theta_2 \equiv 0 \pmod{4\pi}$, the rotation is an exact identity ($I$) and is eliminated with zero global-phase adjustment (`test_four_pi_periodicity_exact_zero_eliminated`, `test_negative_four_pi_exact_zero`, `test_opposite_angles_cancel_to_exact_zero`, `prop_rotation_angle_four_pi_exact`).
- **$2\pi$ Sign Inversion (Global Phase $\pi$)**:
  - Shifting by an odd multiple of $2\pi$ introduces a factor of $-I = e^{i\pi} I$:
    $$R_{\hat{n}}(\theta + 2\pi) = -R_{\hat{n}}(\theta) = e^{i\pi} R_{\hat{n}}(\theta)$$
  - When $\theta_1 + \theta_2 \equiv 2\pi \pmod{4\pi}$, the rotation evaluates to $-I$ and is eliminated with a $+\pi$ global-phase adjustment added to the circuit (`test_two_pi_periodicity_phase_zero_eliminated_with_phase`, `test_negative_two_pi_phase_zero`, `prop_rotation_angle_two_pi_phase`).
  - When a non-zero rotation is reduced by $2\pi$ into $[0, 2\pi)$, $+\pi$ is added to the circuit global phase, ensuring exact statevector amplitude and unitary equivalence (`test_nonzero_rotation_normalized_with_two_pi_shift_adds_phase`, `test_negative_angles_merge_correctly`).
- **Phase Accumulation & Boundaries**:
  - Multiple phase corrections accumulate additively modulo $2\pi$ (`test_multiple_phase_corrections_accumulate_correctly`, `test_multiple_qubits_phase_corrections_accumulate`).
  - Angular tolerance ($10^{-12}$) correctly discriminates tiny numerical inaccuracies ($10^{-14}$) as identity while strictly preserving meaningfully non-zero rotations ($10^{-6}$) (`test_tolerance_does_not_classify_meaningful_nonzero_as_identity`, `test_values_around_normalization_boundaries`).
  - Full unitary equivalence verified via Phase 10 equivalence checker across all randomized multi-qubit test states.

### 3.3 Symbolic Parameter Contracts
- **Preserve Unresolved Symbols**:
  - Under `SymbolicMergePolicy::PreserveUnresolved` (default), unbound symbolic parameters are never assumed to be zero or constants; they remain unchanged as separate gates (`test_unresolved_symbolic_parameters_remain_unchanged_under_preserve_policy`).
  - Unbound symbolic expressions are never evaluated against numerical tolerance, preventing accidental truncation or elimination (`test_symbolic_expression_not_silently_discarded_by_tolerance`).
- **Algebraic Opposites Cancellation**:
  - Unbound symbolic opposites ($p$ and $-p$, or $-p$ and $p$, including simplified forms like $-(-(-x))$) algebraically cancel to exact zero ($0.0$) and eliminate safely without needing numerical bindings (`test_symbolic_algebraic_opposites_cancel_to_identity`, `test_exact_symbolic_opposites_with_simplification`).
- **Invalid / Ill-Defined Symbolic Expressions**:
  - Expressions with ill-defined arithmetic (such as division by zero `x / 0.0`) are detected and rejected from merging, failing closed and leaving the operations intact (`test_unsupported_division_by_zero_symbolic_remains_unchanged`).
- **Symbol Environment Evaluation**:
  - When symbols are bound in `symbol_env`, they evaluate cleanly to concrete floats, merge, normalize, and eliminate if identity (`test_bound_symbol_env_evaluates_and_merges`).
- **Expression Combination**:
  - Under `SymbolicMergePolicy::CombineSymbolic`, expressions are combined using `ParameterExpr::Add` while preserving variable identities intact (`test_symbolic_combine_under_combine_policy`).

### 3.4 Wire Adjacency, Hazard Boundaries & DAG Invariants
- **Direct Wire Adjacency**:
  - Candidate detection operates strictly on direct outgoing edges on typed wire `WireId::Quantum(q)`.
- **Intervening Gates Block Merge**:
  - Any intervening single-qubit gate on the wire ($R_x \to H \to R_x$) blocks merging (`test_intervening_single_qubit_gate_blocks_merge`).
  - Any intervening multi-qubit gate ($R_z \to CX \to R_z$) blocks merging (`test_intervening_two_qubit_gate_blocks_merge`).
- **Preservation of Non-Unitary and Synchronization Boundaries**:
  - `Barrier` strictly prevents merging on synchronized wires (`test_barrier_blocks_merge`).
  - `Measure` and `Reset` operations strictly prevent merging across their boundary (`test_measurement_blocks_merge`, `test_reset_blocks_merge`).
- **Disjoint Wire Independence**:
  - Operations on independent wires commute in the DAG and do not block wire-adjacent rotations on independent wires ($R_x(0.3) \to H(1) \to R_x(0.4) \implies R_x(0.7) \text{ and } H(1)$, verified by `test_disjoint_wire_operations_do_not_block_merge`).
- **Provenance & Transactional Safety**:
  - Fused operations record original operation IDs in provenance (`test_provenance_and_id_stability`).
  - Unmerged operations retain their stable `OperationId`s.
  - Aborted transactions roll back completely, preserving the original circuit (`test_transactional_rollback_preserves_circuit`).
- **Pass Manager & Telemetry**:
  - Integrates with `PassManager`, producing accurate execution telemetry (`test_pass_manager_pipeline_integration`, `test_telemetry_statistics_reporting`).

---

## 4. Test Verification Breakdown & Reconciliation

### 4.1 Reconciled Test Inventory (Actual Active Binaries)

The QCC repository compiles 26 test target binaries: 22 active test targets containing tests, and 4 zero-test binaries (`src/main.rs`, `fuzz_lexer.rs`, `fuzz_parser.rs`, and rustdoc tests). All 22 active binaries pass with zero errors and zero warnings:

| # | Test Target Binary | Passing Tests | Status | Target Description |
| :-: | :--- | :---: | :---: | :--- |
| 1 | `src/lib.rs` (unit tests) | 75 | Passed | Crate unit tests (+3 in Phase 30: `angle.rs`) |
| 2 | `tests/ast.rs` | 30 | Passed | AST nodes and visitor tests |
| 3 | `tests/builder.rs` | 10 | Passed | Circuit builder API tests |
| 4 | `tests/cli.rs` | 20 | Passed | CLI commands and interface tests |
| 5 | `tests/end_to_end.rs` | 9 | Passed | End-to-end compiler pipeline tests |
| 6 | `tests/equivalence.rs` | 23 | Passed | Unitary equivalence checker tests |
| 7 | `tests/frontend_adapter.rs` | 14 | Passed | Frontend adapter boundary tests |
| 8 | `tests/gate_identities.rs` | 23 | Passed | Gate algebra and identity tests |
| 9 | `tests/ir.rs` | 19 | Passed | QccIR DAG structure and wire tests |
| 10 | `tests/ir_rewrites.rs` | 21 | Passed | Transactional DAG rewrites tests |
| 11 | `tests/lexer.rs` | 29 | Passed | QASM source buffer and lexer tests |
| 12 | `tests/measurement.rs` | 11 | Passed | Non-unitary measurement tests |
| 13 | `tests/openqasm_emitter.rs` | 21 | Passed | OpenQASM 3 emitter tests |
| 14 | **`tests/optimizer.rs`** | **113** | **Passed** | Optimizer passes (P28: 35, P29: 39, P30: 39) |
| 15 | `tests/pass_manager.rs` | 14 | Passed | Pass manager pipeline tests |
| 16 | `tests/qasm3.rs` | 210 | Passed | Parser, sema, lowering, conformance, fuzz |
| 17 | `tests/quantum_semantics.rs` | 23 | Passed | Dirac semantics and little-endian tests |
| 18 | `tests/sampling.rs` | 9 | Passed | Deterministic seeded sampling tests |
| 19 | `tests/simulator_dag.rs` | 15 | Passed | DAG-driven simulator execution tests |
| 20 | `tests/simulator_single_qubit.rs` | 19 | Passed | Single-qubit simulator kernel tests |
| 21 | `tests/simulator_two_qubit.rs` | 18 | Passed | Two-qubit simulator kernel tests |
| 22 | `tests/smoke.rs` | 5 | Passed | Smoke and golden diagnostics tests |
| 23 | `src/main.rs` | 0 | Passed | CLI entrypoint binary (0 tests) |
| 24 | `fuzz/fuzz_targets/fuzz_lexer.rs` | 0 | Passed | Fuzz target binary (0 unit tests) |
| 25 | `fuzz/fuzz_targets/fuzz_parser.rs` | 0 | Passed | Fuzz target binary (0 unit tests) |
| 26 | Doc-tests `qcc` | 0 | Passed | Cargo doc-test runner (0 tests) |
| — | **Total Workspace Passing Tests** | **731** | **0 failed, 0 ignored** | **Exact sum of all 22 active targets** |

### 4.2 Exact Arithmetic Reconciliation

1. **Phase 29 Baseline (`bcfdc55`)**:
   - `src/lib.rs` (unit tests): 72 tests
   - Integration tests in `tests/`: 617 tests across 21 test files (including 74 tests in `tests/optimizer.rs`: 35 from Phase 28 + 39 from Phase 29)
   - Phase 29 Baseline Total: $72 + 617 = \mathbf{689}$ tests.

2. **Phase 30 Initial Implementation (`bba0715`)**:
   - `src/quantum/angle.rs` added 3 new unit tests to `src/lib.rs`: $72 \to \mathbf{75}$ tests ($+3$).
   - `tests/optimizer/rotation_merge.rs` added 32 new tests to `tests/optimizer.rs`: $74 \to \mathbf{106}$ tests ($+32$).
   - All other 20 integration test files remained unchanged at 543 tests.
   - Initial Phase 30 Workspace Total: $75 + 543 + 106 = \mathbf{724}$ tests ($689 + 3 + 32 = 724$).

3. **Origin of the Erroneous "734" Count**:
   - In an earlier documentation draft, an outdated template table containing placeholder/fictitious test file names (e.g., `ancilla_tests.rs`, `circuit_builder_tests.rs`, `dag_tests.rs`, `routing_tests.rs`) was copied over.
   - When updating that draft table, `src/lib.rs` was set to 75 and `tests/optimizer.rs` was set to 106, while the remaining rows in that fictitious template happened to sum to 553, producing an incorrect total of 734.
   - The real filesystem contains only the 22 active targets listed above; no tests were moved, renamed, or deleted between Phase 29 and Phase 30.

4. **Phase 30 Audit Additions**:
   - Added 7 new regression, boundary, and symbolic tests to `tests/optimizer/rotation_merge.rs`:
     1. `test_multiple_phase_corrections_accumulate_correctly`: verifies sequential phase accumulation ($+\pi + \pi = 2\pi \equiv 0$).
     2. `test_multiple_qubits_phase_corrections_accumulate`: verifies multi-qubit concurrent phase accumulation ($+\pi + \pi = 2\pi$).
     3. `test_tolerance_does_not_classify_meaningful_nonzero_as_identity`: verifies $10^{-6}$ is preserved while $10^{-14}$ is eliminated under $10^{-12}$ tolerance.
     4. `test_values_around_normalization_boundaries`: verifies normalization at $4\pi \pm \epsilon$ and $2\pi \pm \epsilon$.
     5. `test_unsupported_division_by_zero_symbolic_remains_unchanged`: verifies expressions with division by zero fail closed.
     6. `test_symbolic_expression_not_silently_discarded_by_tolerance`: verifies unbound symbols are never discarded by numerical tolerance.
     7. `test_exact_symbolic_opposites_with_simplification`: verifies nested symbolic opposites like `-(-(-x))` cancel to zero with `x`.
   - `tests/optimizer.rs` increased from 106 to **113** tests ($+7$).
   - Total workspace tests increased from 724 to **731** tests ($75 + 543 + 113 = 731$).

---

## 5. Verification Commands & Exit Gate Evidence
1. `cargo fmt --check`: Passed with zero diffs.
2. `cargo clippy --all-targets -- -D warnings`: Passed with zero warnings.
3. `cargo test --all`: All 731 tests passed across all 26 binaries (22 active targets).
4. `cargo build --all-targets`: Built cleanly in debug profile.
5. `cargo doc --no-deps`: Documentation generated cleanly.
6. `cargo run --example bell`: Bell state preparation and sampling executed with 100% expected outcome distribution.
7. `cargo run --example qasm`: OpenQASM 3 output generated cleanly.
8. Independent reference parser (`openqasm3[parser]==1.0.1`): Validated emitted `examples/qasm/bell.qasm` with 7 AST statements.
9. **Exit Gate Proven**:
   - `test_randomized_circuits_with_injected_rotations_pass_unitary_equivalence`: 30 randomized multi-qubit trials with injected rotation pairs verified against Phase 10 `check_equivalence`.
   - `test_unresolved_symbolic_parameters_remain_unchanged_under_preserve_policy`: Unresolved symbolic cases left unchanged.
   - Zero tolerance leakage on unbound expressions.
