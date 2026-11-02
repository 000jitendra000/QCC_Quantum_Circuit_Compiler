# Implementation Plan — Phase 27: QASM Fuzzing, Conformance and Frontend CLI

## 1. Phase Metadata
- **Phase**: Phase 27
- **Title**: QASM fuzzing, conformance and frontend CLI
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Effort Rating**: **L**
- **Prerequisites**: Phases 17–26 (completed and verified).

---

## 2. Objective
Harden the OpenQASM 3 frontend against malformed inputs, enforce strict resource limits, and provide a fully functional command-line interface (`qcc compile` and `qcc inspect`) with deterministic exit codes and source-located diagnostic rendering.

---

## 3. Scope Boundaries & Invariants
- Enforce strict resource limits: max expression depth (128), max operands per statement (10,000), max parameters per gate (128), max register size (1,000,000).
- Zero panics or hangs on malformed, corrupted, or oversized inputs.
- Stable CLI exit codes: 0 (Success), 1 (CLI/Operational Error), 2 (Compilation Error), 3 (Unsupported Construct Error).
- Do not implement optimizer passes (Milestone M3).
- Do not perform premature workspace restructuring.

---

## 4. Key Deliverables
1. **Fuzz Targets & Corpus**:
   - `fuzz/fuzz_targets/fuzz_lexer.rs`
   - `fuzz/fuzz_targets/fuzz_parser.rs`
   - `fuzz/corpus/` regression suites
   - `fuzz/Cargo.toml`
2. **Conformance Suite**:
   - `tests/qasm3/conformance/fixtures/supported/` (7 fixtures)
   - `tests/qasm3/conformance/fixtures/unsupported/` (12 fixtures)
   - `tests/qasm3/conformance/fixtures/invalid/` (11 fixtures)
   - `tests/qasm3/conformance/mod.rs` (30 integration tests)
   - `tests/qasm3/fuzz_regression.rs` (6 regression tests)
3. **CLI Commands**:
   - `src/cli/mod.rs`
   - `src/cli/compile.rs`
   - `src/cli/inspect.rs`
   - `tests/cli.rs` (17 integration tests)
4. **Documentation**:
   - `docs/qasm-conformance.md`
   - `docs/qasm-fuzzing.md`
   - `docs/cli-reference.md`
   - `docs/project-state/reports/phase-027.md`

---

## 5. Verification Gate
- `cargo fmt --check` passes cleanly.
- `cargo clippy --all-targets --all-features -- -D warnings` reports zero warnings.
- `cargo test --all` passes all tests cleanly.
- `cargo build --all-targets` passes cleanly.
- `cargo doc --no-deps` generates rustdoc without warnings.
- Fuzz smoke tests pass cleanly.
- Bell and QASM examples execute with code 0.

