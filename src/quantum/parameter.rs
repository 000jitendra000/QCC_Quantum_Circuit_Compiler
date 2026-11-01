use std::f64::consts::PI;
use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use num_complex::Complex64;

/// Represents an angle in radians used for parameterized rotation gates.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Angle(f64);

impl Angle {
    /// Two pi constant.
    pub const TWO_PI: f64 = 2.0 * PI;

    /// Creates a new [`Angle`] from radians.
    #[inline]
    pub const fn from_radians(radians: f64) -> Self {
        Self(radians)
    }

    /// Zero radians angle.
    #[inline]
    pub const fn zero() -> Self {
        Self(0.0)
    }

    /// $\pi$ radians angle.
    #[inline]
    pub const fn pi() -> Self {
        Self(PI)
    }

    /// $\pi / 2$ radians angle.
    #[inline]
    pub const fn pi_by_2() -> Self {
        Self(PI / 2.0)
    }

    /// $\pi / 4$ radians angle.
    #[inline]
    pub const fn pi_by_4() -> Self {
        Self(PI / 4.0)
    }

    /// Returns the raw angle value in radians.
    #[inline]
    pub fn radians(&self) -> f64 {
        self.0
    }

    /// Normalizes the angle into the half-open range `[0, 2π)`.
    pub fn normalize(&self) -> Self {
        let mut rad = self.0 % Self::TWO_PI;
        if rad < 0.0 {
            rad += Self::TWO_PI;
        }
        // Handle floating-point edge where rad rounds to 2π
        if rad >= Self::TWO_PI {
            rad = 0.0;
        }
        Self(rad)
    }

    /// Normalizes the angle into the symmetric range `(-π, π]`.
    pub fn normalize_signed(&self) -> Self {
        let norm = self.normalize().0;
        if norm > PI {
            Self(norm - Self::TWO_PI)
        } else {
            Self(norm)
        }
    }

    /// Compares two angles for approximate equality modulo $2\pi$.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        let diff = (self.normalize().0 - other.normalize().0).abs();
        diff <= tolerance || (Self::TWO_PI - diff) <= tolerance
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} rad", self.0)
    }
}

impl Neg for Angle {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Add for Angle {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Angle {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Angle {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Angle {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

/// Represents an explicit circuit-level scalar global phase: $e^{i\phi}$.
///
/// In QCC, global phase is tracked explicitly at the circuit and block level:
/// $$U_{\text{circuit}} = e^{i\phi} U_{\text{operations}}$$
///
/// Never discard global phase during gate synthesis or decomposition, as an overall
/// phase factor becomes an observable relative phase under coherent control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalPhase {
    angle: Angle,
}

impl GlobalPhase {
    /// Creates a zero global phase: $\phi = 0 \implies e^{i \cdot 0} = 1.0 + 0.0i$.
    #[inline]
    pub const fn zero() -> Self {
        Self {
            angle: Angle::zero(),
        }
    }

    /// Creates a global phase from an angle in radians.
    #[inline]
    pub const fn from_radians(radians: f64) -> Self {
        Self {
            angle: Angle::from_radians(radians),
        }
    }

    /// Creates a global phase from an [`Angle`].
    #[inline]
    pub const fn from_angle(angle: Angle) -> Self {
        Self { angle }
    }

    /// Returns the phase angle $\phi$.
    #[inline]
    pub fn angle(&self) -> Angle {
        self.angle
    }

    /// Evaluates the scalar complex factor $e^{i\phi} = \cos(\phi) + i \sin(\phi)$.
    #[inline]
    pub fn factor(&self) -> Complex64 {
        Complex64::new(self.angle.0.cos(), self.angle.0.sin())
    }

    /// Composes this global phase with another global phase ($e^{i(\phi_1 + \phi_2)}$).
    #[inline]
    pub fn compose(&self, other: &Self) -> Self {
        Self {
            angle: self.angle + other.angle,
        }
    }

    /// Inverts the global phase ($e^{-i\phi}$).
    #[inline]
    pub fn inverse(&self) -> Self {
        Self { angle: -self.angle }
    }

    /// Compares two global phases for approximate equality on the unit circle.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        (self.factor() - other.factor()).norm() <= tolerance
    }
}

impl Default for GlobalPhase {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Display for GlobalPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exp(i * {:.6})", self.angle.0)
    }
}

impl Add for GlobalPhase {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.compose(&rhs)
    }
}

impl AddAssign for GlobalPhase {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = self.compose(&rhs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_angle_normalization() {
        let a = Angle::from_radians(3.0 * PI);
        assert!((a.normalize().radians() - PI).abs() < 1e-12);

        let neg = Angle::from_radians(-PI / 2.0);
        assert!((neg.normalize().radians() - 1.5 * PI).abs() < 1e-12);
        assert!((neg.normalize_signed().radians() - (-PI / 2.0)).abs() < 1e-12);
    }

    #[test]
    fn test_angle_approx_eq() {
        let a1 = Angle::from_radians(0.0);
        let a2 = Angle::from_radians(2.0 * PI);
        assert!(a1.approx_eq(&a2, 1e-12));
    }

    #[test]
    fn test_global_phase_factor() {
        let gp_zero = GlobalPhase::zero();
        assert!((gp_zero.factor() - Complex64::new(1.0, 0.0)).norm() < 1e-12);

        let gp_pi = GlobalPhase::from_radians(PI);
        assert!((gp_pi.factor() - Complex64::new(-1.0, 0.0)).norm() < 1e-12);

        let gp_pi_2 = GlobalPhase::from_radians(PI / 2.0);
        assert!((gp_pi_2.factor() - Complex64::new(0.0, 1.0)).norm() < 1e-12);
    }

    #[test]
    fn test_global_phase_composition() {
        let p1 = GlobalPhase::from_radians(PI / 4.0);
        let p2 = GlobalPhase::from_radians(PI / 4.0);
        let combined = p1.compose(&p2);
        assert!(combined.approx_eq(&GlobalPhase::from_radians(PI / 2.0), 1e-12));
    }
}
