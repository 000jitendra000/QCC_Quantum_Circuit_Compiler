# QCC Implementation Report — Phase 77: Routing Search Validation and Comparative Benchmark

```text
Phase: Phase 77
Milestone: Milestone M9 — Multi-Candidate Routing Improvements (M9 CLOSEOUT)
Status: Completed & Accepted
Date: 2026-11-13
Roadmap Plan: QCC Engineering Roadmap v3.1 (Phases 60–85)
Artifacts Generated:
  - tools/independent_oracle/benchmarks/routing_search.py
  - tools/independent_oracle/schema/routing_search_benchmark.schema.json
  - tools/independent_oracle/reports/routing_search_results.json
  - docs/routing-search-results.md
  - tests/routing/regressions/mod.rs
```

---

## 1. Executive Overview

Phase 77 concludes **Milestone M9 (Multi-Candidate Routing Improvements)** by providing end-to-end empirical validation, comparative benchmarking, and non-regression verification of QCC's bounded multi-candidate routing search engine (Phases 74–76).

Using the independent verification oracle infrastructure (Milestone M7), Phase 77 executed an extensive comparative campaign across candidate budgets $B \in \{1, 4, 8, 16\}$ on 24 representative canonical cases stratified by qubit count ($1\text{q}$–$7\text{q}$), topology (`linear` and `ring`), and circuit families, with 3 repeated trials per budget (288 total compilations; 96 unique physical circuits evaluated).

### Key Accomplishments & Evidence
1. **100% Independent Oracle Verification**: Every physical circuit emitted by multi-candidate search across all budgets ($96/96$, **100%**) passed independent Qiskit v2.6.2 oracle verification with **0 semantic mismatches**, **0 ADR-0001 clean-ancilla violations**, and **0 target legality failures**.
2. **Strict Non-Regression Guarantee**: Under identical evaluation criteria, multi-candidate search never selected a candidate strictly worse than the deterministic single-candidate baseline ($B=1$) on any case.
3. **Exact Quality Reconciliation on Corpus**: In all 24 evaluated cases, every candidate budget ($B=4, 8, 16$) produced **identical** physical SWAP counts, CX counts, gate counts, and circuit depths as the deterministic single-candidate baseline ($B=1$). Total physical SWAPs remained strictly **100** across all budgets (**0 improved, 24 unchanged, 0 worsened**).
4. **Candidate Diversification Architecture Implemented & Verified**:
   - Implemented seeded deterministic tie-breaking in `src/routing/sabre/scoring.rs` and `src/routing/sabre/heuristic.rs` using SplitMix64 64-bit hashing of candidate physical operand pairs and candidate seeds.
   - Preserved existing 5-tier comparison hierarchy: `total_score` $\to$ `front_cost` $\to$ `lookahead_cost` $\to$ `decay_factor` $\to$ tie-breaker.
   - Candidate 0 strictly preserves unseeded baseline behavior (`seed: None`), ensuring bit-for-bit identical behavior to single-candidate execution.
   - Distinct trajectories verified: `tests/routing/multi_seed.rs::test_seeded_candidates_produce_distinct_routing_trajectories_on_symmetric_workload` proved that on symmetric topologies with genuine equal-score symmetries (e.g. $K_5$ on 5-qubit ring), seeded candidates produce distinct routing trajectories.
   - Root cause for identical quality on the 24-case corpus: under deterministic `Weighted` initial placement, SABRE's front cost, lookahead cost, and decay tiers 1–4 are sufficiently discriminating that either no critical ties occurred or alternative tie branches resulted in equal SWAP counts and were tie-broken in favor of the baseline by `BestValidSoFar`.
5. **Compilation Overhead**: Compilation wall-clock time grew roughly linearly with candidate count: $1.00\times$ at $B=1$ ($16.93\text{ ms}$), $2.63\times$ at $B=4$ ($44.45\text{ ms}$), $4.67\times$ at $B=8$ ($79.01\text{ ms}$), and $8.74\times$ at $B=16$ ($148.03\text{ ms}$).
6. **Authoritative Default-vs-Opt-In Policy**: Documented the linear scaling penalty without circuit-quality improvement under seed-only derivation on this corpus, establishing that deterministic single-candidate routing ($B=1$) must remain the universal compiler default. Multi-candidate routing search is maintained as strictly opt-in, with explicit qualification that layout diversification is required to explore distinct routing paths.

---

## 2. Deliverables Summary

| Component | Path | Description |
|---|---|---|
| **CLI Options** | `src/cli/compile.rs` | Added `--routing-candidates <N>` (requires `--target`) and `--routing-objective <OBJ>` (`min-swaps`, `min-depth`, `min-cx`, `min-gates`). |
| **Search Objective** | `src/routing/search/objective.rs` | Added `RoutingObjective::single(metric)` convenience constructor for single-metric CLI objectives. |
| **QCC Runner Adapter** | `tools/independent_oracle/runners/qcc_runner.py` | Added absolute path resolution and optional `routing_candidates` / `routing_objective` parameters in `run_qcc_compile`. |
| **Benchmark Schema** | `tools/independent_oracle/schema/routing_search_benchmark.schema.json` | JSON Schema validating environment, configs, cases, aggregate comparisons, and policy decisions. |
| **Benchmark Tool** | `tools/independent_oracle/benchmarks/routing_search.py` | Multi-budget benchmark runner executing repeated trials, deterministic trial hashing checks, manifest extraction, Qiskit oracle verification, and schema validation. |
| **Benchmark Symlink** | `tools/independent_oracle/benchmark/routing_search.py` | Backwards-compatible singular symlink to benchmark tool. |
| **Benchmark Report** | `tools/independent_oracle/reports/routing_search_results.json` | Validated JSON results file containing full metrics, runtimes, and oracle verification summaries. |
| **Empirical Results Doc** | `docs/routing-search-results.md` | Comprehensive report with candidate execution audit, per-circuit tables, scaling charts, and qualified policy rationale. |
| **Search Documentation** | `docs/routing-search.md` | Added Section 9 documenting CLI usage, empirical benchmark findings, and default-vs-opt-in policy. |
| **Repository README** | `README.md` | Updated CLI options table and routing documentation links. |
| **Rust Regressions** | `tests/routing/regressions/mod.rs` | 7 comprehensive regression tests covering determinism, budgets, non-regression, topologies, missing timing, and candidate audit. |
| **Test Integration** | `tests/routing.rs` | Linked `regressions` module into routing test target. |

---

## 3. Empirical Benchmark Summary

### 3.1 Aggregate Comparison Across Budgets

| Candidate Budget ($B$) | Mean Compile Time (ms) | Runtime Multiplier | Total Physical SWAPs | SWAP $\Delta$ vs Baseline | SWAPs (Imp / Unc / Wor) | Total Physical Depth | Total Physical Gates | Oracle Pass Rate |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1 (Baseline)** | 16.93 ms | **1.00x** | 100 | — | 0 / 24 / 0 | 976 | 1,977 | **24 / 24 (100%)** |
| **4** | 44.45 ms | **2.63x** | 100 | +0 | 0 / 24 / 0 | 976 | 1,977 | **24 / 24 (100%)** |
| **8** | 79.01 ms | **4.67x** | 100 | +0 | 0 / 24 / 0 | 976 | 1,977 | **24 / 24 (100%)** |
| **16** | 148.03 ms | **8.74x** | 100 | +0 | 0 / 24 / 0 | 976 | 1,977 | **24 / 24 (100%)** |

### 3.2 Repeated Trials & Independent Oracle Coverage Reconciliation
- **Total Compilations**: $24 \text{ cases} \times 4 \text{ budgets} \times 3 \text{ trials} = \mathbf{288\text{ compilation runs}}$.
- **Deterministic Output Equality**: All 3 trials per case/budget produced identical emitted circuit SHA-256 hashes.
- **Physical Outputs Verified**: $24 \text{ cases} \times 4 \text{ budgets} = \mathbf{96\text{ independent verifications}}$.
  - Passed: **96** (100.0%)
  - Mismatches: **0** (0.0%)
  - Unsupported: **0** (0.0%)
  - Not Run: **0** (0.0%)

---

## 4. Verification Evidence & Quality Gates

All authoritative quality gates passed with zero errors, zero warnings, and zero regressions:

1. **`cargo fmt --check`**: Passed (code 0; clean codebase formatting).
2. **`cargo clippy --all-targets -- -D warnings`**: Passed (code 0; zero warnings across all crates and tests).
3. **`cargo doc --no-deps`**: Passed (code 0; clean rustdoc generation).
4. **`cargo test --test routing`**: Passed (213/213 tests passed, including seeded tie-breaking and regression suites).
5. **`cargo test --test cli`**: Passed (51/51 tests passed, including 6 Phase 77 CLI tests).
6. **`PATH=".venv/bin:$PATH" cargo test --no-fail-fast`**: Passed (1,523 workspace tests passed across all 41 test targets, 0 failed, 0 ignored).
7. **`.venv/bin/python3 -m unittest discover -s tools/independent_oracle/tests -v`**: Passed (61/61 Python oracle tests passed).
8. **JSON Schema Validation**: Passed (100% adherence to `routing_search_benchmark.schema.json`).

---

## 5. Milestone M9 Completion and Next Steps

With Phase 77 completed and audited:
- **Milestone M9 (Multi-Candidate Routing Improvements, Phases 74–77)** is fully implemented, verified, audited, documented, and **accepted**.
- In strict adherence to Roadmap v3.1 Rule 8 ("Evidence-Selected Optimization"):
  - **Milestone M10 (Evidence-Selected Optimizer Improvements, Phases 78–81)** is the next planned milestone.
  - Phase 78 (Benchmark-gap analysis and optimizer work selection) will analyze benchmark data to select concrete optimizer rules before any optimization passes are implemented.
  - No changes have been pushed to remote.
