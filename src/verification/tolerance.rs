//! Configurable numerical tolerances for unitary equivalence checking.

/// Explicit numerical tolerance configuration across distinct mathematical domains.
///
/// Ensures deterministic verification with documented, domain-specific tolerances:
/// - **Statevector amplitude**: Pointwise comparison between reference and candidate states ($| \beta_j - e^{i\theta} \alpha_j |$).
/// - **Matrix reconstruction**: Difference between reconstructed unitary matrix elements ($\| U_B - e^{i\theta} U_A \|$).
/// - **Global phase coherence**: Angular deviation in estimated phase across non-zero amplitudes ($| \Delta\theta |$).
/// - **Amplitude threshold**: Magnitude threshold below which an amplitude is treated as near-zero during phase estimation.
/// - **Input state normalization**: Tolerance for verifying $\left| \sum |\alpha_k|^2 - 1 \right| \le \epsilon_{\text{norm}}$.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerificationTolerances {
    /// Maximum allowable difference for pointwise statevector amplitudes. Default: $1.0 \times 10^{-10}$.
    pub state_tolerance: f64,
    /// Maximum allowable difference for reconstructed unitary matrix elements. Default: $1.0 \times 10^{-10}$.
    pub matrix_tolerance: f64,
    /// Maximum allowable angular phase deviation across non-zero amplitude components. Default: $1.0 \times 10^{-9}$.
    pub phase_tolerance: f64,
    /// Amplitude magnitude threshold below which an amplitude is treated as zero during phase estimation. Default: $1.0 \times 10^{-8}$.
    pub amplitude_threshold: f64,
    /// Maximum allowable deviation of input statevector norm squared from 1.0. Default: $1.0 \times 10^{-8}$.
    pub norm_tolerance: f64,
}

impl VerificationTolerances {
    /// Canonical default pointwise statevector amplitude tolerance ($\epsilon_{\text{state}} = 1.0 \times 10^{-10}$).
    pub const DEFAULT_STATE_TOLERANCE: f64 = 1.0e-10;
    /// Canonical default matrix reconstruction element tolerance ($\epsilon_{\text{matrix}} = 1.0 \times 10^{-10}$).
    pub const DEFAULT_MATRIX_TOLERANCE: f64 = 1.0e-10;
    /// Canonical default global phase coherence tolerance ($\epsilon_{\text{phase}} = 1.0 \times 10^{-9}$).
    pub const DEFAULT_PHASE_TOLERANCE: f64 = 1.0e-9;
    /// Canonical default amplitude zero threshold ($\epsilon_{\text{threshold}} = 1.0 \times 10^{-8}$).
    pub const DEFAULT_AMPLITUDE_THRESHOLD: f64 = 1.0e-8;
    /// Canonical default input state normalization tolerance ($\epsilon_{\text{norm}} = 1.0 \times 10^{-8}$).
    pub const DEFAULT_NORM_TOLERANCE: f64 = 1.0e-8;

    /// Creates a new `VerificationTolerances` instance with custom values.
    pub fn new(
        state_tolerance: f64,
        matrix_tolerance: f64,
        phase_tolerance: f64,
        amplitude_threshold: f64,
        norm_tolerance: f64,
    ) -> Self {
        Self {
            state_tolerance,
            matrix_tolerance,
            phase_tolerance,
            amplitude_threshold,
            norm_tolerance,
        }
    }

    /// Strict tolerance configuration for high-precision analytical checks.
    pub fn strict() -> Self {
        Self {
            state_tolerance: 1.0e-12,
            matrix_tolerance: 1.0e-12,
            phase_tolerance: 1.0e-11,
            amplitude_threshold: 1.0e-10,
            norm_tolerance: 1.0e-10,
        }
    }

    /// Relaxed tolerance configuration for circuits with deep gate cascades or numerical accumulation.
    pub fn relaxed() -> Self {
        Self {
            state_tolerance: 1.0e-7,
            matrix_tolerance: 1.0e-7,
            phase_tolerance: 1.0e-6,
            amplitude_threshold: 1.0e-6,
            norm_tolerance: 1.0e-6,
        }
    }
}

impl Default for VerificationTolerances {
    fn default() -> Self {
        Self {
            state_tolerance: Self::DEFAULT_STATE_TOLERANCE,
            matrix_tolerance: Self::DEFAULT_MATRIX_TOLERANCE,
            phase_tolerance: Self::DEFAULT_PHASE_TOLERANCE,
            amplitude_threshold: Self::DEFAULT_AMPLITUDE_THRESHOLD,
            norm_tolerance: Self::DEFAULT_NORM_TOLERANCE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tolerances() {
        let tol = VerificationTolerances::default();
        assert_eq!(tol.state_tolerance, 1.0e-10);
        assert_eq!(tol.matrix_tolerance, 1.0e-10);
        assert_eq!(tol.phase_tolerance, 1.0e-9);
        assert_eq!(tol.amplitude_threshold, 1.0e-8);
        assert_eq!(tol.norm_tolerance, 1.0e-8);
    }

    #[test]
    fn test_strict_and_relaxed_tolerances() {
        let strict = VerificationTolerances::strict();
        let relaxed = VerificationTolerances::relaxed();
        assert!(strict.state_tolerance < relaxed.state_tolerance);
        assert!(strict.phase_tolerance < relaxed.phase_tolerance);
    }
}
