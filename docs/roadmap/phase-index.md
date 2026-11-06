# QCC Roadmap Phase Index (Phases 1–59)

## Milestone Overview

| Milestone | Phases | Scope | Status | Phase Details File |
|---|:---:|---|:---:|---|
| **M1** | 1–11 | Bell Vertical Slice | **Completed** | [phases-001-011.md](./phases-001-011.md) |
| **M2** | 12–27 | Compiler Foundations and OpenQASM Frontend | **Completed** | [phases-012-027.md](./phases-012-027.md) |
| **M3** | 28–36 | Verified Logical Optimizer | **Completed** | [phases-028-036.md](./phases-028-036.md) |
| **M4** | 37–43 | Generic Target and Basis Translation | **Completed** | [phases-037-043.md](./phases-037-043.md) |
| **M5** | 44–53 | Logical-to-Physical Mapping and Routing | **Completed** | [phases-044-053.md](./phases-044-053.md) |
| **M6** | 54–59 | Scheduling and Integrated Compilation | **Completed** | [phases-054-059.md](./phases-054-059.md) |

---

## Implementation Phases (Phases 1–59)

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