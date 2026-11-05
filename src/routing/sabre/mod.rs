//! SABRE (SWAP-based Bounded-lookahead Routing Engine) Frontier & Lookahead Analysis.
//!
//! Provides dependency frontier analysis, front-layer extraction, and bounded lookahead
//! set construction for the SABRE routing algorithm (Phase 50).
//!
//! # Architecture & Critical Invariants
//! - **Strict Dependency Readiness**: Operations enter the front layer only when all
//!   their predecessor dependencies in the QccIR DAG are satisfied.
//! - **Two-Qubit Frontier**: Front layer and lookahead sets represent two-qubit operations
//!   requiring physical hardware routing.
//! - **Anti-Stale Layout Versioning**: Whenever candidate physical layouts change, physical
//!   operand mappings are updated immediately with monotonic version tracking.
//! - **Phase 50 Bounded Scope**: This module implements frontier and lookahead analysis only.
//!   Heuristic scoring, decay, SWAP selection, and bidirectional refinement belong to Phases 51–52.

pub mod candidate;
pub mod decay;
pub mod front_layer;
pub mod frontier;
pub mod heuristic;
pub mod lookahead;
pub mod scoring;

pub use candidate::{CandidateGenerator, CandidateSwap};
pub use decay::{DecayTable, BASE_DECAY, DEFAULT_DECAY_DELTA};
pub use front_layer::{FrontLayer, FrontLayerOp};
pub use frontier::SabreFrontier;
pub use heuristic::{SabreHeuristic, SabreHeuristicConfig, DEFAULT_LOOKAHEAD_WEIGHT};
pub use lookahead::{LookaheadConfig, LookaheadOp, LookaheadSet};
pub use scoring::{ScoreComponents, SCORE_EPSILON};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::routing::layout_map::LayoutMap;
use crate::routing::pathfinding::RoutingError;
use crate::target::operation::PhysicalQubitId;

/// Complete synchronized frontier and lookahead state for SABRE routing analysis.
///
/// Encapsulates the DAG dependency frontier, active front layer, extended lookahead set,
/// and physical layout mapping with synchronous mutation to prevent stale physical coordinates.
#[derive(Clone, Debug)]
pub struct SabreFrontierState {
    /// Core dependency DAG execution frontier.
    frontier: SabreFrontier,
    /// Current front layer of executable two-qubit operations.
    front_layer: FrontLayer,
    /// Extended lookahead set of upcoming two-qubit operations.
    lookahead: LookaheadSet,
    /// Current logical-to-physical qubit layout.
    layout: LayoutMap,
    /// Lookahead configuration (budget and depth constraints).
    config: LookaheadConfig,
    /// Monotonically increasing layout version counter.
    layout_version: u64,
}

impl SabreFrontierState {
    /// Initializes a new SABRE frontier state from a logical [`Circuit`] and initial layout.
    pub fn new(
        circuit: &Circuit,
        layout: LayoutMap,
        config: LookaheadConfig,
    ) -> Result<Self, RoutingError> {
        layout
            .validate()
            .map_err(|e| RoutingError::Layout(e.to_string()))?;

        let mut frontier = SabreFrontier::new(circuit)?;

        // Transparently advance any initial single-qubit operations so the front layer
        // immediately contains the initial two-qubit operations
        loop {
            let ready_1q = frontier.executable_single_qubit_operations(circuit);
            if ready_1q.is_empty() {
                break;
            }
            for op_id in ready_1q {
                frontier.advance(circuit, op_id)?;
            }
        }

        let front_layer = FrontLayer::from_frontier(&frontier, circuit, &layout)?;
        let lookahead = LookaheadSet::build(&frontier, circuit, &layout, &config)?;

        Ok(Self {
            frontier,
            front_layer,
            lookahead,
            layout,
            config,
            layout_version: 1,
        })
    }

    /// Returns a reference to the active front layer.
    #[inline]
    pub fn front_layer(&self) -> &FrontLayer {
        &self.front_layer
    }

    /// Returns a reference to the extended lookahead set.
    #[inline]
    pub fn lookahead(&self) -> &LookaheadSet {
        &self.lookahead
    }

    /// Returns a reference to the underlying dependency frontier.
    #[inline]
    pub fn frontier(&self) -> &SabreFrontier {
        &self.frontier
    }

    /// Returns a reference to the active logical-to-physical layout map.
    #[inline]
    pub fn layout(&self) -> &LayoutMap {
        &self.layout
    }

    /// Returns the current layout version counter.
    #[inline]
    pub fn layout_version(&self) -> u64 {
        self.layout_version
    }

    /// Returns the lookahead configuration.
    #[inline]
    pub fn config(&self) -> &LookaheadConfig {
        &self.config
    }

    /// Updates the active layout to `new_layout`, immediately synchronizing physical operand
    /// mappings across both the front layer and extended lookahead set.
    ///
    /// Increments `layout_version` to guarantee that stale physical coordinates do not survive.
    /// If validation or mapping fails on either layer, the state remains completely unchanged.
    pub fn update_layout(&mut self, new_layout: LayoutMap) -> Result<(), RoutingError> {
        new_layout
            .validate()
            .map_err(|e| RoutingError::Layout(e.to_string()))?;

        let mut updated_front = self.front_layer.clone();
        updated_front.update_layout(&new_layout)?;

        let mut updated_lookahead = self.lookahead.clone();
        updated_lookahead.update_layout(&new_layout)?;

        self.front_layer = updated_front;
        self.lookahead = updated_lookahead;
        self.layout = new_layout;
        self.layout_version += 1;
        Ok(())
    }

    /// Applies a physical SWAP between physical qubits `u` and `v`, synchronously updating
    /// the layout, front layer, and lookahead set.
    ///
    /// If the physical swap is invalid, the state remains completely unchanged.
    pub fn apply_physical_swap(
        &mut self,
        u: PhysicalQubitId,
        v: PhysicalQubitId,
    ) -> Result<(), RoutingError> {
        let mut new_layout = self.layout.clone();
        new_layout
            .swap_physical(u, v)
            .map_err(|e| RoutingError::Layout(e.to_string()))?;
        self.update_layout(new_layout)
    }

    /// Retires an executed operation from the frontier, updates in-degrees, and recomputes
    /// the front layer and extended lookahead set.
    ///
    /// Also transparently retires any newly ready single-qubit operations.
    /// The update is transactional: if any failure occurs, state is not partially modified.
    pub fn retire_operation(
        &mut self,
        circuit: &Circuit,
        id: OperationId,
    ) -> Result<Vec<OperationId>, RoutingError> {
        let mut sim_frontier = self.frontier.clone();
        let unblocked = sim_frontier.advance(circuit, id)?;

        // Transparently advance any newly ready single-qubit operations
        loop {
            let ready_1q = sim_frontier.executable_single_qubit_operations(circuit);
            if ready_1q.is_empty() {
                break;
            }
            for op_id in ready_1q {
                sim_frontier.advance(circuit, op_id)?;
            }
        }

        let new_front = FrontLayer::from_frontier(&sim_frontier, circuit, &self.layout)?;
        let new_lookahead =
            LookaheadSet::build(&sim_frontier, circuit, &self.layout, &self.config)?;

        self.frontier = sim_frontier;
        self.front_layer = new_front;
        self.lookahead = new_lookahead;

        Ok(unblocked)
    }

    /// Retires all currently ready single-qubit operations from the frontier and refreshes layers.
    pub fn retire_ready_single_qubit_operations(
        &mut self,
        circuit: &Circuit,
    ) -> Result<Vec<OperationId>, RoutingError> {
        let mut sim_frontier = self.frontier.clone();
        let mut retired = Vec::new();
        loop {
            let ready_1q = sim_frontier.executable_single_qubit_operations(circuit);
            if ready_1q.is_empty() {
                break;
            }
            for op_id in ready_1q {
                sim_frontier.advance(circuit, op_id)?;
                retired.push(op_id);
            }
        }

        if !retired.is_empty() {
            let new_front = FrontLayer::from_frontier(&sim_frontier, circuit, &self.layout)?;
            let new_lookahead =
                LookaheadSet::build(&sim_frontier, circuit, &self.layout, &self.config)?;
            self.frontier = sim_frontier;
            self.front_layer = new_front;
            self.lookahead = new_lookahead;
        }

        Ok(retired)
    }

    /// Explicitly forces recomputation of both the front layer and extended lookahead set.
    pub fn recompute(&mut self, circuit: &Circuit) -> Result<(), RoutingError> {
        let new_front = FrontLayer::from_frontier(&self.frontier, circuit, &self.layout)?;
        let new_lookahead =
            LookaheadSet::build(&self.frontier, circuit, &self.layout, &self.config)?;
        self.front_layer = new_front;
        self.lookahead = new_lookahead;
        Ok(())
    }

    /// Returns `true` if the entire circuit has been completely executed.
    #[inline]
    pub fn is_fully_completed(&self) -> bool {
        self.frontier.is_fully_completed()
    }
}
