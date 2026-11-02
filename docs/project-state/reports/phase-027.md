# Phase Completion Report: Phase 27 — QASM Fuzzing, Conformance and Frontend CLI

## 1. Summary
- **Phase Number**: Phase 27
- **Exact Phase Title**: QASM fuzzing, conformance and frontend CLI
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`) — **Final Phase of Milestone M2**
- **Completion Date**: 2026-11-02
- **Engineer / Agent Session**: Antigravity (Advanced Agentic Coding)
- **Status**: **Completed**
- **Exit Gate Satisfied**: **Yes** — The frontend conformance suite and CLI smoke tests pass (`210/210` tests in `tests/qasm3.rs`, `20/20` tests in `tests/cli.rs`, `72/72` unit tests in `src/lib.rs`, `615/615` workspace tests across all 24 test targets, zero warnings, zero errors).

---

## 2. Deliverables & Files Changed

### New Files Created
- `fuzz/fuzz_targets/fuzz_lexer.rs`: Standalone lexer fuzz target with built-in smoke fuzzer (1000 iterations + 14 synthetic edge cases) and libFuzzer compatibility entry points.
- `fuzz/fuzz_targets/fuzz_parser.rs`: Full frontend pipeline fuzz target (lexer, recovery parser, semantic analyzer, lowerer) with built-in smoke fuzzer (1000 iterations + 19 synthetic edge cases).
- `fuzz/corpus/lexer/`: Regression corpus for lexical scanning edge cases (`valid_tokens.qasm`, `long_literal.qasm`, `unclosed_string.qasm`, `huge_whitespace.qasm`).
- `fuzz/corpus/parser/`: Regression corpus for parsing edge cases (`deep_nesting.qasm`, `huge_operands.qasm`, `division_by_zero.qasm`, `unsupported_constructs.qasm`, `malformed_syntax.qasm`, `bell_valid.qasm`).
- `fuzz/Cargo.toml`: Package configuration for continuous fuzzing integration (`cargo-fuzz`).
- `tests/qasm3/conformance/fixtures/supported/`: 7 positive conformance fixtures covering the full supported OpenQASM 3 subset (`bell`, `rotations`, `all_canonical_gates`, `multi_register`, `non_unitary`, `global_phase`, `teleportation`).
- `tests/qasm3/conformance/fixtures/unsupported/`: 13 fail-closed negative conformance fixtures covering Phase 24 unsupported constructs (`if_else`, `for_loop`, `while_loop`, `subroutine`, `gate_def`, `defcal`, `cal_block`, `three_qubit_ccx`, `three_qubit_cswap`, `standard_u3`, `standard_p`, `types`, `legacy_qasm2`).
- `tests/qasm3/conformance/fixtures/invalid/`: 13 syntax and semantic error fixtures (`out_of_bounds`, `undeclared_reg`, `duplicate_reg`, `zero_sized_reg`, `division_by_zero`, `unresolved_param`, `unknown_gate`, `duplicate_operands`, `param_arity`, `qubit_arity`, `deep_nesting`, `oversized_reg`, `type_mismatch`, `measurement_size`).
- `tests/qasm3/conformance/mod.rs`: Comprehensive conformance test suite (34 automated integration tests).
- `tests/qasm3/fuzz_regression.rs`: Fuzz regression test suite (6 automated integration tests) exercising the corpus and hard bounded limits.
- `src/cli/mod.rs`: Top-level CLI dispatcher, argument parsing, help formatting, diagnostic exit-code categorization, and stable exit-code definitions (`0`, `1`, `2`, `3`).
- `src/cli/compile.rs`: Implementation of `qcc compile` with options (`--output`, `--format`, `--global-phase`, `--measure-syntax`, `--bind`).
- `src/cli/inspect.rs`: Implementation of `qcc inspect` with options (`--summary`, `--ast`, `--ir`, `--registers`, `--bind`).
- `tests/cli.rs`: CLI integration test suite (20 automated tests) exercising all commands, view modes, file/stdin I/O, error codes, and rendered diagnostics.
- `docs/qasm-conformance.md`: Authoritative specification of the OpenQASM 3 conformance suite, supported matrix, and fail-closed rules.
- `docs/qasm-fuzzing.md`: Authoritative specification of fuzzing architecture, resource limits, smoke runners, and corpus management.
- `docs/cli-reference.md`: Authoritative user guide for the QCC CLI, subcommand options, exit codes, and diagnostic rendering.
- `docs/project-state/plans/phase-027.md`: Implementation plan for Phase 27.

### Existing Files Modified
- `src/diagnostics/qasm.rs`: Added comprehensive unit test module `test_diagnostic_code_stability_constants` validating that all Phase 23 diagnostic codes (`E0001`–`E0040`) are constant and immutable.
- `src/frontend/parser/expressions.rs`: Implemented `MAX_EXPRESSION_DEPTH = 128` to bound recursion depth and prevent stack overflow on deeply nested expressions.
- `src/frontend/parser/operands.rs`: Implemented `MAX_OPERANDS_PER_STATEMENT = 10,000` to bound memory allocation on huge operand lists.
- `src/frontend/parser/gates.rs`: Implemented `MAX_PARAMETERS_PER_GATE = 128` to bound gate parameter allocations.
- `src/frontend/parser/declarations.rs`: Implemented `MAX_REGISTER_SIZE = 1,000,000` to bound register declarations.
- `src/frontend/lowering/registers.rs`: Enforced `MAX_REGISTER_SIZE` check in register lowering.
- `src/lib.rs`: Exposed `pub mod cli;`.
- `src/main.rs`: Wired `qcc::cli::run(&args)` to return deterministic exit codes.
- `Cargo.toml`: Registered `qcc` binary and `fuzz_lexer` / `fuzz_parser` fuzzing binaries.
- `tests/qasm3.rs`: Registered `conformance` and `fuzz_regression` test modules.
- `docs/project-state/progress.md`: Updated Phase 27 status and Milestone M2 completion ledger.

---

## 3. Audit & Contract Reconciliation

### 3.1 Register Diagnostic Codes (`E0010`–`E0016` Stability)
An audit comparing Phase 23 diagnostic contracts with Phase 27 implementations confirms that **no register diagnostic code meanings were altered in code**:
- `E0010`: `QASM_DUPLICATE_DECLARATION` — Duplicate register declaration.
- `E0011`: `QASM_INVALID_REGISTER_SIZE` — Zero-size or oversized register declaration.
- `E0012`: `QASM_UNDECLARED_REGISTER` — Undeclared register reference.
- `E0013`: `QASM_REGISTER_TYPE_MISMATCH` — Register kind mismatch (e.g. classical bit where qubit expected, or quantum bit used as measurement destination).
- `E0014`: `QASM_INDEX_OUT_OF_BOUNDS` — Register index $\ge size$.
- `E0015`: `QASM_INDEXED_SCALAR` — Indexing scalar register.
- `E0016`: `QASM_UNINDEXED_ARRAY` — Unindexed array where scalar required.

*Reconciliation Findings*:
1. The codebase implementation (`src/diagnostics/qasm.rs`, `src/frontend/semantics/`, and test assertions) has continuously maintained these exact mappings.
2. Unit tests in `src/diagnostics/qasm.rs` (`test_diagnostic_code_stability_constants`) and integration tests in `tests/qasm3/conformance/mod.rs` and `tests/cli.rs` now explicitly assert every single code to prevent regressions.
3. In `src/cli/mod.rs`, `determine_diagnostic_exit_code` was reconciled so that only genuine unsupported feature codes (`E0002`, `E0003`, `E0004`, `E0005`) return `CliExitCode::UnsupportedError` (exit code `3`). All compilation and semantic errors (including `E0010`–`E0016`, `E0020`–`E0026`, `E0030`–`E0032`) deterministically return `CliExitCode::CompileError` (exit code `2`).

### 3.2 Operation and Gate Diagnostic Codes (`E0020`–`E0025` Stability)
The Phase 23 diagnostic contract established:
- `E0020`: `QASM_UNKNOWN_GATE` — Unknown or unrecognized gate identifier (e.g. `foo q[0];` or case mismatch `H q[0];`).
- `E0021`: `QASM_GATE_PARAM_ARITY` — Gate parameter count mismatch (e.g. `rx(0.1, 0.2) q[0];`).
- `E0022`: `QASM_GATE_QUBIT_ARITY` — Gate qubit operand count mismatch (e.g. `cx q[0];`).
- `E0023`: `QASM_DUPLICATE_QUBIT_OPERAND` — Duplicate qubit operand in a multi-qubit gate (e.g. `cx q[0], q[0];`).
- `E0024`: `QASM_INVALID_PARAM_DOMAIN` — Gate-specific parameter domain violation (reserved for gates with bounded parameter domains).
- `E0025`: `QASM_MEASUREMENT_SIZE_MISMATCH` — Whole-register measurement size mismatch (e.g. `c = measure q;` where sizes differ).

*Reconciliation Verification*:
- Inspected all emission sites across parser (`src/frontend/parser/gates.rs:243, 331, 346, 360, 399`, `src/frontend/parser/registers.rs:356`), semantic analyzer (`src/frontend/semantic/checks.rs:90, 112, 130, 197, 227`), and lowerer (`src/frontend/lowering/operations.rs:227, 236, 263, 272, 289, 298, 309, 399`, `src/frontend/lowering/registers.rs:213, 279`).
- Confirmed all emission sites match the Phase 23 contract exactly.
- Added `tests/qasm3/conformance/fixtures/invalid/invalid_unknown_gate.qasm` and integration test `test_conformance_invalid_unknown_gate` (`E0020`).
- Added `test_conformance_invalid_param_domain_diagnostic_helper` (`E0024`).
- Updated `tests/cli.rs` (`test_cli_compile_invalid_operation_codes_fail_with_code_2`) verifying `E0020`, `E0021`, `E0022`, `E0023`, `E0025` each deterministically exit with CLI code 2.

### 3.3 Supported OpenQASM Subset Consistency (Audit of Gate `p`)
An audit across `docs/qasm-supported-subset.md`, `docs/qasm3_support.md`, `src/frontend/unsupported.rs`, `src/frontend/parser/gates.rs`, and all positive fixtures confirmed:
1. **`p` is NOT part of the accepted OpenQASM subset in Milestone M2**.
2. By the authoritative Phase 24 contract, QCC supports strictly 14 canonical operations:
   - Single-qubit: `x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `rx(θ)`, `ry(θ)`, `rz(φ)`
   - Two-qubit: `cx`, `cz`, `swap`
   - Non-unitary: `measure`, `reset`, `barrier`, `gphase`
3. In `src/frontend/parser/gates.rs` and `src/frontend/unsupported.rs`, `p` is classified as `UnsupportedConstruct::UnsupportedGate` (`E0004`), requiring basis decomposition in future synthesis phases (Milestone M4).
4. No fixture in `tests/qasm3/conformance/fixtures/supported/` contains `p`.
5. Added a dedicated fail-closed conformance fixture [`tests/qasm3/conformance/fixtures/unsupported/unsupported_standard_p.qasm`](../../tests/qasm3/conformance/fixtures/unsupported/unsupported_standard_p.qasm) and integration test (`test_conformance_unsupported_standard_p`) asserting that `p` fails closed with `E0004` and exit code `3`.

### 3.4 Fuzzing Classification & Claims
The repository distinguishes between three distinct fuzzing tiers:
1. **Tier 1: Deterministic Mutation-Based Smoke Harnesses (Executed)**:
   - Standalone binaries `fuzz_lexer` and `fuzz_parser` compile under standard stable Rust without external tools.
   - PRNG: `SplitMix64` seeded with `0x853c49e6748fea9b`.
   - Executed 1,000 pseudo-random iterations per target plus 33 synthetic structural edge cases (deep nesting, huge operand lists, oversized numbers, malformed syntax).
   - Executed and validated cleanly via `cargo run --bin fuzz_lexer -- --smoke` and `cargo run --bin fuzz_parser -- --smoke`.
2. **Tier 2: Fixed Regression Corpus Tests (Executed)**:
   - Corpus maintained in `fuzz/corpus/lexer/` (4 files) and `fuzz/corpus/parser/` (6 files).
   - Automated integration suite in `tests/qasm3/fuzz_regression.rs` (6 tests).
   - Directly tests recursion and memory limits (`MAX_EXPRESSION_DEPTH = 128`, `MAX_OPERANDS_PER_STATEMENT = 10,000`, `MAX_REGISTER_SIZE = 1,000,000`, `LexerLimits`).
3. **Tier 3: Coverage-Guided Continuous Fuzzing (Configured, Nightly Only)**:
   - Configured via `fuzz/Cargo.toml` and entry points `fuzz_lexer_target(&[u8])` and `fuzz_parser_target(&[u8])`.
   - **Not executed in this environment**: `cargo-fuzz` and LLVM libFuzzer require nightly Rust toolchains and sanitizers, which violates QCC's mandatory invariant of stable Rust (zero nightly flags). It is documented for external nightly audits.

---

## 4. Exit Gate Verification Evidence

> **EXIT GATE**: Frontend conformance suite and CLI smoke tests pass.

### Verification Commands Executed
```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --all-targets
cargo doc --no-deps
cargo run --example bell
cargo run --example qasm
cargo run --bin fuzz_lexer -- --smoke
cargo run --bin fuzz_parser -- --smoke
cargo run --bin fuzz_lexer -- fuzz/corpus/lexer/
cargo run --bin fuzz_parser -- fuzz/corpus/parser/
cargo test --test cli
cargo test --test smoke
python3 -c "
import sys, glob
sys.path.insert(0, '/tmp/py_packages')
import openqasm3
for f in sorted(glob.glob('tests/qasm3/conformance/fixtures/supported/*.qasm')):
    openqasm3.parse(open(f).read())
"
```

All verification commands exited with status `0`.

### Execution Log / Output Summary
- `cargo test --all`: **615 passed across all 24 test targets**, 0 failed, 0 ignored.
- `cargo test --test qasm3`: **210 passed** (34 conformance + 6 fuzz regression + 170 frontend unit/integration tests).
- `cargo test --test cli`: **20 passed** (all compile, inspect, stdin, error code, and diagnostic tests).
- `src/lib.rs` unit tests: **72 passed** (+1 stability constants test).
- `fuzz_lexer` smoke run: **1000 pseudo-random iterations + 14 synthetic edge cases passed cleanly**.
- `fuzz_parser` smoke run: **1000 pseudo-random iterations + 19 synthetic edge cases passed cleanly**.
- Independent reference parser (`openqasm3` v1.0.1) parsed all 7 supported conformance fixtures without error.

---

## 5. Test Suite Summary & Inventory

### Test Targets Before vs. After Phase 27

| # | Test Target | Phase 26 Baseline (`2a14111`) | Phase 27 Final (`HEAD`) | Delta & Notes |
|:---:|---|:---:|:---:|---|
| 1 | `src/lib.rs` (unit tests) | 71 | 72 | **+1 diagnostic code stability test** |
| 2 | `fuzz/fuzz_targets/fuzz_lexer.rs` (bin) | — | 0 | Binary target |
| 3 | `fuzz/fuzz_targets/fuzz_parser.rs` (bin) | — | 0 | Binary target |
| 4 | `src/main.rs` (unit tests) | 0 | 0 | Unchanged |
| 5 | `tests/ast.rs` | 30 | 30 | Unchanged |
| 6 | `tests/builder.rs` | 10 | 10 | Unchanged |
| 7 | `tests/cli.rs` | — | 20 | **+20 new CLI integration tests** |
| 8 | `tests/end_to_end.rs` | 9 | 9 | Unchanged |
| 9 | `tests/equivalence.rs` | 23 | 23 | Unchanged |
| 10 | `tests/frontend_adapter.rs` | 14 | 14 | Unchanged |
| 11 | `tests/gate_identities.rs` | 23 | 23 | Unchanged |
| 12 | `tests/ir.rs` | 19 | 19 | Unchanged |
| 13 | `tests/ir_rewrites.rs` | 21 | 21 | Unchanged |
| 14 | `tests/lexer.rs` | 29 | 29 | Unchanged |
| 15 | `tests/measurement.rs` | 11 | 11 | Unchanged |
| 16 | `tests/openqasm_emitter.rs` | 21 | 21 | Unchanged |
| 17 | `tests/pass_manager.rs` | 14 | 14 | Unchanged |
| 18 | `tests/qasm3.rs` | 169 | 210 | **+41 new tests** (34 conformance + 6 fuzz + 1 lowering) |
| 19 | `tests/quantum_semantics.rs` | 23 | 23 | Unchanged |
| 20 | `tests/sampling.rs` | 9 | 9 | Unchanged |
| 21 | `tests/simulator_dag.rs` | 15 | 15 | Unchanged |
| 22 | `tests/simulator_single_qubit.rs` | 19 | 19 | Unchanged |
| 23 | `tests/simulator_two_qubit.rs` | 18 | 18 | Unchanged |
| 24 | `tests/smoke.rs` | 5 | 5 | Unchanged |
| **Total** | **All 24 Test Targets** | **553** | **615** | **+62 net new passing tests (0 removed, 0 modified)** |

*Arithmetic Verification*:
$72 + 0 + 0 + 0 + 30 + 10 + 20 + 9 + 23 + 14 + 23 + 19 + 21 + 29 + 11 + 21 + 14 + 210 + 23 + 9 + 15 + 19 + 18 + 5 = \mathbf{615}$.

---

## 6. Architectural Invariants Verified

- [x] **Zero Panics on Malformed Input**: Verified through 2,000+ pseudo-random fuzz iterations and 33 synthetic edge cases.
- [x] **Bounded Nesting Depth**: `MAX_EXPRESSION_DEPTH = 128` prevents stack exhaustion on deeply parenthesized expressions.
- [x] **Bounded Allocations**: Operand count (10,000), parameter count (128), and register size (1,000,000) are capped.
- [x] **Deterministic Stable Exit Codes**: `0` for success, `1` for operational failure, `2` for syntax/semantic errors, `3` for unsupported constructs.
- [x] **Exact Source-Located Diagnostics**: All errors rendered as `file:line:col-endcol: level[code]: message` with notes.
- [x] **Fail-Closed Subset Integrity**: No unsupported construct is silently skipped or partially accepted; gate `p` fails closed with `E0004`.
- [x] **Independent Verification**: Pinned `openqasm3` v1.0.1 reference parser validates all supported conformance fixtures.
- [x] **Workspace Integrity**: Package structure remained clean without premature workspace restructuring.

---

## 7. Milestone M2 Completion Status

With the completion of Phase 27:
- **Milestone M2 (Phases 12–27, 16 granular phases) is 100% COMPLETE**.
- The OpenQASM 3 frontend, parser, AST, semantic analyzer, validator, lowerer, emitter, round-trip engine, fuzzing harness, conformance suite, and CLI are fully implemented and verified.
- **Next Phase**: **Phase 28 — Identity-gate elimination** (beginning Milestone M3: Verified Logical Optimizer).

---

## 8. Approval & Ledger Update

- [x] Exit gate satisfied with full proof.
- [x] Evidence recorded.
- [x] `docs/project-state/progress.md` updated.
- [x] Milestone M2 complete. Stop here for user review and approval before starting Phase 28.
