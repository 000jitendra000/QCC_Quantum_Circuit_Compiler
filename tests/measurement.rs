//! Tests for probability distributions, marginal distributions, and measurement selection.

use qcc::simulator::{
    MeasurementSelection, ProbabilityDistribution, SimulatorError, StateVector,
    PROBABILITY_NORM_TOLERANCE,
};

const TOLERANCE: f64 = 1e-10;

#[test]
fn test_basis_state_probabilities() {
    let sv0 = StateVector::new(1).unwrap(); // |0>
    let dist0 = sv0.probabilities().unwrap();
    assert_eq!(dist0.get(0).unwrap(), 1.0);
    assert_eq!(dist0.get(1).unwrap(), 0.0);

    let sv1 = StateVector::from_basis_state(1, 1).unwrap(); // |1>
    let dist1 = sv1.probabilities().unwrap();
    assert_eq!(dist1.get(0).unwrap(), 0.0);
    assert_eq!(dist1.get(1).unwrap(), 1.0);
}

#[test]
fn test_superposition_probabilities() {
    let mut sv = StateVector::new(1).unwrap();
    sv.apply_h(0).unwrap(); // (|0> + |1>) / sqrt(2)
    let dist = sv.probabilities().unwrap();
    assert!((dist.get(0).unwrap() - 0.5).abs() < TOLERANCE);
    assert!((dist.get(1).unwrap() - 0.5).abs() < TOLERANCE);
}

#[test]
fn test_bell_state_probabilities() {
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 1).unwrap(); // (|00> + |11>) / sqrt(2)

    let dist = sv.probabilities().unwrap();
    assert!((dist.get(0).unwrap() - 0.5).abs() < TOLERANCE);
    assert_eq!(dist.get(1).unwrap(), 0.0);
    assert_eq!(dist.get(2).unwrap(), 0.0);
    assert!((dist.get(3).unwrap() - 0.5).abs() < TOLERANCE);
}

#[test]
fn test_marginal_probabilities_bell_state() {
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 1).unwrap();
    let dist = sv.probabilities().unwrap();

    // Marginal on q0
    let sel_q0 = MeasurementSelection::new(2, vec![0]).unwrap();
    let marginal_q0 = dist.marginal(&sel_q0).unwrap();
    assert_eq!(marginal_q0.len(), 2);
    assert!((marginal_q0.get(0).unwrap() - 0.5).abs() < TOLERANCE);
    assert!((marginal_q0.get(1).unwrap() - 0.5).abs() < TOLERANCE);

    // Marginal on q1
    let sel_q1 = MeasurementSelection::new(2, vec![1]).unwrap();
    let marginal_q1 = dist.marginal(&sel_q1).unwrap();
    assert_eq!(marginal_q1.len(), 2);
    assert!((marginal_q1.get(0).unwrap() - 0.5).abs() < TOLERANCE);
    assert!((marginal_q1.get(1).unwrap() - 0.5).abs() < TOLERANCE);
}

#[test]
fn test_marginal_probabilities_non_adjacent_3_qubit() {
    // (|000> + |101>) / sqrt(2)
    let mut sv = StateVector::new(3).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 2).unwrap();
    let dist = sv.probabilities().unwrap();

    // Measure q0 and q2
    let sel = MeasurementSelection::new(3, vec![0, 2]).unwrap();
    let marginal = dist.marginal(&sel).unwrap();
    assert_eq!(marginal.len(), 4);
    // Index 0: (q0=0, q2=0) -> 0.5
    assert!((marginal.get(0).unwrap() - 0.5).abs() < TOLERANCE);
    // Index 1: (q0=1, q2=0) -> 0.0
    assert_eq!(marginal.get(1).unwrap(), 0.0);
    // Index 2: (q0=0, q2=1) -> 0.0
    assert_eq!(marginal.get(2).unwrap(), 0.0);
    // Index 3: (q0=1, q2=1) -> 0.5
    assert!((marginal.get(3).unwrap() - 0.5).abs() < TOLERANCE);
}

#[test]
fn test_probability_validation_unnormalized_rejection() {
    let unnormalized = vec![0.4, 0.4]; // Sum = 0.8 != 1.0
    let err = ProbabilityDistribution::from_probabilities(1, unnormalized).unwrap_err();
    assert!(matches!(
        err,
        SimulatorError::InvalidProbabilityDistribution { .. }
    ));
}

#[test]
fn test_probability_validation_negative_rejection() {
    let neg = vec![1.2, -0.2];
    let err = ProbabilityDistribution::from_probabilities(1, neg).unwrap_err();
    assert!(matches!(
        err,
        SimulatorError::InvalidProbabilityDistribution { .. }
    ));
}

#[test]
fn test_probability_validation_nan_rejection() {
    let nans = vec![f64::NAN, 1.0];
    let err = ProbabilityDistribution::from_probabilities(1, nans).unwrap_err();
    assert!(matches!(
        err,
        SimulatorError::InvalidProbabilityDistribution { .. }
    ));
}

#[test]
fn test_probability_validation_dimension_mismatch() {
    let wrong_dim = vec![0.5, 0.5, 0.0];
    let err = ProbabilityDistribution::from_probabilities(2, wrong_dim).unwrap_err();
    assert_eq!(
        err,
        SimulatorError::DimensionMismatch {
            num_qubits: 2,
            expected: 4,
            actual: 3,
        }
    );
}

#[test]
fn test_measurement_selection_validation() {
    // Empty selection
    assert_eq!(
        MeasurementSelection::new(2, vec![]).unwrap_err(),
        SimulatorError::EmptyMeasurementSelection
    );

    // Out of bounds
    assert_eq!(
        MeasurementSelection::new(2, vec![0, 2]).unwrap_err(),
        SimulatorError::TargetOutOfBounds {
            target: 2,
            num_qubits: 2,
        }
    );

    // Duplicate qubit
    assert_eq!(
        MeasurementSelection::new(3, vec![1, 2, 1]).unwrap_err(),
        SimulatorError::DuplicateMeasurementQubit { qubit: 1 }
    );

    // All qubits
    let all = MeasurementSelection::all(3).unwrap();
    assert_eq!(all.qubits(), &[0, 1, 2]);
}

#[test]
fn test_floating_point_tails_handling() {
    // Sum is 1.0 + 1e-7 (within tolerance)
    let p0 = 0.5 + 1e-7;
    let p1 = 0.5;
    let dist = ProbabilityDistribution::from_probabilities(1, vec![p0, p1]).unwrap();
    assert!((dist.get(0).unwrap() - p0).abs() < PROBABILITY_NORM_TOLERANCE);
}
