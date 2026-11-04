# Phase 41 Implementation Plan — Basis Translation Engine

## Context & Objectives

Phase 41 of the QCC engineering roadmap implements the **Basis Translation Engine**.
The objective is to translate logical quantum circuits containing arbitrary high-level or un-supported operations (such as Toffoli, SWAP, unsupported entanglers like CX on a CZ-native machine, or general single-qubit unitaries) into sequences of operations strictly supported by a hardware `Target`'s native basis.

This translation must:
1. Preserve already-supported operations without modification.
2. Search and expand candidate decompositions using the fixed template library (Phase 39), single-qubit synthesis engine (Phase 40), and extensible translation rules.
3. Recursively translate introduced operations until every operation in the sequence is supported and legal on the target.
4. Protect against recursive cycles and excessive recursion depth.
5. Select among multiple candidate translation paths using the target `CostModel` (Phase 38).
6. Verify local unitary equivalence before accepting any candidate decomposition.
7. Preserve symbolic parameters and their semantics throughout decomposition.
8. Fail explicitly and diagnostically when no valid translation exists.
9. Enforce final target legality: every emitted operation must pass `target.check_operation_legality`.

## Roadmap Specification

- **Phase Number**: Phase 41
- **Milestone**: Milestone M4: Generic Target and Basis Translation (`M4`)
- **Effort**: **XL**
- **Prerequisites**: Phases 37 (Generic Target data model), 38 (Target capability queries and cost models), 39 (Fixed decomposition template library), 40 (Native single-qubit synthesis).
- **Exit Gate**: Translated circuits pass local unitary equivalence verification and target operation checks.

## Architecture & Design

### 1. Module Structure (`src/translation/`)
- `src/translation/mod.rs`: Public types, re-exports, error models, and high-level translation convenience APIs.
- `src/translation/rules.rs`: `TranslationRule` trait, `TranslationRuleRegistry`, template-backed decomposition rules, single-qubit synthesis translation rules, and symbolic parameter translation rules.
- `src/translation/engine.rs`: `BasisTranslationEngine`, recursive translation search with cycle detection and depth protection, cost-based candidate selection, and local unitary equivalence verification.
- `src/translation/pass.rs`: `BasisTranslationPass` implementing `CompilerPass` for execution within `PassManager`.
- Re-exports in `src/target/mod.rs` and `src/lib.rs`.

### 2. Extensible Rule Registry & Rule Trait (`src/translation/rules.rs`)
- **`TranslationRule` Trait**:
  - `name(&self) -> &'static str`: Stable rule identifier.
  - `target_gate(&self) -> &'static str`: Gate mnemonic this rule decomposes.
  - `quantum_arity(&self) -> usize`: Expected qubit count.
  - `matches(&self, op: &Operation, target: &Target) -> bool`: Checks whether the rule applies to `op` for `target`.
  - `decompose(&self, op: &Operation, target: &Target) -> Result<DecompositionCandidate, TranslationError>`: Expands `op` into replacement operations and returns the added global phase.
- **`DecompositionCandidate`**:
  - `rule_name: &'static str`: Identifying rule.
  - `operations: Vec<Operation>`: Direct expansion operations.
  - `global_phase_delta: f64`: Added scalar global phase $\Delta \phi \in [0, 2\pi)$.
- **Standard Built-in Rules**:
  1. `SwapToCxRule`: Decomposes `swap(a, b)` into 3 `cx` gates using `SwapToCxTemplate`.
  2. `CxToCzRule`: Decomposes `cx(c, t)` into `h(t), cz(c, t), h(t)` using `CxToCzTemplate`.
  3. `CzToCxRule`: Decomposes `cz(c, t)` into `h(t), cx(c, t), h(t)` using `CzToCxTemplate`.
  4. `ToffoliToCxRule`: Decomposes `ccx` / `toffoli` into 15 elementary gates using `ToffoliToCxTemplate`.
  5. `SingleQubitSynthesisRule`: For any unsupported 1Q gate with constant parameters (e.g. `h`, `s`, `t`, `x`, `y`, `z`, or constant rotations), extracts its $2 \times 2$ unitary matrix and synthesizes into the target's preferred basis (`RzSx`, `Zyz`, `Zxz`, `U3`) using `SingleQubitSynthesizer`.
  6. `SymbolicRxRule`: For `rx(expr)` when target supports `rz` and `sx`, emits `rz(pi/2), sx, rz(expr + pi), sx, rz(pi/2)` with $\Delta \phi = \pi/2$.
  7. `SymbolicRyRule`: For `ry(expr)` when target supports `rz` and `sx`, emits `sx, rz(expr + pi), sx, rz(pi)` with $\Delta \phi = \pi/2$.
  8. `SymbolicPhaseRule`: For `p(expr)` or `u1(expr)` when target supports `rz`, emits `rz(expr)` with phase $\Delta \phi = \text{expr}/2$ (or exact evaluation).
  9. `AlternativeEntanglerRule`: Alternative orientations or symmetric variants enabling cost-based selection.

### 3. Basis Translation Engine (`src/translation/engine.rs`)
- **`BasisTranslationEngine`**:
  - `target: Target`
  - `cost_model: CostModel`
  - `registry: TranslationRuleRegistry`
  - `max_depth: usize` (default: 10)
  - `verify_equivalence: bool` (default: true)
  - `equivalence_tolerance: f64` (default: 1e-10)
- **Algorithm Flow**:
  1. Traverse circuit operations in deterministic topological order.
  2. For each operation:
     - Check `target.check_operation_legality(name, params, qubits)`. If `Ok(())`, preserve the operation untouched.
     - If illegal/unsupported, initiate recursive translation:
       `translate_operation_recursive(op, current_depth, call_stack)`:
       - If `current_depth > max_depth`, return `Err(TranslationError::MaxRecursionDepthExceeded)`.
       - If `call_stack.contains(op_signature)`, return `Err(TranslationError::CycleDetected)`.
       - Find candidate rules matching `op`.
       - For each candidate rule:
         - Generate candidate decomposition.
         - Verify local unitary equivalence against `op` (using `Matrix2x2`, `Matrix4x4`, or `Matrix8x8`).
         - Recursively translate any introduced operations that are not yet target-supported.
         - Sum recursive phase contributions: $\Delta \phi_{\text{total}} = \Delta \phi_{\text{candidate}} + \sum \Delta \phi_{\text{recursive}}$.
       - If multiple valid, fully-translated candidates exist:
         - Evaluate each using `cost_model.evaluate_physical_sequence(target, ops)`.
         - Select candidate with minimal `cost.score` (deterministic tie-breaking).
       - If no candidate succeeds:
         - Return `Err(TranslationError::NoViableTranslation)`.
  3. Transactionally replace unsupported nodes in the circuit (or construct the translated circuit preserving registers and metadata).
  4. Update circuit global phase: `circuit.global_phase = normalize_angle(circuit.global_phase + total_phase_delta)`.
  5. Run final target legality validator across all emitted operations.
  6. Return `TranslationReport` with metrics (translated operations count, decomposed operations count, initial/final gate counts, depths, cost scores).

### 4. Compilation Pass (`src/translation/pass.rs`)
- **`BasisTranslationPass`**:
  - Implements `CompilerPass`.
  - Pass ID: `PassId::new("basis_translation")`.
  - Semantic domain: `SemanticDomain::GeneralStructural`.
  - Mutation scope: `PassMutation::CircuitTopology`.
  - Wraps `BasisTranslationEngine` and records pass telemetry.

### 5. Structured Error Model (`src/translation/mod.rs`)
- `TranslationError`:
  - `UnsupportedOperation { op_name: String, operands: Vec<PhysicalQubitId>, reason: String }`
  - `NoViableTranslation { op_name: String, operands: Vec<PhysicalQubitId>, candidates_tried: usize }`
  - `CycleDetected { op_name: String, cycle: Vec<String> }`
  - `MaxRecursionDepthExceeded { op_name: String, depth: usize, max_depth: usize }`
  - `VerificationFailed { op_name: String, max_diff: f64, tolerance: f64 }`
  - `Legality(LegalityError)`
  - `Rewrite(RewriteError)`

## Required Tests (`tests/translation.rs` & `tests/translation/`)

1. **Already-Supported Preservation**: All native gates (`cx`, `rz`, `sx`, `measure`, `barrier`) remain unmodified.
2. **CX $\to$ CZ Translation**: Decomposes CX on CZ-native target using target Hadamards.
3. **CZ $\to$ CX Translation**: Decomposes CZ on CX-native target using target Hadamards.
4. **SWAP Translation**: Decomposes SWAP on CX-native target to 3 CX gates.
5. **Recursive Multi-Layer Translation**: SWAP on CZ-only target decomposes recursively (SWAP $\to$ 3 CX $\to$ 3 CZ + Hadamards $\to$ RZ/SX).
6. **Toffoli Translation**: 3-qubit CCX decomposes into 15 gates and verifies exact unitary matrix.
7. **Supported Rotation Translations**: 1Q unitaries ($H, X, Y, Z, S, T, R_y$) synthesize to $RZ\text{-}SX$ on transmon target.
8. **Symbolic Parameter Preservation**: $R_x(\theta)$ and $R_y(\theta)$ decompose to $RZ\text{-}SX$ sequences preserving `ParameterExpr::Symbol("theta")`.
9. **Cost-Based Candidate Selection**: When both CX-path and CZ-path exist, cost weights decide between them.
10. **Recursive Cycle Detection**: A deliberate $A \to B \to A$ rule loop cleanly returns `CycleDetected`.
11. **Recursion Depth Limit**: A deep recursive rule chain cleanly triggers `MaxRecursionDepthExceeded`.
12. **Explicit Translation Failure**: 2Q gate on target with no 2Q entanglers cleanly returns `NoViableTranslation`.
13. **Final Target Legality**: All emitted operations satisfy arity, parameter count/domains, and directed coupling.
14. **Local Equivalence Verification**: Corrupted rules are rejected during synthesis candidate validation.
