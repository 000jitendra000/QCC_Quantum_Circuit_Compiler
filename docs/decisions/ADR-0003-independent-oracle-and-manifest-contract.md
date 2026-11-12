# ADR-0003: Independent Verification Oracle Boundary and Compilation Manifest Contract

## Status
**Accepted** (2026-11-12 — M7 Session 1)

## Context
Roadmap v3.1 Milestone M7 (Phases 60–70, executed in five sessions) introduces an independent Qiskit-backed verification oracle for QCC v0.2. Previously, `qcc compile` emitted only OpenQASM 3 / QccIR output and optional human-readable `--stats` markdown tables on `stderr`. It did **not** expose initial layout ($L_{\text{initial}}$), final layout ($L_{\text{final}}$), target basis/connectivity, global phase metadata, or structured SWAP/per-gate counts through the CLI.

Parsing human-oriented terminal text is brittle and cannot safely convey bijective/injective layout permutations, global phase, or unavailable optional metadata. Furthermore, Roadmap v3.1 mandates that Qiskit/Python remain strictly external dev/CI tooling in `tools/independent_oracle/` without becoming a dependency of `qcc-core` or normal Rust execution.

## Decision

### 1. Independent Oracle Domain & Boundary (Phase 60 Contract)
1. **In-Repo Dev/CI Tooling Only**: The oracle lives exclusively under `tools/independent_oracle/` and is never a Cargo or runtime dependency of `qcc-core` or the `qcc` CLI binary.
2. **Initial Supported Oracle Domain**: Static unitary circuits over QCC's supported OpenQASM 3 unitary subset (`x`, `y`, `z`, `h`, `s`, `sdg`, `t`, `tdg`, `id`, `rx`, `ry`, `rz`, `cx`, `cz`, `swap`, `gphase`, numeric/bound parameter expressions). Non-unitary operations (`measure`, `reset`), classical control, dynamic CFG, and noise remain outside the unitary oracle domain and must be classified as unsupported rather than passed or treated as unitary.
3. **Strict Separation of Semantic Reference vs Cost Baseline**:
   - The oracle constructs its Qiskit logical reference circuit directly from neutral canonical cases — never from QCC output and never by calling QCC's internal `LayoutEquivalenceChecker`.
   - Qiskit's transpiler is a separate optimization cost/quality baseline (Phase 64 / Phase 70), never the semantic truth source.
4. **Full Physical-State, Layout, Endian & Clean-Ancilla Verification**:
   - Logical qubit `q[0]` is LSB (`little_endian_q0_lsb`).
   - Logical inputs are embedded through `initial_logical_to_physical` ($L_{\text{initial}}$) with all physical qubits outside its image initialized to $|0\rangle$.
   - The QCC-emitted physical circuit is simulated over all $2^{n_{\text{phys}}}$ amplitudes.
   - The independent logical reference output is embedded through `final_logical_to_physical` ($L_{\text{final}}$) with all qubits in `unused_physical_qubits_final` set to $|0\rangle$ (ADR-0001; never traced out).
   - Authoritative semantic equivalence requires agreement up to **one common scalar global phase** across the entire comparison. A separate informational strict phase-accounting diagnostic (`PASS` / `MISMATCH` / `UNVERIFIED`) is tracked independently without overriding semantic equivalence.
5. **Verification Tiers**:
   - Tractable widths: exhaustive basis/unitary matrix comparison (explicitly labeled exhaustive).
   - Larger widths: deterministic basis + seeded normalized random states (explicitly labeled sampled semantic testing, not exhaustive proof).

### 2. Versioned Machine-Readable Compilation Manifest (`qcc.compile_manifest` v1.0.0)
1. **Additive CLI Interface**: `qcc compile` exposes `--emit-manifest <FILE>` (additive; existing stdout, `-o`, `--format`, `--global-phase`, `--measure-syntax`, `--bind`, `--target`, and `--stats` behaviors remain unchanged).
2. **Direct Compiler Data Origin**: Manifest fields are constructed directly from QCC's structured `CompilationResult` (including `physical_circuit`, `layout: CircuitLayout`, `target: Target`, `legality_report`, `schedule_output`, `report: CompilationReport`, and emitted output text) — never parsed from human-readable `--stats` output.
3. **Explicit Layout Direction**:
   - `layout.mapping_direction` is explicitly declared as `"logical_to_physical"`.
   - `layout.initial_logical_to_physical` maps logical `QubitId` integer index $\to$ initial `PhysicalQubitId` integer index.
   - `layout.final_logical_to_physical` maps logical `QubitId` integer index $\to$ final `PhysicalQubitId` integer index after all routing SWAPs.
   - `layout.unused_physical_qubits_initial` and `layout.unused_physical_qubits_final` list sorted unassigned physical qubit indices (`PhysicalQubitId`).
   - Initial and final layouts are never assumed identical.
4. **Explicit Unknowns / Unavailable Fields**:
   - When `--target` is omitted (`compilation_mode: "frontend_only"`), target-dependent fields (`target`, `physical_qubits`, `layout`, `routing`, `legality`, `scheduling`, `internal_verification`) are explicitly serialized as `null` — never guessed or defaulted to an identity layout.
   - Unresolved symbolic `global_phase` reports `radians: null` alongside `is_numeric: false` and the symbolic `expression` string.
   - Optional build commit hash (`compiler.git_commit`) is `null` unless injected at build time via `QCC_GIT_SHA`.
   - Non-deterministic wall-clock compile durations and timestamps are excluded from the manifest so repeated identical runs produce bit-for-bit identical JSON.
5. **Unambiguous SWAP & Gate Metrics**:
   - `metrics.total_gate_count`: unitary `Operation::Gate` count in the final emitted circuit (equals `sum(per_gate_counts.values())`).
   - `metrics.total_operations`: total DAG operations in the final circuit (including barriers and non-unitary operations).
   - `metrics.logical_swaps`: `swap` gates in the pre-compilation logical circuit.
   - `metrics.router_swaps_inserted`: SWAPs inserted by QCC's router.
   - `metrics.final_emitted_swaps`: `swap` gates remaining in the final physical circuit (matches `per_gate_counts["swap"]`).
6. **Fail-Closed Error Handling**:
   - Compilation failures (I/O, syntax, semantic, unsupported construct, pipeline failure) fail with their normal CLI exit codes and do **not** write a manifest file.
   - Manifest write failures fail with exit code `1` (`CliError`) and an explicit error on `stderr`.

## Rationale
- Defining `CompilationManifest` in `src/compiler/manifest.rs` backed directly by `CompilationResult` eliminates stdout-scraping fragility and binds the oracle contract to QCC's typed IR/layout invariants.
- Using `BTreeMap` everywhere and omitting non-deterministic wall-clock durations ensures reproducible manifests for CI caching, artifact hashing, and regression fixtures.
- Keeping `frontend_only` vs `target_compiled` explicit via `Option` / `null` fields honors QCC's Explicit Unknown principle.

## Relationship to Roadmap & Specification
Implements **Roadmap v3.1 — M7 Session 1 (Phases 60 & 63 manifest deliverable)** and upholds **ADR-0001** (strict ancilla zero-state policy via explicit `unused_physical_qubits_final`).

## Implementation Consequences
- **Positive Impacts**: Downstream M7 Sessions 2–5 gain a stable, schema-validated interface from QCC to `tools/independent_oracle/`.
- **Constraints & Costs**: Future manifest additions must preserve backward compatibility or bump `schema_version` (`tools/independent_oracle/schema/compile_manifest.schema.json`).
- **Affected Subsystems**: `src/compiler/{mod,pipeline,result,manifest}.rs`, `src/cli/compile.rs`, `tools/independent_oracle/schema/compile_manifest.schema.json`.

## Required Tests & Verification
1. Manifest schema version, compiler identity, and FIPS 180-4 SHA-256 test vectors.
2. Initial and final layout direction (`logical_to_physical`), non-trivial layout permutations (`initial != final`), and spare ancilla lists (`unused_physical_qubits_final`).
3. Logical vs physical qubit counts (including targets with spare physical qubits).
4. Numeric and symbolic global-phase representation and `gphase` emission flag alignment.
5. Gate-count consistency (`sum(per_gate_counts) == total_gate_count`) and SWAP distinctions (`logical_swaps`, `router_swaps_inserted`, `final_emitted_swaps`).
6. Explicit `null` representation for unavailable fields (`frontend_only`, disabled scheduling, symbolic phase, unset `git_commit`).
7. Compiler failure behavior (no manifest written) and unwritable manifest path failure (`exit code 1`).
8. Bit-for-bit deterministic manifest output across repeated equivalent CLI runs.
9. End-to-end integration tests compiling `examples/routing.qasm` and `examples/showcase.qasm` and reconciling manifest fields against the actual emitted OpenQASM 3 files.

