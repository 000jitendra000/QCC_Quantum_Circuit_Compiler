//! Generic quantum hardware target data model.
//!
//! Provides target modeling, physical qubit identification, operation descriptors,
//! directed/undirected physical connectivity graphs, timing/calibration databases with provenance,
//! capability queries, and cost models.
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
//!   as `MetricValue::Absent` or `DurationCost::Incomplete`/`Unknown`, **never defaulted to zero**.

pub mod calibration;
pub mod connectivity;
pub mod cost;
pub mod direction;
pub mod error;
pub mod legality;
pub mod metrics;
pub mod model;
pub mod operation;
pub mod query;
pub mod resources;
pub mod synthesis_basis;
pub mod timing;

pub use calibration::{
    parse_timestamp_to_epoch_seconds, Provenance, QubitCalibration, QubitCalibrationEntry,
    TargetCalibration, TupleCalibration, TupleCalibrationEntry,
};
pub use connectivity::{CouplingEdge, CouplingGraph, Directionality, OperationConnectivity};
pub use cost::{CircuitCost, CostModel, CostWeights, OperationCost};
pub use direction::{DirectionError, DirectionStatus};
pub use error::{TargetError, TargetResult};
pub use legality::{
    TargetLegalityError, TargetLegalityPass, TargetLegalityReport, TargetLegalityValidator,
    TargetViolation,
};
pub use metrics::{DurationCost, ErrorCost, MetricKind, MetricValue};
pub use model::{Target, TargetBuilder};
pub use operation::{
    PhysicalQubitId, TargetOpId, TargetOperationBuilder, TargetOperationDescriptor,
};
pub use query::{LegalityError, TargetQuery};
pub use resources::{
    ExclusionCondition, MutualExclusionRule, QubitReadoutLineEntry, ResourceCapacityEntry,
    ResourceConflict, ResourceId, ResourceModel, ResourceRequirement, ResourceUsageMode,
    TupleResourceRequirementEntry,
};
pub use synthesis_basis::SingleQubitBasis;
pub use timing::{
    DurationUnit, HardwareDuration, OperationDuration, ReadoutTiming, ReadoutTimingEntry,
    ResetTiming, ResetTimingEntry, TimingModel, TupleDurationEntry,
};
