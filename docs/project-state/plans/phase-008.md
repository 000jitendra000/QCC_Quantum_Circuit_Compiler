# Implementation Plan: Phase 8 — QccIR Execution Adapter

## 1. Context & Objectives
- **Phase Number**: Phase 8
- **Milestone**: Milestone M1: Bell Vertical Slice (`M1`)
- **Primary Goal**: Connect QccIR DAG representation to the statevector simulator with deterministic topological execution, unitary dispatch, terminal measurement sampling, global phase preservation, and strict rejection of dynamic feedback:
  - Pre-execution validation enforcing QccIR structural and semantic integrity.
  - Deterministic topological traversal order using Kahn's algorithm with stable tie-breaking.
  - Unitary gate dispatch to existing single-qubit (`single_qubit.rs`) and two-qubit (`two_qubit.rs`, `controlled.rs`) kernels without duplicating gate math.
  - Application of circuit-level global phase ($e^{i\phi}$) to final statevector amplitudes.
  - Terminal-only measurement integration with existing `TerminalSampler` without re-simulating the circuit per shot.
  - Rejection of unsupported mid-circuit measurement, active reset, and classical dynamic feedback.
  - Comprehensive execution trace containing deterministic operation identities (`OperationId`) and step descriptions.
  - Structured typed errors (`ExecutionError`).
  - Unit and integration tests comparing DAG execution against independent gate-list references.

## 2. Invariants & Conventions
1. **Reuse Existing Kernels**: Never allocate dense $2^n \times 2^n$ operators; dispatch directly to in-place $\mathcal{O}(2^n)$ kernels.
2. **Deterministic Topological Order**: Execution order strictly follows QccIR causal dependencies, never graph insertion order.
3. **Global Phase Preservation**: Explicit circuit global phase $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$ is applied directly to state amplitudes.
4. **No Unsupported Dynamic Execution**: Mid-circuit measurement, active reset, and classical feedback must be rejected with structured errors.
5. **Decoupled Trace**: `ExecutionTrace` records semantic `OperationId`s and execution metadata without polluting simulator mathematical state.

## 3. Module Architecture
- `src/simulator/statevector.rs`: Add `apply_global_phase(angle_rad)` method.
- `src/simulator/trace.rs`: `ExecutionTrace`, `ExecutionStep`, `MeasurementRecord`.
- `src/simulator/operation_dispatch.rs`: `dispatch_gate` function mapping QccIR `Operation::Gate` to `StateVector` kernels.
- `src/simulator/executor.rs`: `CircuitExecutor`, `ExecutionError`, `execute_circuit`, `sample_circuit`.
- `src/simulator/mod.rs`: Re-export executor and trace types.
- `src/errors.rs`: Add `Execution(#[from] ExecutionError)` to `QccError`.
- `src/lib.rs`: Re-export top-level executor items.
- `tests/simulator_dag.rs`: Exhaustive test suite covering Bell states, independent reference comparison, global phase, terminal sampling, and error rejection.

## 4. Execution Steps
1. Add `apply_global_phase` to `StateVector`.
2. Implement `src/simulator/trace.rs`.
3. Implement `src/simulator/operation_dispatch.rs`.
4. Implement `src/simulator/executor.rs` and `ExecutionError`.
5. Update `src/simulator/mod.rs`, `src/errors.rs`, and `src/lib.rs`.
6. Implement `tests/simulator_dag.rs`.
7. Verify full test suite and validation commands (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`, `cargo run --example bell`).
8. Create completion report and update progress ledger.
