# Phase 33 Implementation Plan — Commutation Analysis Rules

## 1. Overview & Objectives

Phase 33 implements mathematically verified commutation analysis rules for quantum operations in QCC. This layer provides exact, verified preconditions for determining whether two operations commute without mutating the circuit or DAG (DAG-aware rewriting is deferred to Phase 34).

The rules enforce:
- Exact pairwise matrix verification: every commuting pair satisfies $B \cdot A = A \cdot B$.
- Disjoint-wire commutation.
- $Z$-axis operations ($Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$) on $CX$ control.
- $X$-axis operations ($X, R_x(\theta)$) on $CX$ target.
- Pauli commutation vs anticommutation on single and multi-qubit wires.
- Multi-qubit controlled gate commutation ($CZ$ with $Z$-axis, $CX$ shared controls, $CX$ shared targets).
- Non-unitary exclusions (barriers, measurements, resets, classical operations).

## 2. Source Files to Create & Modify

### Files to Create
1. `src/commutation/mod.rs`: Core commutation rule registry, result types, preconditions, non-commutation reasons, and public commutation checking API.
2. `src/commutation/pauli.rs`: Pauli algebra, single- and multi-qubit Pauli commutation and anticommutation rules, axis classification.
3. `src/commutation/cx.rs`: Dedicated rules for $CX$ and controlled gate commutation ($Z$-axis on control, $X$-axis on target, $CZ$ rules).
4. `tests/optimizer/commutation.rs`: Comprehensive test suite with matrix verification, negative cases, non-unitary exclusions, and Phase 10 equivalence checks.

### Files to Modify
1. `src/lib.rs`: Expose `pub mod commutation;`.
2. `tests/optimizer.rs`: Link `#[path = "optimizer/commutation.rs"] mod commutation;`.
3. `docs/project-state/progress.md`: Update ledger upon verification.

## 3. Ordered Implementation Tasks

1. **Rule Registry & Semantic Preconditions (`src/commutation/mod.rs`)**:
   - Define `CommutationRule` enum with explicit matched rules.
   - Define `NonCommutationReason` enum with precise exclusion explanations.
   - Define `CommutationResult` enum with structured outcomes (`Commutes`, `DoesNotCommute`, `Unknown`).
   - Define `CommutationPrecondition` specifying supported operation types, operand constraints, parameter constraints, and conditionality.
   - Implement `CommutationRegistry` with deterministic rule dispatch.

2. **Pauli Algebra & Axis Classification (`src/commutation/pauli.rs`)**:
   - Classify single-qubit gates into rotation axes: $X, Y, Z$.
   - Implement single-qubit Pauli commutation ($[P_i, P_i] = 0$) and anticommutation ($\{P_i, P_j\} = 0$ for $i \neq j$).
   - Implement multi-qubit Pauli string commutation based on parity of pairwise single-qubit anticommutations.
   - Implement same-axis rotation commutation ($[R_a(\theta_1), R_a(\theta_2)] = 0$).

3. **CX & Controlled Gate Rules (`src/commutation/cx.rs`)**:
   - Implement $Z$-axis on $CX$ control rule: $Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$ on operand 0.
   - Implement $X$-axis on $CX$ target rule: $X, R_x(\theta)$ on operand 1.
   - Implement $CZ$ diagonal commutation: any $Z$-axis gate on either operand of $CZ$, and $CZ$ with $CZ$.
   - Implement $CX$-$CX$ commutation: shared control with disjoint targets, shared target with disjoint controls.
   - Enforce negative cases: $X$ on control, $Z$ on target, reversed $CX$ operands ($CX(0, 1)$ with $CX(1, 0)$).

4. **Disjoint Wires & Non-Unitary Exclusions (`src/commutation/mod.rs`)**:
   - Exclude non-unitary operations (`Measure`, `Reset`, `Barrier`, `Classical`) by default.
   - Check classical read/write hazards (RAW, WAR, WAW).
   - Commute operations with disjoint quantum and classical wires.

5. **Exhaustive Testing (`tests/optimizer/commutation.rs`)**:
   - Pairwise matrix product comparison ($B \cdot A \approx A \cdot B$) for all enabled rules.
   - Negative test cases verifying non-commuting pairs are rejected.
   - Anticommutation test cases verifying anticommuting Paulis are never classified as commuting.
   - Parameterized rotation sweeps across boundary angles ($0, \pi/2, \pi, 2\pi, 4\pi$) and random angles.
   - Phase 10 circuit unitary equivalence integration.

## 4. Verification Protocol
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo build --all-targets`
5. `cargo doc --no-deps`
6. `cargo run --example bell && cargo run --example qasm`
7. Independent OpenQASM 3 reference parser validation.
