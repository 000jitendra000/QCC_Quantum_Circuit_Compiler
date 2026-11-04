# Phase 43 Implementation Plan — Final Target Legality Validator

## Context & Objectives

Phase 43 of the QCC engineering roadmap implements the **Final Target Legality Validator**.
The objective is to provide an independent, authoritative validator and compiler pass that checks physical quantum circuit operations against a hardware `Target` model:
1. Validate operation identity and declared quantum arity.
2. Validate parameter counts and continuous parameter domains (for both concrete numbers and symbolic expressions).
3. Validate physical qubit IDs against target device capacity and verify no duplicate operands exist.
4. Validate operand directionality and coupling connectivity (distinguishing asymmetric directed coupling from symmetric undirected coupling).
5. Produce precise, actionable compiler diagnostics identifying the offending node (`OperationId`), sequence number, operation mnemonic, physical operands, and the exact target-specific reason for failure.
6. Support execution:
   - Immediately following basis translation (Phase 41/42).
   - Again after routing (Milestone M5) or any subsequent compiler pass/rewrite that alters operations or operand allocations.
7. Preserve the "Explicit Unknown" principle: missing timing or calibration metrics must **never** invalidate an otherwise legal operation, and missing metadata must not be fabricated.

## Roadmap Specification

- **Phase Number**: Phase 43
- **Milestone**: Milestone M4: Generic Target and Basis Translation (`M4`)
- **Effort**: **M**
- **Prerequisites**: Phases 37–42 (Target data model, capabilities, cost models, templates, single-qubit synthesis, basis translation engine, two-qubit direction correction).
- **Exit Gate**: All intentionally illegal target fixtures fail with useful diagnostics, while valid fixtures pass. Milestone M4 complete; unblocks Milestone M5.

## Architecture & Design

### 1. Target Diagnostics (`src/diagnostics/target.rs`)
- **`TargetDiagnosticCode` Enum**:
  - `T0001_UnsupportedOperation`: Operation mnemonic not declared in target basis.
  - `T0002_ArityMismatch`: Operand count does not match operation quantum arity.
  - `T0003_QubitOutOfBounds`: Physical qubit index exceeds device capacity.
  - `T0004_DuplicateOperandQubit`: Duplicate physical qubit in operand tuple.
  - `T0005_IllegalCoupling`: Coupling graph edge or directionality violation.
  - `T0006_ParameterCountMismatch`: Parameter count mismatch against declared domains.
  - `T0007_NonFiniteParameter`: Parameter is NaN or infinite.
  - `T0008_ParameterDomainViolation`: Parameter value violates domain bounds.
- **Diagnostic Generation**:
  - Helper `target_diagnostic(...) -> Diagnostic` attaching standardized codes, descriptive error messages, node IDs, and target-specific notes.

### 2. Target Legality Validator & Pass (`src/target/legality.rs`)
- **`TargetLegalityValidator` Struct**:
  - `target: Target`
  - `allow_barriers: bool` (default: true for synchronization barriers on valid qubits)
  - `validate_circuit(&self, circuit: &Circuit) -> Result<TargetLegalityReport, TargetLegalityError>`
  - `validate_node(&self, node: &NodeData) -> Result<(), TargetViolation>`
  - `is_circuit_legal(&self, circuit: &Circuit) -> bool`
- **`TargetViolation` Struct**:
  - Node ID (`OperationId`), sequence number, operation name, physical operands, underlying `LegalityError`, and rich compiler `Diagnostic`.
- **`TargetLegalityReport` Struct**:
  - Telemetry summary (`operations_examined`, `is_legal`, `violations`, `diagnostics`).
- **`TargetLegalityError` Enum**:
  - `ValidationFailed { circuit, target, violation_count, details, violations }`
  - `DagError(String)`
- **`TargetLegalityPass` Struct**:
  - Implements `CompilerPass` with `PassMutation::AnalysisOnly` for execution in `PassManager` pipelines.

### 3. Re-exports & Workspace Integration
- Re-export `TargetLegalityValidator`, `TargetLegalityPass`, `TargetLegalityReport`, `TargetLegalityError`, and `TargetViolation` in `src/target/mod.rs` and `src/lib.rs`.
- Re-export `target` diagnostic codes and helpers in `src/diagnostics/mod.rs` and `src/lib.rs`.

## Planned Tests (`tests/target_legality.rs` & `tests/target/legality.rs`)

1. **Valid Target Circuits**:
   - Circuits matching target basis, arity, parameters, and physical coupling pass validation cleanly.
2. **Unsupported Operation**:
   - Circuits with undeclared gates fail with `T0001` and list available operations.
3. **Arity Mismatch**:
   - Supplying wrong number of qubits (e.g. 1Q gate with 2 qubits, or 2Q gate with 1 qubit) fails with `T0002`.
4. **Physical Qubit Range / Out of Bounds**:
   - Accessing qubit $N \ge \text{num\_qubits}$ fails with `T0003` showing capacity.
5. **Duplicate Qubit Operands**:
   - `cx(p0, p0)` fails with `T0004`.
6. **Illegal Coupling & Directionality Violations**:
   - Operating on disconnected physical pairs fails with `T0005`.
   - Directed CX on $(0, 1)$ when only $(1, 0)$ is supported fails with `T0005`, noting reversed orientation availability.
7. **Parameter Count & Domain Violations**:
   - Parameter count mismatch fails with `T0006`.
   - `NaN` or `Inf` parameters fail with `T0007`.
8. **Missing Metadata Tolerance**:
   - Target operations with `None` duration and calibration remain completely legal.
9. **Revalidation After Circuit Rewrite**:
   - Circuit passes validation, undergoes an illegal rewrite (e.g. reversing a directed CX or changing to out-of-bounds qubit), and revalidation immediately catches the violation.
10. **PassManager Pipeline Integration**:
    - `TargetLegalityPass` runs successfully in a pipeline after `BasisTranslationPass` and `TwoQubitDirectionPass`.
