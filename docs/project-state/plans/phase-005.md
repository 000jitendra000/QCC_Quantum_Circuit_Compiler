# Implementation Plan: Phase 5 — Terminal Measurement and Seeded Sampling

## 1. Context & Objectives
- **Phase Number**: Phase 5
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Primary Goal**: Implement terminal measurement and reproducible seeded sampling:
  - Compute computational basis probabilities $p_i = |\alpha_i|^2$ from the statevector.
  - Strict probability validation (no silent acceptance of invalid/non-normalized distributions) and careful floating-point tail handling.
  - Seeded RNG interface (`SeededRng`) and canonical deterministic PRNG (`SplitMix64`) for 100% reproducible sampling.
  - Cumulative-distribution function (CDF) construction and $\mathcal{O}(\log(2^n))$ binary-search sampling.
  - Selected-qubit measurement with explicit bit ordering consistent with Phase 2 semantics (`format_measurement_bits`, `ClassicalBitOrder::MsbFirst` and `LsbFirst`).
  - Validation of qubit indices, duplicate rejection, and empty selection rejection.
  - **Single-Evolution Guarantee**: All terminal shots are sampled from the pre-computed distribution; the circuit is never re-evolved per shot.
  - Rejection of unsupported dynamic mid-circuit measurement / feedback in terminal sampling mode.

## 2. Invariants & Conventions
1. **Born Rule**: $P(i) = |\alpha_i|^2$ for basis index $i \in [0, 2^n - 1]$.
2. **Probability Normalization**: $\sum_{i=0}^{2^n-1} P(i) = 1.0 \pm 10^{-5}$.
3. **Decoupled Bit Formatting**: Measurement bits $c_0, c_1, \dots, c_{m-1}$ correspond to the ordered measured qubits.
   - `MsbFirst` (default OpenQASM): $c_{m-1} \dots c_0$
   - `LsbFirst`: $c_0 \dots c_{m-1}$
4. **Zero-Evolution Loop**: The sampling loop performs only CDF binary search and bit extraction; no statevector mutation or circuit re-execution occurs during shot generation.
5. **Deterministic Seeding**: Same seed + same statevector $\implies$ bit-for-bit identical shot sequences and count distributions.

## 3. Ordered Implementation Tasks
1. **Error Infrastructure**: Add `EmptyMeasurementSelection`, `DuplicateMeasurementQubit`, `InvalidShotCount`, `InvalidProbabilityDistribution`, `MidCircuitMeasurementUnsupported` to [`SimulatorError`](src/simulator/error.rs).
2. **Measurement & Distribution (`src/simulator/measurement.rs`)**:
   - `ProbabilityDistribution`: validation, computation from `StateVector`, marginal distribution calculation.
   - `MeasurementSelection`: validated ordered subset of qubits to measure.
3. **Result Data Models (`src/simulator/result.rs`)**:
   - `ShotResult`: single measurement bit vector and formatting.
   - `ExecutionResult`: aggregated counts histogram (`BTreeMap<String, usize>`), shot list, metadata.
4. **Sampling Engine (`src/simulator/sampling.rs`)**:
   - `SeededRng` trait and `SplitMix64` (canonical zero-dependency PRNG).
   - `CumulativeDistribution`: clamped CDF array with binary-search sampling (`partition_point`).
   - `TerminalSampler`: state-evolved-once sampler generating $S$ shots.
5. **StateVector Integration & Simulator Facade**:
   - Update `src/simulator/statevector.rs` with `probabilities()`, `measure_terminal()`.
   - Update `src/simulator/mod.rs` and `src/lib.rs` exports.
6. **Integration & Statistical Testing**:
   - `tests/measurement.rs`: probability calculation, normalization rejection, floating-point tail handling, marginal distributions, selection validation.
   - `tests/sampling.rs`: $|0\rangle$, $|1\rangle$, $H$ superposition statistical test, Bell state ($00$ and $11$ only), seed reproducibility, one-evolution proof (measuring call count / benchmark).
7. **Verification**:
   - `cargo fmt --check`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo test`
   - `cargo doc --no-deps`
8. **Report & Git Checkpoint**:
   - Generate `docs/project-state/reports/phase-005.md`.
   - Update `docs/project-state/progress.md`.
   - Commit `feat(phase-5): implement terminal measurement and seeded sampling`.
