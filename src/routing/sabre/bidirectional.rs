//! SABRE Bidirectional Router and Layout Refinement Engine.
//!
//! Implements bidirectional routing using reverse circuit traversal to derive optimized
//! initial layouts (Li et al., ASPLOS 2019), followed by forward SABRE routing and
//! multi-round iterative layout refinement.
//!
//! # Architecture & Correctness Invariants
//! - **Permutation Conventions**:
//!   - Logical qubits $Q$ and physical qubits $P$ are strictly separated.
//!   - At time $t = 0$, $L_{\text{initial}}(q) = p$ denotes that logical qubit $q$ begins at physical qubit $p$.
//!   - At time $t = T$, $L_{\text{final}}(q) = p'$ denotes where logical qubit $q$ ends after all SWAPs.
//!   - In reverse traversal, routing finishes at time $t = 0$. The resulting physical layout $L_{\text{rev\_final}}$
//!     places early circuit operands in close proximity and is directly used as the candidate forward initial layout:
//!     $$L_{\text{initial}}^{\text{candidate}} = L_{\text{rev\_final}}$$
//!   - Permutations and inverse permutations are never confused.
//! - **Circuit Immutability**:
//!   - The input logical [`Circuit`] is never mutated during reverse analysis.
//! - **ADR-0001 Strict Ancilla Zero-State**:
//!   - Unused physical qubits begin in $|0\rangle$ and must return to $|0\rangle$ at circuit termination.
//!   - Verified by [`LayoutEquivalenceChecker`].
//! - **Provenance Preservation**:
//!   - Every routed physical gate retains its logical provenance, and all inserted SWAPs record explicit causality.
//! - **Target Legality**:
//!   - All routed physical operations satisfy the hardware target's coupling graph and directionality constraints.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::optimizer::algebra::invert_operation;
use crate::routing::cost::RoutingCostWeights;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::LayoutMap;
use crate::routing::metrics::RoutingMetrics;
use crate::routing::pathfinding::RoutingError;
use crate::routing::placement::{place_circuit, PlacementStrategy};
use crate::routing::provenance::RouteProvenanceTracker;
use crate::routing::sabre::front_layer::FrontLayerOp;
use crate::routing::sabre::heuristic::{SabreHeuristic, SabreHeuristicConfig};
use crate::routing::sabre::layout_refinement::{
    is_refinement_improvement, RefinementRoundRecord, RefinementSummary,
};
use crate::routing::sabre::lookahead::LookaheadConfig;
use crate::routing::sabre::SabreFrontierState;
use crate::routing::swap_insertion::{
    emit_physical_swap_with_provenance, ensure_legal_orientation_with_provenance,
};
use crate::target::legality::TargetLegalityValidator;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Configuration options for the SABRE bidirectional router and refinement passes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SabreRouterConfig {
    /// Heuristic configuration (lookahead weight $W$ and decay delta $\delta$).
    pub heuristic_config: SabreHeuristicConfig,
    /// Extended lookahead configuration (budget $W$ and max depth).
    pub lookahead_config: LookaheadConfig,
    /// Number of bidirectional refinement rounds to execute (default: 3).
    /// If 1, runs a single forward pass without reverse refinement.
    pub refinement_rounds: usize,
    /// Initial placement strategy if no initial layout is explicitly provided.
    pub placement_strategy: PlacementStrategy,
    /// Maximum number of physical SWAPs permitted before aborting.
    pub max_swaps: usize,
    /// Maximum number of scheduling loop iterations permitted.
    pub max_iterations: usize,
    /// Whether to automatically verify layout-aware equivalence on small circuits ($\le 16$ qubits).
    pub verify_equivalence: bool,
    /// Whether to validate target legality of the routed physical circuit.
    pub validate_target_legality: bool,
    /// Cost weights used to evaluate multi-objective composite routing quality.
    pub cost_weights: RoutingCostWeights,
    /// Optional random seed recorded in routing metrics for deterministic reproducibility.
    pub random_seed: Option<u64>,
}

impl Default for SabreRouterConfig {
    fn default() -> Self {
        Self {
            heuristic_config: SabreHeuristicConfig::default(),
            lookahead_config: LookaheadConfig::default(),
            refinement_rounds: 3,
            placement_strategy: PlacementStrategy::Weighted,
            max_swaps: 10_000,
            max_iterations: 100_000,
            verify_equivalence: true,
            validate_target_legality: true,
            cost_weights: RoutingCostWeights::default(),
            random_seed: None,
        }
    }
}

/// Comprehensive report produced by the SABRE bidirectional router.
#[derive(Clone, Debug, PartialEq)]
pub struct SabreRouterReport {
    /// Total number of operations in the original logical circuit.
    pub original_operations: usize,
    /// Total number of operations in the routed physical circuit.
    pub routed_operations: usize,
    /// Number of physical SWAP operations inserted in the winning routed circuit.
    pub swaps_inserted: usize,
    /// Cumulative scheduling iterations executed across all forward and reverse refinement passes.
    pub total_iterations: usize,
    /// Cumulative physical SWAP operations emitted across all forward and reverse refinement passes.
    pub total_swaps_executed: usize,
    /// Multi-stage layout tracking initial and final physical qubit assignments.
    pub layout: CircuitLayout,
    /// Detailed provenance tracking mapping physical operations to their logical origin.
    pub provenance: RouteProvenanceTracker,
    /// Comprehensive routing quality metrics, DAG depth, duration, and composite cost.
    pub metrics: RoutingMetrics,
    /// Detailed history and summary of bidirectional refinement passes.
    pub refinement: RefinementSummary,
}

/// Constructs a mathematically exact reversed circuit $C_{rev}$ ($C^\dagger$) without mutating the original circuit.
///
/// # Mathematical Semantics
/// - **Unitary Inversion**: Applies the adjoint $U^\dagger$ to each quantum gate:
///   - Self-inverse gates ($H, X, Y, Z, CX, CZ, SWAP, Id$) retain their canonical identity.
///   - Paired discrete gates ($S \leftrightarrow S^\dagger, T \leftrightarrow T^\dagger$) map to their adjoint counterparts.
///   - Parameterized rotations ($Rx, Ry, Rz, P$) negate their continuous parameters ($\theta \to -\theta$).
/// - **Global Phase Inversion**: Preserves explicit global phase via $(e^{i\phi})^\dagger = e^{-i\phi}$ (`\phi \to -\phi`).
/// - **Topological & Dependency Reversal**: Reverses deterministic topological execution order.
/// - **Barriers**: Cross-wire synchronization barriers are preserved on their affected qubits.
/// - **Provenance**: Every inverted operation records provenance pointing back to its original logical operation ID.
///
/// # Non-Unitary Operation Handling
/// Measurements, resets, and classical branch/register operations are non-unitary and cannot be
/// semantically inverted. Returns [`RoutingError::NonUnitaryReverseTraversal`] if the circuit contains
/// any non-unitary operation.
pub fn reverse_circuit(circuit: &Circuit) -> Result<Circuit, RoutingError> {
    let mut rev = Circuit::new(format!("{}_rev", circuit.name));

    for qreg in &circuit.quantum_registers {
        rev.add_quantum_register(&qreg.name, qreg.size)
            .map_err(|e| RoutingError::Ir(e.to_string()))?;
    }
    for creg in &circuit.classical_registers {
        rev.add_classical_register(&creg.name, creg.size)
            .map_err(|e| RoutingError::Ir(e.to_string()))?;
    }

    // Invert circuit-level global phase: (e^{i\phi})^\dagger = e^{-i\phi}
    rev.set_global_phase(-circuit.global_phase().clone());

    // Iterate through deterministic topological order in reverse
    let topo_order = circuit
        .graph
        .deterministic_topological_order()
        .map_err(|e| RoutingError::Ir(e.to_string()))?;

    for &op_id in topo_order.iter().rev() {
        let op = circuit
            .graph
            .get_operation(op_id)
            .ok_or_else(|| RoutingError::Ir(format!("operation {op_id} not found in graph")))?;

        match op {
            Operation::Gate { .. } => {
                let inverted = invert_operation(op).map_err(|e| {
                    RoutingError::NonUnitaryReverseTraversal(format!(
                        "gate '{op}' cannot be inverted: {e}"
                    ))
                })?;
                rev.add_operation_with_provenance(inverted, vec![op_id])
                    .map_err(|e| RoutingError::Ir(e.to_string()))?;
            }
            Operation::Barrier { qubits } => {
                rev.add_operation_with_provenance(
                    Operation::Barrier {
                        qubits: qubits.clone(),
                    },
                    vec![op_id],
                )
                .map_err(|e| RoutingError::Ir(e.to_string()))?;
            }
            Operation::Measure { .. } => {
                return Err(RoutingError::NonUnitaryReverseTraversal(
                    "measurement operation is non-unitary and cannot be semantically inverted; reverse circuit traversal is strictly restricted to unitary operations and barriers".to_string(),
                ));
            }
            Operation::Reset { .. } => {
                return Err(RoutingError::NonUnitaryReverseTraversal(
                    "reset operation is non-unitary and cannot be semantically inverted; reverse circuit traversal is strictly restricted to unitary operations and barriers".to_string(),
                ));
            }
            Operation::Classical { .. } => {
                return Err(RoutingError::NonUnitaryReverseTraversal(
                    "classical register operation cannot be semantically inverted; reverse circuit traversal is strictly restricted to unitary operations and barriers".to_string(),
                ));
            }
        }
    }

    Ok(rev)
}

/// Emits a single-qubit or non-2Q operation onto the physical circuit mapped through the active layout.
fn emit_non_2q_operation(
    logical_circuit: &Circuit,
    op_id: OperationId,
    routed: &mut Circuit,
    layout: &LayoutMap,
    provenance: &mut RouteProvenanceTracker,
) -> Result<OperationId, RoutingError> {
    let op = logical_circuit
        .graph
        .get_operation(op_id)
        .ok_or_else(|| RoutingError::Ir(format!("operation {op_id} not found")))?;

    match op {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => {
            let mut phys_qubits = Vec::with_capacity(qubits.len());
            for &q in qubits {
                let p = layout
                    .get_physical(q)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q} not assigned")))?;
                phys_qubits.push(QubitId(p.index()));
            }
            let phys_id = routed.add_operation_with_provenance(
                Operation::Gate {
                    gate_name: gate_name.clone(),
                    qubits: phys_qubits,
                    parameters: parameters.clone(),
                },
                vec![op_id],
            )?;
            provenance.record_logical_operation(
                phys_id,
                op_id,
                format!("mapped logical {gate_name}"),
            );
            Ok(phys_id)
        }
        Operation::Measure { qubit, destination } => {
            let p = layout
                .get_physical(*qubit)
                .ok_or_else(|| RoutingError::Layout(format!("qubit {qubit} not assigned")))?;
            let phys_id = routed.add_operation_with_provenance(
                Operation::Measure {
                    qubit: QubitId(p.index()),
                    destination: *destination,
                },
                vec![op_id],
            )?;
            provenance.record_logical_operation(phys_id, op_id, "mapped measurement");
            Ok(phys_id)
        }
        Operation::Reset { qubit } => {
            let p = layout
                .get_physical(*qubit)
                .ok_or_else(|| RoutingError::Layout(format!("qubit {qubit} not assigned")))?;
            let phys_id = routed.add_operation_with_provenance(
                Operation::Reset {
                    qubit: QubitId(p.index()),
                },
                vec![op_id],
            )?;
            provenance.record_logical_operation(phys_id, op_id, "mapped reset");
            Ok(phys_id)
        }
        Operation::Barrier { qubits } => {
            let mut phys_qubits = Vec::with_capacity(qubits.len());
            for &q in qubits {
                let p = layout
                    .get_physical(q)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q} not assigned")))?;
                phys_qubits.push(QubitId(p.index()));
            }
            let phys_id = routed.add_operation_with_provenance(
                Operation::Barrier {
                    qubits: phys_qubits,
                },
                vec![op_id],
            )?;
            provenance.record_logical_operation(phys_id, op_id, "mapped barrier");
            Ok(phys_id)
        }
        Operation::Classical {
            name,
            reads,
            writes,
        } => {
            let phys_id = routed.add_operation_with_provenance(
                Operation::Classical {
                    name: name.clone(),
                    reads: reads.clone(),
                    writes: writes.clone(),
                },
                vec![op_id],
            )?;
            provenance.record_logical_operation(phys_id, op_id, "mapped classical");
            Ok(phys_id)
        }
    }
}

/// The SABRE Bidirectional Router with multi-round layout refinement.
#[derive(Clone, Debug)]
pub struct SabreBidirectionalRouter {
    config: SabreRouterConfig,
}

impl Default for SabreBidirectionalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl SabreBidirectionalRouter {
    /// Constructs a new SABRE bidirectional router with default configuration.
    pub fn new() -> Self {
        Self {
            config: SabreRouterConfig::default(),
        }
    }

    /// Constructs a new SABRE bidirectional router with customized configuration.
    pub fn with_config(config: SabreRouterConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the router configuration.
    #[inline]
    pub fn config(&self) -> &SabreRouterConfig {
        &self.config
    }

    /// Executes a single unidirectional SABRE routing pass.
    ///
    /// Routes `circuit` onto `target` starting from `initial_layout`.
    pub fn route_single_pass(
        &self,
        circuit: &Circuit,
        target: &Target,
        initial_layout: &LayoutMap,
    ) -> Result<
        (
            Circuit,
            CircuitLayout,
            RouteProvenanceTracker,
            RoutingMetrics,
            usize,
        ),
        RoutingError,
    > {
        let run_start = Instant::now();
        let num_logical = circuit.num_qubits();
        let num_physical = target.num_qubits();

        if num_physical < num_logical {
            return Err(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            });
        }

        initial_layout.validate()?;
        let mut circuit_layout = CircuitLayout::new(initial_layout.clone());
        let mut provenance = RouteProvenanceTracker::new();

        // 1. Initialize routed physical circuit
        let mut routed = Circuit::new(format!("{}_sabre", circuit.name));
        routed
            .add_quantum_register("p", num_physical)
            .map_err(|e| RoutingError::Ir(e.to_string()))?;
        for creg in &circuit.classical_registers {
            routed
                .add_classical_register(&creg.name, creg.size)
                .map_err(|e| RoutingError::Ir(e.to_string()))?;
        }
        routed.set_global_phase(circuit.global_phase().clone());

        // 2. Initialize SABRE frontier and heuristic engines
        let mut state = SabreFrontierState::new_routing(
            circuit,
            initial_layout.clone(),
            self.config.lookahead_config.clone(),
        )?;
        let mut heuristic = SabreHeuristic::new(target, self.config.heuristic_config.clone())?;

        let mut iterations = 0usize;
        let mut swaps_inserted = 0usize;

        // 3. Main SABRE routing loop
        while !state.is_fully_completed() {
            iterations += 1;
            if iterations > self.config.max_iterations {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_iterations,
                    details: format!(
                        "scheduling loop exceeded {} iterations",
                        self.config.max_iterations
                    ),
                    partial_swaps: swaps_inserted,
                    partial_iterations: iterations,
                });
            }
            if swaps_inserted > self.config.max_swaps {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_swaps,
                    details: format!("inserted SWAPs exceeded limit of {}", self.config.max_swaps),
                    partial_swaps: swaps_inserted,
                    partial_iterations: iterations,
                });
            }

            // Step A: Immediately execute any ready single-qubit / non-2Q operations
            let ready_1q = state.frontier().executable_single_qubit_operations(circuit);
            if !ready_1q.is_empty() {
                for op_id in ready_1q {
                    emit_non_2q_operation(
                        circuit,
                        op_id,
                        &mut routed,
                        state.layout(),
                        &mut provenance,
                    )?;
                    state.advance_operation(circuit, op_id)?;
                }
                continue;
            }

            // Step B: Check if any front-layer two-qubit operation is directly executable
            let mut executable_front_op: Option<FrontLayerOp> = None;
            for op in state.front_layer().operations() {
                let (p1, p2) = op.physical_operands();
                if target.is_valid_operand_tuple(&op.name, &[p1, p2]) {
                    executable_front_op = Some(op.clone());
                    break;
                }
            }

            if let Some(op) = executable_front_op {
                // Directly legal on target: emit operation to physical circuit
                let orig_op = circuit.graph.get_operation(op.id).unwrap();
                let parameters = match orig_op {
                    Operation::Gate { parameters, .. } => parameters.clone(),
                    _ => Vec::new(),
                };
                let phys_id = routed.add_operation_with_provenance(
                    Operation::Gate {
                        gate_name: op.name.clone(),
                        qubits: vec![
                            QubitId(op.physical_p1.index()),
                            QubitId(op.physical_p2.index()),
                        ],
                        parameters,
                    },
                    vec![op.id],
                )?;
                provenance.record_logical_operation(
                    phys_id,
                    op.id,
                    format!("mapped logical {}", op.name),
                );

                // Advance operation and reset decay table upon forward progress
                state.advance_operation(circuit, op.id)?;
                heuristic.reset_decay();
                continue;
            }

            // Step C: Check if any adjacent front-layer operation needs direction correction
            let mut directional_op: Option<(FrontLayerOp, PhysicalQubitId, PhysicalQubitId)> = None;
            for op in state.front_layer().operations() {
                let (p1, p2) = op.physical_operands();
                if heuristic.distances().distance(p1, p2) == Some(1) {
                    directional_op = Some((op.clone(), p1, p2));
                    break;
                }
            }

            if let Some((op, p1, p2)) = directional_op {
                let (legal_p1, legal_p2, dir_swap) = ensure_legal_orientation_with_provenance(
                    target,
                    &mut routed,
                    &mut circuit_layout,
                    &op.name,
                    p1,
                    p2,
                    Some(op.id),
                    Some(&mut provenance),
                )?;

                if let Some(swap_res) = dir_swap {
                    swaps_inserted += 1;
                    state.apply_physical_swap(swap_res.u, swap_res.v)?;
                    heuristic.record_swap(crate::routing::sabre::CandidateSwap::new(
                        swap_res.u, swap_res.v,
                    ))?;
                }

                // Now legal: emit gate
                let orig_op = circuit.graph.get_operation(op.id).unwrap();
                let parameters = match orig_op {
                    Operation::Gate { parameters, .. } => parameters.clone(),
                    _ => Vec::new(),
                };
                let phys_id = routed.add_operation_with_provenance(
                    Operation::Gate {
                        gate_name: op.name.clone(),
                        qubits: vec![QubitId(legal_p1.index()), QubitId(legal_p2.index())],
                        parameters,
                    },
                    vec![op.id],
                )?;
                provenance.record_logical_operation(
                    phys_id,
                    op.id,
                    format!("mapped logical {}", op.name),
                );

                state.advance_operation(circuit, op.id)?;
                heuristic.reset_decay();
                continue;
            }

            // Step D: No front-layer gate is adjacent: evaluate and insert best candidate SWAP
            let ranked = heuristic.evaluate_and_rank_candidates(&state, target)?;
            if ranked.is_empty() {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_iterations,
                    details:
                        "no viable candidate SWAPs could be generated on target coupling graph"
                            .to_string(),
                    partial_swaps: swaps_inserted,
                    partial_iterations: iterations,
                });
            }

            if swaps_inserted >= self.config.max_swaps {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: self.config.max_swaps,
                    details: format!(
                        "inserted SWAPs exceeded limit of {}: cannot insert physical SWAP #{}",
                        self.config.max_swaps,
                        swaps_inserted + 1
                    ),
                    partial_swaps: swaps_inserted,
                    partial_iterations: iterations,
                });
            }

            let best_cand = ranked[0].candidate;
            let caused_by = state.front_layer().operations().first().map(|op| op.id);

            let _res = emit_physical_swap_with_provenance(
                target,
                &mut routed,
                &mut circuit_layout,
                best_cand.u,
                best_cand.v,
                caused_by,
                swaps_inserted,
                Some(&mut provenance),
            )?;

            swaps_inserted += 1;
            state.apply_physical_swap(best_cand.u, best_cand.v)?;
            heuristic.record_swap(best_cand)?;
        }

        circuit_layout.commit_final();

        let run_duration = run_start.elapsed();
        let metrics = RoutingMetrics::evaluate_with_weights(
            &routed,
            target,
            &circuit_layout,
            &provenance,
            &self.config.cost_weights,
            self.config.random_seed,
            Some(run_duration),
        );

        Ok((routed, circuit_layout, provenance, metrics, iterations))
    }

    /// Routes a logical [`Circuit`] onto `target` with bidirectional layout refinement.
    ///
    /// Accepts an optional pre-computed initial layout. If `None`, computes the initial
    /// placement using the configured `PlacementStrategy`.
    ///
    /// # Errors
    /// Returns [`RoutingError::InsufficientCapacity`] if the target has fewer physical qubits than required.
    /// Returns [`RoutingError::DisconnectedTopology`] if interacting qubits cannot be bridged.
    /// Returns [`RoutingError::IterationLimitExceeded`] if SWAP or scheduling iteration limits are breached.
    pub fn route_circuit(
        &self,
        circuit: &Circuit,
        target: &Target,
        initial_layout: Option<LayoutMap>,
    ) -> Result<(Circuit, SabreRouterReport), RoutingError> {
        let num_logical = circuit.num_qubits();
        let num_physical = target.num_qubits();

        if num_physical < num_logical {
            return Err(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            });
        }

        // 1. Determine base initial layout
        let base_initial = match initial_layout {
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

        // If refinement_rounds <= 1, execute standard forward SABRE without reverse refinement
        if self.config.refinement_rounds <= 1 {
            let (routed, circuit_layout, provenance, metrics, forward_iterations) =
                self.route_single_pass(circuit, target, &base_initial)?;

            let forward_swaps = metrics.routing_swaps_count;
            let forward_depth = metrics.circuit_depth;
            let forward_cost = metrics.weighted_cost;

            let summary = RefinementSummary::new(
                vec![RefinementRoundRecord {
                    round: 1,
                    initial_layout: base_initial.clone(),
                    forward_swaps,
                    forward_depth,
                    forward_cost,
                    reverse_swaps: 0,
                    reverse_cost: 0.0,
                    next_initial_layout: circuit_layout.current().clone(),
                    accepted: true,
                }],
                base_initial,
                circuit_layout.current().clone(),
                1,
                forward_swaps,
                forward_depth,
                forward_cost,
            );

            let report = SabreRouterReport {
                original_operations: circuit.num_operations(),
                routed_operations: routed.num_operations(),
                swaps_inserted: forward_swaps,
                total_iterations: forward_iterations,
                total_swaps_executed: forward_swaps,
                layout: circuit_layout,
                provenance,
                metrics,
                refinement: summary,
            };

            self.validate_report(circuit, &routed, &report, target)?;
            return Ok((routed, report));
        }

        // 2. Multi-round bidirectional refinement
        let reversed_circuit = reverse_circuit(circuit)?;
        let mut current_initial = base_initial.clone();
        let mut rounds_history = Vec::with_capacity(self.config.refinement_rounds);
        let mut total_iterations = 0usize;
        let mut total_swaps_executed = 0usize;

        let mut best_result: Option<(
            Circuit,
            CircuitLayout,
            RouteProvenanceTracker,
            RoutingMetrics,
            usize,
        )> = None;

        for round_idx in 1..=self.config.refinement_rounds {
            // Check remaining candidate budget before forward pass
            if total_swaps_executed > self.config.max_swaps
                || total_iterations > self.config.max_iterations
            {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: if total_swaps_executed > self.config.max_swaps {
                        self.config.max_swaps
                    } else {
                        self.config.max_iterations
                    },
                    details: format!(
                        "candidate cumulative resource budget exhausted before round {round_idx} forward pass: {total_swaps_executed}/{} swaps, {total_iterations}/{} iterations",
                        self.config.max_swaps, self.config.max_iterations
                    ),
                    partial_swaps: total_swaps_executed,
                    partial_iterations: total_iterations,
                });
            }

            let rem_swaps = self.config.max_swaps.saturating_sub(total_swaps_executed);
            let rem_iters = self.config.max_iterations.saturating_sub(total_iterations);

            // Forward Pass: Route C from current_initial clamped to remaining candidate budget
            let mut fwd_config = self.config.clone();
            fwd_config.max_swaps = rem_swaps;
            fwd_config.max_iterations = rem_iters;
            let fwd_router = SabreBidirectionalRouter::with_config(fwd_config);

            let (fwd_routed, fwd_layout, fwd_prov, fwd_metrics, fwd_iters) = match fwd_router
                .route_single_pass(circuit, target, &current_initial)
            {
                Ok(res) => res,
                Err(mut err) => {
                    if let RoutingError::IterationLimitExceeded {
                        limit,
                        partial_swaps,
                        partial_iterations,
                        details,
                    } = &mut err
                    {
                        *partial_swaps += total_swaps_executed;
                        *partial_iterations += total_iterations;
                        *limit = self.config.max_swaps;
                        *details = format!(
                                "inserted SWAPs exceeded limit of {}: cannot insert physical SWAP #{} (total executed: {})",
                                self.config.max_swaps,
                                *partial_swaps + 1,
                                *partial_swaps
                            );
                    }
                    return Err(err);
                }
            };

            total_iterations += fwd_iters;
            total_swaps_executed += fwd_metrics.routing_swaps_count;

            let fwd_swaps = fwd_metrics.routing_swaps_count;
            let fwd_depth = fwd_metrics.circuit_depth;
            let fwd_cost = fwd_metrics.weighted_cost;

            // Determine acceptance
            let is_accepted = match &best_result {
                None => true,
                Some((_, _, _, best_m, _)) => is_refinement_improvement(&fwd_metrics, best_m),
            };

            if is_accepted {
                best_result = Some((
                    fwd_routed,
                    fwd_layout.clone(),
                    fwd_prov,
                    fwd_metrics,
                    round_idx,
                ));
            }

            // Early exit if 0 SWAPs achieved
            if fwd_swaps == 0 {
                rounds_history.push(RefinementRoundRecord {
                    round: round_idx,
                    initial_layout: current_initial.clone(),
                    forward_swaps: fwd_swaps,
                    forward_depth: fwd_depth,
                    forward_cost: fwd_cost,
                    reverse_swaps: 0,
                    reverse_cost: 0.0,
                    next_initial_layout: fwd_layout.current().clone(),
                    accepted: is_accepted,
                });
                break;
            }

            // Check remaining candidate budget before reverse pass
            if total_swaps_executed > self.config.max_swaps
                || total_iterations > self.config.max_iterations
            {
                return Err(RoutingError::IterationLimitExceeded {
                    limit: if total_swaps_executed > self.config.max_swaps {
                        self.config.max_swaps
                    } else {
                        self.config.max_iterations
                    },
                    details: format!(
                        "candidate cumulative resource budget exhausted before round {round_idx} reverse pass: {total_swaps_executed}/{} swaps, {total_iterations}/{} iterations",
                        self.config.max_swaps, self.config.max_iterations
                    ),
                    partial_swaps: total_swaps_executed,
                    partial_iterations: total_iterations,
                });
            }

            let rem_swaps = self.config.max_swaps.saturating_sub(total_swaps_executed);
            let rem_iters = self.config.max_iterations.saturating_sub(total_iterations);

            // Reverse Pass: Route C_rev starting from fwd_layout.final_layout() clamped to remaining candidate budget
            let mut rev_config = self.config.clone();
            rev_config.max_swaps = rem_swaps;
            rev_config.max_iterations = rem_iters;
            let rev_router = SabreBidirectionalRouter::with_config(rev_config);

            let rev_start_layout = fwd_layout.current().clone();
            let (_rev_routed, rev_layout, _rev_prov, rev_metrics, rev_iters) = match rev_router
                .route_single_pass(&reversed_circuit, target, &rev_start_layout)
            {
                Ok(res) => res,
                Err(mut err) => {
                    if let RoutingError::IterationLimitExceeded {
                        limit,
                        partial_swaps,
                        partial_iterations,
                        details,
                    } = &mut err
                    {
                        *partial_swaps += total_swaps_executed;
                        *partial_iterations += total_iterations;
                        *limit = self.config.max_swaps;
                        *details = format!(
                                "inserted SWAPs exceeded limit of {}: cannot insert physical SWAP #{} (total executed: {})",
                                self.config.max_swaps,
                                *partial_swaps + 1,
                                *partial_swaps
                            );
                    }
                    return Err(err);
                }
            };

            total_iterations += rev_iters;
            total_swaps_executed += rev_metrics.routing_swaps_count;

            let rev_swaps = rev_metrics.routing_swaps_count;
            let rev_cost = rev_metrics.weighted_cost;
            let next_initial = rev_layout.current().clone();

            rounds_history.push(RefinementRoundRecord {
                round: round_idx,
                initial_layout: current_initial.clone(),
                forward_swaps: fwd_swaps,
                forward_depth: fwd_depth,
                forward_cost: fwd_cost,
                reverse_swaps: rev_swaps,
                reverse_cost: rev_cost,
                next_initial_layout: next_initial.clone(),
                accepted: is_accepted,
            });

            current_initial = next_initial;
        }

        let (winning_routed, winning_layout, winning_prov, winning_metrics, winning_round) =
            best_result.expect("at least one round executed");

        let winning_swaps = winning_metrics.routing_swaps_count;
        let winning_depth = winning_metrics.circuit_depth;
        let winning_cost = winning_metrics.weighted_cost;

        let summary = RefinementSummary::new(
            rounds_history,
            winning_layout.initial().clone(),
            winning_layout.final_layout().clone(),
            winning_round,
            winning_swaps,
            winning_depth,
            winning_cost,
        );

        let report = SabreRouterReport {
            original_operations: circuit.num_operations(),
            routed_operations: winning_routed.num_operations(),
            swaps_inserted: winning_swaps,
            total_iterations,
            total_swaps_executed,
            layout: winning_layout,
            provenance: winning_prov,
            metrics: winning_metrics,
            refinement: summary,
        };

        self.validate_report(circuit, &winning_routed, &report, target)?;
        Ok((winning_routed, report))
    }

    /// Performs post-routing validation (target legality and semantic layout equivalence).
    fn validate_report(
        &self,
        logical_circuit: &Circuit,
        routed_circuit: &Circuit,
        report: &SabreRouterReport,
        target: &Target,
    ) -> Result<(), RoutingError> {
        if self.config.validate_target_legality {
            let validator = TargetLegalityValidator::new(target.clone());
            let leg_report = validator
                .check_circuit(routed_circuit)
                .map_err(|e| RoutingError::TargetLegality(e.to_string()))?;
            if !leg_report.is_valid() {
                return Err(RoutingError::TargetLegality(leg_report.summary()));
            }
        }

        if self.config.verify_equivalence && target.num_qubits() <= 16 {
            let checker = LayoutEquivalenceChecker::default();
            let verdict = checker
                .check_layout_equivalence(logical_circuit, routed_circuit, &report.layout)
                .map_err(|e| {
                    RoutingError::TargetLegality(format!("equivalence check error: {e}"))
                })?;
            if !verdict.is_equivalent {
                return Err(RoutingError::TargetLegality(format!(
                    "routed circuit is not semantically equivalent to logical circuit: max diff = {}",
                    verdict.max_amplitude_difference
                )));
            }
        }

        Ok(())
    }
}
