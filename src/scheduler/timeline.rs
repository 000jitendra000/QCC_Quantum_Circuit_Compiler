//! Timeline visualization data model and structured layout generator.
//!
//! Organizes scheduled operations into parallel hardware channel lanes (physical qubits,
//! two-qubit couplers, shared drive lines, readout feedlines, and classical registers).
//! Generates structured visualization models suitable for GUI rendering, SVG generation,
//! or terminal ASCII inspection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::node::OperationId;
use crate::ir::wire::ClassicalBitId;
use crate::scheduler::output::{ScheduleOutput, TimingKind};
use crate::scheduler::schedule::Schedule;
use crate::target::operation::PhysicalQubitId;
use crate::target::resources::ResourceId;

/// Kind of execution channel represented by a timeline lane.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TimelineLaneKind {
    /// Dedicated physical qubit execution lane.
    Qubit(PhysicalQubitId),
    /// Physical two-qubit coupler channel lane.
    Coupler(PhysicalQubitId, PhysicalQubitId),
    /// Multiplexed readout feedline channel lane.
    ReadoutLine(u32),
    /// Shared microwave control or drive line lane.
    DriveLine(u32),
    /// Signal acquisition unit or digitizer channel lane.
    AcquisitionUnit(u32),
    /// Classical bit register wire lane.
    ClassicalBit(ClassicalBitId),
    /// Custom named hardware execution channel.
    Custom(String),
}

impl TimelineLaneKind {
    /// Constructs a normalized symmetric coupler lane.
    #[inline]
    pub fn coupler(q0: PhysicalQubitId, q1: PhysicalQubitId) -> Self {
        if q0 <= q1 {
            Self::Coupler(q0, q1)
        } else {
            Self::Coupler(q1, q0)
        }
    }

    /// Stable unique identifier string for this lane.
    pub fn id_string(&self) -> String {
        match self {
            Self::Qubit(q) => format!("qubit_{}", q.0),
            Self::Coupler(q0, q1) => format!("coupler_{}_{}", q0.0, q1.0),
            Self::ReadoutLine(line) => format!("readout_line_{line}"),
            Self::DriveLine(line) => format!("drive_line_{line}"),
            Self::AcquisitionUnit(unit) => format!("acquisition_unit_{unit}"),
            Self::ClassicalBit(bit) => format!("cbit_{}", bit.0),
            Self::Custom(name) => format!("custom_{name}"),
        }
    }

    /// User-friendly label for display.
    pub fn display_label(&self) -> String {
        match self {
            Self::Qubit(q) => format!("Qubit {}", q.0),
            Self::Coupler(q0, q1) => format!("Coupler ({}, {})", q0.0, q1.0),
            Self::ReadoutLine(line) => format!("Feedline {line}"),
            Self::DriveLine(line) => format!("Drive Line {line}"),
            Self::AcquisitionUnit(unit) => format!("Acq Unit {unit}"),
            Self::ClassicalBit(bit) => format!("CBit {}", bit.0),
            Self::Custom(name) => name.clone(),
        }
    }
}

impl fmt::Display for TimelineLaneKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

/// Category of dependency link connecting scheduled operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyLinkKind {
    /// Direct QccIR DAG quantum data dependency.
    DagPredecessor,
    /// Classical wire hazard dependency (RAW, WAR, WAW).
    ClassicalHazard,
    /// Hardware resource contention precedence.
    ResourcePrecedence,
}

/// Visual dependency edge connecting two operations across time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineDependencyLink {
    /// Predecessor / source operation node ID.
    pub source_op_id: OperationId,
    /// Dependent / target operation node ID.
    pub target_op_id: OperationId,
    /// Completion time of the source operation in nanoseconds.
    pub source_end_ns: f64,
    /// Start time of the target operation in nanoseconds.
    pub target_start_ns: f64,
    /// Classification of the dependency edge.
    pub link_kind: DependencyLinkKind,
}

/// A discrete scheduled operation block placed within a timeline lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineBlock {
    /// Stable semantic OperationId.
    pub op_id: OperationId,
    /// Operation mnemonic (e.g. `"cx"`, `"rz"`, `"measure"`).
    pub op_name: String,
    /// Formatted display label (e.g. `"X"`, `"CX(0, 1)"`, `"Measure"`).
    pub label: String,
    /// Start time in nanoseconds ($\ge 0.0$).
    pub start_ns: f64,
    /// Duration in nanoseconds ($\ge 0.0$).
    pub duration_ns: f64,
    /// Completion time in nanoseconds ($= \text{start\_ns} + \text{duration\_ns}$).
    pub end_ns: f64,
    /// Whether this is a zero-duration virtual operation (e.g. virtual Z).
    pub is_instantaneous: bool,
    /// Physical operand qubits.
    pub operands: Vec<PhysicalQubitId>,
    /// Timing confidence classification.
    pub timing_kind: TimingKind,
    /// Operation slack in nanoseconds, if calculated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack_ns: Option<f64>,
    /// Whether this operation lies on the critical execution path (slack $\approx 0.0$).
    pub is_critical: bool,
    /// Hardware resources reserved by this operation block.
    pub resources: Vec<ResourceId>,
}

impl TimelineBlock {
    /// Returns `true` if this block is active at timestamp $t$.
    pub fn is_active_at(&self, t: f64) -> bool {
        if self.is_instantaneous {
            (self.start_ns - t).abs() <= 1e-9
        } else {
            t >= self.start_ns && t < self.end_ns
        }
    }
}

/// Parallel channel lane hosting a sequence of non-overlapping operation blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineLane {
    /// Unique stable lane identifier.
    pub id: String,
    /// Descriptive display label.
    pub name: String,
    /// Hardware channel category.
    pub kind: TimelineLaneKind,
    /// Chronologically sorted execution blocks in this lane.
    pub blocks: Vec<TimelineBlock>,
}

impl TimelineLane {
    /// Constructs a new empty timeline lane.
    pub fn new(kind: TimelineLaneKind) -> Self {
        Self {
            id: kind.id_string(),
            name: kind.display_label(),
            kind,
            blocks: Vec::new(),
        }
    }

    /// Adds an operation block, keeping blocks sorted chronologically.
    pub fn add_block(&mut self, block: TimelineBlock) {
        self.blocks.push(block);
        self.blocks.sort_by(|a, b| {
            a.start_ns
                .total_cmp(&b.start_ns)
                .then_with(|| a.end_ns.total_cmp(&b.end_ns))
                .then_with(|| a.op_id.cmp(&b.op_id))
        });
    }

    /// Total number of blocks in this lane.
    #[inline]
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Returns `true` if this lane has no operation blocks.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Structured, machine-readable timeline visualization data model.
///
/// Suitable for direct serialization to JSON for GUI rendering or terminal inspection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineData {
    /// Name of the circuit.
    pub circuit_name: String,
    /// Name of the target device.
    pub target_name: String,
    /// Total circuit makespan in nanoseconds.
    pub makespan_ns: f64,
    /// Chronologically ordered channel lanes.
    pub lanes: Vec<TimelineLane>,
    /// Dependency links across operations.
    pub dependency_links: Vec<TimelineDependencyLink>,
    /// Operation IDs lying on the critical path.
    pub critical_path: Vec<OperationId>,
    /// Global time bounds: `(min_time_ns, max_time_ns)`.
    pub time_bounds: (f64, f64),
}

impl TimelineData {
    /// Generates timeline visualization data from an active [`Schedule`].
    pub fn from_schedule(schedule: &Schedule) -> Self {
        let output = ScheduleOutput::from_schedule(schedule, None);
        Self::from_output(&output)
    }

    /// Generates timeline visualization data from a [`ScheduleOutput`].
    pub fn from_output(output: &ScheduleOutput) -> Self {
        let mut lanes_map: BTreeMap<TimelineLaneKind, TimelineLane> = BTreeMap::new();
        let mut links = Vec::new();

        // 1. Pre-populate lanes for all touched physical qubits
        for &q in &output.summary.physical_qubits_used {
            let kind = TimelineLaneKind::Qubit(q);
            lanes_map
                .entry(kind.clone())
                .or_insert_with(|| TimelineLane::new(kind));
        }

        // 2. Pre-populate lanes for other reserved resources (couplers, drive lines, feedlines)
        for res in &output.resources_used {
            match res {
                ResourceId::Qubit(q) => {
                    let kind = TimelineLaneKind::Qubit(*q);
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
                ResourceId::Coupler(q0, q1) => {
                    let kind = TimelineLaneKind::coupler(*q0, *q1);
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
                ResourceId::ReadoutLine(line) => {
                    let kind = TimelineLaneKind::ReadoutLine(*line);
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
                ResourceId::DriveLine(line) => {
                    let kind = TimelineLaneKind::DriveLine(*line);
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
                ResourceId::AcquisitionUnit(unit) => {
                    let kind = TimelineLaneKind::AcquisitionUnit(*unit);
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
                ResourceId::Custom(name) => {
                    let kind = TimelineLaneKind::Custom(name.clone());
                    lanes_map
                        .entry(kind.clone())
                        .or_insert_with(|| TimelineLane::new(kind));
                }
            }
        }

        // Fast lookup of end times for link generation
        let mut op_endpoints: BTreeMap<OperationId, (f64, f64)> = BTreeMap::new();

        // 3. Populate blocks into their respective lanes
        for op in &output.operations {
            let Some(start_ns) = op.start_time else {
                continue;
            };
            let Some(duration_ns) = op.timing.duration_ns() else {
                continue;
            };
            let end_ns = op.end_time.unwrap_or(start_ns + duration_ns);
            op_endpoints.insert(op.id, (start_ns, end_ns));

            let is_instantaneous = duration_ns == 0.0;
            let is_critical = op.slack.is_some_and(|s| s.abs() <= 1e-9);

            let label = match op.operands.as_slice() {
                [] => op.name.to_uppercase(),
                [q] => format!("{}({q})", op.name.to_uppercase()),
                [q0, q1] => format!("{}({q0}, {q1})", op.name.to_uppercase()),
                _ => {
                    let ops = op
                        .operands
                        .iter()
                        .map(|q| q.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}({ops})", op.name.to_uppercase())
                }
            };

            let block = TimelineBlock {
                op_id: op.id,
                op_name: op.name.clone(),
                label,
                start_ns,
                duration_ns,
                end_ns,
                is_instantaneous,
                operands: op.operands.clone(),
                timing_kind: op.timing.kind(),
                slack_ns: op.slack,
                is_critical,
                resources: op.resources.iter().map(|r| r.resource.clone()).collect(),
            };

            // Add block to operand qubit lanes
            for &q in &op.operands {
                let kind = TimelineLaneKind::Qubit(q);
                lanes_map
                    .entry(kind.clone())
                    .or_insert_with(|| TimelineLane::new(kind))
                    .add_block(block.clone());
            }

            // If 2Q, also add block to coupler lane
            if op.operands.len() == 2 {
                let kind = TimelineLaneKind::coupler(op.operands[0], op.operands[1]);
                lanes_map
                    .entry(kind.clone())
                    .or_insert_with(|| TimelineLane::new(kind))
                    .add_block(block.clone());
            }

            // If using shared resources (e.g. drive lines, feedlines), add block to those lanes
            for req in &op.resources {
                match &req.resource {
                    ResourceId::DriveLine(line) => {
                        let kind = TimelineLaneKind::DriveLine(*line);
                        lanes_map
                            .entry(kind.clone())
                            .or_insert_with(|| TimelineLane::new(kind))
                            .add_block(block.clone());
                    }
                    ResourceId::ReadoutLine(line) => {
                        let kind = TimelineLaneKind::ReadoutLine(*line);
                        lanes_map
                            .entry(kind.clone())
                            .or_insert_with(|| TimelineLane::new(kind))
                            .add_block(block.clone());
                    }
                    ResourceId::AcquisitionUnit(unit) => {
                        let kind = TimelineLaneKind::AcquisitionUnit(*unit);
                        lanes_map
                            .entry(kind.clone())
                            .or_insert_with(|| TimelineLane::new(kind))
                            .add_block(block.clone());
                    }
                    _ => {}
                }
            }
        }

        // 4. Generate dependency links
        for op in &output.operations {
            let Some(&(target_start, _)) = op_endpoints.get(&op.id) else {
                continue;
            };

            for &pred_id in &op.predecessors {
                if let Some(&(_, source_end)) = op_endpoints.get(&pred_id) {
                    links.push(TimelineDependencyLink {
                        source_op_id: pred_id,
                        target_op_id: op.id,
                        source_end_ns: source_end,
                        target_start_ns: target_start,
                        link_kind: DependencyLinkKind::DagPredecessor,
                    });
                }
            }
        }

        let makespan_ns = output.makespan_ns.unwrap_or(0.0);

        Self {
            circuit_name: output.circuit_name.clone(),
            target_name: output.target_name.clone(),
            makespan_ns,
            lanes: lanes_map.into_values().collect(),
            dependency_links: links,
            critical_path: output.summary.critical_path_operations.clone(),
            time_bounds: (0.0, makespan_ns),
        }
    }

    /// Serializes this timeline data model to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this timeline data model to formatted JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes timeline data from JSON.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Returns a reference to the lane of the specified kind, if present.
    pub fn lane(&self, kind: &TimelineLaneKind) -> Option<&TimelineLane> {
        self.lanes.iter().find(|l| &l.kind == kind)
    }

    /// Returns all blocks active across all lanes at timestamp `time_ns`.
    pub fn blocks_at(&self, time_ns: f64) -> Vec<&TimelineBlock> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();

        for lane in &self.lanes {
            for block in &lane.blocks {
                if block.is_active_at(time_ns) && seen.insert(block.op_id) {
                    result.push(block);
                }
            }
        }

        result
    }

    /// Generates a human-readable ASCII text summary of all timeline lanes and scheduled intervals.
    pub fn to_ascii_timeline(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Timeline: {} on {} (Makespan: {:.2} ns, Critical Path: {} ops)\n",
            self.circuit_name,
            self.target_name,
            self.makespan_ns,
            self.critical_path.len()
        ));
        out.push_str(&format!("{:-<75}\n", ""));

        for lane in &self.lanes {
            out.push_str(&format!("{:<20} | ", lane.name));
            if lane.is_empty() {
                out.push_str("(idle)\n");
                continue;
            }

            let mut block_strs = Vec::new();
            for b in &lane.blocks {
                let crit_mark = if b.is_critical { "*" } else { "" };
                block_strs.push(format!(
                    "[{:.1}ns {}{}{:.1}ns]",
                    b.start_ns, b.label, crit_mark, b.end_ns
                ));
            }
            out.push_str(&block_strs.join(" -> "));
            out.push('\n');
        }

        out.push_str(&format!("{:-<75}\n", ""));
        out.push_str("Legend: * = critical path operation\n");
        out
    }
}
