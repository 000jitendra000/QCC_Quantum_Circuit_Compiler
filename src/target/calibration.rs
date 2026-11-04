//! Hardware calibration metrics, timing models, and provenance tracking.
//!
//! Enforces the Explicit Unknown Principle: unmeasured or unavailable metrics are
//! represented strictly as `None`, never assumed or defaulted to zero.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::target::error::TargetError;
use crate::target::operation::{PhysicalQubitId, TargetOpId};

/// Calibration and timing provenance metadata.
///
/// Records the origin, characterization methodology, and timestamp of supplied hardware metrics.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Provenance {
    /// Identifier or name of the calibration source (e.g. `"backend_snapshot"`, `"datasheet_v2"`).
    pub source: String,
    /// Optional ISO 8601 or wall-clock timestamp string (e.g. `"2026-11-04T05:00:00Z"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Optional characterization or benchmarking method (e.g. `"randomized_benchmarking"`, `"ramsey"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

impl Provenance {
    /// Creates a new provenance record with the specified source name.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            timestamp: None,
            method: None,
        }
    }

    /// Sets the timestamp string.
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Sets the benchmarking or characterization method.
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }
}

/// Physical qubit-level characterization metrics.
///
/// Missing metrics remain `None` (`Unknown`), never assumed to be zero.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QubitCalibration {
    /// Energy relaxation time $T_1$ (e.g. in microseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t1: Option<f64>,
    /// Dephasing time $T_2$ (e.g. in microseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t2: Option<f64>,
    /// Readout assignment error probability ($1 - \mathcal{F}_{\text{readout}} \in [0.0, 1.0]$).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readout_error: Option<f64>,
    /// Readout pulse duration (e.g. in nanoseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readout_duration: Option<f64>,
    /// Qubit operational frequency (e.g. in GHz).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<f64>,
    /// Provenance metadata for this qubit's calibration metrics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl QubitCalibration {
    /// Creates an empty calibration record with all metrics explicitly unknown.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates that all defined values are physically meaningful.
    pub fn validate(&self, qubit: PhysicalQubitId) -> Result<(), TargetError> {
        let q_str = format!("{qubit}");

        if let Some(t1) = self.t1 {
            if !t1.is_finite() || t1 < 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: t1,
                    context: format!("T1 on {q_str}"),
                });
            }
        }
        if let Some(t2) = self.t2 {
            if !t2.is_finite() || t2 < 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: t2,
                    context: format!("T2 on {q_str}"),
                });
            }
        }
        if let Some(err) = self.readout_error {
            if !err.is_finite() || !(0.0..=1.0).contains(&err) {
                return Err(TargetError::InvalidErrorEstimate {
                    value: err,
                    context: format!("readout error on {q_str}"),
                });
            }
        }
        if let Some(dur) = self.readout_duration {
            if !dur.is_finite() || dur < 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: dur,
                    context: format!("readout duration on {q_str}"),
                });
            }
        }
        if let Some(freq) = self.frequency {
            if !freq.is_finite() || freq <= 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: freq,
                    context: format!("frequency on {q_str}"),
                });
            }
        }
        Ok(())
    }
}

/// Operation-specific and operand-tuple-specific calibration metrics.
///
/// Characterizes gate duration and error rate for a designated physical operand tuple.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TupleCalibration {
    /// Execution duration (e.g. in nanoseconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Gate infidelity estimate ($1 - \text{fidelity} \in [0.0, 1.0]$).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_estimate: Option<f64>,
    /// Provenance metadata for this tuple calibration record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl TupleCalibration {
    /// Creates an empty tuple calibration record.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets duration.
    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Sets error estimate.
    pub fn with_error(mut self, error_estimate: f64) -> Self {
        self.error_estimate = Some(error_estimate);
        self
    }

    /// Sets provenance.
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Validates metric values.
    pub fn validate(&self, context: &str) -> Result<(), TargetError> {
        if let Some(err) = self.error_estimate {
            if !err.is_finite() || !(0.0..=1.0).contains(&err) {
                return Err(TargetError::InvalidErrorEstimate {
                    value: err,
                    context: context.to_string(),
                });
            }
        }
        if let Some(dur) = self.duration {
            if !dur.is_finite() || dur < 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: dur,
                    context: context.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Calibration entry for a single physical qubit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QubitCalibrationEntry {
    /// Physical qubit.
    pub qubit: PhysicalQubitId,
    /// Physical characterization metrics.
    #[serde(flatten)]
    pub calibration: QubitCalibration,
}

/// Calibration entry for a specific target operation and operand tuple.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TupleCalibrationEntry {
    /// Target operation identifier.
    pub operation: TargetOpId,
    /// Physical operand tuple.
    pub operands: Vec<PhysicalQubitId>,
    /// Tuple-specific characterization metrics.
    #[serde(flatten)]
    pub calibration: TupleCalibration,
}

/// Complete calibration and timing database for a target device.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetCalibration {
    /// Per-qubit physical characterization data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qubits: Vec<QubitCalibrationEntry>,
    /// Per-operation, per-physical-operand-tuple characterization data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tuples: Vec<TupleCalibrationEntry>,
}

impl TargetCalibration {
    /// Creates an empty calibration database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or updates calibration data for a physical qubit.
    pub fn set_qubit(&mut self, qubit: PhysicalQubitId, cal: QubitCalibration) {
        if let Some(entry) = self.qubits.iter_mut().find(|e| e.qubit == qubit) {
            entry.calibration = cal;
        } else {
            self.qubits.push(QubitCalibrationEntry {
                qubit,
                calibration: cal,
            });
            self.qubits.sort_by_key(|e| e.qubit);
        }
    }

    /// Adds or updates calibration data for an operation on a specific operand tuple.
    pub fn set_tuple(
        &mut self,
        op: impl Into<TargetOpId>,
        operands: impl IntoIterator<Item = PhysicalQubitId>,
        cal: TupleCalibration,
    ) {
        let op_id = op.into();
        let ops: Vec<PhysicalQubitId> = operands.into_iter().collect();
        if let Some(entry) = self
            .tuples
            .iter_mut()
            .find(|e| e.operation == op_id && e.operands == ops)
        {
            entry.calibration = cal;
        } else {
            self.tuples.push(TupleCalibrationEntry {
                operation: op_id,
                operands: ops,
                calibration: cal,
            });
            self.tuples
                .sort_by(|a, b| (&a.operation, &a.operands).cmp(&(&b.operation, &b.operands)));
        }
    }

    /// Retrieves the calibration record for a physical qubit, if present.
    pub fn get_qubit(&self, qubit: PhysicalQubitId) -> Option<&QubitCalibration> {
        self.qubits
            .iter()
            .find(|e| e.qubit == qubit)
            .map(|e| &e.calibration)
    }

    /// Retrieves the calibration record for a specific operation and operand tuple.
    pub fn get_tuple(
        &self,
        op: &TargetOpId,
        operands: &[PhysicalQubitId],
    ) -> Option<&TupleCalibration> {
        self.tuples
            .iter()
            .find(|e| &e.operation == op && e.operands.as_slice() == operands)
            .map(|e| &e.calibration)
    }

    /// Returns the duration for an operation on a physical operand tuple.
    ///
    /// Following the Explicit Unknown Principle, returns `None` if unknown.
    pub fn tuple_duration(&self, op: &TargetOpId, operands: &[PhysicalQubitId]) -> Option<f64> {
        self.get_tuple(op, operands).and_then(|t| t.duration)
    }

    /// Returns the error estimate for an operation on a physical operand tuple.
    ///
    /// Following the Explicit Unknown Principle, returns `None` if unknown.
    pub fn tuple_error(&self, op: &TargetOpId, operands: &[PhysicalQubitId]) -> Option<f64> {
        self.get_tuple(op, operands).and_then(|t| t.error_estimate)
    }

    /// Validates all calibration records against the target's physical capacity and declared operations.
    pub fn validate(
        &self,
        target_name: &str,
        capacity: usize,
        declared_ops: &BTreeSet<TargetOpId>,
    ) -> Result<(), TargetError> {
        for entry in &self.qubits {
            if entry.qubit.index() >= capacity {
                return Err(TargetError::QubitOutOfBounds {
                    qubit: entry.qubit,
                    target: target_name.to_string(),
                    capacity,
                });
            }
            entry.calibration.validate(entry.qubit)?;
        }

        for entry in &self.tuples {
            if !declared_ops.contains(&entry.operation) {
                return Err(TargetError::UnknownOperation {
                    operation: entry.operation.as_str().to_string(),
                    target: target_name.to_string(),
                });
            }
            for &q in &entry.operands {
                if q.index() >= capacity {
                    return Err(TargetError::QubitOutOfBounds {
                        qubit: q,
                        target: target_name.to_string(),
                        capacity,
                    });
                }
            }
            entry.calibration.validate(&format!(
                "operation '{}' on operands {:?}",
                entry.operation, entry.operands
            ))?;
        }

        Ok(())
    }
}
