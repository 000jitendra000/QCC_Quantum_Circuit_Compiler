//! QCC Routing, Layout, and Placement subsystem.
//!
//! Separates logical program qubit identities from physical device locations,
//! maintains injective bidirectional mappings, tracks layout mutations during SWAP insertion,
//! and enforces ADR-0001 strict ancilla zero-state policies.

pub mod cost;
pub mod greedy;
pub mod layout;
pub mod layout_map;
pub mod metrics;
pub mod pathfinding;
pub mod placement;
pub mod provenance;
pub mod route_state;
pub mod swap_insertion;

pub use cost::{RoutingCostError, RoutingCostModel, RoutingCostWeights};
pub use greedy::{BaselineRouter, BaselineRouterConfig, BaselineRouterReport};
pub use layout::CircuitLayout;
pub use layout_map::{Layout, LayoutError, LayoutMap, LogicalQubitId};
pub use metrics::RoutingMetrics;
pub use pathfinding::{are_physical_neighbors, find_shortest_path, path_length, RoutingError};
pub use placement::{
    place_best_of, place_circuit, place_circuit_with_coupling, DegreePlacer, IdentityPlacer,
    InteractionGraph, PlacementError, PlacementResult, PlacementScore, PlacementStrategy,
    RandomPlacer, TopologyDistances, WeightedPlacer,
};
pub use provenance::{RouteOperationKind, RouteProvenanceRecord, RouteProvenanceTracker};
pub use route_state::RouteState;
pub use swap_insertion::{
    emit_physical_swap, emit_physical_swap_with_provenance, ensure_legal_orientation,
    ensure_legal_orientation_with_provenance, route_operands_along_path,
    route_operands_along_path_with_provenance, PathSwapStrategy, SwapInsertionResult,
};
