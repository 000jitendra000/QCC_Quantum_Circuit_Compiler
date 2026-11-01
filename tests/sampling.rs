//! Tests for seeded random sampling, statistical properties, bit ordering, and single-evolution guarantee.

use qcc::quantum::semantics::ClassicalBitOrder;
use qcc::simulator::{
    MeasurementSelection, SimulatorError, SplitMix64, StateVector, TerminalSampler,
};

#[test]
fn test_sample_basis_zero_deterministic() {
    let sv = StateVector::new(1).unwrap(); // |0>
    let sel = MeasurementSelection::new(1, vec![0]).unwrap();
    let rng = SplitMix64::new(42);

    let res = sv.sample_terminal(sel, 100, rng).unwrap();
    let counts = res.counts(ClassicalBitOrder::MsbFirst);

    assert_eq!(counts.get("0"), Some(&100));
    assert_eq!(counts.get("1"), None);
}

#[test]
fn test_sample_basis_one_deterministic() {
    let sv = StateVector::from_basis_state(1, 1).unwrap(); // |1>
    let sel = MeasurementSelection::new(1, vec![0]).unwrap();
    let rng = SplitMix64::new(12345);

    let res = sv.sample_terminal(sel, 100, rng).unwrap();
    let counts = res.counts(ClassicalBitOrder::MsbFirst);

    assert_eq!(counts.get("1"), Some(&100));
    assert_eq!(counts.get("0"), None);
}

#[test]
fn test_sample_hadamard_superposition_statistically_plausible() {
    let mut sv = StateVector::new(1).unwrap();
    sv.apply_h(0).unwrap(); // (|0> + |1>) / sqrt(2)
    let sel = MeasurementSelection::new(1, vec![0]).unwrap();

    let num_shots = 2000;
    let rng = SplitMix64::new(999);
    let res = sv.sample_terminal(sel, num_shots, rng).unwrap();
    let counts = res.counts(ClassicalBitOrder::MsbFirst);

    let count_0 = *counts.get("0").unwrap_or(&0);
    let count_1 = *counts.get("1").unwrap_or(&0);

    assert_eq!(count_0 + count_1, num_shots);
    // 3 sigma for binomial(2000, 0.5) is ~67.
    // Range [850, 1150] is well within 4 sigma, avoiding exact 50/50 count assertions.
    assert!(
        (850..=1150).contains(&count_0),
        "Count for 0 was {}, expected in [850, 1150]",
        count_0
    );
    assert!(
        (850..=1150).contains(&count_1),
        "Count for 1 was {}, expected in [850, 1150]",
        count_1
    );
}

#[test]
fn test_sample_bell_state_produces_only_00_and_11() {
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 1).unwrap(); // (|00> + |11>) / sqrt(2)

    let sel = MeasurementSelection::all(2).unwrap();
    let num_shots = 2000;
    let rng = SplitMix64::new(2026);

    let res = sv.sample_terminal(sel, num_shots, rng).unwrap();
    let counts = res.counts(ClassicalBitOrder::MsbFirst);

    let count_00 = *counts.get("00").unwrap_or(&0);
    let count_11 = *counts.get("11").unwrap_or(&0);

    // Strictly no 01 or 10 allowed!
    assert_eq!(counts.get("01"), None);
    assert_eq!(counts.get("10"), None);

    assert_eq!(count_00 + count_11, num_shots);
    assert!(
        (850..=1150).contains(&count_00),
        "Bell count 00 was {}",
        count_00
    );
    assert!(
        (850..=1150).contains(&count_11),
        "Bell count 11 was {}",
        count_11
    );
}

#[test]
fn test_seed_reproducibility() {
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_h(1).unwrap();
    let sel = MeasurementSelection::all(2).unwrap();

    let seed = 424242;
    let res1 = sv
        .sample_terminal(sel.clone(), 500, SplitMix64::new(seed))
        .unwrap();
    let res2 = sv.sample_terminal(sel, 500, SplitMix64::new(seed)).unwrap();

    // Exact shot-by-shot equality
    assert_eq!(res1.shots(), res2.shots());
    assert_eq!(
        res1.counts(ClassicalBitOrder::MsbFirst),
        res2.counts(ClassicalBitOrder::MsbFirst)
    );
}

#[test]
fn test_different_seeds_produce_distinct_streams() {
    let mut sv = StateVector::new(1).unwrap();
    sv.apply_h(0).unwrap();
    let sel = MeasurementSelection::new(1, vec![0]).unwrap();

    let res1 = sv
        .sample_terminal(sel.clone(), 100, SplitMix64::new(1))
        .unwrap();
    let res2 = sv.sample_terminal(sel, 100, SplitMix64::new(2)).unwrap();

    assert_ne!(res1.shots(), res2.shots());
}

#[test]
fn test_result_bit_ordering_msb_vs_lsb() {
    // Basis state |01> (q0 = 1, q1 = 0)
    let sv = StateVector::from_basis_state(2, 1).unwrap();
    let sel = MeasurementSelection::new(2, vec![0, 1]).unwrap(); // measured: [q0, q1]
    let res = sv.sample_terminal(sel, 10, SplitMix64::new(7)).unwrap();

    // In [q0, q1], q0 is bit 0 (=1) and q1 is bit 1 (=0)
    // MsbFirst displays bit 1 then bit 0: "01"
    let counts_msb = res.counts(ClassicalBitOrder::MsbFirst);
    assert_eq!(counts_msb.get("01"), Some(&10));

    // LsbFirst displays bit 0 then bit 1: "10"
    let counts_lsb = res.counts(ClassicalBitOrder::LsbFirst);
    assert_eq!(counts_lsb.get("10"), Some(&10));
}

#[test]
fn test_zero_shot_count_rejected() {
    let sv = StateVector::new(1).unwrap();
    let sel = MeasurementSelection::new(1, vec![0]).unwrap();
    let err = sv.sample_terminal(sel, 0, SplitMix64::new(42)).unwrap_err();
    assert_eq!(err, SimulatorError::InvalidShotCount { count: 0 });
}

#[test]
fn test_single_evolution_architecture_guarantee() {
    // Demonstrate that 50,000 shots are sampled from a pre-computed distribution
    // without re-evolving the statevector per shot.
    let mut sv = StateVector::new(3).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_h(1).unwrap();
    sv.apply_h(2).unwrap();

    // 1. Convert state to probability distribution once
    let dist = sv.probabilities().unwrap();

    // 2. Build sampler once
    let sel = MeasurementSelection::all(3).unwrap();
    let rng = SplitMix64::new(12345);
    let mut sampler = TerminalSampler::new(&dist, sel, rng);

    // 3. Draw multiple batches of shots from the same evolved state
    let res_batch1 = sampler.sample_shots(1000).unwrap();
    let res_batch2 = sampler.sample_shots(2000).unwrap();

    assert_eq!(res_batch1.num_shots(), 1000);
    assert_eq!(res_batch2.num_shots(), 2000);
    assert_eq!(sampler.shots_drawn(), 3000);
}
