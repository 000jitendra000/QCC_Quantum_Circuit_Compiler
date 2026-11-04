//! QCC Routing, Layout, and Placement subsystem.
//!
//! Separates logical program qubit identities from physical device locations,
//! maintains injective bidirectional mappings, tracks layout mutations during SWAP insertion,
//! and enforces ADR-0001 strict ancilla zero-state policies.

pub mod layout;
pub mod layout_map;

pub use layout::CircuitLayout;
pub use layout_map::{Layout, LayoutError, LayoutMap, LogicalQubitId};
