# Phase 32 Completion Report — One-Qubit Gate Fusion

## 1. Executive Summary

Phase 32 (**One-qubit gate fusion**) of the QCC compiler engineering roadmap has been fully implemented, rigorously verified, and integrated into the logical optimizer subsystem.

This phase introduces:
1. **Sequence Replacement Rewriting** (`src/ir/rewrite/replace.rs` and `src/ir/rewrite/mod.rs`): `replace_sequence` and `CircuitTransaction::replace_sequence` for atomic $M$-to-$N$ operation replacements ($N \in \{0, 1, 2, 3\}$) with formal DAG convexity validation via `fuse::check_convexity`, automatic wire dependency reconnection, and complete parent provenance tracking.
2. **One-Qubit Gate Fusion Pass** (`src/optimizer/passes/one_qubit_fusion.rs`): Implements the `CompilerPass` trait, discovering maximal consecutive single-qubit unitary operations on each quantum wire, multiplying local matrices in left-multiplication execution order ($M = U_k \dots U_1$), decomposing into canonical Euler angles via Phase 31 (`decompose_euler`), extracting and preserving circuit-level scalar global phase, simplifying identity rotations, and replacing sequences atomically without crossing side-effect or hazard boundaries.
3. **Module Registration & Re-Exports** (`src/optimizer/passes/mod.rs`, `src/optimizer/mod.rs`, `src/lib.rs`): Re-exports `OneQubitFusionPass`, `OneQubitFusionConfig`, `OneQubitFusionStats`, and `DEFAULT_FUSION_TOLERANCE`.
4. **Integration Test Suite** (`tests/optimizer/fusion.rs` registered in `tests/optimizer.rs` and `tests/ir_rewrites.rs`): 26 integration tests in `fusion.rs` plus 3 sequence replacement tests in `ir_rewrites.rs` covering identity cancellation, discrete gate fusion, continuous rotation merging, matrix multiplication order sensitivity, global phase accumulation, boundary non-crossing, provenance tracking, and the explicit Exit Gate (50 randomized fused blocks verified by Phase 10 common-phase equivalence checker).

The complete test suite comprises **785 passing tests** across all 27 binaries (23 active test targets) with zero errors, zero warnings, and zero regressions.

---

## 2. Invariant & Mathematical Contract Compliance

1. **Column-Vector Left-Multiplication Order**:
   - In QCC, quantum operations act on column statevectors by left multiplication ($B \cdot A$).
   - For sequential operations $N_1, N_2, \dots, N_k$ on wire $q$, the composite matrix is computed incrementally as:
     $$M \leftarrow U_i \cdot M \quad (i = 1 \dots k, \quad M_{\text{init}} = I)$$
     resulting in $M = U_k \cdot U_{k-1} \cdot \dots \cdot U_1$.
   - Verified by `test_fusion_matrix_multiplication_order_sensitivity`, proving that fusing $X \to H$ produces $H \cdot X \neq X \cdot H$ and is strictly not equivalent to $H \to X$.

2. **Explicit Circuit Global Phase Preservation**:
   - $U_{\text{circuit}} = e^{i\phi} U_{\text{operations}}$.
   - Factoring $M = e^{i\alpha} V$ yields scalar global phase $\alpha$, which is accumulated into the circuit's global phase via `tx.add_global_phase(norm_phase)`.
   - Verified by `test_fusion_global_phase_accumulation_single_wire` and `test_fusion_global_phase_accumulation_multi_wire`. All Phase 10 unitary equivalence checks (`check_equivalence`) pass against reference circuits using one common global phase.

3. **Wire Adjacency Without Artificial Reordering**:
   - Fusion windows are strictly formed along direct dependency edges on `WireId::Quantum(q)`.
   - Operations are never reordered across other operations to synthesize windows.
   - Any barrier (`Barrier`), measurement (`Measure`), reset (`Reset`), classical hazard (`Classical`), or multi-qubit gate (`CX`, `CZ`, `SWAP`) strictly terminates the fusion window.
   - Operations on disjoint wires commute in the DAG and fuse independently without cross-wire interference (`test_fusion_disjoint_wires_independent`).

4. **DAG Convexity & Provenance Tracking**:
   - Any contiguous subsegment of single-qubit gates on a single wire is strictly convex in the DAG because each node only touches `WireId::Quantum(q)`.
   - `stage_replace_sequence` validates convexity using `fuse::check_convexity` before staging any mutation.
   - Every resynthesized operation records the complete slice of original operation IDs in its `node.provenance`.

5. **Fixed-Point Convergence & Cycle Prevention**:
   - If a sequence of operations is already in canonical Euler form with identical parameters, resynthesis is a no-op; the pass skips replacement, increments `windows_rejected`, and terminates cleanly in minimal iterations (`test_fusion_canonical_zyz_idempotent`).

---

## 3. Detailed Deliverables & File Changes

| File | Status | Description |
|---|:---:|---|
| `src/ir/rewrite/replace.rs` | Modified | Added `replace_sequence` and `stage_replace_sequence` for $M$-to-$N$ atomic replacements with DAG convexity check and provenance tracking. |
| `src/ir/rewrite/mod.rs` | Modified | Re-exported `replace_sequence` and added `CircuitTransaction::replace_sequence`. |
| `src/optimizer/passes/one_qubit_fusion.rs` | Created | Implemented `OneQubitFusionPass`, `OneQubitFusionConfig`, `OneQubitFusionStats`, and `DEFAULT_FUSION_TOLERANCE`. |
| `src/optimizer/passes/mod.rs` | Modified | Re-exported `one_qubit_fusion` module, pass, config, and stats. |
| `src/lib.rs` | Modified | Re-exported `OneQubitFusionPass`, `OneQubitFusionConfig`, `OneQubitFusionStats`, and `DEFAULT_FUSION_TOLERANCE`. |
| `tests/optimizer.rs` | Modified | Registered `mod fusion;`. |
| `tests/optimizer/fusion.rs` | Created | Comprehensive integration test suite (26 tests) for single-qubit gate fusion and Exit Gate proof. |
| `tests/ir_rewrites.rs` | Modified | Added 3 integration tests for `replace_sequence` ($M$-to-$N$, $M$-to-0, and non-convex rejection). |
| `docs/project-state/plans/phase-032.md` | Created | Implementation plan for Phase 32. |
| `docs/project-state/reports/phase-032.md` | Created | Phase completion report with test evidence. |
| `docs/project-state/progress.md` | Modified | Updated progress ledger (Phase 32 completed, M3 progress 5/9, 785 tests). |

---

## 4. Test Inventory & Arithmetic Reconciliation

- **Phase 31 Accepted Baseline**: **756 tests** across 23 active test targets (27 binaries).
- **Phase 32 Additions**:
  - `tests/optimizer.rs`: 113 $\to$ 139 (**+26 tests** in `tests/optimizer/fusion.rs`)
  - `tests/ir_rewrites.rs`: 21 $\to$ 24 (**+3 tests** for `replace_sequence`)
  - All other 21 active targets remain strictly unchanged.
- **Phase 32 Total Test Count**: $756 + 26 + 3 = \mathbf{785\text{ tests}}$.

### Target-by-Target Inventory (27 Binaries, 23 Active Targets)

| # | Binary Target | Path | Test Count | Status |
|:---:|---|---|:---:|:---:|
| 1 | Library unittests | `src/lib.rs` | 75 | Passed |
| 2 | Lexer fuzz target | `fuzz/fuzz_targets/fuzz_lexer.rs` | 0 | Passed (entrypoint) |
| 3 | Parser fuzz target | `fuzz/fuzz_targets/fuzz_parser.rs` | 0 | Passed (entrypoint) |
| 4 | Main binary | `src/main.rs` | 0 | Passed (entrypoint) |
| 5 | Integration: AST | `tests/ast.rs` | 30 | Passed |
| 6 | Integration: Builder | `tests/builder.rs` | 10 | Passed |
| 7 | Integration: CLI | `tests/cli.rs` | 20 | Passed |
| 8 | Integration: End-to-end | `tests/end_to_end.rs` | 9 | Passed |
| 9 | Integration: Equivalence | `tests/equivalence.rs` | 23 | Passed |
| 10 | Integration: Euler | `tests/euler.rs` | 25 | Passed |
| 11 | Integration: Frontend Adapter | `tests/frontend_adapter.rs` | 14 | Passed |
| 12 | Integration: Gate Identities | `tests/gate_identities.rs` | 23 | Passed |
| 13 | Integration: IR | `tests/ir.rs` | 19 | Passed |
| 14 | Integration: IR Rewrites | `tests/ir_rewrites.rs` | 24 | Passed (+3) |
| 15 | Integration: Lexer | `tests/lexer.rs` | 29 | Passed |
| 16 | Integration: Measurement | `tests/measurement.rs` | 11 | Passed |
| 17 | Integration: OpenQASM Emitter | `tests/openqasm_emitter.rs` | 21 | Passed |
| 18 | Integration: Optimizer | `tests/optimizer.rs` | 139 | Passed (+26) |
| 19 | Integration: Pass Manager | `tests/pass_manager.rs` | 14 | Passed |
| 20 | Integration: QASM 3 | `tests/qasm3.rs` | 210 | Passed |
| 21 | Integration: Quantum Semantics | `tests/quantum_semantics.rs` | 23 | Passed |
| 22 | Integration: Sampling | `tests/sampling.rs` | 9 | Passed |
| 23 | Integration: Simulator DAG | `tests/simulator_dag.rs` | 15 | Passed |
| 24 | Integration: Simulator Single-Qubit | `tests/simulator_single_qubit.rs` | 19 | Passed |
| 25 | Integration: Simulator Two-Qubit | `tests/simulator_two_qubit.rs` | 18 | Passed |
| 26 | Integration: Smoke | `tests/smoke.rs` | 5 | Passed |
| 27 | Doc-tests | `src/lib.rs` | 0 | Passed |
| **Total** | | | **785** | **Zero errors, zero warnings** |

---

## 5. Verification Commands & Exit Gate Proof

All verification protocol commands passed with zero errors and zero warnings:

1. **Formatting**:
   ```bash
   cargo fmt --check
   ```
   *Result*: Clean (code 0).

2. **Linting**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
   *Result*: Clean, zero warnings (code 0).

3. **Automated Test Suite**:
   ```bash
   cargo test --all
   ```
   *Result*: 785 passed across all binaries, 0 failed, 0 ignored (code 0).

4. **Target Compilation**:
   ```bash
   cargo build --all-targets
   ```
   *Result*: Successful (code 0).

5. **Documentation**:
   ```bash
   cargo doc --no-deps
   ```
   *Result*: Clean build of rustdoc documentation (code 0).

6. **Examples**:
   ```bash
   cargo run --example bell && cargo run --example qasm
   ```
   *Result*: Both examples executed and completed successfully (code 0).

7. **Independent Reference Parser Validation**:
   ```bash
   PYTHONPATH=/tmp/py_packages python3 -c 'import openqasm3; print(openqasm3.parse(open("examples/qasm/bell.qasm").read()))'
   ```
   *Result*: AST parsed successfully without error (code 0).

8. **Explicit Exit Gate Proof**:
   > **EXIT GATE**: Random fused blocks pass common-phase equivalence.

   Verified by `test_random_one_qubit_blocks_exit_gate` in `tests/optimizer/fusion.rs`: 50 randomized single-qubit blocks (lengths 2 to 8, random selections from $\{X, Y, Z, H, S, S^\dagger, T, R_x(\theta), R_y(\theta), R_z(\theta)\}$) were synthesized, fused via `OneQubitFusionPass`, and verified with Phase 10 `check_equivalence`. All 50 trials passed common-phase equivalence with maximum amplitude difference bounded by machine epsilon ($\le 10^{-14}$).
