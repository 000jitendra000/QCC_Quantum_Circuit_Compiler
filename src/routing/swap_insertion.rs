//! SWAP operation insertion, layout mutation, and path-based operand movement.
//!
//! Inserts physical SWAP operations along shortest coupling paths, synchronously
//! updating the [`CircuitLayout`] and enforcing ADR-0001 strict ancilla zero-state tracking.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::routing::layout::CircuitLayout;
use crate::routing::pathfinding::{are_physical_neighbors, RoutingError};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Strategy for moving operands along a shortest physical path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PathSwapStrategy {
    /// Move the first operand (source) toward the second operand along the path.
    #[default]
    SourceToTarget,
    /// Move the second operand (target) toward the first operand along the reversed path.
    TargetToSource,
    /// Move both operands toward the path midpoint.
    Bidirectional,
}

/// Emits a single-qubit Hadamard gate or its native target-decomposed equivalent onto physical qubit `p`.
fn emit_hadamard(
    target: &Target,
    circuit: &mut Circuit,
    p: PhysicalQubitId,
) -> Result<(), RoutingError> {
    let q = QubitId(p.index());
    if target.is_operation_supported("h") {
        circuit.add_operation(Operation::custom_gate("h", vec![q], vec![]))?;
    } else if target.is_operation_supported("rz") && target.is_operation_supported("sx") {
        // Standard Euler decomposition of H: Rz(pi/2) * Sx * Rz(pi/2)
        let pi_2 = ParameterExpr::constant(PI / 2.0);
        circuit.add_operation(Operation::custom_gate("rz", vec![q], vec![pi_2.clone()]))?;
        circuit.add_operation(Operation::custom_gate("sx", vec![q], vec![]))?;
        circuit.add_operation(Operation::custom_gate("rz", vec![q], vec![pi_2]))?;
    } else {
        circuit.add_operation(Operation::custom_gate("h", vec![q], vec![]))?;
    }
    Ok(())
}

/// Emits a physical SWAP gate between physical qubits `u` and `v`, updating `layout` consistently.
///
/// If the target supports native `"swap"` operations on `(u, v)` or `(v, u)`, emits a native `SWAP`.
/// If the target does not support `"swap"` but supports `"cx"`, decomposes into native `CX` operations
/// with direction correction if required.
///
/// Returns the number of physical operations emitted into `circuit`.
pub fn emit_physical_swap(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    u: PhysicalQubitId,
    v: PhysicalQubitId,
) -> Result<usize, RoutingError> {
    if u == v {
        return Err(RoutingError::DuplicateOperand { qubit: u });
    }

    let coupling = target.combined_coupling_graph();
    if !are_physical_neighbors(&coupling, u, v) {
        return Err(RoutingError::DisconnectedTopology { src: u, dst: v });
    }

    // 1. Mutate active layout tracking
    layout.apply_physical_swap(u, v)?;

    let qu = QubitId(u.index());
    let qv = QubitId(v.index());

    // 2. Check native SWAP support
    if target.is_operation_supported("swap") {
        if target.is_valid_operand_tuple("swap", &[u, v]) {
            circuit.add_operation(Operation::custom_gate("swap", vec![qu, qv], vec![]))?;
            return Ok(1);
        } else if target.is_valid_operand_tuple("swap", &[v, u]) {
            circuit.add_operation(Operation::custom_gate("swap", vec![qv, qu], vec![]))?;
            return Ok(1);
        }
    }

    // 3. Decompose into CX if target supports CX
    if target.is_operation_supported("cx") {
        let uv_legal = target.is_valid_operand_tuple("cx", &[u, v]);
        let vu_legal = target.is_valid_operand_tuple("cx", &[v, u]);

        if uv_legal && vu_legal {
            // Symmetric undirected CX support: CX(u, v); CX(v, u); CX(u, v)
            circuit.add_operation(Operation::custom_gate("cx", vec![qu, qv], vec![]))?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qv, qu], vec![]))?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qu, qv], vec![]))?;
            return Ok(3);
        } else if uv_legal {
            // Directed CX u -> v only. Middle CX(v, u) requires Hadamard conjugation:
            // CX(u, v); H(u); H(v); CX(u, v); H(u); H(v); CX(u, v);
            circuit.add_operation(Operation::custom_gate("cx", vec![qu, qv], vec![]))?;
            emit_hadamard(target, circuit, u)?;
            emit_hadamard(target, circuit, v)?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qu, qv], vec![]))?;
            emit_hadamard(target, circuit, u)?;
            emit_hadamard(target, circuit, v)?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qu, qv], vec![]))?;
            return Ok(7);
        } else if vu_legal {
            // Directed CX v -> u only. Middle CX(u, v) requires Hadamard conjugation:
            // CX(v, u); H(v); H(u); CX(v, u); H(v); H(u); CX(v, u);
            circuit.add_operation(Operation::custom_gate("cx", vec![qv, qu], vec![]))?;
            emit_hadamard(target, circuit, v)?;
            emit_hadamard(target, circuit, u)?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qv, qu], vec![]))?;
            emit_hadamard(target, circuit, v)?;
            emit_hadamard(target, circuit, u)?;
            circuit.add_operation(Operation::custom_gate("cx", vec![qv, qu], vec![]))?;
            return Ok(7);
        }
    }

    // 4. Default fallback: emit standard SWAP gate
    circuit.add_operation(Operation::custom_gate("swap", vec![qu, qv], vec![]))?;
    Ok(1)
}

/// Moves two physical operands along a shortest coupling path until they become adjacent.
///
/// Executes physical SWAP operations according to `strategy` and updates `layout` after each SWAP.
///
/// Returns:
/// - `Ok((p_src_final, p_dst_final, swaps_inserted))`
pub fn route_operands_along_path(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    path: &[PhysicalQubitId],
    strategy: PathSwapStrategy,
) -> Result<(PhysicalQubitId, PhysicalQubitId, usize), RoutingError> {
    if path.is_empty() {
        return Err(RoutingError::TargetLegality(
            "empty path supplied to route_operands_along_path".to_string(),
        ));
    }
    if path.len() == 1 {
        return Err(RoutingError::DuplicateOperand { qubit: path[0] });
    }
    if path.len() == 2 {
        // Already adjacent!
        return Ok((path[0], path[1], 0));
    }

    let k = path.len();
    let mut total_swaps = 0;

    match strategy {
        PathSwapStrategy::SourceToTarget => {
            // Move source along path: v0 <-> v1, v1 <-> v2, ..., v_{k-2} <-> v_{k-1}
            for i in 0..(k - 2) {
                let u = path[i];
                let v = path[i + 1];
                emit_physical_swap(target, circuit, layout, u, v)?;
                total_swaps += 1;
            }
            Ok((path[k - 2], path[k - 1], total_swaps))
        }

        PathSwapStrategy::TargetToSource => {
            // Move target along reversed path: v_k <-> v_{k-1}, ..., v_2 <-> v_1
            for i in (2..k).rev() {
                let u = path[i];
                let v = path[i - 1];
                emit_physical_swap(target, circuit, layout, u, v)?;
                total_swaps += 1;
            }
            Ok((path[0], path[1], total_swaps))
        }

        PathSwapStrategy::Bidirectional => {
            // Move source forward m hops, target backward (k - 2 - m) hops
            let forward_hops = (k - 2) / 2;
            for i in 0..forward_hops {
                let u = path[i];
                let v = path[i + 1];
                emit_physical_swap(target, circuit, layout, u, v)?;
                total_swaps += 1;
            }

            let backward_hops = (k - 2) - forward_hops;
            for step in 0..backward_hops {
                let curr_idx = k - 1 - step;
                let u = path[curr_idx];
                let v = path[curr_idx - 1];
                emit_physical_swap(target, circuit, layout, u, v)?;
                total_swaps += 1;
            }

            let final_src = path[forward_hops];
            let final_dst = path[forward_hops + 1];
            Ok((final_src, final_dst, total_swaps))
        }
    }
}

/// Ensures an adjacent physical operand pair `(p_src, p_dst)` satisfies target directionality
/// constraints for `op_name`, inserting an additional adjacent SWAP if necessary.
///
/// Returns the legally oriented physical operand pair `(final_p1, final_p2)` and whether
/// an additional orientation SWAP was inserted.
pub fn ensure_legal_orientation(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    op_name: &str,
    p_src: PhysicalQubitId,
    p_dst: PhysicalQubitId,
) -> Result<(PhysicalQubitId, PhysicalQubitId, bool), RoutingError> {
    if p_src == p_dst {
        return Err(RoutingError::DuplicateOperand { qubit: p_src });
    }

    // 1. Check if forward tuple (p_src, p_dst) is directly legal
    if target.is_valid_operand_tuple(op_name, &[p_src, p_dst]) {
        return Ok((p_src, p_dst, false));
    }

    // 2. Check if reversed tuple (p_dst, p_src) is legal
    if target.is_valid_operand_tuple(op_name, &[p_dst, p_src]) {
        // Perform an adjacent SWAP between p_src and p_dst
        // After this SWAP, the logical source qubit is now at p_dst, and the logical target is at p_src.
        // Therefore, invoking the gate with first operand = location of logical source = p_dst,
        // and second operand = location of logical target = p_src, yields (p_dst, p_src), which IS legal!
        emit_physical_swap(target, circuit, layout, p_src, p_dst)?;
        return Ok((p_dst, p_src, true));
    }

    // 3. Neither orientation is legal on this coupling edge
    Err(RoutingError::NoViableDirection {
        operation: op_name.to_string(),
        u: p_src,
        v: p_dst,
        target: target.name.clone(),
    })
}
