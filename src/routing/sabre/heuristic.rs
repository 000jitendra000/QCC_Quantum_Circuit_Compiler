//! Core SABRE Heuristic evaluation, candidate scoring, and decay coordination.
//!
//! Evaluates candidate physical SWAPs against hypothetical layouts without mutating
//! active compiler state, scoring resulting front-layer distances, extended-lookahead
//! distances, and physical decay multipliers with deterministic tie-breaking.
//!
//! # Mathematical Formulation (ASPLOS 2019, Li et al.)
//! For each candidate physical SWAP $(u, v)$ on a coupling edge adjacent to
//! unresolved front-layer operands:
//! 1. Hypothetical Layout $\pi'$:
//!    $$\pi' = \text{swap}(\pi, u, v)$$
//! 2. Normalized Front-Layer Distance Cost ($H_F$):
//!    $$H_F(\pi') = \frac{1}{|F|} \sum_{g=(q_1, q_2) \in F} D(\pi'(q_1), \pi'(q_2))$$
//! 3. Normalized Extended-Lookahead Distance Cost ($H_E$):
//!    $$H_E(\pi') = \begin{cases} \frac{1}{|E|} \sum_{g=(q_1, q_2) \in E} D(\pi'(q_1), \pi'(q_2)) & |E| > 0 \\ 0.0 & |E| = 0 \end{cases}$$
//! 4. Combined Basic Distance Cost:
//!    $$\text{Cost}(\pi') = H_F(\pi') + W \cdot H_E(\pi')$$
//! 5. Decay-Scaled Candidate Score:
//!    $$\text{Score}(\text{SWAP}) = \max(\text{decay}(u), \text{decay}(v)) \times \text{Cost}(\pi')$$
//!
//! The candidate with the minimum score is selected. Ties are broken deterministically.

use serde::{Deserialize, Serialize};

use crate::routing::pathfinding::RoutingError;
use crate::routing::placement::distance::TopologyDistances;
use crate::routing::sabre::candidate::{CandidateGenerator, CandidateSwap};
use crate::routing::sabre::decay::{DecayTable, DEFAULT_DECAY_DELTA};
use crate::routing::sabre::scoring::ScoreComponents;
use crate::routing::sabre::SabreFrontierState;
use crate::target::connectivity::CouplingGraph;
use crate::target::model::Target;

/// Default lookahead weight multiplier ($W$) in SABRE literature.
pub const DEFAULT_LOOKAHEAD_WEIGHT: f64 = 0.5;

/// Configuration parameters for the SABRE distance heuristic and decay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SabreHeuristicConfig {
    /// Extended lookahead weight multiplier ($W \ge 0$).
    ///
    /// Controls the influence of upcoming lookahead operations relative to
    /// immediate front-layer operations. Default: 0.5.
    pub lookahead_weight: f64,

    /// Decay increment $\delta \ge 0$ added to physical qubits when swapped.
    ///
    /// Penalizes repeated swapping of the same physical qubits to avoid oscillation.
    /// Default: 0.001.
    pub decay_delta: f64,
}

impl Default for SabreHeuristicConfig {
    fn default() -> Self {
        Self {
            lookahead_weight: DEFAULT_LOOKAHEAD_WEIGHT,
            decay_delta: DEFAULT_DECAY_DELTA,
        }
    }
}

impl SabreHeuristicConfig {
    /// Creates a new configuration with default parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the lookahead weight parameter $W$.
    ///
    /// # Errors
    /// Returns [`RoutingError::InvalidConfig`] if `weight < 0.0` or is non-finite.
    pub fn with_lookahead_weight(mut self, weight: f64) -> Result<Self, RoutingError> {
        if !weight.is_finite() || weight < 0.0 {
            return Err(RoutingError::InvalidConfig(format!(
                "lookahead weight must be finite and non-negative, got {weight}"
            )));
        }
        self.lookahead_weight = weight;
        Ok(self)
    }

    /// Sets the decay increment parameter $\delta$.
    ///
    /// # Errors
    /// Returns [`RoutingError::InvalidConfig`] if `delta < 0.0` or is non-finite.
    pub fn with_decay_delta(mut self, delta: f64) -> Result<Self, RoutingError> {
        if !delta.is_finite() || delta < 0.0 {
            return Err(RoutingError::InvalidConfig(format!(
                "decay delta must be finite and non-negative, got {delta}"
            )));
        }
        self.decay_delta = delta;
        Ok(self)
    }

    /// Validates configuration parameters.
    pub fn validate(&self) -> Result<(), RoutingError> {
        if !self.lookahead_weight.is_finite() || self.lookahead_weight < 0.0 {
            return Err(RoutingError::InvalidConfig(format!(
                "lookahead weight must be finite and non-negative, got {}",
                self.lookahead_weight
            )));
        }
        if !self.decay_delta.is_finite() || self.decay_delta < 0.0 {
            return Err(RoutingError::InvalidConfig(format!(
                "decay delta must be finite and non-negative, got {}",
                self.decay_delta
            )));
        }
        Ok(())
    }
}

/// The SABRE Heuristic evaluation engine.
#[derive(Clone, Debug)]
pub struct SabreHeuristic {
    /// Heuristic configuration (lookahead weight and decay delta).
    config: SabreHeuristicConfig,
    /// Precomputed all-pairs shortest paths on the hardware coupling graph.
    distances: TopologyDistances,
    /// Dynamic physical qubit decay table tracking swap frequency.
    decay: DecayTable,
    /// Optional random seed for tie-breaking.
    seed: Option<u64>,
}

impl SabreHeuristic {
    /// Constructs a new SABRE heuristic engine from a hardware [`Target`] with no seed.
    pub fn new(target: &Target, config: SabreHeuristicConfig) -> Result<Self, RoutingError> {
        Self::with_seed(target, config, None)
    }

    /// Constructs a new SABRE heuristic engine from a hardware [`Target`] with an optional seed.
    pub fn with_seed(
        target: &Target,
        config: SabreHeuristicConfig,
        seed: Option<u64>,
    ) -> Result<Self, RoutingError> {
        config.validate()?;
        let coupling = target.combined_coupling_graph();
        let distances = TopologyDistances::from_coupling_graph(&coupling);
        let decay = DecayTable::with_delta(target.num_qubits(), config.decay_delta)?;
        Ok(Self {
            config,
            distances,
            decay,
            seed,
        })
    }

    /// Constructs a new SABRE heuristic engine directly from a [`CouplingGraph`] with no seed.
    pub fn from_coupling_graph(
        coupling: &CouplingGraph,
        config: SabreHeuristicConfig,
    ) -> Result<Self, RoutingError> {
        Self::from_coupling_graph_with_seed(coupling, config, None)
    }

    /// Constructs a new SABRE heuristic engine directly from a [`CouplingGraph`] with an optional seed.
    pub fn from_coupling_graph_with_seed(
        coupling: &CouplingGraph,
        config: SabreHeuristicConfig,
        seed: Option<u64>,
    ) -> Result<Self, RoutingError> {
        config.validate()?;
        let distances = TopologyDistances::from_coupling_graph(coupling);
        let decay = DecayTable::with_delta(coupling.num_qubits, config.decay_delta)?;
        Ok(Self {
            config,
            distances,
            decay,
            seed,
        })
    }

    /// Returns the optional tie-breaking seed.
    #[inline]
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// Returns a reference to the heuristic configuration.
    #[inline]
    pub fn config(&self) -> &SabreHeuristicConfig {
        &self.config
    }

    /// Returns a reference to the topology distance matrix.
    #[inline]
    pub fn distances(&self) -> &TopologyDistances {
        &self.distances
    }

    /// Returns a reference to the decay table.
    #[inline]
    pub fn decay(&self) -> &DecayTable {
        &self.decay
    }

    /// Returns a mutable reference to the decay table.
    #[inline]
    pub fn decay_mut(&mut self) -> &mut DecayTable {
        &mut self.decay
    }

    /// Evaluates a candidate SWAP $(u, v)$ against a hypothetical layout mutation.
    ///
    /// # Safety and Immutability
    /// Does NOT mutate `state`, its active layout, or the heuristic's decay table.
    /// Evaluates distances strictly on a cloned hypothetical layout $\pi' = \text{swap}(\pi, u, v)$.
    pub fn score_candidate(
        &self,
        candidate: CandidateSwap,
        state: &SabreFrontierState,
    ) -> Result<ScoreComponents, RoutingError> {
        // 1. Create hypothetical layout by applying the candidate swap
        let mut hypothetical_layout = state.layout().clone();
        hypothetical_layout
            .swap_physical(candidate.u, candidate.v)
            .map_err(|e| RoutingError::Layout(e.to_string()))?;

        let penalty = self.distances.disconnected_penalty();

        // 2. Compute normalized Front-Layer distance under hypothetical layout
        let front_ops = state.front_layer().operations();
        let front_cost = if front_ops.is_empty() {
            0.0
        } else {
            let mut sum = 0usize;
            for op in front_ops {
                let (q1, q2) = op.logical_operands();
                let p1 = hypothetical_layout
                    .get_physical(q1)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q1} not assigned")))?;
                let p2 = hypothetical_layout
                    .get_physical(q2)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q2} not assigned")))?;
                sum += self.distances.distance_or(p1, p2, penalty);
            }
            sum as f64 / front_ops.len() as f64
        };

        // 3. Compute normalized Extended-Lookahead distance under hypothetical layout
        let lookahead_ops = state.lookahead().operations();
        let lookahead_cost = if lookahead_ops.is_empty() {
            0.0
        } else {
            let mut sum = 0usize;
            for op in lookahead_ops {
                let (q1, q2) = op.logical_operands();
                let p1 = hypothetical_layout
                    .get_physical(q1)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q1} not assigned")))?;
                let p2 = hypothetical_layout
                    .get_physical(q2)
                    .ok_or_else(|| RoutingError::Layout(format!("qubit {q2} not assigned")))?;
                sum += self.distances.distance_or(p1, p2, penalty);
            }
            sum as f64 / lookahead_ops.len() as f64
        };

        // 4. Combine basic distance cost
        let w = self.config.lookahead_weight;
        let weighted_lookahead_cost = w * lookahead_cost;
        let basic_cost = front_cost + weighted_lookahead_cost;

        // 5. Apply decay factor for candidate physical qubits
        let decay_factor = self.decay.decay_factor(candidate);
        let total_score = decay_factor * basic_cost;

        Ok(ScoreComponents::new(
            candidate,
            front_cost,
            lookahead_cost,
            w,
            weighted_lookahead_cost,
            basic_cost,
            decay_factor,
            total_score,
        ))
    }

    /// Evaluates and ranks all candidate SWAPs for the current front layer.
    ///
    /// Generates candidate SWAPs along coupling edges incident to front-layer operands,
    /// scores each candidate under its hypothetical layout, and sorts the results
    /// deterministically.
    ///
    /// Returns an ordered list of scored candidates where index 0 is the best (lowest score) candidate.
    pub fn evaluate_and_rank_candidates(
        &self,
        state: &SabreFrontierState,
        target: &Target,
    ) -> Result<Vec<ScoreComponents>, RoutingError> {
        let coupling = target.combined_coupling_graph();
        self.evaluate_and_rank_candidates_with_coupling(state, &coupling)
    }

    /// Evaluates and ranks candidate SWAPs using a standalone [`CouplingGraph`].
    ///
    /// # Errors
    /// Returns [`RoutingError::DisconnectedTopology`] if any operation in the front layer
    /// has physical operands residing in disconnected components of the coupling graph,
    /// where physical routing is impossible.
    pub fn evaluate_and_rank_candidates_with_coupling(
        &self,
        state: &SabreFrontierState,
        coupling: &CouplingGraph,
    ) -> Result<Vec<ScoreComponents>, RoutingError> {
        // Fail fast if any front-layer operation is impossible to route due to disconnected physical components
        for op in state.front_layer().operations() {
            let (p1, p2) = op.physical_operands();
            if self.distances.distance(p1, p2).is_none() {
                return Err(RoutingError::DisconnectedTopology { src: p1, dst: p2 });
            }
        }

        let candidates = CandidateGenerator::generate_candidates_with_coupling(
            state.front_layer(),
            coupling,
            &self.distances,
        );

        let mut scored = Vec::with_capacity(candidates.len());
        for cand in candidates {
            let score = self.score_candidate(cand, state)?;
            scored.push(score);
        }

        let seed = self.seed;
        scored.sort_unstable_by(|a, b| a.cmp_deterministic_with_seed(b, seed));
        Ok(scored)
    }

    /// Evaluates candidate SWAPs and returns the single best candidate, if any viable candidate exists.
    pub fn best_candidate(
        &self,
        state: &SabreFrontierState,
        target: &Target,
    ) -> Result<Option<ScoreComponents>, RoutingError> {
        let ranked = self.evaluate_and_rank_candidates(state, target)?;
        Ok(ranked.into_iter().next())
    }

    /// Records the execution of candidate SWAP $(u, v)$ in the decay table.
    ///
    /// # Errors
    /// Returns [`RoutingError::QubitOutOfBounds`] if either qubit is out of bounds.
    #[inline]
    pub fn record_swap(&mut self, candidate: CandidateSwap) -> Result<(), RoutingError> {
        self.decay.record_swap(candidate)
    }

    /// Resets all decay factors back to baseline ($1.0$).
    ///
    /// Must be invoked whenever any front-layer gate is satisfied and executed.
    #[inline]
    pub fn reset_decay(&mut self) {
        self.decay.reset();
    }
}
