# Phase 11 Implementation Plan: Bell Vertical-Slice Acceptance

## Objective
Deliver and formally verify the end-to-end Bell state vertical slice acceptance test for **Milestone M1 (Bell Vertical Slice)**.
This phase integrates all Milestone 1 components into a coherent, verifiable pipeline:
$$\text{Rust CircuitBuilder} \longrightarrow \text{QccIR DAG} \longrightarrow \text{CircuitValidator} \longrightarrow \text{CircuitExecutor} \longrightarrow \text{Terminal Sampler} \longrightarrow \text{OpenQASM 3 Emitter}$$
It confirms that QCC correctly constructs, validates, simulates, unitarily certifies, samples, and exports quantum circuits with zero nightly features and zero unverified assumptions.

---

## 1. Scope & Modules

### Files to Create:
- `tests/end_to_end/bell.rs`: Comprehensive end-to-end test suite for the Bell vertical slice.
- `tests/end_to_end.rs`: Cargo integration test target declaring `mod bell;`.
- `docs/project-state/reports/phase-011.md`: Phase completion report documenting acceptance verification and Milestone M1 sign-off.

### Files to Update:
- `examples/bell.rs`: Elevate the example into a complete vertical slice demonstration printing circuit DAG info, statevector amplitudes, terminal measurement sampling, and emitted OpenQASM 3.
- `.github/workflows/ci.yml`: Ensure CI executes all unit/integration tests, doc tests, and example binaries (`cargo run --example bell`, `cargo run --example qasm`).
- `docs/project-state/progress.md`: Update progress ledger marking Phase 11 and Milestone M1 as complete.

---

## 2. Mathematical Contracts & Invariants

1. **Little-Endian Statevector Indexing**:
   - $q[0]$ is the least-significant bit (LSB).
   - In 2-qubit basis $|q_1 q_0\rangle$:
     - Index $0 = |00\rangle$
     - Index $1 = |01\rangle$
     - Index $2 = |10\rangle$
     - Index $3 = |11\rangle$
   - Statevector for $|\Phi^+\rangle = \frac{|00\rangle + |11\rangle}{\sqrt{2}}$ must have amplitudes:
     - $a_0 = \frac{1}{\sqrt{2}} \approx 0.7071067811865475$
     - $a_1 = 0$
     - $a_2 = 0$
     - $a_3 = \frac{1}{\sqrt{2}} \approx 0.7071067811865475$
   - Component-wise amplitude tolerance: $|a_i - a_i^{\text{expected}}| < 10^{-10}$.

2. **Unitary Core vs Terminal Measurement Separation**:
   - Unitary evolution: $H(q[0]); CX(q[0], q[1])$ evolves $|00\rangle \to |\Phi^+\rangle$.
   - Phase 10 `EquivalenceChecker` verifies this unitary core against an independent reference circuit ($H(0); H(1); CZ(0, 1); H(1)$).
   - Equivalence checker rejects circuits with non-unitary operations (e.g. measurements) with structured errors.
   - Equivalence checker rejects inequivalent variants ($H(0); X(1)$ or $H(0); CZ(0, 1)$).

3. **Terminal Measurement Sampling**:
   - Simulated strictly once: statevector evolved through unitary core; terminal measurements sampled via CDF binary search.
   - For $N = 2000$ shots with seeded RNG (`SplitMix64::new(42)`):
     - Only outcomes `"00"` and `"11"` are observed.
     - Out-of-support states `"01"` and `"10"` have count exactly $0$.
     - Expected counts for `"00"` and `"11"` are within 4-$\sigma$ bounds ($850 \le \text{count} \le 1150$).
     - Deterministic reproducibility: same seed yields identical shot sequences.

4. **OpenQASM 3 Deterministic Emission & Independent Validation**:
   - Emitted OpenQASM 3.0 matches `examples/qasm/bell.qasm` character-for-character.
   - Independent parser validation is checked via Python `openqasm3` if available, and honestly reported if absent in the offline sandbox.

---

## 3. Step-by-Step Implementation Sequence

1. Write `docs/project-state/plans/phase-011.md`.
2. Create `tests/end_to_end/bell.rs` and `tests/end_to_end.rs` covering all acceptance tests:
   - Full pipeline integration (Builder $\to$ QccIR $\to$ Simulator $\to$ Sampler $\to$ QASM).
   - Exact statevector amplitude verification under little-endian convention.
   - Unitary equivalence verification against reference circuit.
   - Unitary checker rejection of inequivalent circuits and non-unitary circuits.
   - Seeded terminal measurement sampling with 2000 shots.
   - Classical bit ordering validation (`MsbFirst` vs `LsbFirst`).
   - Deterministic reproducibility of sampling.
   - OpenQASM 3 golden file matching.
   - Candid independent parser validation.
3. Update `examples/bell.rs` to demonstrate the complete vertical slice from creation to execution, sampling, and export.
4. Update `.github/workflows/ci.yml` with example execution and doc testing.
5. Run full verification suite (`cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo run --example bell`, `cargo run --example qasm`, `cargo doc --no-deps`).
6. Draft completion report in `docs/project-state/reports/phase-011.md`.
7. Update progress ledger in `docs/project-state/progress.md`.
8. Commit with `feat(phase-11): complete Bell vertical slice`.
