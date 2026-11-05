//! Stable, deterministic schedule serialization and machine-readable output representation.
//!
//! Preserves physical operand identities, hardware resource requirements, timing provenance,
//! and explicit duration categories ([`OperationDuration::Known`], [`OperationDuration::Estimated`],
//! [`OperationDuration::Unknown`]).
//!
//! **Non-Negotiable Invariant**: [`OperationTiming::Unknown`] is explicitly preserved and **never**
//! converted, defaulted, or coerced to zero or another fabricated numerical value.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ir::node::OperationId;
use crate::ir::wire::ClassicalBitId;
use crate::scheduler::schedule::{Schedule, ScheduledOperation};
use crate::scheduler::timeline::TimelineData;
use crate::scheduler::ScheduleError;
use crate::target::error::TargetError;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::target::resources::{ResourceId, ResourceRequirement};
use crate::target::timing::{DurationUnit, HardwareDuration, OperationDuration};

/// Normalizes floating-point timestamps so that `-0.0` becomes `+0.0`.
#[inline]
fn normalize_time(t: f64) -> f64 {
    if t == 0.0 {
        0.0
    } else {
        t
    }
}

/// Exact floating-point comparator normalizing signed zeros.
#[inline]
fn exact_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    normalize_time(a).total_cmp(&normalize_time(b))
}

/// Timing category classification for an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TimingKind {
    /// Empirically calibrated duration with benchmark provenance.
    Known,
    /// Model-based or datasheet nominal estimated duration.
    Estimated,
    /// Uncalibrated or missing duration (never coerced to zero).
    Unknown,
}

/// Explicit operation timing representation distinguishing calibrated, estimated, and unknown durations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OperationTiming {
    /// Empirically benchmarked duration.
    Known {
        /// Calibrated duration in nanoseconds ($\ge 0.0$).
        duration_ns: f64,
        /// Original hardware duration unit.
        unit: DurationUnit,
        /// Original numeric duration in its declared unit.
        original_value: f64,
    },
    /// Model-derived estimated duration.
    Estimated {
        /// Estimated duration in nanoseconds ($\ge 0.0$).
        duration_ns: f64,
        /// Original hardware duration unit.
        unit: DurationUnit,
        /// Original numeric duration in its declared unit.
        original_value: f64,
    },
    /// Uncalibrated or unknown duration.
    ///
    /// Following the Explicit Unknown Principle, missing timing data is represented
    /// as unknown and must never be interpreted as zero.
    Unknown,
}

impl OperationTiming {
    /// Constructs a known timing value in nanoseconds.
    #[inline]
    pub fn known_ns(ns: f64) -> Self {
        let ns = normalize_time(ns);
        Self::Known {
            duration_ns: ns,
            unit: DurationUnit::Nanoseconds,
            original_value: ns,
        }
    }

    /// Constructs an estimated timing value in nanoseconds.
    #[inline]
    pub fn estimated_ns(ns: f64) -> Self {
        let ns = normalize_time(ns);
        Self::Estimated {
            duration_ns: ns,
            unit: DurationUnit::Nanoseconds,
            original_value: ns,
        }
    }

    /// Returns the duration in nanoseconds if known or estimated, or `None` if unknown.
    ///
    /// **Never returns zero or fabricated defaults for unknown timing.**
    #[inline]
    pub fn duration_ns(&self) -> Option<f64> {
        match self {
            Self::Known { duration_ns, .. } | Self::Estimated { duration_ns, .. } => {
                Some(*duration_ns)
            }
            Self::Unknown => None,
        }
    }

    /// Returns the timing category classification.
    #[inline]
    pub fn kind(&self) -> TimingKind {
        match self {
            Self::Known { .. } => TimingKind::Known,
            Self::Estimated { .. } => TimingKind::Estimated,
            Self::Unknown => TimingKind::Unknown,
        }
    }

    /// Returns `true` if this timing is empirically known.
    #[inline]
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known { .. })
    }

    /// Returns `true` if this timing is estimated.
    #[inline]
    pub fn is_estimated(&self) -> bool {
        matches!(self, Self::Estimated { .. })
    }

    /// Returns `true` if this timing is unknown.
    #[inline]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Converts a target [`OperationDuration`] into an [`OperationTiming`] using the clock period.
    pub fn from_operation_duration(
        dur: &OperationDuration,
        clock_period_ns: Option<f64>,
    ) -> Result<Self, TargetError> {
        match dur {
            OperationDuration::Known(hd) => {
                let ns = hd.to_nanoseconds(clock_period_ns)?;
                Ok(Self::Known {
                    duration_ns: normalize_time(ns),
                    unit: hd.unit,
                    original_value: hd.value,
                })
            }
            OperationDuration::Estimated(hd) => {
                let ns = hd.to_nanoseconds(clock_period_ns)?;
                Ok(Self::Estimated {
                    duration_ns: normalize_time(ns),
                    unit: hd.unit,
                    original_value: hd.value,
                })
            }
            OperationDuration::Unknown => Ok(Self::Unknown),
        }
    }

    /// Converts this timing back into a target [`OperationDuration`].
    pub fn to_operation_duration(&self) -> OperationDuration {
        match self {
            Self::Known {
                unit,
                original_value,
                ..
            } => OperationDuration::Known(HardwareDuration {
                value: *original_value,
                unit: *unit,
            }),
            Self::Estimated {
                unit,
                original_value,
                ..
            } => OperationDuration::Estimated(HardwareDuration {
                value: *original_value,
                unit: *unit,
            }),
            Self::Unknown => OperationDuration::Unknown,
        }
    }
}

/// Exported physical operation with complete timing, operands, and resource provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduledOperationOutput {
    /// Stable semantic OperationId.
    pub id: OperationId,
    /// Sequence number in DAG topological order.
    pub sequence_number: u64,
    /// Target operation mnemonic (e.g. `"cx"`, `"rz"`, `"measure"`).
    pub name: String,
    /// Physical operand qubits.
    pub operands: Vec<PhysicalQubitId>,
    /// Classical bits read (RAW hazard tracking).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classical_reads: Vec<ClassicalBitId>,
    /// Classical bits written (WAW and WAR hazard tracking).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classical_writes: Vec<ClassicalBitId>,
    /// Start time in nanoseconds, if scheduled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    /// Explicit timing category and duration details.
    pub timing: OperationTiming,
    /// Calculated completion time in nanoseconds, if start and duration are known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    /// Hardware resource requirements reserved during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRequirement>,
    /// DAG predecessors constraining this operation's placement.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predecessors: Vec<OperationId>,
    /// Calculated operation slack in nanoseconds ($\text{start}_{\text{ALAP}} - \text{start}_{\text{ASAP}}$).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack: Option<f64>,
}

impl ScheduledOperationOutput {
    /// Creates an exported operation from an active [`ScheduledOperation`].
    pub fn from_scheduled(op: &ScheduledOperation, timing: Option<OperationTiming>) -> Self {
        let timing = timing.unwrap_or_else(|| OperationTiming::known_ns(op.duration));
        let start = normalize_time(op.start_time);
        let end = normalize_time(op.end_time);

        Self {
            id: op.id,
            sequence_number: op.sequence_number,
            name: op.name.clone(),
            operands: op.operands.clone(),
            classical_reads: op.classical_reads.clone(),
            classical_writes: op.classical_writes.clone(),
            start_time: Some(start),
            timing,
            end_time: Some(end),
            resources: op.resources.clone(),
            predecessors: op.predecessors.clone(),
            slack: op.slack.map(normalize_time),
        }
    }

    /// Reconstructs a [`ScheduledOperation`] if this operation has known start and duration.
    ///
    /// Returns [`ScheduleError::UnknownDuration`] if the operation timing is unknown,
    /// adhering to the Explicit Unknown Principle.
    pub fn to_scheduled(&self) -> Result<ScheduledOperation, ScheduleError> {
        let Some(start) = self.start_time else {
            return Err(ScheduleError::IncompleteScheduleData(format!(
                "operation '{}' (op#{}) lacks start_time",
                self.name, self.id.0
            )));
        };

        let Some(duration) = self.timing.duration_ns() else {
            return Err(ScheduleError::UnknownDuration {
                node_id: self.id,
                op_name: self.name.clone(),
                operands: self.operands.clone(),
            });
        };

        let start = normalize_time(start);
        let duration = normalize_time(duration);

        let mut op = ScheduledOperation::new(
            self.id,
            self.sequence_number,
            self.name.clone(),
            self.operands.clone(),
            start,
            duration,
            self.resources.clone(),
            self.predecessors.clone(),
        )
        .with_classical_reads(self.classical_reads.clone())
        .with_classical_writes(self.classical_writes.clone());

        if let Some(slack) = self.slack {
            op = op.with_slack(normalize_time(slack));
        }

        Ok(op)
    }
}

/// Statistical summary of a physical schedule export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleSummary {
    /// Total number of operations recorded in the schedule output.
    pub total_operations: usize,
    /// Number of operations with concrete scheduled start times.
    pub scheduled_operations: usize,
    /// Number of operations with explicitly unknown timing.
    pub unknown_timing_operations: usize,
    /// Sorted list of all physical qubits touched by operations in this schedule.
    pub physical_qubits_used: Vec<PhysicalQubitId>,
    /// Operation IDs of all critical path operations (slack within $10^{-9}$ ns of 0.0).
    pub critical_path_operations: Vec<OperationId>,
    /// Peak number of operations actively executing in parallel.
    pub peak_concurrency: usize,
}

/// Comprehensive, machine-readable schedule export document.
///
/// Encapsulates the entire physical schedule with deterministic ordering, timing provenance,
/// physical operand identities, reserved hardware resources, and summary metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleOutput {
    /// Schema version for format compatibility.
    pub schema_version: String,
    /// Name of the compiled circuit.
    pub circuit_name: String,
    /// Target hardware architecture name.
    pub target_name: String,
    /// Total schedule makespan in nanoseconds, if all operations have known timing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub makespan_ns: Option<f64>,
    /// Optional requested makespan limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_makespan_ns: Option<f64>,
    /// Deterministically sorted operations: ordered by `(start_time, end_time, sequence_number, id)`.
    pub operations: Vec<ScheduledOperationOutput>,
    /// Distinct hardware resources reserved across the entire schedule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources_used: Vec<ResourceId>,
    /// Statistical schedule summary.
    pub summary: ScheduleSummary,
}

impl ScheduleOutput {
    /// The current stable schema version.
    pub const CURRENT_SCHEMA_VERSION: &'static str = "qcc.schedule.v1";

    /// Constructs an exported schedule output from an active [`Schedule`] and optional target calibration.
    pub fn from_schedule(schedule: &Schedule, target: Option<&Target>) -> Self {
        let clock_period = target.and_then(|t| t.timing.as_ref().and_then(|tm| tm.clock_period_ns));

        let mut operations = Vec::with_capacity(schedule.len());
        let mut qubits_set: BTreeSet<PhysicalQubitId> = BTreeSet::new();
        let mut resources_set: BTreeSet<ResourceId> = BTreeSet::new();
        let mut critical_ops = Vec::new();

        for op in schedule.operations() {
            for &q in &op.operands {
                qubits_set.insert(q);
            }
            for req in &op.resources {
                resources_set.insert(req.resource.clone());
            }
            if op.slack.is_some_and(|s| s.abs() <= 1e-9) {
                critical_ops.push(op.id);
            }

            // Derive explicit timing from target if available
            let timing = if let Some(target) = target {
                let dur = target.operation_duration(&op.name, &op.operands);
                OperationTiming::from_operation_duration(&dur, clock_period)
                    .unwrap_or_else(|_| OperationTiming::known_ns(op.duration))
            } else {
                OperationTiming::known_ns(op.duration)
            };

            operations.push(ScheduledOperationOutput::from_scheduled(op, Some(timing)));
        }

        // Sort deterministically by (start_time, end_time, sequence_number, id)
        operations.sort_by(|a, b| {
            let start_cmp = match (a.start_time, b.start_time) {
                (Some(s1), Some(s2)) => exact_cmp(s1, s2),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            start_cmp
                .then_with(|| match (a.end_time, b.end_time) {
                    (Some(e1), Some(e2)) => exact_cmp(e1, e2),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then_with(|| a.sequence_number.cmp(&b.sequence_number))
                .then_with(|| a.id.cmp(&b.id))
        });

        // Compute peak parallel operations
        let mut peak_concurrency = 0;
        let mut transition_times = Vec::new();
        for op in schedule.operations() {
            transition_times.push(op.start_time);
            transition_times.push(op.end_time);
        }
        transition_times.sort_by(|a, b| exact_cmp(*a, *b));
        transition_times.dedup_by(|a, b| exact_cmp(*a, *b).is_eq());

        for t in transition_times {
            let active = schedule.active_operations_at(t).len();
            if active > peak_concurrency {
                peak_concurrency = active;
            }
        }

        let summary = ScheduleSummary {
            total_operations: operations.len(),
            scheduled_operations: operations.iter().filter(|o| o.start_time.is_some()).count(),
            unknown_timing_operations: operations.iter().filter(|o| o.timing.is_unknown()).count(),
            physical_qubits_used: qubits_set.into_iter().collect(),
            critical_path_operations: critical_ops,
            peak_concurrency,
        };

        Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION.to_string(),
            circuit_name: schedule.circuit_name.clone(),
            target_name: schedule.target_name.clone(),
            makespan_ns: Some(normalize_time(schedule.makespan())),
            requested_makespan_ns: schedule.requested_makespan().map(normalize_time),
            operations,
            resources_used: resources_set.into_iter().collect(),
            summary,
        }
    }

    /// Reconstructs a full [`Schedule`] from this exported representation.
    ///
    /// Validates that all operations have valid, finite start times and durations.
    /// If any operation has [`OperationTiming::Unknown`] or missing start time,
    /// returns [`ScheduleError::UnknownDuration`] or [`ScheduleError::IncompleteScheduleData`].
    pub fn to_schedule(&self) -> Result<Schedule, ScheduleError> {
        let mut schedule = Schedule::new(&self.circuit_name, &self.target_name);

        if let Some(req_makespan) = self.requested_makespan_ns {
            schedule = schedule.with_requested_makespan(req_makespan);
        }

        for op_out in &self.operations {
            let scheduled_op = op_out.to_scheduled()?;
            schedule.add_operation(scheduled_op);
        }

        // Validate makespan consistency if recorded
        if let Some(expected_makespan) = self.makespan_ns {
            let actual = schedule.makespan();
            if (actual - expected_makespan).abs() > 1e-9 {
                return Err(ScheduleError::IncompleteScheduleData(format!(
                    "reconstructed makespan {actual:.4}ns does not match exported makespan {expected_makespan:.4}ns"
                )));
            }
        }

        Ok(schedule)
    }

    /// Serializes this schedule output to a compact JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this schedule output to a pretty-printed, indented JSON string.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a [`ScheduleOutput`] from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Generates structured timeline visualization data for this schedule.
    pub fn generate_timeline(&self) -> TimelineData {
        TimelineData::from_output(self)
    }
}
