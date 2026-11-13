# Phase 81 — Final Independent Benchmark Evidence Audit

## 1. Audit Charter & Governance

| Attribute | Specification |
|---|---|
| **Phase Audited** | **Phase 81 — Independent optimizer validation and release decision** |
| **Milestone** | Milestone M10: Evidence-Selected Optimizer Improvements (`M10`) |
| **Audit Checkpoint** | `b9c7f04` on branch `main` |
| **Preceding Accepted Checkpoint** | `6a6dc97` (Phase 80 documentation correction) |
| **Audit Nature** | **Audit-Only**: Inspect implementation, verify raw data, reconcile arithmetic, audit timing & regressions, assess classification, and establish audit verdict |
| **Audit Status** | **Complete** |
| **Final Audit Verdict** | **`PASS WITH QUALIFICATIONS`** (Core benchmark experiment, delta calculations, 100% oracle verification, and keep-opt-in release decision are fully verified; secondary baseline totals, depth distribution, and timing figures require factual reconciliation) |

---

## 2. Inventory of Audited Artifacts

The audit inspected the exact committed files at checkpoint `b9c7f04`:
1. Benchmark Runner: [`tools/independent_oracle/benchmarks/optimizer_validation.py`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/benchmarks/optimizer_validation.py)
2. JSON Schema: [`tools/independent_oracle/schema/optimizer_validation.schema.json`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/schema/optimizer_validation.schema.json)
3. Benchmark Unit Tests: [`tools/independent_oracle/tests/test_optimizer_validation_benchmark.py`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/tests/test_optimizer_validation_benchmark.py)
4. Aggregate Benchmark Results: [`tools/independent_oracle/reports/optimizer_validation_aggregate.json`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/reports/optimizer_validation_aggregate.json)
5. Raw Trial Log Records: [`tools/independent_oracle/reports/optimizer_validation_trials.jsonl`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/reports/optimizer_validation_trials.jsonl)
6. CLI Implementation: [`src/cli/compile.rs`](file:///home/ronanki_jitendra/rust/QCC/src/cli/compile.rs)
7. CLI Tests: [`tests/cli.rs`](file:///home/ronanki_jitendra/rust/QCC/tests/cli.rs)
8. Phase 81 Report: [`docs/project-state/reports/phase-081.md`](file:///home/ronanki_jitendra/rust/QCC/docs/project-state/reports/phase-081.md)
9. Progress Ledger: [`docs/project-state/progress.md`](file:///home/ronanki_jitendra/rust/QCC/docs/project-state/progress.md)

---

## 3. Reconciliation of Compilation and Oracle Trial Counts

The report claims 130 workloads evaluated under paired configurations A and B with 3 repeated trials per configuration. The audit inspected the execution pipeline and raw trial logs to verify trial accounting.

### Exact Process Execution Counts

| Entity | Count | Details | Status |
|---|:---:|---|:---:|
| **Canonical Workloads Evaluated** | **130** | 82 positive cancellation opportunity cases + 48 stratified controls | **PASS** |
| **Control A Timed Trials** | **390** | 130 workloads $\times$ 3 repeated trials | **PASS** |
| **Treatment B Timed Trials** | **390** | 130 workloads $\times$ 3 repeated trials | **PASS** |
| **Total Timed Compilation Trials** | **780** | Exactly 780 measured compilation process executions recorded in `trials.jsonl` | **PASS** |
| **Warm-up Compilations (Unmeasured)** | **260** | 130 Control warm-up + 130 Treatment warm-up (discarded from timing stats) | **PASS** |
| **Total CLI Process Invocations** | **1,040** | 780 timed + 260 warm-up invocations of `qcc compile` | **PASS** |
| **Successful Compilations** | **1,040** | 1,040 / 1,040 succeeded (`returncode == 0`, valid QASM + manifest emitted) | **PASS** |
| **Compilation Failures or Timeouts** | **0** | Zero operational errors, zero syntax/semantic crashes | **PASS** |
| **Distinct Control Output Circuits** | **130** | 130 unique physical circuits for Configuration A | **PASS** |
| **Distinct Treatment Output Circuits** | **130** | 130 unique physical circuits for Configuration B | **PASS** |
| **Treatment B Oracle Simulations** | **130** | 130 representative Treatment physical circuits independently simulated via Qiskit | **PASS** |
| **Treatment B Oracle Passes** | **130** | 130 / 130 verified unitary equivalent under one common global phase | **PASS** |
| **Control A Oracle Simulations** | **130** | 130 representative Control physical circuits independently simulated via Qiskit | **PASS** |
| **Control A Oracle Passes** | **130** | 130 / 130 verified unitary equivalent under one common global phase | **PASS** |
| **Combined Oracle Verification** | **260** | **260 / 260** physical circuits verified (0 failures, 0 mismatches, 0 unsupported, 0 unexecuted) | **PASS** |
| **Total Oracle Failures** | **0** | Zero equivalence failures, zero phase mismatches, zero ancilla violations | **PASS** |

### Clarification of Representative Output Verification

The benchmark ran the independent Qiskit oracle on the representative physical output of each unique workload (`trial_2`).
- **Justification**: QCC is bit-for-bit deterministic. The audit verified across trial runs that repeated compilations of the identical input with identical flags produce identical `sha256` circuit digests. Running expensive Qiskit statevector evolution 3 times on bit-for-bit identical QASM bytes is mathematically redundant.
- **Reporting Clarification**: The report headline "130/130 Oracle PASS" refers to **130 unique physical circuits evaluated and verified** (one per canonical workload), not 780 separate oracle simulations.

### Supplemental Verification: Independent Oracle Verification of Control A

To resolve the verification asymmetry between Treatment B and Control A, the benchmark tooling was augmented with a dedicated verification runner: [`tools/independent_oracle/benchmarks/verify_control_a.py`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/benchmarks/verify_control_a.py).

The independent Qiskit oracle evaluated all 130 Control A representative physical circuits (`trial_2`) against their canonical logical reference under identical strict criteria:
- **Full Physical Statevector Comparison**: Independent Qiskit `Statevector` evolution across all $2^{n_{\text{phys}}}$ states.
- **Canonical Logical Reference**: Canonical reference state independently constructed by Qiskit from benchmark definition.
- **Layout Permutation**: Physical output permuted by initial and final logical-to-physical layout maps (`initial_logical_to_physical` and `final_logical_to_physical`).
- **One Common Global Phase**: Unitary equivalence evaluated under a single common global phase factor ($e^{i\phi}$).
- **ADR-0001 Clean Ancilla Policy**: Every unused physical qubit outside the final layout map starts in $|0\rangle$ and returns to $|0\rangle$ at circuit termination ($\max |a_k| < 10^{-10}$).

**Control A Verification Results**:
- **Workloads Evaluated**: 130 / 130
- **Verification Passes**: 130 / 130 (100.0%)
- **Verification Failures**: 0
- **ADR-0001 Clean Ancilla Compliance**: 130 / 130 (100.0%)
- **Total Execution Duration**: 42.90s
- **Retained Artifact**: [`tools/independent_oracle/reports/control_a_verification_results.json`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/reports/control_a_verification_results.json)
- **Automated Regression Test**: Covered in [`tools/independent_oracle/tests/test_optimizer_validation_benchmark.py`](file:///home/ronanki_jitendra/rust/QCC/tools/independent_oracle/tests/test_optimizer_validation_benchmark.py) (`test_control_a_artifact_complete_and_all_pass`).

Together with Treatment B (130/130 PASS), **combined independent verification is established for 260 / 260 physical circuits** with zero mismatches, zero unsupported cases, and zero unexecuted cases.

---

## 4. Audit of the 538 Physical-Operation Reduction

The audit reconstructed the aggregate difference between Control A and Treatment B from the raw per-workload data.

### Reconciliation Table: Aggregate Metrics

| Metric | Phase 81 Report | Recomputed Raw JSON | Discrepancy | Status |
|---|:---:|:---:|---|:---:|
| **Total Workloads** | 130 | 130 | 0 | **PASS** |
| **Mutating Workloads** | 60 | 60 | 0 | **PASS** |
| **Logical Cancellation Pairs** | 63 | 63 | 0 | **PASS** |
| **Logical Operations Removed** | 126 | 126 | 0 | **PASS** |
| **Control Physical Gates Baseline** | 16,842 | **15,653** | +1,189 drafting artifact | **FAIL (Corrected)** |
| **Treatment Physical Gates Baseline** | 16,304 | **15,115** | +1,189 drafting artifact | **FAIL (Corrected)** |
| **Net Physical Gates Saved** | **538** | **538** | **0 (Exact match)** | **PASS** |
| **Control Physical 2Q Baseline** | 5,461 | **5,138** | +323 drafting artifact | **FAIL (Corrected)** |
| **Treatment Physical 2Q Baseline** | 5,301 | **4,978** | +323 drafting artifact | **FAIL (Corrected)** |
| **Net Physical 2Q Gates Saved** | **160** | **160** | **0 (Exact match)** | **PASS** |
| **Control Router SWAPs Baseline** | 823 | **1,205** | -382 drafting artifact | **FAIL (Corrected)** |
| **Treatment Router SWAPs Baseline** | 817 | **1,191** | -374 drafting artifact | **FAIL (Corrected)** |
| **Net Router SWAPs Saved** | -6 | **-14** | -8 drafting artifact | **FAIL (Corrected)** |
| **Control Scheduled Depth Baseline** | 11,280 | **7,586** | +3,694 drafting artifact | **FAIL (Corrected)** |
| **Treatment Scheduled Depth Baseline** | 11,048 | **7,300** | +3,748 drafting artifact | **FAIL (Corrected)** |
| **Net Scheduled Depth Saved** | -232 | **-286** | -54 drafting artifact | **FAIL (Corrected)** |
| **Total Gates: Improved / Unchanged / Worsened** | 59 / 70 / 1 | 59 / 70 / 1 | 0 (Exact match) | **PASS** |
| **2Q Gates: Improved / Unchanged / Worsened** | 59 / 70 / 1 | **57 / 70 / 3** | Accidentally copied gate line | **FAIL (Corrected)** |
| **SWAPs: Improved / Unchanged / Worsened** | 10 / 117 / 3 | 10 / 117 / 3 | 0 (Exact match) | **PASS** |
| **Depth: Improved / Unchanged / Worsened** | 54 / 73 / 3 | **46 / 78 / 6** | Drafting artifact | **FAIL (Corrected)** |

### Causal Decomposition of the 538 Physical-Operation Savings

The audit traced the exact pipeline mechanism explaining how removing 126 logical operations yielded 538 fewer physical operations without double-counting:

$$\Delta (\text{Total Physical Gates}) = \Delta (\text{Two-Qubit Gates}) + \Delta (\text{Single-Qubit Gates}) = 160 + 378 = 538$$

1. **Direct and Routing Two-Qubit Savings (160 2Q gates)**:
   - **Direct Logical CX/CZ Cancellation**: 126 two-qubit operations were eliminated directly by `CommutativeTwoQubitCancellationPass`.
   - **Router SWAP Contribution**: Across the corpus, router SWAPs were reduced by a net 14 SWAPs ($1,205 - 1,191 = 14$).
     - On standard superconducting targets (`linear`, `ring`), each SWAP decomposes into 3 physical CX gates ($\Delta \text{CX} = 3 \times \Delta \text{SWAP}$).
     - On generic targets (`linear-generic`, `ring-generic`), each SWAP is a native 2-qubit operation ($\Delta \text{2Q} = 1 \times \Delta \text{SWAP}$).
     - Sum across standard targets: $+30\text{ CX saved}$.
     - Sum across generic targets: $+4\text{ 2Q saved}$.
     - Net two-qubit routing contribution: $30 + 4 = 34\text{ gates}$.
   - **Reconciliation**: $126\text{ (direct logical CX)} + 34\text{ (net routing 2Q)} = \mathbf{160\text{ physical two-qubit gates saved}}$.
2. **Cascading Downstream Single-Qubit Savings (378 single-qubit gates)**:
   - **Formal Classification**: `Observed aggregate reduction; exact stage-level causal attribution unavailable.`
   - **Qualitative Mechanisms Identified**:
     - *Unblocking Subsequent Optimizer Passes*: Case introspection (e.g. `c0039`) confirms that removing intervening two-qubit gates exposes adjacent single-qubit rotations on previously separated wires. In the fixed-point loop, `rotation_merge` immediately merged previously blocked rotation pairs, and `one_qubit_fusion` fused newly adjacent rotations into single Euler blocks.
     - *Basis Translation Amplification*: In QCC's native superconducting basis, un-fused single-qubit rotations synthesize into multi-pulse sequences ($R_z(\theta)$, $\sqrt{X}$, $R_z(\lambda)$). Eliminating logical single-qubit gates upstream saves multiple native hardware pulses downstream.
     - *SWAP Decomposition Single-Qubit Overhead*: Each avoided router SWAP (net 14 SWAPs saved across the corpus) avoids single-qubit pulses required for hardware decomposition or direction correction.
   - **Stage-Level Attribution Limitation**: Because QCC's compilation trials retained final physical artifacts and terminal manifests, but did not serialize intermediate circuit snapshots after each individual pass for all 130 workloads, the quantitative partitioning of the 378 single-qubit gate reduction across specific compiler stages cannot be precisely reconstructed. It is therefore strictly reported as an observed aggregate difference between Control A and Treatment B final outputs.
3. **Metric Definition**:
   The 538 figure is **not** an intermediate-pass sum. It is the exact, un-double-counted difference between the final physical operation counts of Configuration A ($15,653$) and Configuration B ($15,115$).

---

## 5. Audit of Compilation Timing

### Timing Measurement Boundaries & Setup
- **Timing Boundary**: The benchmark measures wall-clock duration around `subprocess.run([binary, "compile", ...])`.
- **Included**: Process creation, binary loading, OpenQASM 3 parsing, logical optimization, basis translation, routing, scheduling, layout-aware verification, physical QASM serialization, manifest generation, and pass report generation.
- **Excluded**: Independent Qiskit oracle verification (run in Python after compilation terminates).
- **Process Startup**: Included in the measured time.

### Statistical Analysis of Raw Observations (390 observations each)

| Metric | Control (A) | Treatment (B) | Paired Difference ($B - A$) |
|---|:---:|:---:|:---:|
| **All Observations Mean** | 25.055 ms | 25.283 ms | +0.228 ms |
| **All Observations Median** | 14.387 ms | 14.028 ms | -0.359 ms |
| **All Observations Stdev** | 23.190 ms | 23.386 ms | 2.391 ms |
| **All Observations P95** | 67.393 ms | 67.479 ms | +0.086 ms |
| **Per-Workload Medians Mean** | 25.075 ms | 25.184 ms | **+0.109 ms** |
| **Per-Workload Medians Median** | 14.155 ms | 13.575 ms | **-0.008 ms** |
| **Runtime Multiplier** | 1.000x | **1.004x** | **+0.4% overhead** |
| **Mean Pass Duration (`commutative_cancellation`)** | — | **0.151 ms** | **< 0.6% of compilation** |

### Per-Family Paired Timing Breakdown

| Circuit Family | Workloads ($N$) | Control Mean | Treatment Mean | Paired Mean Delta | Paired Median Delta |
|---|:---:|:---:|:---:|:---:|:---:|
| `cx_asymmetric_pairs` | 46 | 40.141 ms | 40.979 ms | +0.838 ms | +1.002 ms |
| `ghz_entanglement` | 6 | 6.319 ms | 5.642 ms | -0.678 ms | -0.287 ms |
| `global_and_relative_phase` | 6 | 9.056 ms | 8.678 ms | -0.377 ms | -0.035 ms |
| `logical_swap_stress` | 17 | 20.783 ms | 19.939 ms | -0.845 ms | -1.379 ms |
| `parameterized_bound` | 16 | 6.280 ms | 6.135 ms | -0.145 ms | -0.163 ms |
| `qft_like_nonadjacent` | 6 | 15.441 ms | 16.205 ms | +0.765 ms | +0.438 ms |
| `random_layered_dense` | 27 | 27.731 ms | 27.415 ms | -0.316 ms | -0.006 ms |
| `single_qubit_rotations` | 6 | 4.322 ms | 4.747 ms | +0.425 ms | +0.487 ms |

### Timing Audit Finding & Qualification
- The reported $21.68\text{ ms}$ and $21.72\text{ ms}$ numbers were artifacts from an earlier draft slice; the true per-workload median mean is **$25.08\text{ ms}$** (Control) vs **$25.18\text{ ms}$** (Treatment), with a paired mean delta of **$+0.109\text{ ms}$** ($1.004\times$).
- **Statistical Qualification**: With $N = 3$ trials per configuration, the trial standard deviation ($\approx 2.4\text{ ms}$) exceeds the paired mean difference ($0.11\text{ ms}$). These 3 trials provide preliminary evidence that `CommutativeTwoQubitCancellationPass` does not introduce macro-level performance degradation, but they cannot prove microsecond-level timing differences with statistical significance.

---

## 6. Audit of Routing Regressions

The audit inspected the three workloads exhibiting SWAP regressions: `c0140`, `c0333`, and `c0665`.

| Workload ID | Family | Target | Initial Layout Changed? | Control SWAPs | Treatment SWAPs | Net SWAP Delta | Net Gate Delta | Net 2Q Delta |
|---|---|---|:---:|:---:|:---:|:---:|:---:|:---:|
| `c0140` | `logical_swap_stress` | `ring` (7q) | **YES** | 3 | 4 | +1 | -1 | +1 |
| `c0333` | `random_layered_dense` | `ring` (7q) | **NO** | 12 | 14 | +2 | -11 | +4 |
| `c0665` | `cx_asymmetric_pairs` | `ring-generic` (7q) | **YES** | 35 | 39 | +4 | +2 | +2 |

### Detailed Investigation of Causal Mechanisms

1. **Initial Placement Divergence (`c0140`, `c0665`)**:
   - In `c0140`, Control A initial layout was `{0:0, 1:4, 2:6, 3:5, 4:2, 5:3, 6:1}`, while Treatment B initial layout was `{0:6, 1:3, 2:1, 3:2, 4:0, 5:4, 6:5}`.
   - In `c0665`, Control A initial layout was `{0:3, 1:2, 2:1, 3:0, 4:6, 5:5, 6:4}`, while Treatment B initial layout was `{0:4, 1:5, 2:6, 3:0, 4:1, 5:2, 6:3}`.
   - **Root Cause**: `PlacementStrategy::Weighted` constructs an interaction frequency graph across all two-qubit gates. Removing two logical CX gates changes the graph edge weights, causing the placement algorithm to select a different initial physical qubit assignment before routing begins.
2. **SABRE Front-Layer Heuristic Divergence (`c0333`)**:
   - In `c0333`, initial layouts were identical: `{0:5, 1:0, 2:4, 3:1, 4:6, 5:3, 6:2}`.
   - **Root Cause**: Removing two logical CX gates altered the front-layer DAG dependency set at early routing steps. The greedy SABRE heuristic lookahead evaluated distances over a different operation set, selecting an alternate sequence of routing SWAPs that proved slightly less efficient for later layers.
3. **Correction to Report**:
   The Phase 81 report described the regression mechanism as purely "SABRE front-layer alteration." The audit establishes that **interaction-graph placement divergence is the primary driver in 2 out of 3 cases**, while front-layer lookahead divergence drove the remaining case.
4. **Verification of Linear-Topology Claim**:
   The claim that all linear-topology workloads had zero SWAP regressions was verified: across all 65 linear-topology workloads (`linear`: 39, `linear-generic`: 26), exactly **zero** workloads experienced SWAP increases (+3 improved, 62 unchanged, 0 regressed).

---

## 7. Audit of Independent Correctness

The audit verified the independent verification architecture against QCC non-negotiable project invariants:

1. **Oracle Independence**:
   - Python/Qiskit tooling lives strictly in `tools/independent_oracle/`.
   - Never imports or invokes QCC's internal Rust Phase 10 verifier.
   - Construct canonical logical reference independently in Qiskit via `run_logical_reference()`.
2. **Full Physical Statevector Comparison**:
   - Evaluates full $2^{n_{\text{phys}}}$ statevector evolution via Qiskit's `Statevector`.
   - Embeds logical reference with unused ancillas initialized strictly to $\|0\rangle$.
   - Validates that every physical qubit outside the final layout map returns to $\|0\rangle$ at circuit termination ($\|a_k\| < 10^{-10}$). Tracing out unused qubits is prohibited.
3. **One Single Common Global Phase**:
   - Compares physical statevector against logical reference under one common global phase: $e^{i\phi} |\psi_{\text{ref}}\rangle$. Independent phase per basis state is rejected.
4. **Strict Phase Accounting Separation**:
   - Common-global-phase equivalence is authoritative.
   - Informational strict phase accounting reported 114 `PASS`, 16 `UNVERIFIED` (symbolic bindings), 0 `MISMATCH`.
5. **Target Legality**:
   - Every physical circuit passed `TargetLegalityValidator` inside `qcc compile` before manifest emission.

---

## 8. Audit of CLI and Scope Changes

The audit inspected the Git diff of checkpoint `b9c7f04`:
1. `src/cli/compile.rs`:
   - Adds `pub enable_commutative_cancellation: bool` to `CompileOptions` with default `false`.
   - Adds `--enable-commutative-cancellation` CLI flag.
   - Wires flag to `compiler_config.optimization.enable_commutative_cancellation` in target compilation and `opt_config.enable_commutative_cancellation` in logical compilation.
   - Default behavior is completely unchanged (`false`).
2. `tests/cli.rs`:
   - Adds 2 comprehensive CLI integration tests (`test_cli_compile_enable_commutative_cancellation_logical` and `test_cli_compile_enable_commutative_cancellation_target`).
3. Zero Scope Creep:
   - Zero Phase 82 hardware profile work introduced.
   - Zero modifications to Phase 80 cancellation algorithm.
   - Zero tuning of SABRE routing parameters.

---

## 9. Outcome Classification & Release Recommendation Assessment

### Evaluation of `C_MIXED_RESULT`
- **Substantiation**: The empirical data proves clear physical benefit on 59 workloads (538 physical gates saved, 160 two-qubit gates saved, 10 SWAP improvements), but also establishes 3 secondary routing regressions (+1 to +4 SWAPs) and 1 net physical gate regression (+2 gates).
- **Compliance**: Classifying this outcome as `A_CLEAR_BENEFIT` would be a false positive; classifying as `D_NO_MEASURABLE_BENEFIT` or `E_NEGATIVE_RESULT` would ignore the substantial positive gains across 59 circuits. `C_MIXED_RESULT` is strictly accurate and evidence-based.

### Evaluation of Keep Opt-In Recommendation
- The recommendation to keep `enable_commutative_cancellation = false` by default is fully supported by the audited evidence. Enabling the pass by default would regress SWAP counts on certain dense 7-qubit ring topologies.
- Default enablement should remain deferred until multi-candidate routing co-design or topology-aware thresholding is introduced.

---

## 10. Summary of Corrections to Phase 81 Documentation

The following factual corrections have been identified and reconciled:
1. **Total Physical Gates Baseline**: Updated from 16,842 (Control) / 16,304 (Treatment) to **15,653** (Control) / **15,115** (Treatment); net savings remains exact at **-538 gates**.
2. **Two-Qubit Gates Baseline**: Updated from 5,461 (Control) / 5,301 (Treatment) to **5,138** (Control) / **4,978** (Treatment); net savings remains exact at **-160 gates**.
3. **Two-Qubit Distribution**: Corrected from `59 / 70 / 1` to **57 improved / 70 unchanged / 3 worsened**.
4. **Router SWAPs Baseline**: Updated from 823 (Control) / 817 (Treatment) to **1,205** (Control) / **1,191** (Treatment); net router SWAPs saved is **-14 SWAPs**.
5. **Scheduled Depth**: Updated from 11,280 / 11,048 (-232) to **7,586** (Control) / **7,300** (Treatment), net delta **-286 cycles** (-3.77%); distribution is **46 improved / 78 unchanged / 6 worsened**.
6. **Timing Statistics**: Updated from $21.68\text{ ms}$ / $21.72\text{ ms}$ to per-workload median mean of **$25.08\text{ ms}$** (Control) / **$25.18\text{ ms}$** (Treatment), paired mean delta **$+0.109\text{ ms}$** ($1.004\times$).
7. **Regression Mechanism**: Clarified that interaction-graph placement divergence (`PlacementStrategy::Weighted`) drove 2 regressions (`c0140`, `c0665`), while SABRE front-layer lookahead divergence drove 1 regression (`c0333`).
8. **Independent Verification of Control A**: Verified all 130 Control A representative physical circuits via the independent Qiskit oracle, establishing **260/260 combined passes** (130 Control A + 130 Treatment B) with zero mismatches and 100% ADR-0001 clean-ancilla compliance.
9. **Single-Qubit Attribution Classification**: Formally classified the 378 non-two-qubit gate reduction as an `Observed aggregate reduction; exact stage-level causal attribution unavailable.`

---

## 11. Final Audit Verdict

> **FINAL AUDIT VERDICT: `PASS WITH QUALIFICATIONS`**
>
> The core Phase 81 comparative benchmark experiment is mathematically sound, reproducible, and compliant with all QCC non-negotiable project invariants. The 538 physical-gate reduction, 160 two-qubit reduction, 100% independent oracle equivalence across all physical outputs (**260/260 PASS**: 130 Control A + 130 Treatment B), 100% ADR-0001 clean-ancilla compliance, `C_MIXED_RESULT` classification, and keep-opt-in release decision are fully verified by raw evidence.
>
> **Audit Qualifications**:
> 1. *Non-Two-Qubit Reduction Attribution*: The 378 single-qubit gate reduction is formally classified as an `Observed aggregate reduction; exact stage-level causal attribution unavailable.` While qualitative mechanisms (downstream rotation merging, Euler fusion, SWAP decomposition pulses) are established, quantitative stage-by-stage decomposition across all 130 workloads is not recorded in retained artifacts.
> 2. *Compilation Timing Precision*: The 3-trial timing data ($1.004\times$ multiplier, $+0.109\text{ ms}$ paired median mean delta) provides preliminary evidence demonstrating the absence of macro-level compiler slowdown, rather than microsecond-level statistical certainty.

