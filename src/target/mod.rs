//! Generic quantum hardware target data model.
//!
//! Provides target modeling, physical qubit identification, operation descriptors,
//! directed/undirected physical connectivity graphs, and timing/calibration databases with provenance.
//!
//! # Core Invariants
//! - **No Hardcoded Vendor Bases**: Hardware capabilities, operations, and couplings are
//!   dynamically defined per `Target`.
//! - **Decoupled Identity**: Hardware-native operations (`TargetOpId`) are strictly separate
//!   from the universal logical gate catalog (`GateId`).
//! - **Typed Physical Qubits**: Physical qubits are represented by [`PhysicalQubitId`], distinct
//!   from logical [`crate::ir::wire::QubitId`].
//! - **Operation-Specific Directionality**: Directed asymmetric couplings (such as $CX$) and
//!   symmetric undirected couplings (such as $CZ$) are strictly non-interchangeable.
//! - **The Explicit Unknown Principle**: Missing calibration and timing metrics are represented
//!   as `None`, **never defaulted to zero**.

pub mod calibration;
pub mod connectivity;
pub mod error;
pub mod model;
pub mod operation;

pub use calibration::{
    Provenance, QubitCalibration, QubitCalibrationEntry, TargetCalibration, TupleCalibration,
    TupleCalibrationEntry,
};
pub use connectivity::{CouplingEdge, CouplingGraph, Directionality, OperationConnectivity};
pub use error::{TargetError, TargetResult};
pub use model::{Target, TargetBuilder};
pub use operation::{
    PhysicalQubitId, TargetOpId, TargetOperationBuilder, TargetOperationDescriptor,
};
