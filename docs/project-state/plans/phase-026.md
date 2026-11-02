# Phase Plan: Phase 26 — QASM Import/Export Round-Trip

## 1. Metadata
- **Phase Number**: Phase 26
- **Milestone**: Milestone M2: Compiler Foundations and OpenQASM Frontend (`M2`)
- **Relative Effort**: **L**
- **Prerequisites Satisfied**: Phase 9 (OpenQASM 3 emitter) and Phases 15–25 (frontend lexer, parser, AST, semantic analysis, diagnostics, and AST-to-QccIR lowering) are complete and verified in [`docs/project-state/progress.md`](../progress.md).
- **Source Document Page**: Page 17 (`QCC_Quantum_Circuit_Compiler_Engineering_Roadmap(1).docx`)

## 2. Objective & Scope Boundaries
- **Objective**: Validate that importing supported OpenQASM 3 programs, lowering them to QccIR, and exporting them back to OpenQASM 3 preserves supported quantum and classical semantics. Validate emitter and parser interoperability independently using the pinned reference parser (`openqasm3` v1.0.1).
- **Explicit Inclusions**:
  - `tests/fixtures/qasm3/`: Golden OpenQASM 3 corpus covering supported feature set:
    - `bell.qasm` (Bell state preparation and measurement)
    - `rotations.qasm` (parameterized rotations: `rx`, `ry`, `rz` with arithmetic and $\pi$ constants)
    - `non_unitary.qasm` (measurement, active reset, selective barrier)
    - `multi_register.qasm` (multiple quantum and classical registers, non-adjacent qubit operands)
    - `global_phase.qasm` (explicit `gphase` directives and gate sequences)
    - `teleportation.qasm` (standard quantum teleportation benchmark in supported subset)
  - `tests/qasm3/roundtrip.rs`: Comprehensive round-trip test suite:
    - Golden-file round trips (source $\to$ QccIR $\to$ emitted $\to$ independent parser $\to$ re-lowered QccIR).
    - Independent parser validation of all emitted QASM text via Python `openqasm3.parse()`.
    - Double round-trip idempotency ($C_1 \xrightarrow{\text{emit}} Q_1 \xrightarrow{\text{lower}} C_2 \xrightarrow{\text{emit}} Q_2$ with $Q_1 == Q_2$ and $C_2 \equiv C_1$).
    - Formatting insensitivity (whitespace, comments, blank lines, assignment vs arrow measurement syntax do not affect semantic equivalence).
    - Operation, register, parameter, and global-phase preservation where representable.
    - Property-based randomized circuit round trips.
    - Unitary equivalence check for pure unitary circuits and behavioral sampling comparison for measurement circuits.
  - `docs/qasm3_support.md`: Detailed specification of QCC's OpenQASM 3 support, grammar subset, emitter conventions, round-trip fidelity guarantees, and deferred constructs.
  - `tests/qasm3.rs`: Register `roundtrip` module in the integration test suite.
- **Explicit Exclusions**:
  - Do not compare formatting byte-for-byte as the definition of semantic equivalence; formatting may legitimately normalize.
  - Do not use QCC's own parser as the sole validation of emitted text; independent parser verification is mandatory.
  - Do not expand the supported OpenQASM language subset in this phase.
  - Do not start Phase 27 fuzzing, conformance expansion, or frontend CLI.
  - Do not restructure unrelated compiler crates or modules.

## 3. Architecture & Semantic Invariants Checklist
- [x] Rust toolchain remains strictly stable.
- [x] Little-endian qubit indexing preserved ($q_0$ as LSB).
- [x] Column-vector matrix convention preserved ($B \cdot A$ for $A$ before $B$).
- [x] Gate operand order: CX operand 0 = control, operand 1 = target.
- [x] Explicit circuit global phase tracked and preserved ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$).
- [x] Classical wire hazards (RAW, WAR, WAW) preserved through round-trip emission and re-lowering.
- [x] QccIR stable semantic `OperationId` maintained; no raw petgraph indices exposed.
- [x] QccIR graph validated (`circuit.validate()`) with zero invariant errors on all round-tripped circuits.
- [x] Non-unitary circuits (measure, reset) validated via behavioral sampling and structural checks, never passed to unitary equivalence checker.

## 4. Step-by-Step Implementation Sequence
1. **Fixture Creation**: Create representative OpenQASM 3 test fixtures in `tests/fixtures/qasm3/`.
2. **Round-Trip Test Suite**: Implement `tests/qasm3/roundtrip.rs` with golden tests, independent parser validation, randomized circuits, formatting tolerance, and idempotency tests.
3. **Module Registration**: Register `roundtrip` in `tests/qasm3.rs`.
4. **Documentation**: Write comprehensive `docs/qasm3_support.md`.
5. **Verification**: Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`, `cargo build --all-targets`, `cargo doc --no-deps`, Bell/QASM examples, and external `openqasm3` parser checks.
6. **Progress Ledger & Report**: Document results in `docs/project-state/reports/phase-026.md` and update `docs/project-state/progress.md`.

## 5. Expected Files and Modules
- New files:
  - `tests/fixtures/qasm3/bell.qasm`
  - `tests/fixtures/qasm3/rotations.qasm`
  - `tests/fixtures/qasm3/non_unitary.qasm`
  - `tests/fixtures/qasm3/multi_register.qasm`
  - `tests/fixtures/qasm3/global_phase.qasm`
  - `tests/fixtures/qasm3/teleportation.qasm`
  - `tests/qasm3/roundtrip.rs`
  - `docs/qasm3_support.md`
- Existing files to modify:
  - `tests/qasm3.rs`
  - `docs/project-state/progress.md`

## 6. Exit Gate Verification Target
> **EXIT GATE**: Independent parse and semantic round-trip suite passes.

