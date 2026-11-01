# QCC Roadmap Phase Index (Phases 1–105)

This index provides a complete, searchable summary of all 105 implementation phases of the QCC (Quantum Circuit Compiler) engineering roadmap.

## Milestone Overview

| Milestone | Phases | Scope | Status | Phase Details File |
|---|:---:|---|:---:|---|
| **M1** | 1–11 | Bell Vertical Slice (v1.0 Critical Path) | Not Started | [phases-001-011.md](./phases-001-011.md) |
| **M2** | 12–27 | Compiler Foundations and OpenQASM Frontend (v1.0 Critical Path) | Not Started | [phases-012-027.md](./phases-012-027.md) |
| **M3** | 28–36 | Verified Logical Optimizer (v1.0 Critical Path) | Not Started | [phases-028-036.md](./phases-028-036.md) |
| **M4** | 37–43 | Generic Target and Basis Translation (v1.0 Critical Path) | Not Started | [phases-037-043.md](./phases-037-043.md) |
| **M5** | 44–53 | Logical-to-Physical Mapping and Routing (v1.0 Critical Path) | Not Started | [phases-044-053.md](./phases-044-053.md) |
| **M6** | 54–59 | Scheduling and Integrated Compilation (v1.0 Critical Path) | Not Started | [phases-054-059.md](./phases-054-059.md) |
| **M7** | 60–67 | Dynamic Simulation and Non-Unitary Semantics (v1.0 Critical Path) | Not Started | [phases-060-067.md](./phases-060-067.md) |
| **M8** | 68–82 | Differential Validation and Interoperability (v1.0 Critical Path) | Not Started | [phases-068-082.md](./phases-068-082.md) |
| **M9** | 83–89 | Developer Experience and Release Engineering (v1.0 Critical Path) | Not Started | [phases-083-089.md](./phases-083-089.md) |
| **Research** | 90–105 | Hardware Awareness, CFG, KAK and MPS (Advanced Research) | Not Started | [phases-090-105.md](./phases-090-105.md) |

---

## Complete 105-Phase Index

| Phase # | Exact Phase Title | Milestone | Prerequisites | Effort | Exit Gate Summary | Source Page | Detail Link |
|:---:|---|:---:|---|:---:|---|:---:|:---:|
| 1 | Repository scaffold, CI and error infrastructure | `M1` | None. | **M** | CI passes on a clean checkout; CLI version command and smoke test work. | Page 6 | [phases-001-011.md#phase-001](./phases-001-011.md) |
| 2 | Quantum semantics and gate definitions | `M1` | Phase 1. | **M** | Semantic convention document and gate matrix tests pass. | Page 6 | [phases-001-011.md#phase-002](./phases-001-011.md) |
| 3 | Single-qubit in-place statevector kernels | `M1` | Phase 2. | **M** | Randomized one-qubit tests pass for every valid target index. | Page 7 | [phases-001-011.md#phase-003](./phases-001-011.md) |
| 4 | Two-qubit and controlled-gate kernels | `M1` | Phase 3. | **L** | Two-qubit kernels pass basis and randomized superposition tests. | Page 7 | [phases-001-011.md#phase-004](./phases-001-011.md) |
| 5 | Terminal measurement and seeded sampling | `M1` | Phases 3–4. | **M** | Terminal sampling passes deterministic and statistical tests. | Page 8 | [phases-001-011.md#phase-005](./phases-001-011.md) |
| 6 | DAG-based QccIR | `M1` | Phases 1–2. | **XL** | All QccIR invariants are executable checks and representative circuits s... | Page 8 | [phases-001-011.md#phase-006](./phases-001-011.md) |
| 7 | Rust circuit builder API | `M1` | Phase 6. | **M** | Builder creates a valid QccIR circuit and returns structured errors for ... | Page 9 | [phases-001-011.md#phase-007](./phases-001-011.md) |
| 8 | QccIR execution adapter | `M1` | Phases 3–7. | **L** | DAG execution matches the reference gate-list simulator for supported ci... | Page 9 | [phases-001-011.md#phase-008](./phases-001-011.md) |
| 9 | OpenQASM 3 emitter | `M1` | Phases 6–7. | **M** | Bell output parses independently and preserves the supported circuit sem... | Page 10 | [phases-001-011.md#phase-009](./phases-001-011.md) |
| 10 | Global-phase-aware unitary equivalence checker | `M1` | Phases 2–4 and 8. | **XL** | Known equivalent and deliberately inequivalent circuits are reliably dis... | Page 10 | [phases-001-011.md#phase-010](./phases-001-011.md) |
| 11 | Bell vertical-slice acceptance | `M1` | Phases 1–10. | **M** | End-to-end Bell acceptance passes from a clean checkout. Milestone M2 — ... | Page 11 | [phases-001-011.md#phase-011](./phases-001-011.md) |
| 12 | Pass manager interface | `M2` | Phase 6. | **L** | Pass manager contract is stable and tested. | Page 11 | [phases-012-027.md#phase-012](./phases-012-027.md) |
| 13 | Transactional DAG rewrite infrastructure | `M2` | Phases 6 and 12. | **XL** | Invalid rewrites are rejected without leaving a corrupted graph. | Page 12 | [phases-012-027.md#phase-013](./phases-012-027.md) |
| 14 | Gate algebra and inverse registry | `M2` | Phases 2 and 10. | **M** | All enabled identities pass independent matrix checks. | Page 12 | [phases-012-027.md#phase-014](./phases-012-027.md) |
| 15 | OpenQASM parser dependency evaluation | `M2` | Phase 11. | **M** | ADR documents selected parser, supported subset, license and fallback. | Page 13 | [phases-012-027.md#phase-015](./phases-012-027.md) |
| 16 | QASM frontend adapter boundary | `M2` | Phase 15. | **M** | Frontend dependency can be swapped without modifying QccIR. | Page 13 | [phases-012-027.md#phase-016](./phases-012-027.md) |
| 17 | QASM source buffer and lexer | `M2` | Phase 16 if parser is handwritten; otherwise test adapter lexer behavior. | **L** | Lexer corpus and fuzz smoke tests pass. | Page 14 | [phases-012-027.md#phase-017](./phases-012-027.md) |
| 18 | OpenQASM AST model | `M2` | Phase 17 or selected oq3 adapter. | **L** | AST covers documented input subset. | Page 14 | [phases-012-027.md#phase-018](./phases-012-027.md) |
| 19 | QASM register declaration parsing | `M2` | Phase 18. | **M** | Register syntax and semantic bounds tests pass. | Page 14 | [phases-012-027.md#phase-019](./phases-012-027.md) |
| 20 | Standard gate statement parsing | `M2` | Phases 18–19. | **M** | Gate parser tests pass for documented subset. | Page 15 | [phases-012-027.md#phase-020](./phases-012-027.md) |
| 21 | Parameter expression parser | `M2` | Phases 18 and 20. | **L** | Expressions round-trip or evaluate with explicit error behavior. | Page 15 | [phases-012-027.md#phase-021](./phases-012-027.md) |
| 22 | Measurement, reset and barrier parser | `M2` | Phases 19–21. | **M** | Non-unitary syntax tests pass. | Page 16 | [phases-012-027.md#phase-022](./phases-012-027.md) |
| 23 | Semantic analysis and diagnostics | `M2` | Phases 19–22. | **L** | Semantic negative corpus passes. | Page 16 | [phases-012-027.md#phase-023](./phases-012-027.md) |
| 24 | Unsupported syntax detection | `M2` | Phase 23. | **M** | Unsupported-feature conformance tests pass. | Page 16 | [phases-012-027.md#phase-024](./phases-012-027.md) |
| 25 | AST-to-QccIR lowering | `M2` | Phases 6, 7 and 18–24. | **L** | Lowering semantic equivalence passes. | Page 17 | [phases-012-027.md#phase-025](./phases-012-027.md) |
| 26 | QASM import/export round-trip | `M2` | Phases 9 and 15–25. | **L** | Independent parse and semantic round-trip suite passes. | Page 17 | [phases-012-027.md#phase-026](./phases-012-027.md) |
| 27 | QASM fuzzing, conformance and frontend CLI | `M2` | Phases 17–26. | **L** | Frontend conformance suite and CLI smoke tests pass. Milestone M3 — Veri... | Page 17 | [phases-012-027.md#phase-027](./phases-012-027.md) |
| 28 | Identity-gate elimination | `M3` | Phases 10, 12–14 and 27. | **S** | Every enabled identity rule passes randomized equivalence. | Page 18 | [phases-028-036.md#phase-028](./phases-028-036.md) |
| 29 | Inverse-gate cancellation | `M3` | Phase 28. | **M** | Pass passes unitary equivalence and DAG invariants. | Page 18 | [phases-028-036.md#phase-029](./phases-028-036.md) |
| 30 | Same-axis rotation merging | `M3` | Phase 29. | **M** | Rotation merge passes randomized unitary checks. | Page 19 | [phases-028-036.md#phase-030](./phases-028-036.md) |
| 31 | Single-qubit Euler decomposition | `M3` | Phases 10 and 30. | **XL** | Matrix reconstruction and phase tests pass over randomized cases. | Page 19 | [phases-028-036.md#phase-031](./phases-028-036.md) |
| 32 | One-qubit gate fusion | `M3` | Phases 13, 30–31. | **L** | Random fused blocks pass common-phase equivalence. | Page 19 | [phases-028-036.md#phase-032](./phases-028-036.md) |
| 33 | Commutation analysis rules | `M3` | Phases 10, 14 and 12. | **L** | All commutation rules pass pairwise matrix tests. | Page 20 | [phases-028-036.md#phase-033](./phases-028-036.md) |
| 34 | DAG-aware commutation rewriting | `M3` | Phases 13 and 33. | **XL** | Pass is deterministic, bounded and equivalence-tested. | Page 20 | [phases-028-036.md#phase-034](./phases-028-036.md) |
| 35 | Optimizer integration and reports | `M3` | Phases 12–14 and 28–34. | **L** | Default optimizer pipeline produces reproducible validated output. Miles... | Page 21 | [phases-028-036.md#phase-035](./phases-028-036.md) |
| 36 | Canonical logical gate catalog | `M3` | Phases 2, 14 and 35. | **M** | Catalog is stable and covered by signature tests. | Page 21 | [phases-028-036.md#phase-036](./phases-028-036.md) |
| 37 | Generic Target data model | `M4` | Phases 24–25 and 36. | **L** | Synthetic target fixtures validate correctly. | Page 21 | [phases-037-043.md#phase-037](./phases-037-043.md) |
| 38 | Target capability queries and cost models | `M4` | Phase 37. | **M** | Capability and cost queries pass fixtures with partial metadata. | Page 22 | [phases-037-043.md#phase-038](./phases-037-043.md) |
| 39 | Fixed decomposition template library | `M4` | Phases 10, 27, 31 and 37. | **L** | Template library passes full local matrix comparison. | Page 22 | [phases-037-043.md#phase-039](./phases-037-043.md) |
| 40 | Native single-qubit synthesis | `M4` | Phases 31, 37 and 38. | **XL** | Random unitary tests and target legality checks pass. | Page 23 | [phases-037-043.md#phase-040](./phases-037-043.md) |
| 41 | Basis translation engine | `M4` | Phases 37–40. | **XL** | Translated circuits pass local equivalence and target operation checks. | Page 23 | [phases-037-043.md#phase-041](./phases-037-043.md) |
| 42 | Two-qubit direction correction | `M4` | Phases 37, 39 and 41. | **M** | Direction correction tests pass for both orientations. | Page 23 | [phases-037-043.md#phase-042](./phases-037-043.md) |
| 43 | Final target legality validator | `M4` | Phases 37–42. | **M** | All intentionally illegal target fixtures fail with useful diagnostics. ... | Page 24 | [phases-037-043.md#phase-043](./phases-037-043.md) |
| 44 | Logical/physical layout model | `M5` | Phases 37–43. | **M** | Layout invariants are validated and round-trip correctly. | Page 24 | [phases-044-053.md#phase-044](./phases-044-053.md) |
| 45 | Layout-aware equivalence | `M5` | Phases 10 and 44. | **L** | All router tests use layout-aware semantic comparison. | Page 25 | [phases-044-053.md#phase-045](./phases-044-053.md) |
| 46 | Initial placement algorithms | `M5` | Phases 37, 38 and 44. | **L** | Placement fixtures yield reproducible valid maps. | Page 25 | [phases-044-053.md#phase-046](./phases-044-053.md) |
| 47 | Baseline shortest-path router | `M5` | Phases 39, 43–46. | **XL** | Baseline router passes topology and layout-aware semantic tests. | Page 25 | [phases-044-053.md#phase-047](./phases-044-053.md) |
| 48 | SWAP insertion and dependency maintenance | `M5` | Phases 13, 44–47. | **L** | Each route rewrite passes QccIR validation and layout-aware equivalence. | Page 26 | [phases-044-053.md#phase-048](./phases-044-053.md) |
| 49 | Routing objectives and metrics | `M5` | Phases 44–48. | **M** | Baseline benchmark corpus is reproducible. | Page 26 | [phases-044-053.md#phase-049](./phases-044-053.md) |
| 50 | SABRE front-layer analysis | `M5` | Phases 44–49. | **L** | Front-layer and extended-set analysis is independently tested. | Page 26 | [phases-044-053.md#phase-050](./phases-044-053.md) |
| 51 | SABRE heuristic, candidate scoring and decay | `M5` | Phase 50. | **XL** | Heuristic fixture rankings and repeatability tests pass. | Page 27 | [phases-044-053.md#phase-051](./phases-044-053.md) |
| 52 | Bidirectional SABRE and layout refinement | `M5` | Phase 51. | **XL** | Bidirectional routing is validated and its layout transformations are co... | Page 27 | [phases-044-053.md#phase-052](./phases-044-053.md) |
| 53 | Router validation and comparative benchmarks | `M5` | Phases 47–52. | **XL** | Routing tests and benchmark report are reproducible. Milestone M6 — Sche... | Page 28 | [phases-044-053.md#phase-053](./phases-044-053.md) |
| 54 | Target timing and resource model | `M6` | Phases 37–38 and 53. | **L** | Timing and resource fixtures validate and preserve unknown data. | Page 28 | [phases-054-059.md#phase-054](./phases-054-059.md) |
| 55 | ASAP scheduler | `M6` | Phases 43, 53–54. | **L** | ASAP result passes independent schedule validation. | Page 29 | [phases-054-059.md#phase-055](./phases-054-059.md) |
| 56 | ALAP scheduler | `M6` | Phase 55. | **L** | ALAP schedule passes timing and dependency validation. | Page 29 | [phases-054-059.md#phase-056](./phases-054-059.md) |
| 57 | Resource-constrained scheduling | `M6` | Phases 54–56. | **XL** | Synthetic resource-conflict schedules pass validator. | Page 29 | [phases-054-059.md#phase-057](./phases-054-059.md) |
| 58 | Schedule serialization and visualization data | `M6` | Phases 55–57. | **M** | Schedule output round-trips and matches validator. | Page 30 | [phases-054-059.md#phase-058](./phases-054-059.md) |
| 59 | Integrated compilation pipeline | `M6` | Phases 28–58. | **XL** | End-to-end compilation output is legal, semantically verified and option... | Page 30 | [phases-054-059.md#phase-059](./phases-054-059.md) |
| 60 | Mid-circuit measurement semantics | `M7` | Phases 5–6 and 59. | **L** | Measurement/collapse tests pass with explicit classical result. | Page 30 | [phases-060-067.md#phase-060](./phases-060-067.md) |
| 61 | Trajectory simulator | `M7` | Phase 60. | **XL** | Dynamic trajectory mode passes deterministic and statistical tests. | Page 31 | [phases-060-067.md#phase-061](./phases-060-067.md) |
| 62 | Exact branch enumeration | `M7` | Phases 60–61. | **XL** | Branch probabilities and states match analytical small examples. | Page 31 | [phases-060-067.md#phase-062](./phases-060-067.md) |
| 63 | Reset semantics | `M7` | Phases 60–62. | **L** | Trajectory and exact reset tests pass. | Page 32 | [phases-060-067.md#phase-063](./phases-060-067.md) |
| 64 | Non-unitary semantic verification | `M7` | Phases 60–63 and 10. | **XL** | Non-unitary tests reject inappropriate verifier modes and validate suppo... | Page 32 | [phases-060-067.md#phase-064](./phases-060-067.md) |
| 65 | Density-matrix reference simulator | `M7` | Phase 64. | **XL** | Reference engine passes small-system math tests under explicit limits. | Page 32 | [phases-060-067.md#phase-065](./phases-060-067.md) |
| 66 | Basic noise channel framework | `M7` | Phases 37, 64–65. | **XL** | Channel and density-matrix tests pass. | Page 33 | [phases-060-067.md#phase-066](./phases-060-067.md) |
| 67 | Simulator performance and resource limits | `M7` | Phases 3–5 and 60–66. | **L** | Resource estimator and benchmark baseline are checked in. Milestone M8 —... | Page 33 | [phases-060-067.md#phase-067](./phases-060-067.md) |
| 68 | Qiskit Aer differential testing | `M8` | Phases 11, 26 and 67. | **L** | Shared gate-set corpus passes or discrepancies are isolated and documented. | Page 34 | [phases-068-082.md#phase-068](./phases-068-082.md) |
| 69 | Cirq and qsim differential testing | `M8` | Phase 68. | **M** | Interop checks pass for declared common subset. | Page 34 | [phases-068-082.md#phase-069](./phases-068-082.md) |
| 70 | Stim stabilizer validation | `M8` | Phases 2, 5 and 68. | **M** | Stim checks run only on validated stabilizer-compatible inputs. | Page 34 | [phases-068-082.md#phase-070](./phases-068-082.md) |
| 71 | Unified differential test harness | `M8` | Phases 68–70. | **L** | Harness creates self-contained reproducible differential failures. | Page 35 | [phases-068-082.md#phase-071](./phases-068-082.md) |
| 72 | Provider-neutral backend API | `M8` | Phases 37, 43 and 71. | **L** | Mock backend passes lifecycle and capability tests. | Page 35 | [phases-068-082.md#phase-072](./phases-068-082.md) |
| 73 | Local simulator backend adapter | `M8` | Phases 61–67 and 72. | **M** | Local simulator adapter passes the common backend suite. | Page 36 | [phases-068-082.md#phase-073](./phases-068-082.md) |
| 74 | OpenQASM execution artifact and dry run | `M8` | Phases 9, 43, 59 and 72. | **M** | Artifact is self-describing and dry-run detects incompatibility. | Page 36 | [phases-068-082.md#phase-074](./phases-068-082.md) |
| 75 | Provider-neutral remote job lifecycle | `M8` | Phase 72. | **L** | Mock lifecycle is fully tested and resumable. | Page 36 | [phases-068-082.md#phase-075](./phases-068-082.md) |
| 76 | Credential and secret handling | `M8` | Phase 75. | **M** | Secret-handling tests pass and credentials stay optional. | Page 37 | [phases-068-082.md#phase-076](./phases-068-082.md) |
| 77 | IBM Quantum adapter | `M8` | Phases 72–76. | **XL** | Mock tests pass; live test is opt-in and access-dependent. | Page 37 | [phases-068-082.md#phase-077](./phases-068-082.md) |
| 78 | AWS Braket adapter | `M8` | Phases 72–76. | **XL** | Mock/provider-format tests pass and compatibility matrix is documented. | Page 37 | [phases-068-082.md#phase-078](./phases-068-082.md) |
| 79 | Additional provider adapters | `M8` | Phases 72–76. | **XL per adapter** | Each adapter has official-source evidence and independent tests. | Page 38 | [phases-068-082.md#phase-079](./phases-068-082.md) |
| 80 | Remote execution result normalization | `M8` | Phases 75–79. | **L** | Normalization passes provider-independent fixtures. | Page 38 | [phases-068-082.md#phase-080](./phases-068-082.md) |
| 81 | Microsoft QIR interoperability | `M8` | Phases 6 and 72. | **XL** | Profile and external validation are documented. | Page 39 | [phases-068-082.md#phase-081](./phases-068-082.md) |
| 82 | Backend capability matrix and preflight | `M8` | Phases 72–81. | **M** | Capability preflight is deterministic and source-backed. Milestone M9 — ... | Page 39 | [phases-068-082.md#phase-082](./phases-068-082.md) |
| 83 | Complete CLI command architecture | `M9` | Phases 27, 35, 43, 59 and 82. | **L** | CLI end-to-end tests pass. | Page 39 | [phases-083-089.md#phase-083](./phases-083-089.md) |
| 84 | AST/DAG/QccIR inspection | `M9` | Phases 6, 18 and 83. | **M** | Inspection output preserves essential compiler structure. | Page 40 | [phases-083-089.md#phase-084](./phases-083-089.md) |
| 85 | Circuit and topology visualization | `M9` | Phases 6, 44, 53 and 84. | **L** | Golden circuit and topology drawings pass. | Page 40 | [phases-083-089.md#phase-085](./phases-083-089.md) |
| 86 | Compilation and optimization reports | `M9` | Phases 35, 43, 53, 59 and 84. | **M** | Reports are reproducible and accurately scoped. | Page 41 | [phases-083-089.md#phase-086](./phases-083-089.md) |
| 87 | Reproducibility manifests | `M9` | Phases 5, 35, 53, 67 and 72. | **M** | Manifest records all required inputs without secrets. | Page 41 | [phases-083-089.md#phase-087](./phases-083-089.md) |
| 88 | Benchmarking framework | `M9` | Phases 3–5, 23, 53 and 67. | **L** | Benchmark baseline and methodology are checked in. | Page 41 | [phases-083-089.md#phase-088](./phases-083-089.md) |
| 89 | Documentation, packaging and release engineering | `M9` | Phases 83–88. | **L** | Release candidate builds and documentation examples pass. Advanced resea... | Page 42 | [phases-083-089.md#phase-089](./phases-083-089.md) |
| 90 | Calibration-aware cost model | `Research` | Phases 37–38, 54 and 59. | **XL** | Calibration-aware mode is opt-in and transparent. | Page 42 | [phases-090-105.md#phase-090](./phases-090-105.md) |
| 91 | User-defined gate AST and semantics | `Research` | Phases 18–24. | **L** | Gate definition semantics have a test-backed contract. | Page 42 | [phases-090-105.md#phase-091](./phases-090-105.md) |
| 92 | User-defined gate expansion | `Research` | Phase 91 and QccIR rewrite API. | **L** | Expanded circuits pass equivalence and graph validation. | Page 43 | [phases-090-105.md#phase-092](./phases-090-105.md) |
| 93 | Control-flow graph representation | `Research` | Phase 6. | **XL** | CFG invariants and state-flow model are documented and tested. | Page 43 | [phases-090-105.md#phase-093](./phases-090-105.md) |
| 94 | Classical expressions and conditions | `Research` | Phases 21, 23 and 93. | **L** | Classical expression tests and dependency checks pass. | Page 44 | [phases-090-105.md#phase-094](./phases-090-105.md) |
| 95 | Conditional branch lowering | `Research` | Phases 93–94 and QASM AST. | **XL** | Conditional programs pass structural and execution tests. | Page 44 | [phases-090-105.md#phase-095](./phases-090-105.md) |
| 96 | Loop representation and validation | `Research` | Phases 93–95. | **XL** | Loop CFG and validation tests pass. | Page 44 | [phases-090-105.md#phase-096](./phases-090-105.md) |
| 97 | Dynamic CFG execution | `Research` | Phases 60–66 and 93–96. | **XL** | CFG execution passes behavior and trace checks. | Page 45 | [phases-090-105.md#phase-097](./phases-090-105.md) |
| 98 | KAK mathematical foundations | `Research` | Phases 10, 31 and 39. | **XL** | Reference examples validate the mathematical specification. | Page 45 | [phases-090-105.md#phase-098](./phases-090-105.md) |
| 99 | KAK numerical decomposition | `Research` | Phase 98. | **XL** | Random reconstruction suite passes over declared domain. | Page 45 | [phases-090-105.md#phase-099](./phases-090-105.md) |
| 100 | KAK target-specific synthesis | `Research` | Phases 39–43 and 99. | **XL** | Target-specific KAK circuits pass unitary and legality checks. | Page 46 | [phases-090-105.md#phase-100](./phases-090-105.md) |
| 101 | MPS simulation foundations | `Research` | Phases 3–5 and 67. | **XL** | Foundation and exact small-state representation are validated. | Page 46 | [phases-090-105.md#phase-101](./phases-090-105.md) |
| 102 | MPS gate updates and truncation | `Research` | Phase 101. | **XL** | Reference comparison and error reporting pass. | Page 46 | [phases-090-105.md#phase-102](./phases-090-105.md) |
| 103 | MPS benchmarking and simulator selection | `Research` | Phases 67 and 101–102. | **L** | Selection is advisory and benchmark-supported. | Page 47 | [phases-090-105.md#phase-103](./phases-090-105.md) |
| 104 | Advanced optimizer research | `Research` | Phases 33–35 and 98–100. | **XL** | Experimental passes remain gated and reproducible. | Page 47 | [phases-090-105.md#phase-104](./phases-090-105.md) |
| 105 | Final architecture audit and research roadmap | `Research` | All completed milestones. | **L** | A stable baseline, test evidence and prioritized research backlog exist. | Page 48 | [phases-090-105.md#phase-105](./phases-090-105.md) |