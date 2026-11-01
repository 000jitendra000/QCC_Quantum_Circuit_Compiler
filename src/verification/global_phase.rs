//! Numerically robust common global-phase estimation and coherence validation.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::simulator::statevector::StateVector;
use crate::verification::tolerance::VerificationTolerances;

/// Computes the signed shortest angular distance between two angles in radians,
/// mapped strictly to the interval $[-\pi, \pi]$.
pub fn shortest_angle_difference(a: f64, b: f64) -> f64 {
    let diff = (a - b).rem_euclid(2.0 * PI);
    if diff > PI {
        diff - 2.0 * PI
    } else {
        diff
    }
}

/// Computes the absolute shortest angular distance between two angles in radians ($[0, \pi]$).
#[inline]
pub fn angular_distance(a: f64, b: f64) -> f64 {
    shortest_angle_difference(a, b).abs()
}

/// Result of estimating a single common global phase across multiple output states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhaseEstimationResult {
    /// A single candidate global phase was successfully estimated from the collective overlap.
    Success {
        /// Estimated scalar global phase angle $\theta \in (-\pi, \pi]$ such that $B \approx e^{i\theta} A$.
        estimated_phase: f64,
        /// Unit-modulus phase factor $e^{i\theta} = \cos\theta + i\sin\theta$.
        phase_factor: Complex64,
        /// Mean normalized inner product magnitude $\frac{|S|}{m} \in [0.0, 1.0]$.
        mean_overlap: f64,
        /// Maximum angular deviation in local phase across all significant amplitude components.
        max_phase_deviation: f64,
        /// State index and amplitude index corresponding to the largest local phase deviation, if any.
        worst_phase_component: Option<(usize, usize, f64)>,
    },
    /// The reference and candidate states are nearly orthogonal or have near-zero aggregate overlap,
    /// so no common global phase can reconcile them.
    IncoherentOrOrthogonal {
        /// Mean inner product magnitude $\frac{|S|}{m}$.
        mean_overlap: f64,
    },
    /// No test state pairs were provided.
    EmptyInputs,
}

/// Robust common global-phase estimator across multiple statevector pairs.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalPhaseEstimator;

impl GlobalPhaseEstimator {
    /// Estimates a single common global phase $\theta$ satisfying $b^{(k)} \approx e^{i\theta} a^{(k)}$
    /// for all pairs of reference ($a^{(k)}$) and candidate ($b^{(k)}$) output statevectors.
    ///
    /// # Mathematical Method
    /// 1. Computes the collective Hilbert-Schmidt aggregate overlap:
    ///    $$S = \sum_{k=1}^m \langle a^{(k)}, b^{(k)} \rangle = \sum_{k=1}^m \sum_{j=0}^{2^n - 1} (a_j^{(k)})^* b_j^{(k)}$$
    /// 2. If $\frac{|S|}{m} < \epsilon_{\text{threshold}}$, states are nearly orthogonal and no phase can reconcile them.
    /// 3. Extracts candidate unit-modulus phase:
    ///    $$\theta = \operatorname{atan2}(\operatorname{Im}(S), \operatorname{Re}(S)), \quad P = e^{i\theta}$$
    /// 4. Checks local phase coherence $\Delta\theta = |\operatorname{angle\_diff}(\operatorname{arg}(b_j/a_j), \theta)|$
    ///    only on components where $|a_j| > \epsilon_{\text{threshold}}$ and $|b_j| > \epsilon_{\text{threshold}}$,
    ///    completely avoiding division by zero or near-zero amplitudes.
    pub fn estimate_common_phase(
        pairs: &[(&StateVector, &StateVector)],
        tolerances: &VerificationTolerances,
    ) -> PhaseEstimationResult {
        if pairs.is_empty() {
            return PhaseEstimationResult::EmptyInputs;
        }

        let m = pairs.len() as f64;
        let mut aggregate_s = Complex64::new(0.0, 0.0);

        for &(ref_state, cand_state) in pairs {
            let ref_amps = ref_state.amplitudes();
            let cand_amps = cand_state.amplitudes();
            let dim = ref_amps.len().min(cand_amps.len());

            for j in 0..dim {
                // (a_j)^* * b_j
                aggregate_s += ref_amps[j].conj() * cand_amps[j];
            }
        }

        let mean_overlap = aggregate_s.norm() / m;

        if mean_overlap < tolerances.amplitude_threshold {
            return PhaseEstimationResult::IncoherentOrOrthogonal { mean_overlap };
        }

        let theta = aggregate_s.im.atan2(aggregate_s.re);
        let phase_factor = Complex64::new(theta.cos(), theta.sin());

        let mut max_phase_deviation = 0.0;
        let mut worst_phase_component = None;

        for (k, &(ref_state, cand_state)) in pairs.iter().enumerate() {
            let ref_amps = ref_state.amplitudes();
            let cand_amps = cand_state.amplitudes();
            let dim = ref_amps.len().min(cand_amps.len());

            for j in 0..dim {
                let a = ref_amps[j];
                let b = cand_amps[j];

                if a.norm() > tolerances.amplitude_threshold
                    && b.norm() > tolerances.amplitude_threshold
                {
                    let ratio = b / a;
                    let local_theta = ratio.im.atan2(ratio.re);
                    let dev = angular_distance(local_theta, theta);

                    if dev > max_phase_deviation {
                        max_phase_deviation = dev;
                        worst_phase_component = Some((k, j, dev));
                    }
                }
            }
        }

        PhaseEstimationResult::Success {
            estimated_phase: theta,
            phase_factor,
            mean_overlap,
            max_phase_deviation,
            worst_phase_component,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortest_angle_difference_near_boundary() {
        let pi_minus = PI - 1e-10;
        let pi_plus = -PI + 1e-10;
        let dist = angular_distance(pi_minus, pi_plus);
        assert!(dist < 1e-9);

        assert_eq!(angular_distance(0.0, 0.0), 0.0);
        assert!((angular_distance(0.0, PI) - PI).abs() < 1e-15);
    }

    #[test]
    fn test_estimate_common_phase_identical_states() {
        let s1 = StateVector::new(2).unwrap();
        let s2 = StateVector::new(2).unwrap();
        let tol = VerificationTolerances::default();

        let res = GlobalPhaseEstimator::estimate_common_phase(&[(&s1, &s2)], &tol);
        match res {
            PhaseEstimationResult::Success {
                estimated_phase,
                mean_overlap,
                max_phase_deviation,
                ..
            } => {
                assert!(estimated_phase.abs() < 1e-12);
                assert!((mean_overlap - 1.0).abs() < 1e-12);
                assert!(max_phase_deviation < 1e-12);
            }
            _ => panic!("Expected Success, got {res:?}"),
        }
    }

    #[test]
    fn test_estimate_common_phase_with_known_phase() {
        let s1 = StateVector::new(1).unwrap();
        let mut s2 = StateVector::new(1).unwrap();
        let target_phase = PI / 3.0;
        s2.apply_global_phase(target_phase);
        let tol = VerificationTolerances::default();

        let res = GlobalPhaseEstimator::estimate_common_phase(&[(&s1, &s2)], &tol);
        match res {
            PhaseEstimationResult::Success {
                estimated_phase,
                max_phase_deviation,
                ..
            } => {
                assert!((estimated_phase - target_phase).abs() < 1e-12);
                assert!(max_phase_deviation < 1e-12);
            }
            _ => panic!("Expected Success, got {res:?}"),
        }
    }

    #[test]
    fn test_orthogonal_states_detected() {
        let s1 = StateVector::from_basis_state(1, 0).unwrap();
        let s2 = StateVector::from_basis_state(1, 1).unwrap();
        let tol = VerificationTolerances::default();

        let res = GlobalPhaseEstimator::estimate_common_phase(&[(&s1, &s2)], &tol);
        assert!(matches!(
            res,
            PhaseEstimationResult::IncoherentOrOrthogonal { .. }
        ));
    }
}
