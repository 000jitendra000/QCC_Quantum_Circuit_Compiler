//! Continuous rotation angle semantics, normalization, and periodicity conventions.
//!
//! # Semantics & Periodicity Contracts
//!
//! 1. **$4\pi$ Identity Periodicity**:
//!    - For single-qubit rotations $R_x(\theta)$, $R_y(\theta)$, $R_z(\theta)$, the continuous operator is:
//!      $$R_{\hat{n}}(\theta) = \exp\left(-i \frac{\theta}{2} \hat{n} \cdot \vec{\sigma}\right) = \cos\left(\frac{\theta}{2}\right) I - i \sin\left(\frac{\theta}{2}\right) (\hat{n} \cdot \vec{\sigma})$$
//!    - Because $SU(2)$ double-covers $SO(3)$, the exact matrix periodicity is $4\pi$:
//!      $$R_{\hat{n}}(\theta + 4\pi) = \cos\left(\frac{\theta}{2} + 2\pi\right) I - i \sin\left(\frac{\theta}{2} + 2\pi\right) (\hat{n} \cdot \vec{\sigma}) = R_{\hat{n}}(\theta)$$
//!    - Shifting an angle by any integer multiple of $4\pi$ incurs **zero** global phase difference.
//!
//! 2. **$2\pi$ Sign Inversion (Global Phase $\pi$)**:
//!    - Shifting an angle by an odd integer multiple of $2\pi$ inverts the sign of the operator:
//!      $$R_{\hat{n}}(\theta + 2\pi) = \cos\left(\frac{\theta}{2} + \pi\right) I - i \sin\left(\frac{\theta}{2} + \pi\right) (\hat{n} \cdot \vec{\sigma}) = -R_{\hat{n}}(\theta) = e^{i\pi} R_{\hat{n}}(\theta)$$
//!    - In quantum circuit semantics, this is an overall scalar factor of $-1 = e^{i\pi}$.
//!    - When normalizing continuous rotation angles into $[0, 2\pi)$, any reduction by an odd multiple
//!      of $2\pi$ **must** contribute a $+\pi$ adjustment to the circuit's explicit scalar global phase.
//!    - Discarding this distinction corrupts relative phase in coherent superpositions and causes
//!      unitary equivalence checks to fail.
//!
//! 3. **Exact Zero vs Phase Zero Identity Classification**:
//!    - **Exact Zero** ($\theta \equiv 0 \pmod{4\pi}$): $R_{\hat{n}}(\theta) = I$. Eliminable with $0$ phase.
//!    - **Phase Zero** ($\theta \equiv 2\pi \pmod{4\pi}$): $R_{\hat{n}}(\theta) = -I = e^{i\pi} I$. Eliminable
//!      with a $+\pi$ global phase adjustment.

use std::f64::consts::PI;

pub use crate::optimizer::algebra::{
    normalize_rotation_angle_2pi as algebra_normalize_2pi,
    normalize_rotation_angle_exact as algebra_normalize_4pi,
};
pub use crate::optimizer::passes::identity::RotationIdentityKind;
pub use crate::quantum::parameter::Angle;

/// Mathematical constant $2\pi$.
pub const TWO_PI: f64 = 2.0 * PI;

/// Mathematical constant $4\pi$ (exact $SU(2)$ rotation periodicity).
pub const FOUR_PI: f64 = 4.0 * PI;

/// Normalization mode for continuous rotation angles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RotationNormalizationMode {
    /// Normalizes angles into $[0, 2\pi)$ and adds $+\pi$ to the circuit global phase
    /// whenever an odd multiple of $2\pi$ is shifted out (standard canonical representation).
    #[default]
    TwoPiWithPhase,
    /// Normalizes angles into $[0, 4\pi)$ without adjusting global phase for non-identities,
    /// relying strictly on the exact $4\pi$ double-cover period.
    FourPiExact,
}

/// Structured result of normalizing a continuous rotation angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationNormalizedAngle {
    /// The normalized angle value in radians.
    pub angle: f64,
    /// Scalar global phase correction in radians ($0.0$ or $\pi$) required to maintain exact equivalence:
    /// $$R_{\hat{n}}(\theta) = e^{i \cdot \text{global\_phase\_correction}} R_{\hat{n}}(\text{angle})$$
    pub global_phase_correction: f64,
    /// Classification of the resulting angle as an identity (if applicable).
    pub identity_kind: RotationIdentityKind,
}

impl RotationNormalizedAngle {
    /// Creates a new `RotationNormalizedAngle`.
    pub const fn new(
        angle: f64,
        global_phase_correction: f64,
        identity_kind: RotationIdentityKind,
    ) -> Self {
        Self {
            angle,
            global_phase_correction,
            identity_kind,
        }
    }

    /// Returns `true` if the normalized rotation is an exact zero identity ($\theta \equiv 0 \pmod{4\pi}$).
    #[inline]
    pub fn is_exact_zero(&self) -> bool {
        matches!(self.identity_kind, RotationIdentityKind::ExactZero)
    }

    /// Returns `true` if the normalized rotation is a phase zero identity ($\theta \equiv 2\pi \pmod{4\pi}$).
    #[inline]
    pub fn is_phase_zero(&self) -> bool {
        matches!(self.identity_kind, RotationIdentityKind::PhaseZero { .. })
    }

    /// Returns `true` if the normalized rotation is an identity (either exact zero or phase zero).
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.is_exact_zero() || self.is_phase_zero()
    }
}

/// Normalizes a continuous single-qubit rotation angle under the specified mode and tolerance.
///
/// Ensures exact unitary semantics:
/// $$R_{\hat{n}}(\theta_{\text{in}}) = e^{i \cdot \text{result.global\_phase\_correction}} R_{\hat{n}}(\text{result.angle})$$
pub fn normalize_rotation_angle(
    angle: f64,
    tolerance: f64,
    mode: RotationNormalizationMode,
) -> RotationNormalizedAngle {
    if !angle.is_finite() {
        return RotationNormalizedAngle {
            angle,
            global_phase_correction: 0.0,
            identity_kind: RotationIdentityKind::NotIdentity,
        };
    }

    // First reduce modulo 4pi (exact SU(2) double-cover periodicity)
    let rem_4pi = angle.rem_euclid(FOUR_PI);

    // Check for exact zero identity (0 mod 4pi)
    let dist_0 = rem_4pi.min(FOUR_PI - rem_4pi);
    if dist_0 <= tolerance {
        return RotationNormalizedAngle {
            angle: 0.0,
            global_phase_correction: 0.0,
            identity_kind: RotationIdentityKind::ExactZero,
        };
    }

    // Check for phase zero identity (2pi mod 4pi, R(2pi) = -I = exp(i*pi) * I)
    let dist_2pi = (rem_4pi - TWO_PI).abs();
    if dist_2pi <= tolerance {
        return RotationNormalizedAngle {
            angle: 0.0,
            global_phase_correction: PI,
            identity_kind: RotationIdentityKind::PhaseZero {
                phase_adjustment: PI,
            },
        };
    }

    match mode {
        RotationNormalizationMode::TwoPiWithPhase => {
            if rem_4pi >= TWO_PI {
                let normalized = rem_4pi - TWO_PI;
                // Double check boundary near 2pi
                if normalized >= TWO_PI - tolerance {
                    RotationNormalizedAngle {
                        angle: 0.0,
                        global_phase_correction: 0.0,
                        identity_kind: RotationIdentityKind::ExactZero,
                    }
                } else {
                    RotationNormalizedAngle {
                        angle: normalized,
                        global_phase_correction: PI,
                        identity_kind: RotationIdentityKind::NotIdentity,
                    }
                }
            } else {
                RotationNormalizedAngle {
                    angle: rem_4pi,
                    global_phase_correction: 0.0,
                    identity_kind: RotationIdentityKind::NotIdentity,
                }
            }
        }
        RotationNormalizationMode::FourPiExact => RotationNormalizedAngle {
            angle: rem_4pi,
            global_phase_correction: 0.0,
            identity_kind: RotationIdentityKind::NotIdentity,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_zero_identities() {
        let norm = normalize_rotation_angle(0.0, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(norm.is_exact_zero());
        assert_eq!(norm.angle, 0.0);
        assert_eq!(norm.global_phase_correction, 0.0);

        let norm_4pi =
            normalize_rotation_angle(FOUR_PI, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(norm_4pi.is_exact_zero());
        assert_eq!(norm_4pi.angle, 0.0);
        assert_eq!(norm_4pi.global_phase_correction, 0.0);

        let norm_neg_4pi =
            normalize_rotation_angle(-FOUR_PI, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(norm_neg_4pi.is_exact_zero());
        assert_eq!(norm_neg_4pi.angle, 0.0);
        assert_eq!(norm_neg_4pi.global_phase_correction, 0.0);
    }

    #[test]
    fn test_phase_zero_identities() {
        let norm =
            normalize_rotation_angle(TWO_PI, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(norm.is_phase_zero());
        assert_eq!(norm.angle, 0.0);
        assert!((norm.global_phase_correction - PI).abs() < 1e-12);

        let norm_neg_2pi =
            normalize_rotation_angle(-TWO_PI, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(norm_neg_2pi.is_phase_zero());
        assert_eq!(norm_neg_2pi.angle, 0.0);
        assert!((norm_neg_2pi.global_phase_correction - PI).abs() < 1e-12);
    }

    #[test]
    fn test_nonzero_rotation_normalization() {
        let theta = 0.75;
        let norm =
            normalize_rotation_angle(theta, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        assert!(!norm.is_identity());
        assert!((norm.angle - 0.75).abs() < 1e-12);
        assert_eq!(norm.global_phase_correction, 0.0);

        // theta + 2pi -> theta with +pi global phase
        let norm_2pi = normalize_rotation_angle(
            theta + TWO_PI,
            1e-12,
            RotationNormalizationMode::TwoPiWithPhase,
        );
        assert!(!norm_2pi.is_identity());
        assert!((norm_2pi.angle - 0.75).abs() < 1e-12);
        assert!((norm_2pi.global_phase_correction - PI).abs() < 1e-12);

        // theta + 4pi -> theta with 0.0 global phase
        let norm_4pi = normalize_rotation_angle(
            theta + FOUR_PI,
            1e-12,
            RotationNormalizationMode::TwoPiWithPhase,
        );
        assert!(!norm_4pi.is_identity());
        assert!((norm_4pi.angle - 0.75).abs() < 1e-12);
        assert_eq!(norm_4pi.global_phase_correction, 0.0);
    }
}
