# Phase 44 Completion Report — Logical/Physical Layout Model

## 1. Executive Summary

- **Phase Number**: Phase 44
- **Phase Title**: Logical/physical layout model
- **Milestone**: Milestone M5: Logical-to-Physical Mapping and Routing (`M5`)
- **Exit Gate**: Layout invariants are validated and round-trip correctly. Logical and physical identities must never be accidentally interchangeable (**PASSED**)
- **Baseline Git Checkpoint**: `60c9cd6` (`feat(phase-43): implement final target legality validator`)
- **Status**: Completed and fully verified with 100% test pass across all targets.

Phase 44 establishes the **Logical/Physical Layout Model** for the Quantum Circuit Compiler (QCC), initiating Milestone M5. It cleanly decouples logical program variables (`QubitId`) from physical device locations (`PhysicalQubitId`), providing an injective bidirectional mapping representation, explicit tracking of unused physical ancillae under ADR-0001, physical SWAP layout mutation tracking, multi-stage lifecycle layout triples ($L_{\text{initial}}$, $L_{\text{current}}$, $L_{\text{final}}$), and validating deterministic serialization.

Key architectural deliverables:
1. **Separation of Logical and Physical Identities**:
   - Reused existing logical [`QubitId`](file:///home/ronanki_jitendra/rust/QCC/src/ir/wire.rs#L10-L18) (wrapping `usize`, displaying as `q[i]`) with convenience alias `LogicalQubitId`.
   - Reused existing physical [`PhysicalQubitId`](file:///home/ronanki_jitendra/rust/QCC/src/target/operation.rs#L19-L50) (wrapping `u32`, displaying as `p[i]`).
   - The Rust type system strictly prohibits accidental interchangeability without explicit cast/conversion.
2. **Injective Layout Mapping Core ([`src/routing/layout_map.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout_map.rs))**:
   - [`LayoutMap`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout_map.rs#L155-L162) (and convenience alias `Layout`): Backed by deterministic `BTreeMap` structures for forward (`logical_to_physical`) and reverse (`physical_to_logical`) lookups.
   - Comprehensive query methods: `get_physical`, `get_logical`, `contains_logical`, `is_physical_used`, `is_physical_idle`, `assigned_logical_qubits`, `assigned_physical_qubits`, `unused_physical_qubits`, `unassigned_logical_qubits`.
   - Invariant validation (`validate`): Enforces bounds, equal map cardinalities, forward-to-reverse consistency, and reverse-to-forward consistency.
   - Structured error handling via [`LayoutError`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout_map.rs#L21-L90).
3. **Partial Mappings & ADR-0001 Ancilla Policy**:
   - Full support for circuits occupying a subset of physical qubits ($m \le n$).
   - Explicit identification of unused physical qubits (`unused_physical_qubits`) enabling downstream layout-aware verification (Phase 45) to verify the required zero-state condition ($|0\rangle$) without silent truncation.
4. **Physical SWAP Mutation**:
   - [`swap_physical(p1, p2)`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout_map.rs#L382-L425): Accurately updates active qubit assignments when a routing SWAP gate executes on physical wires:
     - Swapping two occupied sites interchanges their logical occupants.
     - Swapping an occupied site with an idle site migrates the logical qubit and frees the previous physical site.
     - Swapping two idle sites acts as a verified no-op.
5. **Multi-Stage Circuit Layout Lifecycle ([`src/routing/layout.rs`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout.rs))**:
   - [`CircuitLayout`](file:///home/ronanki_jitendra/rust/QCC/src/routing/layout.rs#L22-L29): Preserves initial layout ($L_{\text{initial}}$ at $t=0$), active layout during routing ($L_{\text{current}}$), and termination layout ($L_{\text{final}}$ after all routing SWAPs).
   - `apply_physical_swap` mutates `current` and synchronizes `final_layout` while guaranteeing that `initial` remains completely immutable.
6. **Deterministic Serialization & Validating Deserialization**:
   - Custom serde implementation via `RawLayoutMap` and `RawCircuitLayout` running `.validate()` during deserialization to reject duplicate assignments, out-of-bounds IDs, or inconsistent maps fail-closed.

---

## 2. Key Deliverables & Architecture

### 2.1 Distinct Logical and Physical Types

```rust
// Logical program variable (e.g., from QccIR circuit wire)
pub struct QubitId(pub usize); // displays as q[0]

// Physical device location (e.g., node in target coupling graph)
pub struct PhysicalQubitId(pub u32); // displays as p[0]
```
These types cannot be interchanged, preventing bugs where physical hardware indices are mistakenly treated as logical program wires or vice versa.

### 2.2 Routing SWAP Mutation Semantics

```text
Initial (Identity):
  q[0] -> p[0], q[1] -> p[1], idle: p[2], p[3]

Router Action: SWAP(p[0], p[1])
  q[0] -> p[1], q[1] -> p[0], idle: p[2], p[3]

Router Action: SWAP(p[1], p[2]) (migration into idle ancilla)
  q[0] -> p[2], q[1] -> p[0], idle: p[1], p[3]

Result:
  L_initial: q[0]->p[0], q[1]->p[1] (strictly preserved)
  L_final:   q[0]->p[2], q[1]->p[0] (active mapping at circuit exit)
  Ancillae:  p[1] and p[3] must satisfy ADR-0001 zero-state check in Phase 45
```

---

## 3. Verification & Test Evidence

### 3.1 Test Suite Breakdown (`tests/routing/layout.rs`)

| Test Name | Focus | Result |
| :--- | :--- | :---: |
| `test_identity_layouts_and_nontrivial_permutations` | Identity layout and non-trivial permutations with SWAP equivalence | **PASSED** |
| `test_partial_mappings_where_logical_qubits_occupy_part_of_device` | Partial device mappings and ancilla migration into idle qubits | **PASSED** |
| `test_forward_and_reverse_lookup_roundtrips` | Forward $q \to p$ and reverse $p \to q$ bidirectional lookup consistency | **PASSED** |
| `test_duplicate_physical_assignments_and_invalid_mappings` | Duplicate physical/logical assignments and bounds checks rejection | **PASSED** |
| `test_serialization_roundtrips_and_invalid_serialized_layouts` | Valid JSON roundtrip and fail-closed deserialization rejection | **PASSED** |
| `test_layouts_with_different_logical_and_physical_capacities` | Capacity bounds, `CapacityExceeded`, and mismatch rejection | **PASSED** |
| `test_logical_and_physical_identities_distinctness` | Type safety, representation differences, and compile-time separation | **PASSED** |
| `test_circuit_layout_tracking_across_routing_swaps` | $L_{\text{initial}}$ preservation, $L_{\text{final}}$ mutation, and ancilla tracking | **PASSED** |
| `test_target_and_circuit_integration` | End-to-end `Circuit` and `Target` layout construction | **PASSED** |

### 3.2 Verification Commands Summary

```bash
# 1. Routing integration tests
cargo test --test routing
# Output: 9 passed; 0 failed; 0 ignored

# 2. Workspace unit tests (including src/routing/ tests)
cargo test --lib
# Output: 104 passed; 0 failed; 0 ignored

# 3. Full test suite (all 35 test targets)
cargo test
# Output: 1,080 passed; 0 failed; 0 ignored

# 4. Code formatting check
cargo fmt --check
# Output: Clean (0 diffs)

# 5. Clippy lints check
cargo clippy --all-targets -- -D warnings
# Output: Clean (0 warnings, 0 errors)

# 6. Rustdoc generation check
cargo doc --no-deps
# Output: Clean (0 warnings, 0 dead links)

# 7. Independent OpenQASM 3 Reference Parser (pinned v1.0.1)
PYTHONPATH=/tmp/py_packages cargo test test_independent -- --nocapture
# Output: 5 passed; 0 failed; 0 ignored

# 8. Milestone Executables
cargo run --example bell
cargo run --example qasm
# Output: Both executed successfully
```

---

## 4. Progress Ledger & Next Steps

- **Milestone M5 Progress**: Phase 44 is completed (1 / 10 phases complete in M5).
- **Total Project Progress**: 44 / 105 phases complete.
- **Next Phase**: **Phase 45: Layout-Aware Equivalence** (verifying routed circuits accounting for $L_{\text{initial}}$, $L_{\text{final}}$, and ADR-0001 ancilla zero-state preservation).
