//! QCC Routing, Layout, and Placement subsystem.
//!
//! Separates logical program qubit identities from physical device locations,
//! maintains injective bidirectional mappings, tracks layout mutations during SWAP insertion,
//! and enforces ADR-0001 strict ancilla zero-state policies.

pub mod greedy;
pub mod layout;
pub mod layout_map;
pub mod pathfinding;
pub mod placement;
pub mod swap_insertion;

pub use greedy::{BaselineRouter, BaselineRouterConfig, BaselineRouterReport};
pub use layout::CircuitLayout;
pub use layout_map::{Layout, LayoutError, LayoutMap, LogicalQubitId};
pub use pathfinding::{are_physical_neighbors, find_shortest_path, path_length, RoutingError};
pub use placement::{
    place_best_of, place_circuit, place_circuit_with_coupling, DegreePlacer, IdentityPlacer,
    InteractionGraph, PlacementError, PlacementResult, PlacementScore, PlacementStrategy,
    RandomPlacer, TopologyDistances, WeightedPlacer,
};
pub use swap_insertion::{
    emit_physical_swap, ensure_legal_orientation, route_operands_along_path, PathSwapStrategy,
};
