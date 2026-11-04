//! Baseline Shortest-Path Router.
//!
//! A deterministic pathfinding router using BFS/Dijkstra on physical coupling graphs
//! to route executable two-qubit operations while maintaining exact layout tracking.
//!
//! # Architecture Invariants
//! - **Identity Distinction**: Logical qubits and physical qubits are strictly separated.
//! - **Deterministic Traversal**: Dependency-respecting DAG scheduling using Kahn's algorithm
//!   with tie-breaking by stable [`OperationId`].
//! - **ADR-0001 Ancilla Policy**: Unused physical qubits start in $|0\rangle$ and layout
//!   tracking accurately captures their final locations.
//! - **Target Legality**: All routed physical gates satisfy target operand tuples and directionality.
//! - **Not SABRE**: This is the baseline shortest-path router and must never be labeled as SABRE.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::LayoutMap;
use crate::routing::pathfinding::{find_shortest_path, RoutingError};
use crate::routing::placement::{place_circuit, PlacementStrategy};
use crate::routing::swap_insertion::{
    ensure_legal_orientation, route_operands_along_path, PathSwapStrategy,
};
use crate::target::legality::TargetLegalityValidator;
use crate::target::model::Target;
use crate::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Configuration options for the baseline shortest-path router.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineRouterConfig {
    /// Strategy for computing initial layout if not provided explicitly by caller.
    pub placement_strategy: PlacementStrategy,
    /// SWAP movement strategy along shortest paths.
    pub swap_strategy: PathSwapStrategy,
    /// Maximum number of physical SWAPs permitted before aborting.
    pub max_swaps: usize,
    /// Maximum number of scheduling loop iterations.
    pub max_iterations: usize,
    /// Whether to automatically verify layout-aware equivalence on small circuits ($\le 16$ qubits).
    pub verify_equivalence: bool,
    /// Whether to validate target legality of the routed physical circuit.
    pub validate_target_legality: bool,
}

impl Default for BaselineRouterConfig {
    fn default() -> Self {
        Self {
            placement_strategy: PlacementStrategy::Weighted,
            swap_strategy: PathSwapStrategy::SourceToTarget,
            max_swaps: 10_000,
            max_iterations: 100_000,
            verify_equivalence: true,
            validate_target_legality: true,
        }
    }
}

/// Comprehensive report produced by the baseline router upon successful circuit routing.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineRouterReport {
    /// Total number of operations in the original logical circuit.
    pub original_operations: usize,
    /// Total number of operations in the routed physical circuit.
    pub routed_operations: usize,
    /// Number of physical SWAP operations inserted during routing.
    pub swaps_inserted: usize,
    /// Multi-stage layout tracking initial, current, and final qubit assignments.
    pub layout: CircuitLayout,
}

/// The Baseline Shortest-Path Router.
#[derive(Clone, Debug)]
pub struct BaselineRouter {
    config: BaselineRouterConfig,
}

impl Default for BaselineRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl BaselineRouter {
    /// Creates a new baseline router with default configuration.
    pub fn new() -> Self {
        Self {
            config: BaselineRouterConfig::default(),
        }
    }

    /// Creates a new baseline router with custom configuration.
    pub fn with_config(config: BaselineRouterConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the active configuration.
    pub fn config(&self) -> &BaselineRouterConfig {
        &self.config
    }

    /// Routes a logical [`Circuit`] onto the hardware [`Target`] device.
    ///
    /// Accepts an optional pre-computed initial layout. If `None`, computes the initial
    /// placement using the configured `PlacementStrategy`.
    ///
    /// # Errors
    /// Returns [`RoutingError::InsufficientCapacity`] if the target has fewer physical qubits
    /// than the circuit requires.
    /// Returns [`RoutingError::DisconnectedTopology`] if interacting qubits are in unreachable components.
    /// Returns [`RoutingError::IterationLimitExceeded`] if SWAP or iteration limits are breached.
    pub fn route_circuit(
        &self,
        circuit: &Circuit,
        target: &Target,
        initial_layout: Option<LayoutMap>,
    ) -> Result<(Circuit, BaselineRouterReport), RoutingError> {
        let num_logical = circuit.num_qubits();
        let num_physical = target.num_qubits();

        if num_physical < num_logical {
            return Err(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            });
        }

        // 1. Determine initial layout
        let initial_map = match initial_layout {
            Some(layout) => {
                layout.validate()?;
                if layout.num_logical() != num_logical || layout.num_physical() != num_physical {
                    return Err(RoutingError::InsufficientCapacity {
                        required: num_logical,
                        capacity: num_physical,
                    });
                }
                layout
            }
            None => {
                let placement_res = place_circuit(circuit, target, self.config.placement_strategy)?;
                placement_res.layout
            }
        };

        let mut circuit_layout = CircuitLayout::new(initial_map);
        let coupling = target.combined_coupling_graph();

        // 2. Initialize routed physical circuit
        let mut routed = Circuit::new(format!("{}_routed", circuit.name));
        routed
            .add_quantum_register("p", num_physical)
            .map_err(|e| RoutingError::Ir(e.to_string()))?;

        for reg in &circuit.classical_registers {
            routed
                .add_classical_register(&reg.name, reg.size)
                .map_err(|e| RoutingError::Ir(e.to_string()))?;
        }
        routed.set_global_phase(circuit.global_phase().clone());

        // 3. Compute initial DAG in-degrees for Kahn's deterministic traversal
        let mut in_degrees: HashMap<OperationId, usize> = HashMap::new();
        let mut ready_set: BTreeSet<OperationId> = BTreeSet::new();

        let ordered_node_ids = circuit
            .graph
            .deterministic_topological_order()
            .map_err(|e| RoutingError::Ir(e.to_string()))?;

        for &id in &ordered_node_ids {
            let deg = circuit.graph.predecessors(id).len();
            in_degrees.insert(id, deg);
            if deg == 0 {
                ready_set.insert(id);
            }
        }

        let mut iterations = 0;
        let mut swaps_inserted = 0;
        let mut completed_count = 0;

        // 4. Scheduling & Routing Loop
        while !ready_set.is_empty() {
            iterations += 1;
            if iterations > self.config.max_iterations {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_iterations,
                    details: format!(
                        "scheduling loop exceeded {} iterations",
                        self.config.max_iterations
                    ),
                });
            }
            if swaps_inserted > self.config.max_swaps {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_swaps,
                    details: format!("inserted SWAPs exceeded limit of {}", self.config.max_swaps),
                });
            }

            // Find best ready operation to schedule:
            // Priority 1: Ready single-qubit / non-2Q operations (can be emitted immediately)
            // Priority 2: Ready 2-qubit operations that are already legal on their mapped qubits
            // Priority 3: First ready 2-qubit operation (lowest OperationId for determinism)
            let mut chosen_op_id: Option<OperationId> = None;

            for &id in &ready_set {
                let op = circuit.graph.get_operation(id).expect("valid node");
                match op {
                    Operation::Gate { qubits, .. } if qubits.len() == 2 => {
                        let q1 = qubits[0];
                        let q2 = qubits[1];
                        let p1 = circuit_layout.current().get_physical(q1).ok_or_else(|| {
                            RoutingError::Layout(format!(
                                "logical qubit {q1} not assigned in layout"
                            ))
                        })?;
                        let p2 = circuit_layout.current().get_physical(q2).ok_or_else(|| {
                            RoutingError::Layout(format!(
                                "logical qubit {q2} not assigned in layout"
                            ))
                        })?;

                        let gate_name = match op {
                            Operation::Gate { gate_name, .. } => gate_name.as_str(),
                            _ => unreachable!(),
                        };

                        if target.is_valid_operand_tuple(gate_name, &[p1, p2]) {
                            // Directly legal! Priority 2 candidate
                            chosen_op_id = Some(id);
                            break;
                        }
                    }
                    _ => {
                        // Single-qubit, barrier, measure, reset, classical: Priority 1
                        chosen_op_id = Some(id);
                        break;
                    }
                }
            }

            // If no immediately legal operation was found, pick the first ready 2-qubit operation
            let op_id = chosen_op_id.unwrap_or_else(|| *ready_set.iter().next().unwrap());
            let op = circuit
                .graph
                .get_operation(op_id)
                .expect("valid node")
                .clone();

            // Process the operation
            match op {
                Operation::Gate {
                    gate_name,
                    qubits,
                    parameters,
                } if qubits.len() == 2 => {
                    let q_src = qubits[0];
                    let q_dst = qubits[1];
                    let p_src = circuit_layout
                        .current()
                        .get_physical(q_src)
                        .ok_or_else(|| {
                            RoutingError::Layout(format!("qubit {q_src} unmapped in layout"))
                        })?;
                    let p_dst = circuit_layout
                        .current()
                        .get_physical(q_dst)
                        .ok_or_else(|| {
                            RoutingError::Layout(format!("qubit {q_dst} unmapped in layout"))
                        })?;

                    if p_src == p_dst {
                        return Err(RoutingError::DuplicateOperand { qubit: p_src });
                    }

                    // Check if already legal on (p_src, p_dst)
                    if target.is_valid_operand_tuple(&gate_name, &[p_src, p_dst]) {
                        // Emit directly
                        routed.add_operation(Operation::Gate {
                            gate_name,
                            qubits: vec![QubitId(p_src.index()), QubitId(p_dst.index())],
                            parameters,
                        })?;
                    } else {
                        // Find shortest coupling path between p_src and p_dst
                        let path = find_shortest_path(&coupling, p_src, p_dst)?;

                        // Route operands along path until adjacent
                        let (adj_src, adj_dst, path_swaps) = route_operands_along_path(
                            target,
                            &mut routed,
                            &mut circuit_layout,
                            &path,
                            self.config.swap_strategy,
                        )?;
                        swaps_inserted += path_swaps;

                        // Ensure legal orientation on the adjacent edge
                        let (final_p1, final_p2, dir_swapped) = ensure_legal_orientation(
                            target,
                            &mut routed,
                            &mut circuit_layout,
                            &gate_name,
                            adj_src,
                            adj_dst,
                        )?;
                        if dir_swapped {
                            swaps_inserted += 1;
                        }

                        if swaps_inserted > self.config.max_swaps {
                            return Err(RoutingError::IterationLimitExceeded {
                                limit: self.config.max_swaps,
                                details: format!(
                                    "inserted SWAPs ({swaps_inserted}) exceeded limit of {}",
                                    self.config.max_swaps
                                ),
                            });
                        }

                        // Emit the 2-qubit gate on the legally oriented physical operands
                        routed.add_operation(Operation::Gate {
                            gate_name,
                            qubits: vec![QubitId(final_p1.index()), QubitId(final_p2.index())],
                            parameters,
                        })?;
                    }
                }

                Operation::Gate {
                    gate_name,
                    qubits,
                    parameters,
                } => {
                    // Single-qubit or multi-qubit general gate
                    let phys_qubits: Result<Vec<QubitId>, RoutingError> = qubits
                        .iter()
                        .map(|&q| {
                            let p = circuit_layout.current().get_physical(q).ok_or_else(|| {
                                RoutingError::Layout(format!("qubit {q} unmapped in layout"))
                            })?;
                            Ok(QubitId(p.index()))
                        })
                        .collect();
                    let phys_qubits = phys_qubits?;

                    routed.add_operation(Operation::Gate {
                        gate_name,
                        qubits: phys_qubits,
                        parameters,
                    })?;
                }

                Operation::Measure { qubit, destination } => {
                    let p = circuit_layout
                        .current()
                        .get_physical(qubit)
                        .ok_or_else(|| {
                            RoutingError::Layout(format!("qubit {qubit} unmapped in layout"))
                        })?;
                    routed.add_operation(Operation::Measure {
                        qubit: QubitId(p.index()),
                        destination,
                    })?;
                }

                Operation::Reset { qubit } => {
                    let p = circuit_layout
                        .current()
                        .get_physical(qubit)
                        .ok_or_else(|| {
                            RoutingError::Layout(format!("qubit {qubit} unmapped in layout"))
                        })?;
                    routed.add_operation(Operation::Reset {
                        qubit: QubitId(p.index()),
                    })?;
                }

                Operation::Barrier { qubits } => {
                    let phys_qubits: Result<Vec<QubitId>, RoutingError> = qubits
                        .iter()
                        .map(|&q| {
                            let p = circuit_layout.current().get_physical(q).ok_or_else(|| {
                                RoutingError::Layout(format!("qubit {q} unmapped in layout"))
                            })?;
                            Ok(QubitId(p.index()))
                        })
                        .collect();
                    let phys_qubits = phys_qubits?;

                    routed.add_operation(Operation::Barrier {
                        qubits: phys_qubits,
                    })?;
                }

                Operation::Classical {
                    name,
                    reads,
                    writes,
                } => {
                    routed.add_operation(Operation::Classical {
                        name,
                        reads,
                        writes,
                    })?;
                }
            }

            // Advance Kahn's algorithm: decrement successor in-degrees
            ready_set.remove(&op_id);
            completed_count += 1;

            for (succ_id, _) in circuit.graph.successors(op_id) {
                if let Some(deg) = in_degrees.get_mut(&succ_id) {
                    *deg -= 1;
                    if *deg == 0 {
                        ready_set.insert(succ_id);
                    }
                }
            }
        }

        if completed_count != circuit.num_operations() {
            return Err(RoutingError::Ir(format!(
                "routing completed {} operations out of {} in circuit DAG (possible cycle)",
                completed_count,
                circuit.num_operations()
            )));
        }

        // 5. Finalize layout
        circuit_layout.commit_final();

        // 6. Target legality validation
        if self.config.validate_target_legality {
            let validator = TargetLegalityValidator::new(target.clone());
            let report = validator
                .check_circuit(&routed)
                .map_err(|e| RoutingError::TargetLegality(e.to_string()))?;
            if !report.is_valid() {
                return Err(RoutingError::TargetLegality(report.summary()));
            }
        }

        // 7. Layout-aware equivalence validation (for unitary circuits up to 16 qubits)
        if self.config.verify_equivalence && num_physical <= 16 {
            let checker = LayoutEquivalenceChecker::default();
            let verdict = checker
                .check_layout_equivalence(circuit, &routed, &circuit_layout)
                .map_err(|e| {
                    RoutingError::TargetLegality(format!("equivalence check error: {e}"))
                })?;
            if !verdict.is_equivalent {
                return Err(RoutingError::TargetLegality(format!(
                    "routed circuit failed layout-aware equivalence: {}",
                    verdict
                        .worst_mismatch
                        .as_ref()
                        .map(|m| format!("{m:?}"))
                        .unwrap_or_else(|| "amplitude mismatch".to_string())
                )));
            }
        }

        let report = BaselineRouterReport {
            original_operations: circuit.num_operations(),
            routed_operations: routed.num_operations(),
            swaps_inserted,
            layout: circuit_layout,
        };

        Ok((routed, report))
    }
}
