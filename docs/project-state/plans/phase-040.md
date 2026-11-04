# Phase 40 Implementation Plan — Native Single-Qubit Synthesis

## Context & Objectives

Phase 40 of the QCC engineering roadmap implements native single-qubit synthesis into target-supported instruction families.
Before general multi-qubit basis translation (Phase 41), the compiler must be able to synthesize arbitrary one-qubit unitaries into declared target instruction sets, preserving explicit global phase, normalizing angles, and handling Euler singularities deterministically.

## Roadmap Specification

- **Phase Number**: Phase 40
- **Milestone**: Milestone M4: Generic Target and Basis Translation (`M4`)
- **Effort**: **XL**
- **Prerequisites**: Phases 31 (Single-qubit Euler decomposition), 37 (Generic Target data model), 38 (Target capability queries and cost models).
- **Exit Gate**: Random unitary synthesis tests and target legality checks pass.

## Architecture & Design

### 1. Target Single-Qubit Basis Model (`src/target/synthesis_basis.rs`)
- Define `SingleQubitBasis`:
  - `Zyz`: Generic $Z\text{-}Y\text{-}Z$ ($R_z, R_y$).
  - `Zxz`: Generic $Z\text{-}X\text{-}Z$ ($R_z, R_x$).
  - `RzSx`: Target-native $R_z, SX$ microwave/virtual transmon family ($R_z, SX$).
  - `U3`: Single-operation Euler format ($U3$ / $U$).
- Add query helpers to `Target` / `TargetQuery`:
  - `supports_single_qubit_basis(&self, basis: SingleQubitBasis, qubit: PhysicalQubitId) -> bool`
  - `supported_single_qubit_bases(&self, qubit: PhysicalQubitId) -> Vec<SingleQubitBasis>`
  - `preferred_single_qubit_basis(&self, qubit: PhysicalQubitId) -> Result<SingleQubitBasis, SynthesisError>`

### 2. Native Single-Qubit Synthesizer (`src/synthesis/single_qubit.rs`)
- **`SingleQubitSynthesizer`**: Configurable synthesis engine with numerical tolerances.
- **`SynthesizedOneQubitSequence`**:
  - `operations: Vec<Operation>` in execution order (left multiplication).
  - `physical_qubit: PhysicalQubitId`.
  - `global_phase: GlobalPhase`.
  - `basis: SingleQubitBasis`.
  - Helper `reconstruct_matrix(&self) -> Matrix2x2`: multiplies operation matrices in left-multiplication order and scales by $e^{i\alpha}$.
  - Helper `verify_equivalence(&self, target_matrix: &Matrix2x2, tolerance: f64) -> Result<(), SynthesisError>`.
  - Helper `verify_legality(&self, target: &Target) -> Result<(), LegalityError>`.
  - Helper `into_circuit(&self, name: impl Into<String>) -> Circuit`.
- **`SynthesisError`**: Structured domain error for decomposition failures, unsupported basis, missing capabilities, out-of-bounds qubits, legality violations, and verification discrepancies.

### 3. Synthesis Paths & Mathematical Formulations
- **`Zyz`**:
  - $U = e^{i\alpha} \left( R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda) \right)$.
  - Execution order: $R_z(\lambda) \to R_y(\theta) \to R_z(\phi)$.
- **`Zxz`**:
  - $U = e^{i\alpha} \left( R_z(\phi) \cdot R_x(\theta) \cdot R_z(\lambda) \right)$.
  - Execution order: $R_z(\lambda) \to R_x(\theta) \to R_z(\phi)$.
- **`RzSx`**:
  - $SX = e^{i\pi/4} R_x(\pi/2)$.
  - $R_x(\theta) = e^{i\pi/2} \left( R_z(\pi/2) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \cdot R_z(\pi/2) \right)$.
  - From $Z\text{-}Y\text{-}Z$:
    $U = e^{i(\alpha + \pi/2)} \left( R_z(\phi + \pi) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \cdot R_z(\lambda) \right)$.
  - Execution order: $R_z(\lambda) \to SX \to R_z(\theta + \pi) \to SX \to R_z(\phi + \pi)$.
  - Spinor angle normalization with exact $2\pi$ phase compensation.
- **`U3`**:
  - $U(\theta, \phi, \lambda) = e^{i(\phi + \lambda)/2} \left( R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda) \right)$.
  - Emits `u(theta, phi, lambda)` or `u3(theta, phi, lambda)`.
  - Global phase: $\alpha - (\phi + \lambda)/2$.

### 4. Angle Normalization & Singularity Resolution
- Gimbal-lock singularities ($\theta \approx 0$ and $\theta \approx \pi$) resolved deterministically via Phase 31 `decompose_euler` convention ($\lambda = 0$).
- Continuous angles normalized to canonical ranges $[0, 2\pi)$ with spinor shifts $(k \pmod 2) \cdot \pi$ tracked in the scalar global phase.
- Automatic target legality checking on all emitted operations.

### 5. Verification & Tests (`tests/synthesis/single_qubit.rs`)
- Random Haar unitaries reconstruction across all supported bases.
- Target legality checks on synthetic targets (directed CX, undirected CZ, native ECR, U3, and unsupported targets).
- Special angles ($\theta = 0, \pi/2, \pi$, identities, Paulis, Cliffords).
- Phase-sensitive tests proving exact matrix equality $U_{\text{target}} = e^{i\alpha} V_{\text{ops}}$.
- Structured error handling on unsupported targets.
