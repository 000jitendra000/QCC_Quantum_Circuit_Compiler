# Milestone M7 Session 4 Completion Report

**Session**: M7 Session 4 — Difficult Deterministic Corpus, Systematic Fault Injection & Fair Qiskit Baseline  
**Roadmaps / Phases Covered**: Roadmap v3.1 **Phase 67** (`Oracle self-tests and fault-injection validation`), **Phase 68** (`Deterministic circuit corpus and scale-up campaign`), and **Phase 70** (`Fair Qiskit baseline — preparation for v0.2 acceptance`)  
**Baseline Commit**: `236e2f8` (M7 Session 3)  
**Date Completed**: 2026-11-12  

---

## 1. Architecture & Deliverables

Zero Rust source files or Cargo runtime dependencies were modified. All Session 4 tooling lives cleanly under `tools/independent_oracle/`:

1. **Result & Metadata Schemas (`tools/independent_oracle/schema/`)**:
   - Extended [`canonical_case.schema.json`](../../../tools/independent_oracle/schema/canonical_case.schema.json) with optional strict `generator_metadata`.
   - Added [`corpus_summary.schema.json`](../../../tools/independent_oracle/schema/corpus_summary.schema.json) (`qcc.corpus_summary` v`1.0.0`), [`fault_campaign_summary.schema.json`](../../../tools/independent_oracle/schema/fault_campaign_summary.schema.json) (`qcc.fault_campaign_summary` v`1.0.0`), [`benchmark_trial.schema.json`](../../../tools/independent_oracle/schema/benchmark_trial.schema.json) (`qcc.benchmark_trial` v`1.0.0`), and [`benchmark_aggregate.schema.json`](../../../tools/independent_oracle/schema/benchmark_aggregate.schema.json) (`qcc.benchmark_aggregate` v`1.0.0`).
2. **1,080-Case Difficult Deterministic Corpus (`tools/independent_oracle/corpus/`)**:
   - Generator [`generate_corpus.py`](../../../tools/independent_oracle/corpus/generate_corpus.py) (`v1.0.0`, `master_seed=20261112`) + separate executor [`run_corpus.py`](../../../tools/independent_oracle/corpus/run_corpus.py).
   - Committed deterministic corpus [`corpus_v1.jsonl`](../../../tools/independent_oracle/corpus/corpus_v1.jsonl) (`sha256 = f865990ea401f7174bbe777371bf74c32c6f24da6aed1388a604858314226b6f`), [`corpus_generation_summary.json`](../../../tools/independent_oracle/corpus/corpus_generation_summary.json), and [`corpus_execution_summary.json`](../../../tools/independent_oracle/reports/corpus_execution_summary.json).
3. **Systematic Fault-Injection Campaign (`tools/independent_oracle/fault_injection/run_campaign.py`)**:
   - Exercises **40 deterministic mutations** across **all 17 required mutation families**, preserving failure artifacts + `mutation_description.json` without mutating original canonical fixtures, summarized in [`fault_campaign_summary.json`](../../../tools/independent_oracle/reports/fault_campaign_summary.json).
4. **Fair Matched-$N$ QCC-vs-Qiskit Baseline (`tools/independent_oracle/benchmark/run_baseline.py`)**:
   - Runs **18 representative comparable workloads** (`linear` & `ring` targets, shared `{rz, sx, cx}` basis, widths 2–6) with $N = 5$ trials per compiler (`90` QCC trials + `90` Qiskit trials), semantically verifying **every single trial** on both sides through the Session 3 oracle before counting costs.
   - Outputs [`benchmark_trials.jsonl`](../../../tools/independent_oracle/reports/benchmark_trials.jsonl) (`180` schema-validated per-trial records) and [`benchmark_aggregate.json`](../../../tools/independent_oracle/reports/benchmark_aggregate.json).
5. **Automated Session 4 Tests ([`tests/test_oracle_session4.py`](../../../tools/independent_oracle/tests/test_oracle_session4.py))**: 7 automated integration tests.

---

## 2. Corpus Coverage & Execution Results

- **Size & Uniqueness**: `total_cases = 1,080`, `distinct_case_ids = 1,080`, `distinct_logical_hashes = 1,080`, `duplicate_logical_hashes = 0`. Repeated generation reproduces bit-for-bit identical `corpus_v1.jsonl` (`sha256: f865990ea401f7174bbe777371bf74c32c6f24da6aed1388a604858314226b6f`).
- **By Topology**: `linear: 272`, `linear-generic: 272`, `ring: 272`, `ring-generic: 264`.
- **By Logical Qubits**: `1Q: 95`, `2Q: 220`, `3Q: 187`, `4Q: 187`, `5Q: 182`, `6Q: 121`, `7Q: 88` (`1Q -> 2Q` physical targets exercise real spare-ancilla routing/uncomputation; note QCC CLI currently sets physical width to `max(logical_qubits, 2)`).
- **By Family (`135` each)**: `single_qubit_rotations`, `cx_asymmetric_pairs`, `ghz_entanglement`, `qft_like_nonadjacent`, `logical_swap_stress`, `random_layered_dense`, `global_and_relative_phase`, `parameterized_bound`.
- **By Interaction Density**: `dense: 455`, `medium: 253`, `sparse: 210`, `single_qubit_only: 162`.
- **Depth Target Range**: `min = 4`, `median = 12.0`, `max = 26` (`1-5: 52`, `6-10: 373`, `11-20: 508`, `21+: 147`).
- **By Verification Mode**: `exhaustive_unitary: 836` ($m \le 5$), `sampled_statevector: 244` ($m \ge 6$ plus explicit $m=4,5$ slice, `16` seeded random + basis states, explicitly flagged `exhaustive: false`).
- **Execution Results**:
  - `exhaustive_passed = 836 / 836` (`0` failed)
  - `sampled_passed = 244 / 244` (`0` failed)
  - `unsupported_count = 0`, `invalid_or_error_count = 0`
  - `max_amplitude_error_observed = 2.29e-15`, `max_residual_ancilla_observed = 0.0`

---

## 3. Fault-Injection Campaign Results

- **Attempted**: `40` mutations across **17 distinct mutation families** (`seed = 20261113`).
- **Detected Correctly**: **`40 / 40` (`100.0%`)**, `misclassified = 0`, `undetected_semantic_defects = 0`, deterministic `results_sha256` identical across repeated runs.
- **Classification Breakdown**:
  - `MISMATCH`: `27` (`single_qubit_gate_mutation` 3, `incorrect_rotation_angle` 3, `missing_gate` 3, `extra_gate` 3, `reversed_cx_control_target` 3, `incorrect_swap_behavior` 2, `incorrect_initial_layout` 2, `incorrect_final_layout` 2, `relative_phase_defect` 2, `residual_ancilla_population` 2, `residual_ancilla_entanglement` 2)
  - `INVALID_INPUT`: `9` (`incorrect_physical_qubit_count` 2, `corrupted_emitted_qasm` 2, `corrupted_manifest_hash` 2, `invalid_or_incomplete_layout_metadata` 3)
  - `UNSUPPORTED`: `2` (`unsupported_non_unitary_operation`: physical `reset` & logical `measure`)
  - `PASS` *(with informational `strict_phase_accounting = MISMATCH`)*: `2` (`corrupted_global_phase_metadata` 2 — proved informational phase-accounting catches manifest phase tampering without overriding common-phase semantic equivalence)

---

## 4. Fair QCC-vs-Qiskit Compilation-Cost Baseline

- **Pinned Versions**: `qcc 0.1.0` (`CompilerPipeline::default`: `O2`, `SabreBidirectionalRouter`, `Weighted` placement) vs `qiskit 2.6.2` (`optimization_level=2`, `basis_gates=["rz", "sx", "cx"]`, `VF2Layout -> SabreLayout`, `SabreSwap` decay heuristic).
- **Explicit Fairness & Compute-Budget Disclosures**:
  - QCC exposes **no randomized search seed** via CLI and executes **1 deterministic configuration** per run (`internal_search_trials = 1`); the `5` repeated QCC runs measure timing repeatability of that deterministic configuration, not search variance.
  - Qiskit `optimization_level=2` executes **`20` internal randomized SABRE layout/swap trials** (`max_iterations = 2`, `routing_trials = 20`) per external `seed_transpiler` call (`[1000..1004]`), inspected dynamically from the preset pass manager. Thus `equal_internal_compute_budget = false`.
  - **Timing separation**: QCC `compile_wall_time_ms` wraps black-box subprocess CLI execution (includes process launch + file I/O), whereas Qiskit `compile_wall_time_ms` wraps in-process `pm.run()`. Neither includes verification/simulation time.
  - **SWAP metric clarity**: Both emit `{rz, sx, cx}` (`final_emitted_swaps == 0`); QCC exposes `router_swaps_inserted` via manifest, whereas Qiskit `transpile()` decomposes SWAPs into 3 CXs (`router_swaps_inserted = null`).
  - **Unavailable comparisons**: `linear-generic`, `ring-generic`, and `frontend_only` are explicitly classified `UNAVAILABLE` by `check_target_comparability` due to mismatched bases/`sxdg` importer gaps/missing hardware topology.
- **Baseline Results (`18` comparable workloads $\times$ `5` trials each)**:
  - **Semantic Verification Gate**: QCC `90 / 90 PASS`, Qiskit `90 / 90 PASS`.
  - **Median CX Count (QCC vs Qiskit Win / Tie / Loss)**: **`0 / 5 / 13`**
  - **Median Circuit Depth (QCC vs Qiskit Win / Tie / Loss)**: **`1 / 2 / 15`**
  - Qiskit's 20-trial SABRE + `ConsolidateBlocks`/KAK 2Q optimization yields lower median CX count on 13/18 workloads and lower depth on 15/18 workloads, directly documenting concrete optimization/routing gaps for M9 and M10 without spin.

---

## 5. Verification Evidence

- `.venv/bin/python3 tools/independent_oracle/corpus/run_corpus.py`: `1,080 / 1,080 PASS` (`836` exhaustive, `244` sampled, `0` failed, exit code `0`).
- `.venv/bin/python3 tools/independent_oracle/fault_injection/run_campaign.py`: `40 / 40` detected across 17 families (`0` misclassified, `0` undetected semantic defects, exit code `0`).
- `.venv/bin/python3 tools/independent_oracle/benchmark/run_baseline.py --trials 5`: `180 / 180` semantically verified trials (`90` QCC, `90` Qiskit, exit code `0`).
- `.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`: **41 tests passed**, 0 failed (`43.287s`, exit code `0`).
- Schema validations (`Draft202012Validator`), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`, `cargo test --lib --test cli --test compiler_end_to_end`, and full `cargo test --no-fail-fast`: all passed cleanly with zero warnings and zero Rust source modifications.
