# Milestone M7 Implementation Report — Independent Verification Oracle v0.2 (Phases 60–70)

```text
Milestone: Milestone M7 (Independent Verification Oracle v0.2)
Phases Covered: Phases 60–70 (Executed in 5 Practical Sessions)
Status: Completed & Accepted
Roadmap: QCC Engineering Roadmap v3.1
Artifacts: tools/independent_oracle/, .github/workflows/independent-oracle.yml, ADR-0003, ADR-0004
```

---

## 1. Executive Summary

Milestone M7 inaugurated **Roadmap v3.1**, building an isolated, automated Python/Qiskit verification oracle under `tools/independent_oracle/`. The oracle independently reconstructs logical references, executes QCC black-box, simulates physical OpenQASM 3 circuits in Qiskit, normalizes layout and little-endian qubit ordering, verifies full physical statevectors against one single common global phase, enforces ADR-0001 clean-ancilla compliance, and preserves reproducible failure artifacts. Python and Qiskit are strictly isolated to dev/CI tooling and are **never** dependencies of the Rust compiler crate.

---

## 2. Five Implementation Sessions & Deliverables

| Session | Phases Covered | Core Deliverables & Verification Artifacts |
|:---:|:---:|---|
| **1** | 60, 63 (manifest) | Established [ADR-0003](../../decisions/ADR-0003-independent-oracle-and-manifest-contract.md) and [`qcc.compile_manifest` v1.0.0](../../../tools/independent_oracle/schema/compile_manifest.schema.json). Implemented Rust CLI flag `--emit-manifest <FILE>` generating manifests directly from compiler data structures (`CompilationResult`). |
| **2** | 61, 63 (QASM import) | Pinned Python toolchain: `python==3.12`, `qiskit==2.6.2`, `qiskit-qasm3-import==0.6.0`, `openqasm3==1.0.1`. Established [ADR-0004](../../decisions/ADR-0004-qasm3-importer-register-collision-fallback.md) implementing an AST register alpha-renaming adapter for Qiskit compatibility. Verified 5 real QCC OpenQASM 3 fixtures. |
| **3** | 62, 64–66 | Built 10 neutral canonical test fixtures and independent runners. Implemented statevector comparator with layout/endianness normalization, single common global phase verification ($\epsilon < 10^{-12}$), ADR-0001 full-state clean-ancilla checking, and informational strict phase accounting. |
| **4** | 67, 68, 70 (baseline) | **Fault Injection**: Injected 40 mutations across 17 fault families; caught **40 / 40 faults (100%)**.<br>**Scale-Up Corpus**: Expanded to **1,080 deterministic cases**; achieved **1,080 / 1,080 PASS**.<br>**Matched-Basis Qiskit Baseline**: Compared QCC vs. Qiskit O2 across 18 workloads $\times$ 5 trials on `{rz, sx, cx}` (90/90 PASS; identified optimization/routing gap motivating M9 & M10). |
| **5** | 69, 70 (acceptance) | Integrated CI workflow [`.github/workflows/independent-oracle.yml`](../../../.github/workflows/independent-oracle.yml) with 30-day artifact retention. Built automated CI check runner [`ci_check.py`](../../../tools/independent_oracle/ci_check.py). Milestone M7 accepted. |

---

## 3. Fair Matched-Basis Qiskit Baseline Summary

Compared QCC against Qiskit `PassManager` (level 2) across 18 workloads $\times$ 5 trials on `{rz, sx, cx}`:
- **Verification**: Both compilers achieved **90 / 90 PASS** independent verification.
- **Fairness Disclosures**: QCC ran 1 deterministic SABRE configuration; Qiskit ran 20 internal randomized trials per call.
- **Median Results (QCC vs. Qiskit Win / Tie / Loss)**: CX count `0 / 5 / 13`, Depth `1 / 2 / 15`, directly motivating Milestones M9 and M10.

| Workload | Target | Width | QCC Median (CX / Depth / SWAPs) | Qiskit O2 Median (CX / Depth) | Verification |
|---|:---:|:---:|:---:|:---:|:---:|
| `03_bell_pair` | `linear` | $2 \to 2$ | `1 / 4 / 0` | `1 / 4` | **PASS / PASS** |
| `04_logical_swap` | `linear` | $2 \to 2$ | `3 / 8 / 0` | `0 / 3` | **PASS / PASS** |
| `05_nontrivial_routing` | `linear` | $3 \to 3$ | `6 / 9 / 1` | `4 / 14` | **PASS / PASS** |
| `07_qft4_showcase_linear` | `linear` | $4 \to 4$ | `27 / 38 / 3` | `15 / 34` | **PASS / PASS** |
| `08_qft4_showcase_ring` | `ring` | $4 \to 4$ | `24 / 36 / 2` | `16 / 29` | **PASS / PASS** |
| `c0037_dense_6q_linear` | `linear` | $6 \to 6$ | `60 / 97 / 13` | `48 / 81` | **PASS / PASS** |
| `c0069_dense_4q_linear` | `linear` | $4 \to 4$ | `69 / 137 / 14` | `52 / 110` | **PASS / PASS** |

---

## 4. Verification Evidence & Exit Gate

- **Corpus Pass Rate**: 1,080 / 1,080 cases passed (836 exhaustive full-$2^m$ unitary comparisons + 244 sampled checks; max amplitude error $2.29 \times 10^{-15}$).
- **Fault Sensitivity**: 40 / 40 injected faults caught with zero misclassifications.
- **Exit Gate**: All 5 sessions completed, all tests passed with zero warnings. Milestone M7 accepted.
