# Phase Plan: Phase 1 — Repository scaffold, CI and error infrastructure

## 1. Metadata
- **Phase Number**: Phase 1
- **Milestone**: Milestone M1: Bell Vertical Slice
- **Relative Effort**: M
- **Prerequisites Satisfied**: None (Foundational phase)
- **Source Document Page**: Page 6 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Establish a reproducible Rust development base before quantum code accumulates.
- **Explicit Inclusions**:
  - `Cargo.toml` with package metadata, `thiserror`, and `proptest` dev-dependency.
  - Generate and verify `Cargo.lock`.
  - `src/lib.rs` and `src/main.rs` with minimal `--version` CLI handling.
  - `src/errors.rs` top-level typed error structure (`QccError`, `Result`).
  - `src/diagnostics/` (`mod.rs`, `span.rs`, `diagnostic.rs`) source span model and deterministic diagnostic formatting.
  - `tests/smoke.rs` containing CLI version test, golden diagnostic formatting test, and proptest valid-span property.
  - `.github/workflows/ci.yml` running fmt, clippy, and test on stable Rust.
  - Updated `README.md` documenting clean-checkout setup and developer commands.
- **Explicit Exclusions**:
  - No quantum gate matrices, statevectors, or simulation kernels (Phase 2+).
  - No QccIR data structures or petgraph graphs (Phase 6).
  - No OpenQASM parsing or emission (Phase 9+).
  - No premature CLI commands or framework.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable (stable 1.80+).
- [x] Zero nightly feature flags or unstable APIs.
- [x] No quantum semantics altered (none implemented yet).
- [x] Zero blanket warning suppressions.
- [x] Errors and diagnostics cleanly decoupled.
- [x] Version derived directly from Cargo package metadata.

## 4. Step-by-Step Implementation Sequence
1. Create `Cargo.toml`.
2. Fetch dependencies and generate `Cargo.lock`.
3. Implement `src/diagnostics/span.rs` (source span invariants and formatting).
4. Implement `src/diagnostics/diagnostic.rs` (diagnostic levels, codes, messages, and golden formatting).
5. Implement `src/diagnostics/mod.rs`.
6. Implement `src/errors.rs` (`QccError` categories).
7. Implement `src/lib.rs` exposing public API.
8. Implement `src/main.rs` with `--version` and `--help` handling.
9. Implement `tests/smoke.rs` (version test, golden test, proptest property).
10. Create `.github/workflows/ci.yml`.
11. Update `README.md` with development commands.
12. Run full verification suite: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`, `cargo run -- --version`.
13. Produce Phase 1 completion report and update `docs/project-state/progress.md`.

## 5. Expected Files and Modules
- New files:
  - `Cargo.toml`
  - `Cargo.lock`
  - `src/lib.rs`
  - `src/main.rs`
  - `src/errors.rs`
  - `src/diagnostics/mod.rs`
  - `src/diagnostics/span.rs`
  - `src/diagnostics/diagnostic.rs`
  - `tests/smoke.rs`
  - `.github/workflows/ci.yml`
  - `docs/project-state/plans/phase-001.md`
  - `docs/project-state/reports/phase-001.md`
- Modified files:
  - `README.md`
  - `docs/project-state/progress.md`

## 6. Verification & Test Plan
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --test smoke`
- Verify CLI: `cargo run -- --version`
- Clean checkout verification.

## 7. Exit Gate Verification Target
> **EXIT GATE CRITERIA**: CI passes on a clean checkout; CLI version command and smoke test work.

