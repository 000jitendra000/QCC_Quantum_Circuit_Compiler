//! QCC Routing, Layout, and Placement subsystem.
//!
//! Separates logical program qubit identities from physical device locations,
//! maintains injective bidirectional mappings, tracks layout mutations during SWAP insertion,
//! and enforces ADR-0001 strict ancilla zero-state policies.

pub mod layout;
pub mod layout_map;
pub mod placement;

pub use layout::CircuitLayout;
pub use layout_map::{Layout, LayoutError, LayoutMap, LogicalQubitId};
pub use placement::{
    place_best_of, place_circuit, place_circuit_with_coupling, DegreePlacer, IdentityPlacer,
    InteractionGraph, PlacementError, PlacementResult, PlacementScore, PlacementStrategy,
    RandomPlacer, TopologyDistances, WeightedPlacer,
};
