//! Hardware timing, duration representations, and measurement/reset models.
//!
//! Enforces the Explicit Unknown Principle: unmeasured or uncalibrated durations are
//! represented strictly as [`OperationDuration::Unknown`], **never assumed or defaulted to zero**.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::target::error::TargetError;
use crate::target::operation::{PhysicalQubitId, TargetOpId};

/// Standard time and duration units for target hardware descriptions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DurationUnit {
    /// Picoseconds ($10^{-12}$ s or $10^{-3}$ ns).
    Picoseconds,
    /// Nanoseconds ($10^{-9}$ s or $1.0$ ns baseline).
    Nanoseconds,
    /// Microseconds ($10^{-6}$ s or $10^3$ ns).
    Microseconds,
    /// Milliseconds ($10^{-3}$ s or $10^6$ ns).
    Milliseconds,
    /// Seconds ($1.0$ s or $10^9$ ns).
    Seconds,
    /// Hardware clock cycles / sample steps (requires target `clock_period_ns`).
    Cycles,
}

impl DurationUnit {
    /// Returns the conversion factor to nanoseconds for absolute time units.
    ///
    /// For [`DurationUnit::Cycles`], returns `None` as conversion depends on hardware clock rate.
    #[inline]
    pub const fn conversion_to_ns_factor(&self) -> Option<f64> {
        match self {
            Self::Picoseconds => Some(0.001),
            Self::Nanoseconds => Some(1.0),
            Self::Microseconds => Some(1_000.0),
            Self::Milliseconds => Some(1_000_000.0),
            Self::Seconds => Some(1_000_000_000.0),
            Self::Cycles => None,
        }
    }

    /// Converts a value in this unit to nanoseconds given an optional target clock cycle period.
    pub fn as_nanoseconds(
        &self,
        value: f64,
        clock_period_ns: Option<f64>,
    ) -> Result<f64, TargetError> {
        if !value.is_finite() || value < 0.0 {
            return Err(TargetError::InvalidDuration {
                value,
                context: format!("duration in unit '{self}'"),
            });
        }

        let res = match self {
            Self::Cycles => {
                let Some(period) = clock_period_ns else {
                    return Err(TargetError::InvalidDurationUnit {
                        unit: "Cycles".to_string(),
                        context: "target clock period is not configured".to_string(),
                    });
                };
                if !period.is_finite() || period <= 0.0 {
                    return Err(TargetError::InconsistentTiming {
                        context: "clock_period_ns".to_string(),
                        reason: format!("clock period must be positive and finite, got {period}"),
                    });
                }
                value * period
            }
            _ => {
                let factor = self.conversion_to_ns_factor().unwrap();
                value * factor
            }
        };

        if !res.is_finite() {
            return Err(TargetError::InvalidDuration {
                value: res,
                context: format!("duration overflow converting in unit '{self}' to nanoseconds"),
            });
        }
        let res = if res == 0.0 { 0.0 } else { res };
        Ok(res)
    }

    /// Parses a duration unit from a string slice (e.g. `"ns"`, `"us"`, `"µs"`, `"cycles"`).
    pub fn from_str_unit(s: &str) -> Result<Self, TargetError> {
        match s.trim().to_lowercase().as_str() {
            "ps" | "pico" | "picoseconds" => Ok(Self::Picoseconds),
            "ns" | "nano" | "nanoseconds" => Ok(Self::Nanoseconds),
            "us" | "µs" | "micro" | "microseconds" => Ok(Self::Microseconds),
            "ms" | "milli" | "milliseconds" => Ok(Self::Milliseconds),
            "s" | "sec" | "second" | "seconds" => Ok(Self::Seconds),
            "cycle" | "cycles" | "dt" | "sample" | "samples" => Ok(Self::Cycles),
            _ => Err(TargetError::InvalidDurationUnit {
                unit: s.to_string(),
                context: "unrecognized duration unit identifier".to_string(),
            }),
        }
    }
}

impl fmt::Display for DurationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Picoseconds => write!(f, "ps"),
            Self::Nanoseconds => write!(f, "ns"),
            Self::Microseconds => write!(f, "µs"),
            Self::Milliseconds => write!(f, "ms"),
            Self::Seconds => write!(f, "s"),
            Self::Cycles => write!(f, "cycles"),
        }
    }
}

/// Concrete hardware duration value paired with an explicit physical unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HardwareDuration {
    /// Numerical duration value ($\ge 0.0$).
    pub value: f64,
    /// Physical or clock unit.
    pub unit: DurationUnit,
}

impl HardwareDuration {
    /// Constructs a duration in picoseconds.
    #[inline]
    pub const fn picoseconds(value: f64) -> Self {
        Self {
            value,
            unit: DurationUnit::Picoseconds,
        }
    }

    /// Constructs a duration in nanoseconds.
    #[inline]
    pub const fn nanoseconds(value: f64) -> Self {
        Self {
            value,
            unit: DurationUnit::Nanoseconds,
        }
    }

    /// Constructs a duration in microseconds.
    #[inline]
    pub const fn microseconds(value: f64) -> Self {
        Self {
            value,
            unit: DurationUnit::Microseconds,
        }
    }

    /// Constructs a duration in milliseconds.
    #[inline]
    pub const fn milliseconds(value: f64) -> Self {
        Self {
            value,
            unit: DurationUnit::Milliseconds,
        }
    }

    /// Constructs a duration in seconds.
    #[inline]
    pub const fn seconds(value: f64) -> Self {
        Self {
            value,
            unit: DurationUnit::Seconds,
        }
    }

    /// Constructs a duration in clock cycles or sample steps.
    #[inline]
    pub const fn cycles(cycles: f64) -> Self {
        Self {
            value: cycles,
            unit: DurationUnit::Cycles,
        }
    }

    /// Converts this duration to nanoseconds given optional clock period.
    #[inline]
    pub fn to_nanoseconds(&self, clock_period_ns: Option<f64>) -> Result<f64, TargetError> {
        self.unit.as_nanoseconds(self.value, clock_period_ns)
    }

    /// Validates that the duration value is non-negative and finite.
    pub fn validate(&self, context: &str, clock_period_ns: Option<f64>) -> Result<(), TargetError> {
        if !self.value.is_finite() || self.value < 0.0 {
            return Err(TargetError::InvalidDuration {
                value: self.value,
                context: format!("{context} ({})", self.unit),
            });
        }
        if self.unit == DurationUnit::Cycles {
            if let Some(period) = clock_period_ns {
                if !period.is_finite() || period <= 0.0 {
                    return Err(TargetError::InconsistentTiming {
                        context: format!("{context} (Cycles)"),
                        reason: format!("clock period must be positive and finite, got {period}"),
                    });
                }
            } else {
                return Err(TargetError::InvalidDurationUnit {
                    unit: "Cycles".to_string(),
                    context: format!("{context} requires configured clock_period_ns"),
                });
            }
        }
        Ok(())
    }
}

impl fmt::Display for HardwareDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.unit)
    }
}

/// Operation duration representation distinguishing known, estimated, and unknown values.
///
/// **Non-Negotiable Invariant**: [`OperationDuration::Unknown`] must **never** be interpreted or
/// defaulted to zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OperationDuration {
    /// Empirically benchmarked / calibrated duration with provenance.
    Known(HardwareDuration),
    /// Nominal datasheet or model-derived estimated duration.
    Estimated(HardwareDuration),
    /// Unmeasured or uncalibrated duration.
    ///
    /// Following the Explicit Unknown Principle, missing timing data is represented
    /// as unknown, never assumed to be zero.
    Unknown,
}

impl OperationDuration {
    /// Creates a known duration in nanoseconds.
    #[inline]
    pub const fn known_ns(ns: f64) -> Self {
        Self::Known(HardwareDuration::nanoseconds(ns))
    }

    /// Creates an estimated duration in nanoseconds.
    #[inline]
    pub const fn estimated_ns(ns: f64) -> Self {
        Self::Estimated(HardwareDuration::nanoseconds(ns))
    }

    /// Returns `true` if this duration is empirically known.
    #[inline]
    pub const fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }

    /// Returns `true` if this duration is a model estimate.
    #[inline]
    pub const fn is_estimated(&self) -> bool {
        matches!(self, Self::Estimated(_))
    }

    /// Returns `true` if this duration is unknown.
    #[inline]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Returns the underlying hardware duration, if present.
    #[inline]
    pub fn hardware_duration(&self) -> Option<&HardwareDuration> {
        match self {
            Self::Known(d) | Self::Estimated(d) => Some(d),
            Self::Unknown => None,
        }
    }

    /// Converts this duration to nanoseconds, returning `None` if unknown.
    ///
    /// # Errors
    /// Returns [`TargetError`] if cycle conversion is requested without a valid clock period
    /// or if numerical values are invalid.
    pub fn to_nanoseconds(&self, clock_period_ns: Option<f64>) -> Result<Option<f64>, TargetError> {
        match self {
            Self::Known(d) | Self::Estimated(d) => d.to_nanoseconds(clock_period_ns).map(Some),
            Self::Unknown => Ok(None),
        }
    }

    /// Returns the exact nanosecond duration if empirically known, returning `None` if estimated or unknown.
    pub fn exact_nanoseconds(
        &self,
        clock_period_ns: Option<f64>,
    ) -> Result<Option<f64>, TargetError> {
        match self {
            Self::Known(d) => d.to_nanoseconds(clock_period_ns).map(Some),
            Self::Estimated(_) | Self::Unknown => Ok(None),
        }
    }

    /// Validates internal consistency and physical bounds.
    pub fn validate(&self, context: &str, clock_period_ns: Option<f64>) -> Result<(), TargetError> {
        match self {
            Self::Known(d) => d.validate(&format!("{context} (known)"), clock_period_ns),
            Self::Estimated(d) => d.validate(&format!("{context} (estimated)"), clock_period_ns),
            Self::Unknown => Ok(()),
        }
    }
}

impl fmt::Display for OperationDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(d) => write!(f, "{d} (calibrated)"),
            Self::Estimated(d) => write!(f, "{d} (estimated)"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Readout / measurement timing profile for a physical qubit or readout line.
///
/// Physical readout typically involves distinct pulse, integration, and depletion phases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadoutTiming {
    /// Readout drive / stimulus pulse duration.
    pub pulse_duration: OperationDuration,
    /// Signal acquisition / integration window duration.
    pub acquisition_duration: OperationDuration,
    /// Optional resonator depletion / ring-down duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depletion_duration: Option<OperationDuration>,
    /// Total measurement duration from trigger to channel availability.
    pub total_duration: OperationDuration,
}

impl ReadoutTiming {
    /// Creates a readout timing profile with pulse, acquisition, and total durations.
    pub fn new(
        pulse_duration: OperationDuration,
        acquisition_duration: OperationDuration,
        total_duration: OperationDuration,
    ) -> Self {
        Self {
            pulse_duration,
            acquisition_duration,
            depletion_duration: None,
            total_duration,
        }
    }

    /// Sets the resonator depletion / ring-down duration.
    pub fn with_depletion(mut self, depletion_duration: OperationDuration) -> Self {
        self.depletion_duration = Some(depletion_duration);
        self
    }

    /// Validates the readout timing specification.
    pub fn validate(
        &self,
        qubit: PhysicalQubitId,
        clock_period_ns: Option<f64>,
    ) -> Result<(), TargetError> {
        let q_ctx = format!("qubit {qubit} readout");
        self.pulse_duration
            .validate(&format!("{q_ctx} pulse"), clock_period_ns)?;
        self.acquisition_duration
            .validate(&format!("{q_ctx} acquisition"), clock_period_ns)?;
        self.total_duration
            .validate(&format!("{q_ctx} total"), clock_period_ns)?;

        if let Some(depl) = &self.depletion_duration {
            depl.validate(&format!("{q_ctx} depletion"), clock_period_ns)?;
        }

        // Cross-check durations if both pulse and total are known in nanoseconds
        if let (Some(p_ns), Some(t_ns)) = (
            self.pulse_duration.to_nanoseconds(clock_period_ns)?,
            self.total_duration.to_nanoseconds(clock_period_ns)?,
        ) {
            if p_ns > t_ns {
                return Err(TargetError::InconsistentTiming {
                    context: q_ctx,
                    reason: format!(
                        "readout pulse duration ({p_ns:.2} ns) exceeds total measurement duration ({t_ns:.2} ns)"
                    ),
                });
            }
        }

        Ok(())
    }
}

/// Reset timing profile for a physical qubit.
///
/// Models active reset pulse duration and post-reset thermalization / settling delays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResetTiming {
    /// Active reset drive or measurement-feedback cycle duration.
    pub pulse_duration: OperationDuration,
    /// Settling or relaxation duration prior to subsequent gate execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settling_duration: Option<OperationDuration>,
    /// Total reset duration.
    pub total_duration: OperationDuration,
}

impl ResetTiming {
    /// Creates a reset timing profile with pulse and total durations.
    pub fn new(pulse_duration: OperationDuration, total_duration: OperationDuration) -> Self {
        Self {
            pulse_duration,
            settling_duration: None,
            total_duration,
        }
    }

    /// Sets the settling / relaxation delay.
    pub fn with_settling(mut self, settling_duration: OperationDuration) -> Self {
        self.settling_duration = Some(settling_duration);
        self
    }

    /// Validates the reset timing specification.
    pub fn validate(
        &self,
        qubit: PhysicalQubitId,
        clock_period_ns: Option<f64>,
    ) -> Result<(), TargetError> {
        let q_ctx = format!("qubit {qubit} reset");
        self.pulse_duration
            .validate(&format!("{q_ctx} pulse"), clock_period_ns)?;
        self.total_duration
            .validate(&format!("{q_ctx} total"), clock_period_ns)?;

        if let Some(settling) = &self.settling_duration {
            settling.validate(&format!("{q_ctx} settling"), clock_period_ns)?;
        }

        if let (Some(p_ns), Some(t_ns)) = (
            self.pulse_duration.to_nanoseconds(clock_period_ns)?,
            self.total_duration.to_nanoseconds(clock_period_ns)?,
        ) {
            if p_ns > t_ns {
                return Err(TargetError::InconsistentTiming {
                    context: q_ctx,
                    reason: format!(
                        "reset pulse duration ({p_ns:.2} ns) exceeds total reset duration ({t_ns:.2} ns)"
                    ),
                });
            }
        }

        Ok(())
    }
}

/// Readout timing entry for a specific physical qubit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadoutTimingEntry {
    /// Physical qubit identifier.
    pub qubit: PhysicalQubitId,
    /// Readout timing characteristics.
    #[serde(flatten)]
    pub timing: ReadoutTiming,
}

/// Reset timing entry for a specific physical qubit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResetTimingEntry {
    /// Physical qubit identifier.
    pub qubit: PhysicalQubitId,
    /// Reset timing characteristics.
    #[serde(flatten)]
    pub timing: ResetTiming,
}

/// Duration entry for a specific target operation and operand tuple.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TupleDurationEntry {
    /// Target operation identifier.
    pub operation: TargetOpId,
    /// Physical operand tuple.
    pub operands: Vec<PhysicalQubitId>,
    /// Tuple-specific operation duration.
    pub duration: OperationDuration,
}

/// Comprehensive hardware timing model describing operation durations and clock constraints.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimingModel {
    /// Clock cycle period in nanoseconds (sample step $dt$), if target uses discrete timing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_period_ns: Option<f64>,
    /// Default nominal operation durations per target operation mnemonic.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub default_durations: BTreeMap<TargetOpId, OperationDuration>,
    /// Tuple-specific operation durations (takes precedence over default durations).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tuple_durations: Vec<TupleDurationEntry>,
    /// Qubit-specific readout timing characteristics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readout_timings: Vec<ReadoutTimingEntry>,
    /// Qubit-specific reset timing characteristics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reset_timings: Vec<ResetTimingEntry>,
}

impl TimingModel {
    /// Creates an empty timing model with no default durations or clock period.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the discrete clock cycle period in nanoseconds.
    pub fn with_clock_period_ns(mut self, period_ns: f64) -> Self {
        self.clock_period_ns = Some(period_ns);
        self
    }

    /// Sets default duration for an operation mnemonic.
    pub fn set_default_duration(&mut self, op: impl Into<TargetOpId>, duration: OperationDuration) {
        self.default_durations.insert(op.into(), duration);
    }

    /// Sets tuple-specific duration for an operation and physical operand tuple.
    pub fn set_tuple_duration(
        &mut self,
        op: impl Into<TargetOpId>,
        operands: impl IntoIterator<Item = PhysicalQubitId>,
        duration: OperationDuration,
    ) {
        let op_id = op.into();
        let ops: Vec<PhysicalQubitId> = operands.into_iter().collect();
        if let Some(entry) = self
            .tuple_durations
            .iter_mut()
            .find(|e| e.operation == op_id && e.operands == ops)
        {
            entry.duration = duration;
        } else {
            self.tuple_durations.push(TupleDurationEntry {
                operation: op_id,
                operands: ops,
                duration,
            });
            self.tuple_durations
                .sort_by(|a, b| (&a.operation, &a.operands).cmp(&(&b.operation, &b.operands)));
        }
    }

    /// Sets readout timing for a physical qubit.
    pub fn set_readout_timing(&mut self, qubit: PhysicalQubitId, timing: ReadoutTiming) {
        if let Some(entry) = self.readout_timings.iter_mut().find(|e| e.qubit == qubit) {
            entry.timing = timing;
        } else {
            self.readout_timings
                .push(ReadoutTimingEntry { qubit, timing });
            self.readout_timings.sort_by_key(|e| e.qubit);
        }
    }

    /// Retrieves readout timing for a physical qubit.
    pub fn get_readout_timing(&self, qubit: PhysicalQubitId) -> Option<&ReadoutTiming> {
        self.readout_timings
            .iter()
            .find(|e| e.qubit == qubit)
            .map(|e| &e.timing)
    }

    /// Sets reset timing for a physical qubit.
    pub fn set_reset_timing(&mut self, qubit: PhysicalQubitId, timing: ResetTiming) {
        if let Some(entry) = self.reset_timings.iter_mut().find(|e| e.qubit == qubit) {
            entry.timing = timing;
        } else {
            self.reset_timings.push(ResetTimingEntry { qubit, timing });
            self.reset_timings.sort_by_key(|e| e.qubit);
        }
    }

    /// Retrieves reset timing for a physical qubit.
    pub fn get_reset_timing(&self, qubit: PhysicalQubitId) -> Option<&ResetTiming> {
        self.reset_timings
            .iter()
            .find(|e| e.qubit == qubit)
            .map(|e| &e.timing)
    }

    /// Resolves the operation duration for an operation on a physical operand tuple.
    ///
    /// Following the Explicit Unknown Principle, returns [`OperationDuration::Unknown`] if uncalibrated,
    /// **never zero**.
    pub fn duration_of(&self, op: &TargetOpId, operands: &[PhysicalQubitId]) -> OperationDuration {
        if let Some(entry) = self
            .tuple_durations
            .iter()
            .find(|e| &e.operation == op && e.operands.as_slice() == operands)
        {
            return entry.duration.clone();
        }

        if let Some(d) = self.default_durations.get(op) {
            return d.clone();
        }

        OperationDuration::Unknown
    }

    /// Resolves the operation duration in nanoseconds for an operation on a physical operand tuple.
    ///
    /// Returns `Ok(Some(ns))` if known/estimated, `Ok(None)` if unknown, or `Err` if units are invalid.
    pub fn duration_ns(
        &self,
        op: &TargetOpId,
        operands: &[PhysicalQubitId],
    ) -> Result<Option<f64>, TargetError> {
        self.duration_of(op, operands)
            .to_nanoseconds(self.clock_period_ns)
    }

    /// Validates the timing model against physical qubit capacity and declared operations.
    pub fn validate(
        &self,
        target_capacity: usize,
        declared_ops: &BTreeSet<TargetOpId>,
    ) -> Result<(), TargetError> {
        if let Some(period) = self.clock_period_ns {
            if !period.is_finite() || period <= 0.0 {
                return Err(TargetError::InconsistentTiming {
                    context: "TimingModel.clock_period_ns".to_string(),
                    reason: format!("clock period must be positive and finite, got {period}"),
                });
            }
        }

        for (op_id, dur) in &self.default_durations {
            if !declared_ops.contains(op_id) {
                return Err(TargetError::UnknownOperation {
                    operation: op_id.as_str().to_string(),
                    target: "TimingModel".to_string(),
                });
            }
            dur.validate(
                &format!("default duration for '{op_id}'"),
                self.clock_period_ns,
            )?;
        }

        for entry in &self.tuple_durations {
            if !declared_ops.contains(&entry.operation) {
                return Err(TargetError::UnknownOperation {
                    operation: entry.operation.as_str().to_string(),
                    target: "TimingModel".to_string(),
                });
            }
            for &q in &entry.operands {
                if q.index() >= target_capacity {
                    return Err(TargetError::QubitOutOfBounds {
                        qubit: q,
                        target: "TimingModel".to_string(),
                        capacity: target_capacity,
                    });
                }
            }
            entry.duration.validate(
                &format!(
                    "tuple duration for '{}' on {:?}",
                    entry.operation, entry.operands
                ),
                self.clock_period_ns,
            )?;
        }

        for entry in &self.readout_timings {
            if entry.qubit.index() >= target_capacity {
                return Err(TargetError::QubitOutOfBounds {
                    qubit: entry.qubit,
                    target: "TimingModel".to_string(),
                    capacity: target_capacity,
                });
            }
            entry.timing.validate(entry.qubit, self.clock_period_ns)?;
        }

        for entry in &self.reset_timings {
            if entry.qubit.index() >= target_capacity {
                return Err(TargetError::QubitOutOfBounds {
                    qubit: entry.qubit,
                    target: "TimingModel".to_string(),
                    capacity: target_capacity,
                });
            }
            entry.timing.validate(entry.qubit, self.clock_period_ns)?;
        }

        Ok(())
    }
}
