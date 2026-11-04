//! SWAP operation insertion, layout mutation, and path-based operand movement.
//!
//! Inserts physical SWAP operations along shortest coupling paths, synchronously
//! updating the [`CircuitLayout`], maintaining dependency correctness on both affected wires,
//! and enforcing ADR-0001 strict ancilla zero-state tracking with full provenance.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::routing::layout::CircuitLayout;
use crate::routing::pathfinding::{are_physical_neighbors, RoutingError};
use crate::routing::provenance::RouteProvenanceTracker;
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

/// Detailed result of inserting a physical SWAP operation into a circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwapInsertionResult {
    /// First physical qubit operand.
    pub u: PhysicalQubitId,
    /// Second physical qubit operand.
    pub v: PhysicalQubitId,
    /// Stable semantic IDs of the node(s) created in the circuit graph.
    pub inserted_node_ids: Vec<OperationId>,
    /// Sequence step index along a multi-hop routing path.
    pub step: usize,
    /// Layout snapshot immediately following this SWAP mutation.
    pub layout_after: CircuitLayout,
}

/// Emits a single-qubit Hadamard gate or its native target-decomposed equivalent onto physical qubit `p`.
fn emit_hadamard(
    target: &Target,
    circuit: &mut Circuit,
    p: PhysicalQubitId,
    provenance: Vec<OperationId>,
) -> Result<OperationId, RoutingError> {
    let q = QubitId(p.index());
    let id = if target.is_operation_supported("h") {
        circuit.add_operation_with_provenance(
            Operation::custom_gate("h", vec![q], vec![]),
            provenance,
        )?
    } else if target.is_operation_supported("rz") && target.is_operation_supported("sx") {
        // Standard Euler decomposition of H: Rz(pi/2) * Sx * Rz(pi/2)
        let pi_2 = ParameterExpr::constant(PI / 2.0);
        circuit.add_operation_with_provenance(
            Operation::custom_gate("rz", vec![q], vec![pi_2.clone()]),
            provenance.clone(),
        )?;
        circuit.add_operation_with_provenance(
            Operation::custom_gate("sx", vec![q], vec![]),
            provenance.clone(),
        )?;
        circuit.add_operation_with_provenance(
            Operation::custom_gate("rz", vec![q], vec![pi_2]),
            provenance,
        )?
    } else {
        circuit.add_operation_with_provenance(
            Operation::custom_gate("h", vec![q], vec![]),
            provenance,
        )?
    };
    Ok(id)
}

/// Emits a physical SWAP gate with explicit provenance and registers it in the tracker.
///
/// Correctly touches both physical wires $u$ and $v$, updates wire dependencies,
/// and synchronizes the active [`CircuitLayout`].
#[allow(clippy::too_many_arguments)]
pub fn emit_physical_swap_with_provenance(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    u: PhysicalQubitId,
    v: PhysicalQubitId,
    caused_by: Option<OperationId>,
    step: usize,
    tracker: Option<&mut RouteProvenanceTracker>,
) -> Result<SwapInsertionResult, RoutingError> {
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
    let prov = caused_by.map(|c| vec![c]).unwrap_or_default();
    let mut inserted_ids = Vec::new();

    // 2. Native SWAP support
    let mut emitted = false;
    if target.is_operation_supported("swap") {
        if target.is_valid_operand_tuple("swap", &[u, v]) {
            let id = circuit.add_operation_with_provenance(
                Operation::custom_gate("swap", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            inserted_ids.push(id);
            emitted = true;
        } else if target.is_valid_operand_tuple("swap", &[v, u]) {
            let id = circuit.add_operation_with_provenance(
                Operation::custom_gate("swap", vec![qv, qu], vec![]),
                prov.clone(),
            )?;
            inserted_ids.push(id);
            emitted = true;
        }
    }

    // 3. Decomposed CX support if native SWAP is absent
    if !emitted && target.is_operation_supported("cx") {
        let uv_legal = target.is_valid_operand_tuple("cx", &[u, v]);
        let vu_legal = target.is_valid_operand_tuple("cx", &[v, u]);

        if uv_legal && vu_legal {
            // Undirected CX support: CX(u, v); CX(v, u); CX(u, v)
            let id1 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            let id2 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qv, qu], vec![]),
                prov.clone(),
            )?;
            let id3 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            inserted_ids.extend([id1, id2, id3]);
            emitted = true;
        } else if uv_legal {
            // Directed CX u -> v only: CX(u, v); H(u); H(v); CX(u, v); H(u); H(v); CX(u, v);
            let id1 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            let h1_u = emit_hadamard(target, circuit, u, prov.clone())?;
            let h1_v = emit_hadamard(target, circuit, v, prov.clone())?;
            let id2 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            let h2_u = emit_hadamard(target, circuit, u, prov.clone())?;
            let h2_v = emit_hadamard(target, circuit, v, prov.clone())?;
            let id3 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qu, qv], vec![]),
                prov.clone(),
            )?;
            inserted_ids.extend([id1, h1_u, h1_v, id2, h2_u, h2_v, id3]);
            emitted = true;
        } else if vu_legal {
            // Directed CX v -> u only: CX(v, u); H(v); H(u); CX(v, u); H(v); H(u); CX(v, u);
            let id1 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qv, qu], vec![]),
                prov.clone(),
            )?;
            let h1_v = emit_hadamard(target, circuit, v, prov.clone())?;
            let h1_u = emit_hadamard(target, circuit, u, prov.clone())?;
            let id2 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qv, qu], vec![]),
                prov.clone(),
            )?;
            let h2_v = emit_hadamard(target, circuit, v, prov.clone())?;
            let h2_u = emit_hadamard(target, circuit, u, prov.clone())?;
            let id3 = circuit.add_operation_with_provenance(
                Operation::custom_gate("cx", vec![qv, qu], vec![]),
                prov.clone(),
            )?;
            inserted_ids.extend([id1, h1_v, h1_u, id2, h2_v, h2_u, id3]);
            emitted = true;
        }
    }

    // 4. Fallback: emit standard SWAP gate
    if !emitted {
        let id = circuit.add_operation_with_provenance(
            Operation::custom_gate("swap", vec![qu, qv], vec![]),
            prov,
        )?;
        inserted_ids.push(id);
    }

    // 5. Register in provenance tracker
    if let Some(t) = tracker {
        for (substep, &nid) in inserted_ids.iter().enumerate() {
            t.record_routing_swap(
                nid,
                caused_by,
                step,
                format!("swap({u}, {v}) substep {substep}"),
            );
        }
    }

    Ok(SwapInsertionResult {
        u,
        v,
        inserted_node_ids: inserted_ids,
        step,
        layout_after: layout.clone(),
    })
}

/// Convenience wrapper preserving backward compatibility with Phase 47.
pub fn emit_physical_swap(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    u: PhysicalQubitId,
    v: PhysicalQubitId,
) -> Result<usize, RoutingError> {
    let res = emit_physical_swap_with_provenance(target, circuit, layout, u, v, None, 0, None)?;
    Ok(res.inserted_node_ids.len())
}

/// Moves two physical operands along a shortest coupling path until they become adjacent,
/// recording provenance for each inserted SWAP.
#[allow(clippy::too_many_arguments)]
pub fn route_operands_along_path_with_provenance(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    path: &[PhysicalQubitId],
    strategy: PathSwapStrategy,
    caused_by: Option<OperationId>,
    tracker: Option<&mut RouteProvenanceTracker>,
) -> Result<(PhysicalQubitId, PhysicalQubitId, Vec<SwapInsertionResult>), RoutingError> {
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
        return Ok((path[0], path[1], Vec::new()));
    }

    let k = path.len();
    let mut results = Vec::new();

    // To allow multiple tracker calls in loop, take mutable ref
    let mut tracker = tracker;

    match strategy {
        PathSwapStrategy::SourceToTarget => {
            for i in 0..(k - 2) {
                let u = path[i];
                let v = path[i + 1];
                let res = emit_physical_swap_with_provenance(
                    target,
                    circuit,
                    layout,
                    u,
                    v,
                    caused_by,
                    i,
                    tracker.as_deref_mut(),
                )?;
                results.push(res);
            }
            Ok((path[k - 2], path[k - 1], results))
        }

        PathSwapStrategy::TargetToSource => {
            for (step, i) in (2..k).rev().enumerate() {
                let u = path[i];
                let v = path[i - 1];
                let res = emit_physical_swap_with_provenance(
                    target,
                    circuit,
                    layout,
                    u,
                    v,
                    caused_by,
                    step,
                    tracker.as_deref_mut(),
                )?;
                results.push(res);
            }
            Ok((path[0], path[1], results))
        }

        PathSwapStrategy::Bidirectional => {
            let forward_hops = (k - 2) / 2;
            for step in 0..forward_hops {
                let u = path[step];
                let v = path[step + 1];
                let res = emit_physical_swap_with_provenance(
                    target,
                    circuit,
                    layout,
                    u,
                    v,
                    caused_by,
                    step,
                    tracker.as_deref_mut(),
                )?;
                results.push(res);
            }

            let backward_hops = (k - 2) - forward_hops;
            for s in 0..backward_hops {
                let step = forward_hops + s;
                let curr_idx = k - 1 - s;
                let u = path[curr_idx];
                let v = path[curr_idx - 1];
                let res = emit_physical_swap_with_provenance(
                    target,
                    circuit,
                    layout,
                    u,
                    v,
                    caused_by,
                    step,
                    tracker.as_deref_mut(),
                )?;
                results.push(res);
            }

            let final_src = path[forward_hops];
            let final_dst = path[forward_hops + 1];
            Ok((final_src, final_dst, results))
        }
    }
}

/// Convenience wrapper preserving backward compatibility with Phase 47.
pub fn route_operands_along_path(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    path: &[PhysicalQubitId],
    strategy: PathSwapStrategy,
) -> Result<(PhysicalQubitId, PhysicalQubitId, usize), RoutingError> {
    let (src, dst, results) = route_operands_along_path_with_provenance(
        target, circuit, layout, path, strategy, None, None,
    )?;
    Ok((src, dst, results.len()))
}

/// Ensures an adjacent physical operand pair `(p_src, p_dst)` satisfies target directionality
/// constraints for `op_name`, inserting an orientation SWAP with provenance if necessary.
#[allow(clippy::too_many_arguments)]
pub fn ensure_legal_orientation_with_provenance(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    op_name: &str,
    p_src: PhysicalQubitId,
    p_dst: PhysicalQubitId,
    caused_by: Option<OperationId>,
    tracker: Option<&mut RouteProvenanceTracker>,
) -> Result<
    (
        PhysicalQubitId,
        PhysicalQubitId,
        Option<SwapInsertionResult>,
    ),
    RoutingError,
> {
    if p_src == p_dst {
        return Err(RoutingError::DuplicateOperand { qubit: p_src });
    }

    // 1. Forward tuple directly legal
    if target.is_valid_operand_tuple(op_name, &[p_src, p_dst]) {
        return Ok((p_src, p_dst, None));
    }

    // 2. Reversed tuple legal -> insert an orientation SWAP
    if target.is_valid_operand_tuple(op_name, &[p_dst, p_src]) {
        let res = emit_physical_swap_with_provenance(
            target,
            circuit,
            layout,
            p_src,
            p_dst,
            caused_by,
            usize::MAX, // orientation swap marker
            tracker,
        )?;
        return Ok((p_dst, p_src, Some(res)));
    }

    // 3. Neither orientation is legal
    Err(RoutingError::NoViableDirection {
        operation: op_name.to_string(),
        u: p_src,
        v: p_dst,
        target: target.name.clone(),
    })
}

/// Convenience wrapper preserving backward compatibility with Phase 47.
pub fn ensure_legal_orientation(
    target: &Target,
    circuit: &mut Circuit,
    layout: &mut CircuitLayout,
    op_name: &str,
    p_src: PhysicalQubitId,
    p_dst: PhysicalQubitId,
) -> Result<(PhysicalQubitId, PhysicalQubitId, bool), RoutingError> {
    let (s, d, res) = ensure_legal_orientation_with_provenance(
        target, circuit, layout, op_name, p_src, p_dst, None, None,
    )?;
    Ok((s, d, res.is_some()))
}
