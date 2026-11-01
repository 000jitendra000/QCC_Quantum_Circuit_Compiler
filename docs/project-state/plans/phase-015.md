# Phase 15 Implementation Plan: OpenQASM Parser Dependency Evaluation

## 1. Overview
- **Phase**: Phase 15
- **Title**: OpenQASM parser dependency evaluation
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Status**: Complete

## 2. Objectives
- Evaluate candidate Rust OpenQASM 3 parsing crates (`oq3_lexer`, `oq3_parser`, `oq3_syntax`, `oq3_semantics`).
- Investigate versions, licenses, MSRV, maintenance health, and dependency footprints.
- Construct a representative 10-program OpenQASM 3 test corpus covering Bell states, registers, standard gates, parameterized gates, measurement, reset, barrier, invalid syntax, malformed operands, and unsupported features.
- Evaluate alternative strategies: `oq3_*` stack, handwritten restricted parser, and Python parser bridge.
- Formulate and record the architecture decision in `ADR-0002` backed by `docs/research/qasm-parser-evaluation.md` and reproducible experiments in `experiments/oq3-evaluation/`.

## 3. Execution Steps
1. Perform preflight checks (clean git status, verify commit `784f6a9`).
2. Search crates.io, GitHub repositories, and docs.rs for `oq3_*` crates and document primary source findings.
3. Build evaluation corpus under `experiments/oq3-evaluation/corpus/`.
4. Run empirical evaluation using pinned independent parser `openqasm3[parser]==1.0.1` and record results in `py_parser_results.json`.
5. Create Rust evaluation harness in `experiments/oq3-evaluation/` to benchmark adaptation complexity into QccIR.
6. Author comprehensive technology evaluation document `docs/research/qasm-parser-evaluation.md`.
7. Author `ADR-0002: OpenQASM 3 Frontend Parser Strategy and Adapter Architecture` in `docs/adr/ADR-0002-qasm-parser.md` and `docs/decisions/ADR-0002-qasm-parser.md`.
8. Execute verification suite (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`, examples).
9. Update progress ledger `docs/project-state/progress.md` and write completion report `docs/project-state/reports/phase-015.md`.
10. Commit with `docs(phase-15): evaluate and select OpenQASM parser strategy`.
