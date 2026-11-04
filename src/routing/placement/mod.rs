//! Initial physical qubit placement algorithms for quantum circuits.
//!
//! Provides deterministic strategies to compute an initial physical layout
//! ($L_{\text{initial}}$) mapping logical program qubits onto device physical sites
//! before routing:
//! - **Identity**: Straightforward $q_i \to p_i$ baseline placement.
//! - **Random**: Deterministic seeded randomized exploration.
//! - **Degree**: Matches logical interaction degrees to physical topology degrees.
//! - **Weighted**: Optimizes heavy interaction edges and minimizes weighted physical distances.

pub mod degree;
pub mod distance;
pub mod interaction_graph;
pub mod score;
pub mod strategy;
pub mod trivial;
pub mod weighted;

pub use degree::DegreePlacer;
pub use distance::TopologyDistances;
pub use interaction_graph::InteractionGraph;
pub use score::PlacementScore;
pub use strategy::{PlacementError, PlacementResult, PlacementStrategy};
pub use trivial::{IdentityPlacer, RandomPlacer};
pub use weighted::WeightedPlacer;

use crate::ir::circuit::Circuit;
use crate::target::connectivity::CouplingGraph;
use crate::target::model::Target;

/// Computes an initial physical layout for `circuit` on `target` using the specified `strategy`.
pub fn place_circuit(
    circuit: &Circuit,
    target: &Target,
    strategy: PlacementStrategy,
) -> Result<PlacementResult, PlacementError> {
    if target.num_qubits() == 0 {
        return Err(PlacementError::ZeroPhysicalQubits);
    }
    let coupling = target.combined_coupling_graph();
    place_circuit_with_coupling(circuit, &coupling, strategy)
}

/// Computes an initial physical layout for `circuit` using an explicit physical [`CouplingGraph`].
pub fn place_circuit_with_coupling(
    circuit: &Circuit,
    coupling: &CouplingGraph,
    strategy: PlacementStrategy,
) -> Result<PlacementResult, PlacementError> {
    if coupling.num_qubits == 0 {
        return Err(PlacementError::ZeroPhysicalQubits);
    }

    let interactions = InteractionGraph::from_circuit(circuit);
    let distances = TopologyDistances::from_coupling_graph(coupling);

    match strategy {
        PlacementStrategy::Identity => IdentityPlacer::new().place(&interactions, &distances),
        PlacementStrategy::Random { seed } => {
            RandomPlacer::new(seed).place(&interactions, &distances)
        }
        PlacementStrategy::Degree => DegreePlacer::new().place(&interactions, &distances),
        PlacementStrategy::Weighted => WeightedPlacer::new().place(&interactions, &distances),
    }
}

/// Evaluates a list of candidate strategies and returns the placement result with the best score.
pub fn place_best_of(
    circuit: &Circuit,
    target: &Target,
    strategies: &[PlacementStrategy],
) -> Result<PlacementResult, PlacementError> {
    if strategies.is_empty() {
        return place_circuit(circuit, target, PlacementStrategy::default());
    }

    let mut best_result: Option<PlacementResult> = None;

    for &strat in strategies {
        let res = place_circuit(circuit, target, strat)?;
        match &best_result {
            None => best_result = Some(res),
            Some(curr) => {
                if res.score < curr.score {
                    best_result = Some(res);
                }
            }
        }
    }

    Ok(best_result.expect("strategies list was non-empty"))
}
