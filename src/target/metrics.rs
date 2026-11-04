//! Metric representations, availability classifications, and cost bounds.
//!
//! Enforces the Explicit Unknown Principle by strictly distinguishing between
//! measured empirical metrics, estimated nominal metrics, and absent metrics.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::target::calibration::Provenance;

/// Qualitative origin of a hardware metric value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MetricKind {
    /// Empirically benchmarked on hardware via experimental characterization.
    Measured,
    /// Estimated from data-sheet, simulation, or nominal design specifications.
    Estimated,
}

impl fmt::Display for MetricKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured => write!(f, "Measured"),
            Self::Estimated => write!(f, "Estimated"),
        }
    }
}

/// Tri-state hardware metric value preserving certainty and provenance.
///
/// Prevents collapsing missing or nominal values into fabricated empirical numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MetricValue<T> {
    /// Empirically measured hardware metric with explicit provenance.
    Measured {
        /// Measured value.
        value: T,
        /// Experimental characterization provenance.
        provenance: Provenance,
    },
    /// Model-based or data-sheet estimate.
    Estimated {
        /// Estimated value.
        value: T,
        /// Optional specification provenance.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provenance: Option<Provenance>,
    },
    /// Uncalibrated or missing value.
    ///
    /// Following the Explicit Unknown Principle, absent data is never substituted with zero.
    Absent,
}

impl<T> MetricValue<T> {
    /// Constructs a measured metric value with provenance.
    pub fn measured(value: T, provenance: Provenance) -> Self {
        Self::Measured { value, provenance }
    }

    /// Constructs an estimated metric value with optional provenance.
    pub fn estimated(value: T, provenance: Option<Provenance>) -> Self {
        Self::Estimated { value, provenance }
    }

    /// Constructs an absent metric value.
    pub fn absent() -> Self {
        Self::Absent
    }

    /// Returns `true` if this metric is completely absent / unknown.
    #[inline]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Returns `true` if this metric was empirically measured.
    #[inline]
    pub const fn is_measured(&self) -> bool {
        matches!(self, Self::Measured { .. })
    }

    /// Returns `true` if this metric is a model-based or nominal estimate.
    #[inline]
    pub const fn is_estimated(&self) -> bool {
        matches!(self, Self::Estimated { .. })
    }

    /// Returns a reference to the contained value, if present.
    #[inline]
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Measured { value, .. } | Self::Estimated { value, .. } => Some(value),
            Self::Absent => None,
        }
    }

    /// Consumes the metric and returns the contained value, if present.
    #[inline]
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Measured { value, .. } | Self::Estimated { value, .. } => Some(value),
            Self::Absent => None,
        }
    }

    /// Returns a reference to the metric's provenance metadata, if present.
    #[inline]
    pub fn provenance(&self) -> Option<&Provenance> {
        match self {
            Self::Measured { provenance, .. } => Some(provenance),
            Self::Estimated { provenance, .. } => provenance.as_ref(),
            Self::Absent => None,
        }
    }

    /// Returns the classification kind of this metric, if present.
    #[inline]
    pub fn kind(&self) -> Option<MetricKind> {
        match self {
            Self::Measured { .. } => Some(MetricKind::Measured),
            Self::Estimated { .. } => Some(MetricKind::Estimated),
            Self::Absent => None,
        }
    }

    /// Maps a `MetricValue<T>` to `MetricValue<U>` by applying a function to the contained value.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> MetricValue<U> {
        match self {
            Self::Measured { value, provenance } => MetricValue::Measured {
                value: f(value),
                provenance,
            },
            Self::Estimated { value, provenance } => MetricValue::Estimated {
                value: f(value),
                provenance,
            },
            Self::Absent => MetricValue::Absent,
        }
    }

    /// Returns the contained value or a provided fallback.
    #[inline]
    pub fn unwrap_or(self, default: T) -> T {
        self.into_value().unwrap_or(default)
    }
}

impl<T: fmt::Display> fmt::Display for MetricValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured { value, provenance } => {
                write!(f, "{value} (measured via {})", provenance.source)
            }
            Self::Estimated {
                value,
                provenance: Some(prov),
            } => {
                write!(f, "{value} (estimated from {})", prov.source)
            }
            Self::Estimated {
                value,
                provenance: None,
            } => {
                write!(f, "{value} (estimated)")
            }
            Self::Absent => write!(f, "unknown"),
        }
    }
}

/// Evaluation result for circuit duration cost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DurationCost {
    /// Exact duration calculated when all constituent operations have known timing.
    Exact {
        /// Total critical path duration in nanoseconds.
        total_ns: f64,
        /// Whether all constituent metrics were empirically measured (true) or include estimates (false).
        is_measured_only: bool,
    },
    /// Incomplete duration because some constituent operations lack timing metrics.
    Incomplete {
        /// Accumulated known duration for calibrated operations.
        accumulated_known_ns: f64,
        /// Number of operations lacking duration metrics.
        uncalibrated_operations: usize,
    },
    /// No timing metrics are available for any operation.
    Unknown,
}

impl DurationCost {
    /// Returns `true` if the duration is completely and exactly known.
    #[inline]
    pub const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact { .. })
    }

    /// Returns `true` if the duration is partially known with missing operations.
    #[inline]
    pub const fn is_incomplete(&self) -> bool {
        matches!(self, Self::Incomplete { .. })
    }

    /// Returns `true` if the duration is completely unknown.
    #[inline]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Returns the exact duration in nanoseconds if completely known.
    #[inline]
    pub fn exact_ns(&self) -> Option<f64> {
        match self {
            Self::Exact { total_ns, .. } => Some(*total_ns),
            _ => None,
        }
    }

    /// Returns the accumulated known duration in nanoseconds, if any.
    #[inline]
    pub fn accumulated_ns(&self) -> Option<f64> {
        match self {
            Self::Exact { total_ns, .. } => Some(*total_ns),
            Self::Incomplete {
                accumulated_known_ns,
                ..
            } => Some(*accumulated_known_ns),
            Self::Unknown => None,
        }
    }
}

impl fmt::Display for DurationCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact {
                total_ns,
                is_measured_only,
            } => {
                let tag = if *is_measured_only {
                    "measured"
                } else {
                    "estimated"
                };
                write!(f, "{total_ns:.2} ns ({tag})")
            }
            Self::Incomplete {
                accumulated_known_ns,
                uncalibrated_operations,
            } => {
                write!(
                    f,
                    ">={accumulated_known_ns:.2} ns (incomplete: {uncalibrated_operations} uncalibrated ops)"
                )
            }
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Evaluation result for circuit infidelity / error cost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ErrorCost {
    /// Exact error estimate calculated when all operations have known error estimates.
    Exact {
        /// Upper bound on circuit infidelity: $1 - \prod (1 - \epsilon_g) \approx \sum \epsilon_g$.
        infidelity_bound: f64,
        /// Whether all constituent metrics were empirically measured (true) or include estimates (false).
        is_measured_only: bool,
    },
    /// Incomplete error cost because some operations lack calibration.
    Incomplete {
        /// Accumulated infidelity from known calibrated operations.
        known_infidelity: f64,
        /// Number of operations lacking error estimates.
        uncalibrated_operations: usize,
    },
    /// No error estimates are available for any operation.
    Unknown,
}

impl ErrorCost {
    /// Returns `true` if the error cost is completely and exactly known.
    #[inline]
    pub const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact { .. })
    }

    /// Returns `true` if the error cost is partially known with missing operations.
    #[inline]
    pub const fn is_incomplete(&self) -> bool {
        matches!(self, Self::Incomplete { .. })
    }

    /// Returns `true` if the error cost is completely unknown.
    #[inline]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Returns the exact infidelity bound if completely known.
    #[inline]
    pub fn infidelity_bound(&self) -> Option<f64> {
        match self {
            Self::Exact {
                infidelity_bound, ..
            } => Some(*infidelity_bound),
            _ => None,
        }
    }
}

impl fmt::Display for ErrorCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact {
                infidelity_bound,
                is_measured_only,
            } => {
                let tag = if *is_measured_only {
                    "measured"
                } else {
                    "estimated"
                };
                write!(f, "{infidelity_bound:.6} ({tag})")
            }
            Self::Incomplete {
                known_infidelity,
                uncalibrated_operations,
            } => {
                write!(
                    f,
                    ">={known_infidelity:.6} (incomplete: {uncalibrated_operations} uncalibrated ops)"
                )
            }
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
