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
    /// Optional expiration timestamp string after which this calibration is considered stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
}

impl Provenance {
    /// Creates a new provenance record with the specified source name.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            timestamp: None,
            method: None,
            valid_until: None,
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

    /// Sets the valid-until expiration timestamp string.
    pub fn with_valid_until(mut self, valid_until: impl Into<String>) -> Self {
        self.valid_until = Some(valid_until.into());
        self
    }

    /// Validates whether this calibration provenance record is fresh relative to `reference_timestamp`.
    ///
    /// # Errors
    /// - Returns [`TargetError::InvalidTimestamp`] if either timestamp is malformed or if
    ///   the calibration timestamp is significantly in the future relative to the reference.
    /// - Returns [`TargetError::StaleCalibration`] if the age exceeds `max_age_seconds` or if
    ///   the reference timestamp is past `valid_until`.
    pub fn validate_freshness(
        &self,
        reference_timestamp: &str,
        max_age_seconds: u64,
        context: &str,
    ) -> Result<(), TargetError> {
        let ref_epoch = parse_timestamp_to_epoch_seconds(reference_timestamp)?;

        if let (Some(ts), Some(valid_until_str)) = (&self.timestamp, &self.valid_until) {
            let cal_epoch = parse_timestamp_to_epoch_seconds(ts)?;
            let exp_epoch = parse_timestamp_to_epoch_seconds(valid_until_str)?;
            if exp_epoch < cal_epoch {
                return Err(TargetError::InvalidTimestamp {
                    timestamp: valid_until_str.clone(),
                    reason: format!(
                        "valid_until expiration timestamp ('{valid_until_str}') precedes calibration timestamp ('{ts}')"
                    ),
                });
            }
        }

        if let Some(valid_until_str) = &self.valid_until {
            let exp_epoch = parse_timestamp_to_epoch_seconds(valid_until_str)?;
            if ref_epoch > exp_epoch {
                let age = (ref_epoch - exp_epoch) as u64;
                return Err(TargetError::StaleCalibration {
                    context: context.to_string(),
                    timestamp: valid_until_str.clone(),
                    reference: reference_timestamp.to_string(),
                    age_seconds: age,
                    max_age_seconds,
                });
            }
        }

        if let Some(ts) = &self.timestamp {
            let cal_epoch = parse_timestamp_to_epoch_seconds(ts)?;
            if ref_epoch + 60 < cal_epoch {
                return Err(TargetError::InvalidTimestamp {
                    timestamp: ts.clone(),
                    reason: format!(
                        "calibration timestamp is in the future relative to reference '{reference_timestamp}'"
                    ),
                });
            }

            let age = if ref_epoch > cal_epoch {
                (ref_epoch - cal_epoch) as u64
            } else {
                0
            };

            if age > max_age_seconds {
                return Err(TargetError::StaleCalibration {
                    context: context.to_string(),
                    timestamp: ts.clone(),
                    reference: reference_timestamp.to_string(),
                    age_seconds: age,
                    max_age_seconds,
                });
            }
        }

        Ok(())
    }

    /// Checks whether this record is stale without throwing an error for age expiration.
    pub fn is_stale(
        &self,
        reference_timestamp: &str,
        max_age_seconds: u64,
    ) -> Result<bool, TargetError> {
        match self.validate_freshness(reference_timestamp, max_age_seconds, "staleness_check") {
            Ok(()) => Ok(false),
            Err(TargetError::StaleCalibration { .. }) => Ok(true),
            Err(e) => Err(e),
        }
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

    /// Validates the freshness of all calibration entries relative to a reference timestamp.
    ///
    /// # Errors
    /// Returns [`TargetError::StaleCalibration`] or [`TargetError::InvalidTimestamp`] if any
    /// qubit or tuple calibration record exceeds `max_age_seconds` or has an expired `valid_until`.
    pub fn validate_freshness(
        &self,
        reference_timestamp: &str,
        max_age_seconds: u64,
    ) -> Result<(), TargetError> {
        for entry in &self.qubits {
            if let Some(prov) = &entry.calibration.provenance {
                prov.validate_freshness(
                    reference_timestamp,
                    max_age_seconds,
                    &format!("qubit {}", entry.qubit),
                )?;
            }
        }
        for entry in &self.tuples {
            if let Some(prov) = &entry.calibration.provenance {
                prov.validate_freshness(
                    reference_timestamp,
                    max_age_seconds,
                    &format!("operation '{}' on {:?}", entry.operation, entry.operands),
                )?;
            }
        }
        Ok(())
    }
}

/// Converts a calendar date to days relative to Unix epoch (1970-01-01) using Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let m = if month <= 2 { month + 9 } else { month - 3 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + (doe as i64) - 719468
}

/// Parses an ISO 8601, RFC 3339, or integer timestamp string into UTC epoch seconds.
///
/// Supports:
/// - ISO 8601 with UTC: `"2026-11-04T05:00:00Z"`, `"2026-11-04T05:00:00.123Z"`
/// - ISO 8601 with offset: `"2026-11-04T05:00:00+02:00"`, `"2026-11-04T05:00:00-05:00"`
/// - ISO 8601 date-only: `"2026-11-04"` (interpreted as 00:00:00 UTC)
/// - Raw integer Unix epoch seconds: `"1793768400"`
pub fn parse_timestamp_to_epoch_seconds(ts: &str) -> Result<i64, TargetError> {
    let trimmed = ts.trim();
    if trimmed.is_empty() {
        return Err(TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: "timestamp cannot be empty".to_string(),
        });
    }

    // Try parsing as integer epoch seconds first
    if let Ok(epoch) = trimmed.parse::<i64>() {
        return Ok(epoch);
    }

    let (date_part, rest) = if let Some(idx) = trimmed.find(['T', 't', ' ']) {
        (&trimmed[..idx], &trimmed[idx + 1..])
    } else {
        (trimmed, "")
    };

    let date_parts: Vec<&str> = date_part.split('-').collect();
    if date_parts.len() != 3 {
        return Err(TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: "expected date in format YYYY-MM-DD".to_string(),
        });
    }

    let year = date_parts[0]
        .parse::<i64>()
        .map_err(|_| TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: "invalid year".to_string(),
        })?;
    let month = date_parts[1]
        .parse::<u32>()
        .map_err(|_| TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: "invalid month".to_string(),
        })?;
    let day = date_parts[2]
        .parse::<u32>()
        .map_err(|_| TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: "invalid day".to_string(),
        })?;

    if month == 0 || month > 12 {
        return Err(TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: format!("month {month} must be in range 1..=12"),
        });
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let max_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => unreachable!(),
    };
    if day == 0 || day > max_days {
        return Err(TargetError::InvalidTimestamp {
            timestamp: ts.to_string(),
            reason: format!("day {day} invalid for month {month}"),
        });
    }

    let mut hour = 0u32;
    let mut min = 0u32;
    let mut sec = 0u32;
    let mut tz_offset_seconds = 0i64;

    if !rest.is_empty() {
        let mut time_str = rest;
        if let Some(idx) = time_str.find(['Z', 'z']) {
            time_str = &time_str[..idx];
        } else if let Some(idx) = time_str.rfind(['+', '-']) {
            let offset_str = &time_str[idx..];
            time_str = &time_str[..idx];
            let sign = if offset_str.starts_with('+') { 1 } else { -1 };
            let offset_body = &offset_str[1..];
            let (oh, om) = if let Some((h, m)) = offset_body.split_once(':') {
                (h.parse::<i64>(), m.parse::<i64>())
            } else if offset_body.len() == 4 {
                (
                    offset_body[..2].parse::<i64>(),
                    offset_body[2..].parse::<i64>(),
                )
            } else if offset_body.len() == 2 {
                (offset_body.parse::<i64>(), Ok(0))
            } else {
                return Err(TargetError::InvalidTimestamp {
                    timestamp: ts.to_string(),
                    reason: "invalid timezone offset".to_string(),
                });
            };
            match (oh, om) {
                (Ok(h), Ok(m)) if (0..=23).contains(&h) && (0..=59).contains(&m) => {
                    tz_offset_seconds = sign * (h * 3600 + m * 60);
                }
                _ => {
                    return Err(TargetError::InvalidTimestamp {
                        timestamp: ts.to_string(),
                        reason: "invalid timezone offset values".to_string(),
                    });
                }
            }
        }

        if let Some((main_time, _frac)) = time_str.split_once('.') {
            time_str = main_time;
        }

        let time_parts: Vec<&str> = time_str.split(':').collect();
        if time_parts.len() >= 2 {
            hour = time_parts[0]
                .parse::<u32>()
                .map_err(|_| TargetError::InvalidTimestamp {
                    timestamp: ts.to_string(),
                    reason: "invalid hour".to_string(),
                })?;
            min = time_parts[1]
                .parse::<u32>()
                .map_err(|_| TargetError::InvalidTimestamp {
                    timestamp: ts.to_string(),
                    reason: "invalid minute".to_string(),
                })?;
            if time_parts.len() >= 3 {
                sec = time_parts[2]
                    .parse::<u32>()
                    .map_err(|_| TargetError::InvalidTimestamp {
                        timestamp: ts.to_string(),
                        reason: "invalid second".to_string(),
                    })?;
            }
        } else {
            return Err(TargetError::InvalidTimestamp {
                timestamp: ts.to_string(),
                reason: "expected time in format HH:MM or HH:MM:SS".to_string(),
            });
        }

        if hour > 23 || min > 59 || sec > 60 {
            return Err(TargetError::InvalidTimestamp {
                timestamp: ts.to_string(),
                reason: format!("time values out of range: {hour:02}:{min:02}:{sec:02}"),
            });
        }
    }

    let days = days_from_civil(year, month, day);
    let seconds_in_day = (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);
    let epoch_utc = days * 86400 + seconds_in_day - tz_offset_seconds;
    Ok(epoch_utc)
}
