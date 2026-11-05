//! Timeline schedule representation and scheduled operation data models.
//!
//! Represents physical operations bound to concrete execution intervals $[t_{\text{start}}, t_{\text{end}})$
//! on a target device with dedicated hardware resource reservations.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::node::OperationId;
use crate::ir::wire::ClassicalBitId;
use crate::target::operation::PhysicalQubitId;
use crate::target::resources::ResourceRequirement;

/// A physical quantum or classical operation bound to a concrete time interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduledOperation {
    /// Stable semantic identifier of the QccIR DAG node.
    pub id: OperationId,
    /// Topological sequence number from original circuit analysis.
    pub sequence_number: u64,
    /// Operation mnemonic (e.g. `"cx"`, `"sx"`, `"rz"`, `"measure"`).
    pub name: String,
    /// Physical device qubits acted upon by this operation.
    pub operands: Vec<PhysicalQubitId>,
    /// Classical bits read by this operation (RAW inducing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classical_reads: Vec<ClassicalBitId>,
    /// Classical bits written by this operation (WAW and WAR inducing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classical_writes: Vec<ClassicalBitId>,
    /// Start time in nanoseconds ($\ge 0.0$).
    pub start_time: f64,
    /// Duration in nanoseconds ($\ge 0.0$).
    pub duration: f64,
    /// Completion / end time in nanoseconds ($= \text{start\_time} + \text{duration}$).
    pub end_time: f64,
    /// Hardware resource requirements reserved during $[\text{start\_time}, \text{end\_time})$.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRequirement>,
    /// Direct DAG predecessor IDs whose completion constrained this operation's start time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predecessors: Vec<OperationId>,
    /// Optional operation slack in nanoseconds ($\text{Slack} = \text{start}_{\text{ALAP}} - \text{start}_{\text{ASAP}}$).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack: Option<f64>,
}

impl ScheduledOperation {
    /// Constructs a new scheduled operation with calculated end time.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: OperationId,
        sequence_number: u64,
        name: impl Into<String>,
        operands: Vec<PhysicalQubitId>,
        start_time: f64,
        duration: f64,
        resources: Vec<ResourceRequirement>,
        predecessors: Vec<OperationId>,
    ) -> Self {
        let start_time = if start_time == 0.0 { 0.0 } else { start_time };
        let duration = if duration == 0.0 { 0.0 } else { duration };
        let end_time = start_time + duration;
        let end_time = if end_time == 0.0 { 0.0 } else { end_time };
        Self {
            id,
            sequence_number,
            name: name.into(),
            operands,
            classical_reads: Vec::new(),
            classical_writes: Vec::new(),
            start_time,
            duration,
            end_time,
            resources,
            predecessors,
            slack: None,
        }
    }

    /// Sets operation slack in nanoseconds.
    pub fn with_slack(mut self, slack: f64) -> Self {
        self.slack = Some(if slack == 0.0 { 0.0 } else { slack });
        self
    }

    /// Returns the operation slack in nanoseconds, if calculated.
    #[inline]
    pub fn slack(&self) -> Option<f64> {
        self.slack
    }

    /// Sets classical bit reads.
    pub fn with_classical_reads(mut self, reads: Vec<ClassicalBitId>) -> Self {
        self.classical_reads = reads;
        self
    }

    /// Sets classical bit writes.
    pub fn with_classical_writes(mut self, writes: Vec<ClassicalBitId>) -> Self {
        self.classical_writes = writes;
        self
    }

    /// Returns `true` if this operation is actively executing at timestamp `time`.
    ///
    /// # Mathematical Semantics
    /// - For positive durations ($d > 0.0$), uses the canonical half-open interval $[t_{\text{start}}, t_{\text{end}})$:
    ///   an operation is active iff $t_{\text{start}} \le t < t_{\text{end}}$.
    /// - For zero-duration operations ($d = 0.0$), an operation is instantaneous and active strictly at $t = t_{\text{start}}$.
    #[inline]
    pub fn is_active_at(&self, time: f64) -> bool {
        if self.duration == 0.0 {
            time == self.start_time
        } else {
            time >= self.start_time && time < self.end_time
        }
    }

    /// Returns `true` if this operation is active at timestamp `time`, with an optional tolerance for zero-duration ops.
    #[inline]
    pub fn is_active_at_with_tolerance(&self, time: f64, epsilon: f64) -> bool {
        if self.duration == 0.0 {
            if epsilon > 0.0 {
                (time - self.start_time).abs() <= epsilon
            } else {
                time == self.start_time
            }
        } else {
            time >= self.start_time && time < self.end_time
        }
    }

    /// Returns `true` if this operation's execution interval overlaps with another's
    /// under mathematically exact half-open interval semantics.
    ///
    /// # Mathematical Half-Open Interval Semantics
    ///
    /// - An operation with positive duration $d > 0$ occupies the half-open interval $[s, e)$ where $e = s + d$.
    /// - Two positive-duration operations $[s_1, e_1)$ and $[s_2, e_2)$ overlap iff their intersection is non-empty:
    ///   $$\max(s_1, s_2) < \min(e_1, e_2)$$
    ///   - Any positive overlap $\delta > 0$ (including sub-nanosecond, picosecond, and femtosecond overlaps)
    ///     is strictly detected as an overlap without being silently classified as non-overlapping.
    ///   - Exact boundary-touching operations ($e_1 = s_2$ or $e_2 = s_1$) do not overlap because
    ///     $[s_1, e_1) \cap [e_1, e_2) = \emptyset$ (evaluated as $\max(s_1, s_2) < \min(e_1, e_2) \iff e_1 < e_1$, which is identically `false`).
    /// - An instantaneous zero-duration operation ($d = 0.0$) at $t_1$ overlaps with a positive-duration operation $[s_2, e_2)$
    ///   iff $t_1$ falls strictly inside the open interval $(s_2, e_2)$:
    ///   $$s_2 < t_1 < e_2$$
    ///   - A zero-duration operation at the start boundary ($t_1 = s_2$) or end boundary ($t_1 = e_2$) does not overlap.
    /// - Two zero-duration operations do not overlap.
    #[inline]
    pub fn overlaps(&self, other: &Self) -> bool {
        let self_is_zero = self.duration == 0.0;
        let other_is_zero = other.duration == 0.0;

        if self_is_zero && other_is_zero {
            false
        } else if self_is_zero {
            self.start_time > other.start_time && self.start_time < other.end_time
        } else if other_is_zero {
            other.start_time > self.start_time && other.start_time < self.end_time
        } else {
            let max_start = self.start_time.max(other.start_time);
            let min_end = self.end_time.min(other.end_time);
            max_start < min_end
        }
    }

    /// Overlap check with tolerance parameter provided for API compatibility.
    /// Evaluates strictly under half-open interval semantics without eroding interval boundaries.
    #[inline]
    pub fn overlaps_with_tolerance(&self, other: &Self, _epsilon: f64) -> bool {
        self.overlaps(other)
    }

    /// Returns `true` if this operation acts on physical qubit `qubit`.
    pub fn touches_qubit(&self, qubit: PhysicalQubitId) -> bool {
        self.operands.contains(&qubit)
    }
}

impl fmt::Display for ScheduledOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:.2}ns - {:.2}ns] {} (op#{} seq#{}) on {:?}",
            self.start_time,
            self.end_time,
            self.name,
            self.id.0,
            self.sequence_number,
            self.operands
        )
    }
}

/// Comprehensive physical circuit execution schedule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// Name of the scheduled circuit.
    pub circuit_name: String,
    /// Name of the target hardware machine.
    pub target_name: String,
    /// Scheduled operations sorted deterministically by start time, end time, sequence number, and ID.
    pub operations: Vec<ScheduledOperation>,
    /// Total circuit makespan (maximum end time across all operations in nanoseconds).
    pub makespan: f64,
    /// Optional requested makespan limit against which the schedule was constructed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_makespan: Option<f64>,
    /// Fast lookup index mapping OperationId to operations vector index.
    #[serde(skip)]
    id_to_idx: HashMap<OperationId, usize>,
}

impl Schedule {
    /// Creates a new empty schedule.
    pub fn new(circuit_name: impl Into<String>, target_name: impl Into<String>) -> Self {
        Self {
            circuit_name: circuit_name.into(),
            target_name: target_name.into(),
            operations: Vec::new(),
            makespan: 0.0,
            requested_makespan: None,
            id_to_idx: HashMap::new(),
        }
    }

    /// Sets the requested makespan window in nanoseconds.
    pub fn with_requested_makespan(mut self, makespan: f64) -> Self {
        self.requested_makespan = Some(if makespan == 0.0 { 0.0 } else { makespan });
        self
    }

    /// Returns the requested makespan window in nanoseconds, if specified.
    #[inline]
    pub fn requested_makespan(&self) -> Option<f64> {
        self.requested_makespan
    }

    /// Returns the operation slack for the given ID in nanoseconds, if calculated.
    pub fn slack(&self, id: OperationId) -> Option<f64> {
        self.get(id).and_then(|op| op.slack)
    }

    /// Returns a map of all operation slacks in nanoseconds.
    pub fn slacks(&self) -> HashMap<OperationId, f64> {
        self.operations
            .iter()
            .filter_map(|op| op.slack.map(|s| (op.id, s)))
            .collect()
    }

    /// Returns references to operations on the critical path (slack within $10^{-9}$ ns of 0.0).
    pub fn critical_path(&self) -> Vec<&ScheduledOperation> {
        self.critical_path_with_tolerance(1e-9)
    }

    /// Returns references to operations on the critical path within a specified tolerance.
    pub fn critical_path_with_tolerance(&self, tolerance: f64) -> Vec<&ScheduledOperation> {
        self.operations
            .iter()
            .filter(|op| op.slack.is_some_and(|s| s.abs() <= tolerance))
            .collect()
    }

    /// Rebuilds the internal fast-lookup map.
    pub fn rebuild_index(&mut self) {
        self.id_to_idx.clear();
        for (idx, op) in self.operations.iter().enumerate() {
            self.id_to_idx.insert(op.id, idx);
        }
    }

    /// Adds a scheduled operation, maintaining deterministic order and updating makespan.
    pub fn add_operation(&mut self, op: ScheduledOperation) {
        if op.end_time > self.makespan {
            self.makespan = op.end_time;
        }
        self.operations.push(op);
        self.sort_and_reindex();
    }

    /// Sorts operations deterministically by `(start_time, end_time, sequence_number, id)` and rebuilds index.
    fn sort_and_reindex(&mut self) {
        self.operations.sort_by(|a, b| {
            a.start_time
                .total_cmp(&b.start_time)
                .then_with(|| a.end_time.total_cmp(&b.end_time))
                .then_with(|| a.sequence_number.cmp(&b.sequence_number))
                .then_with(|| a.id.cmp(&b.id))
        });
        self.rebuild_index();
    }

    /// Returns a reference to the scheduled operation with the given ID.
    pub fn get(&self, id: OperationId) -> Option<&ScheduledOperation> {
        if self.id_to_idx.is_empty() && !self.operations.is_empty() {
            self.operations.iter().find(|op| op.id == id)
        } else {
            self.id_to_idx
                .get(&id)
                .and_then(|&idx| self.operations.get(idx))
        }
    }

    /// Number of operations in the schedule.
    #[inline]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns `true` if the schedule contains no operations.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Total duration / makespan of the schedule in nanoseconds.
    #[inline]
    pub fn makespan(&self) -> f64 {
        self.makespan
    }

    /// Slice of all scheduled operations in deterministic order.
    #[inline]
    pub fn operations(&self) -> &[ScheduledOperation] {
        &self.operations
    }

    /// Returns all operations that act on a designated physical qubit, in chronological order.
    pub fn qubit_timeline(&self, qubit: PhysicalQubitId) -> Vec<&ScheduledOperation> {
        self.operations
            .iter()
            .filter(|op| op.touches_qubit(qubit))
            .collect()
    }

    /// Returns all operations active at the specified timestamp.
    pub fn active_operations_at(&self, time: f64) -> Vec<&ScheduledOperation> {
        self.operations
            .iter()
            .filter(|op| op.is_active_at(time))
            .collect()
    }

    /// Serializes this schedule to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this schedule to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Exports this schedule to a stable, machine-readable [`crate::scheduler::output::ScheduleOutput`] representation.
    pub fn to_output(
        &self,
        target: Option<&crate::target::model::Target>,
    ) -> crate::scheduler::output::ScheduleOutput {
        crate::scheduler::output::ScheduleOutput::from_schedule(self, target)
    }

    /// Generates structured timeline visualization data for this schedule.
    pub fn generate_timeline(&self) -> crate::scheduler::timeline::TimelineData {
        crate::scheduler::timeline::TimelineData::from_schedule(self)
    }

    /// Deserializes a schedule from JSON.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let mut schedule: Self = serde_json::from_str(json_str)?;
        schedule.rebuild_index();
        Ok(schedule)
    }
}
