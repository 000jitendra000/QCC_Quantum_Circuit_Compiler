# Phase 42 Implementation Plan — Two-Qubit Direction Correction

## Context & Objectives

Phase 42 of the QCC engineering roadmap implements **Two-qubit direction correction**.
Hardware quantum processors often impose asymmetric physical coupling constraints: a two-qubit entangling gate (such as Controlled-NOT / CX) may be physically executable with physical control qubit $u$ and physical target qubit $v$, but not with $u$ as target and $v$ as control.

The objective of Phase 42 is to detect when a direction-sensitive operation uses an illegal physical operand orientation and correct it to satisfy target connectivity constraints:
1. Detect illegal operand orientations for direction-sensitive operations against target hardware connectivity.
2. Implement and verify the canonical CX reversal identity using local Hadamards:
   $$CX(c, t) = (H_c \otimes H_t) \cdot CX(t, c) \cdot (H_c \otimes H_t)$$
3. Distinguish coupling-edge direction (hardware tuple legality) from CX operand semantics (operand 0 = control, operand 1 = target).
4. Preserve symmetric operations (such as $CZ$ and $SWAP$) without unnecessary direction corrections or gate insertions.
5. Support target-specific correction alternatives only when explicitly registered.
6. Include correction costs in candidate evaluation using the target `CostModel` with deterministic tie-breaking.
7. Verify all correction sequences using the project's established little-endian matrix convention and column-vector left-multiplication order ($B \cdot A$).
8. Explicitly reject operations when no valid direction correction exists (never silently retain an illegal orientation).
9. Integrate cleanly with the existing target query model, basis translation engine, and pass manager.

## Roadmap Specification

- **Phase Number**: Phase 42
- **Milestone**: Milestone M4: Generic Target and Basis Translation (`M4`)
- **Effort**: **M**
- **Prerequisites**: Phases 37 (Generic Target data model), 39 (Fixed decomposition template library), 41 (Basis translation engine).
- **Exit Gate**: Direction correction tests pass for both orientations. Every direction-sensitive operation is either corrected into a legal target orientation or explicitly rejected before routing/export.

## Architecture & Design

### 1. Target Direction Analysis (`src/target/direction.rs`)
- **`DirectionStatus` Enum**:
  - `Legal`: Forward physical operand tuple $(u, v)$ is directly legal on target.
  - `NeedsReversal`: Forward tuple $(u, v)$ is illegal, but reversed tuple $(v, u)$ is legal.
  - `NoCoupling`: Neither orientation is supported between physical qubits $u$ and $v$.
  - `UnsupportedOperation`: Operation is not supported on the target.
  - `ArityMismatch`: Operation quantum arity is not 2.
- **`DirectionError` Enum**:
  - `NoViableDirection { operation, u, v, target, reason }`: Structured error when no coupling exists or all correction paths fail.
  - `VerificationFailed { operation, u, v, max_diff, tolerance }`: Matrix verification discrepancy.
  - `Legality(String)`: Target legality failure after correction.
- **`Target` Direction Extensions**:
  - `check_direction(&self, op_name: &str, u: PhysicalQubitId, v: PhysicalQubitId) -> DirectionStatus`
  - `is_operation_directed(&self, op_name: &str) -> bool`
  - `is_direction_sensitive(&self, op_name: &str) -> bool`

### 2. Translation Direction Rules & Corrector (`src/translation/direction.rs`)
- **`CxDirectionCorrectionRule`**:
  - Implements `TranslationRule` for integration with `TranslationRuleRegistry` and `BasisTranslationEngine`.
  - Matches `cx(c, t)` when $(c, t)$ is not permitted on target, but $(t, c)$ is permitted.
  - Emits $[H(c), H(t), CX(t, c), H(c), H(t)]$.
- **`CzDirectionCommutationRule`**:
  - Matches $CZ(c, t)$ when $(c, t)$ is not permitted on directed $CZ$ target, but $(t, c)$ is permitted.
  - Emits $[CZ(t, c)]$ with zero Hadamards (since $CZ$ is symmetric).
- **Target-Specific Correction Alternatives**:
  - `TargetDirectionRule` trait and `DirectionCorrectionRegistry` for registering and evaluating hardware-specific correction alternatives.
- **Cost Evaluation & Candidate Selection**:
  - When multiple correction alternatives exist, evaluates sequence cost with `CostModel::evaluate_physical_sequence` and selects deterministically.
- **Local Unitary Matrix Verification**:
  - Strict local equivalence testing comparing $(H \otimes H) \cdot CX_{1,0} \cdot (H \otimes H)$ against $CX_{0,1}$ ($< 10^{-12}$) in little-endian ordering ($q_0$ is LSB).
- **`TwoQubitDirectionCorrector`**:
  - High-level standalone corrector inspecting circuit 2Q gates, performing direction corrections, recursively lowering non-native gates (such as $H$ into $RZ\text{-}SX$), and validating final target legality.
- **`TwoQubitDirectionPass`**:
  - Implements `CompilerPass` for pipeline execution in `PassManager`.

### 3. Registry & Engine Integration
- Register `CxDirectionCorrectionRule` and `CzDirectionCommutationRule` in `TranslationRuleRegistry::with_default_rules()`.
- Export direction types in `src/target/mod.rs`, `src/translation/mod.rs`, and `src/lib.rs`.

## Planned Tests (`tests/translation/direction.rs`)

1. **Analytical & Numerical Matrix Equivalence**:
   - $CX(0, 1) = (H_0 \otimes H_1) \cdot CX(1, 0) \cdot (H_0 \otimes H_1)$ in little-endian ordering.
   - $CX(1, 0) = (H_0 \otimes H_1) \cdot CX(0, 1) \cdot (H_0 \otimes H_1)$.
   - Global phase is preserved identically ($\Delta \phi = 0$).
2. **One-Way CX Target Fixtures**:
   - Target with only edge $(1, 0)$ correctly reverses $CX(0, 1) \to CX(1, 0)$ with Hadamards.
   - Target with only edge $(1, 0)$ preserves $CX(1, 0)$ without modifications.
3. **Both Operand Orientations**:
   - Target with $(0, 1)$ only reverses $CX(1, 0) \to CX(0, 1)$.
   - Target with $(1, 0)$ only reverses $CX(0, 1) \to CX(1, 0)$.
4. **Symmetric Operations**:
   - $CZ(0, 1)$ and $CZ(1, 0)$ on undirected target receive zero modifications.
   - $CZ(0, 1)$ on directed $(1, 0)$ target is commuted to $CZ(1, 0)$ with zero Hadamards.
   - $SWAP(0, 1)$ is not modified unnecessarily.
5. **Explicit Failure for Impossible Directions**:
   - Disconnected qubits $(0, 2)$ fail with structured error `NoViableDirection` / `NoViableTranslation`.
   - Never silently retain an illegal orientation.
6. **Target-Specific Correction Alternatives & Cost-Based Selection**:
   - Register custom alternative and verify deterministic selection via `CostModel`.
7. **End-to-End Pipeline & Recursive Lowering**:
   - Directing CX on an $RZ\text{-}SX$ target lowers Hadamards to native $RZ\text{-}SX$ rotations while keeping the legal $CX(1, 0)$ entangler.
   - Final target legality check passes on all emitted operations.
