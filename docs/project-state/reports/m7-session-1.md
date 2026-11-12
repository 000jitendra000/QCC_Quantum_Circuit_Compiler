# M7 Session 1 Completion Report — Contract and Manifest-First Rust Work

## 1. Summary
- **Session / Phases Covered**: M7 Session 1 (Phase 60 contract + Phase 63 manifest-first Rust deliverable)
- **Milestone**: Milestone M7: Independent Verification Oracle (`M7` / v0.2)
- **Completion Date**: 2026-11-12
- **Status**: **Completed** (Exit Gate Verified)

## 2. Deliverables & Files Changed
- **Created**:
  - [`docs/decisions/ADR-0003-independent-oracle-and-manifest-contract.md`](../../decisions/ADR-0003-independent-oracle-and-manifest-contract.md) — Formalizes the oracle boundary, supported unitary domain, layout direction (`logical_to_physical`), little-endian convention (`q[0]` LSB), common-global-phase + informational strict phase-accounting rules, ADR-0001 clean-ancilla verification, and versioned manifest schema contract.
  - [`tools/independent_oracle/schema/compile_manifest.schema.json`](../../../tools/independent_oracle/schema/compile_manifest.schema.json) — JSON Schema (Draft 2020-12) for `qcc.compile_manifest` (`1.0.0`).
  - [`tools/independent_oracle/README.md`](../../../tools/independent_oracle/README.md) — Documents Session 1 schema/contract artifacts while keeping Python/Qiskit out of Rust builds and deferring Qiskit integration to Sessions 2–5.
  - [`src/compiler/manifest.rs`](../../../src/compiler/manifest.rs) — Implements `CompilationManifest`, typed metadata structs, `ManifestBuildContext`, `CompilationManifest::from_compilation_result`, `CompilationManifest::from_frontend_circuit`, and pure-Rust FIPS 180-4 `sha256_hex` + 5 unit tests.
  - [`tests/compiler/manifest.rs`](../../../tests/compiler/manifest.rs) — 4 compiler-level integration tests verifying non-trivial layout permutations, directed target metadata, spare ancilla tracking, ASAP scheduling metadata, symbolic/numeric global phase, and determinism.
- **Modified**:
  - [`src/compiler/result.rs`](../../../src/compiler/result.rs) — Added `target: Target` and `input_global_phase: ParameterExpr` (with `target()` and `input_global_phase()` accessors) to `CompilationResult`.
  - [`src/compiler/pipeline.rs`](../../../src/compiler/pipeline.rs) — Populated `target` and `input_global_phase` on `CompilationResult`.
  - [`src/compiler/mod.rs`](../../../src/compiler/mod.rs), [`src/lib.rs`](../../../src/lib.rs) — Exported `manifest` module and public manifest types.
  - [`src/cli/compile.rs`](../../../src/cli/compile.rs) — Added additive `--emit-manifest <FILE>` flag to `qcc compile`, building the manifest directly from `CompilationResult` (or frontend `Circuit` in frontend-only mode) without stdout scraping.
  - [`tests/compiler_end_to_end.rs`](../../../tests/compiler_end_to_end.rs) — Registered `tests/compiler/manifest.rs`.
  - [`tests/cli.rs`](../../../tests/cli.rs) — Added 9 CLI integration tests covering help text, missing flag argument, full `showcase.qasm` QASM-vs-manifest reconciliation, SWAP accounting (`linear` vs `linear-generic`), `frontend_only` `null` fields, byte-for-byte determinism, fail-closed compiler errors (no manifest written), unwritable manifest path (`exit code 1`), and parameter bindings / unbound symbol rejection.
  - [`docs/project-state/progress.md`](../progress.md) — Recorded M7 Session 1 exit-gate evidence and advanced active next task to **M7 Session 2**.

## 3. Verification Commands & Exit Gate Evidence
- `cargo fmt --check`: Passed (exit code `0`)
- `cargo clippy --all-targets -- -D warnings`: Passed (exit code `0`, zero warnings)
- `cargo doc --no-deps`: Passed (exit code `0`, zero warnings)
- `PATH="/home/ronanki_jitendra/rust/QCC/.venv/bin:$PATH" cargo test --no-fail-fast -- --quiet`: Passed (exit code `0`, **1,401 tests passed across all 37 non-empty test targets**, 0 failed, 0 ignored)
- `jsonschema.validate` against `tools/independent_oracle/schema/compile_manifest.schema.json`: Passed (exit code `0` on both `target_compiled` and `frontend_only` manifests)

## 4. Architectural Invariants Verified
- [x] Stable Rust toolchain only (zero nightly flags, zero external crypto crates).
- [x] Zero Qiskit or Python dependencies added to `Cargo.toml`, `qcc-core`, or normal Rust execution.
- [x] Explicit `logical_to_physical` mapping direction and `little_endian_q0_lsb` convention.
- [x] Initial and final layouts tracked distinctly (`initial_logical_to_physical` vs `final_logical_to_physical`), with `unused_physical_qubits_final` enforcing ADR-0001.
- [x] Unavailable optional metadata represented explicitly as `null` (`None`), never fabricated or guessed.
- [x] Deterministic JSON serialization (`BTreeMap` everywhere; no non-deterministic wall-clock fields).
