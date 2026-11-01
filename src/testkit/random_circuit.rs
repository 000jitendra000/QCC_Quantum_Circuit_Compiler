//! Random quantum circuit and normalized statevector generation for property testing.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::ir::circuit::Circuit;
use crate::ir::error::IrError;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::quantum::gate::Gate;
use crate::quantum::parameter::Angle;
use crate::simulator::error::SimulatorError;
use crate::simulator::sampling::SeededRng;
use crate::simulator::statevector::StateVector;

/// Generates a Haar-random normalized complex statevector on $n$ qubits using the Box-Muller transform.
///
/// Samples $2^n$ complex numbers with independent standard normal real and imaginary parts,
/// then normalizes the resulting vector to unit Euclidean norm.
pub fn generate_random_statevector(
    num_qubits: usize,
    rng: &mut impl SeededRng,
) -> Result<StateVector, SimulatorError> {
    let dim = 1usize << num_qubits;
    let mut amplitudes = Vec::with_capacity(dim);
    let mut sum_sqr = 0.0;

    for _ in 0..dim {
        // Sample standard complex normal via Box-Muller
        let u1 = rng.next_f64().max(1.0e-15);
        let u2 = rng.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        let re = r * theta.cos();
        let im = r * theta.sin();

        sum_sqr += re * re + im * im;
        amplitudes.push(Complex64::new(re, im));
    }

    let norm = sum_sqr.sqrt();
    let inv_norm = 1.0 / norm;

    for amp in &mut amplitudes {
        *amp *= inv_norm;
    }

    StateVector::from_amplitudes(num_qubits, amplitudes)
}

/// Generates a random valid unitary circuit on $n$ qubits of the specified gate depth.
pub fn generate_random_unitary_circuit(
    name: impl Into<String>,
    num_qubits: usize,
    depth: usize,
    rng: &mut impl SeededRng,
) -> Result<Circuit, IrError> {
    let mut circuit = Circuit::new(name);
    circuit.add_quantum_register("q", num_qubits)?;

    if num_qubits == 0 {
        return Ok(circuit);
    }

    for _ in 0..depth {
        let op_choice = (rng.next_u64() % if num_qubits >= 2 { 14 } else { 11 }) as usize;
        let q0 = (rng.next_u64() % num_qubits as u64) as usize;

        match op_choice {
            0 => {
                circuit.add_operation(Operation::gate(&Gate::X, vec![QubitId(q0)]))?;
            }
            1 => {
                circuit.add_operation(Operation::gate(&Gate::Y, vec![QubitId(q0)]))?;
            }
            2 => {
                circuit.add_operation(Operation::gate(&Gate::Z, vec![QubitId(q0)]))?;
            }
            3 => {
                circuit.add_operation(Operation::gate(&Gate::H, vec![QubitId(q0)]))?;
            }
            4 => {
                circuit.add_operation(Operation::gate(&Gate::S, vec![QubitId(q0)]))?;
            }
            5 => {
                circuit.add_operation(Operation::gate(&Gate::Sdg, vec![QubitId(q0)]))?;
            }
            6 => {
                circuit.add_operation(Operation::gate(&Gate::T, vec![QubitId(q0)]))?;
            }
            7 => {
                circuit.add_operation(Operation::gate(&Gate::Tdg, vec![QubitId(q0)]))?;
            }
            8 => {
                let theta = (rng.next_f64() * 4.0 - 2.0) * PI;
                circuit.add_operation(Operation::gate(
                    &Gate::Rx(Angle::from_radians(theta)),
                    vec![QubitId(q0)],
                ))?;
            }
            9 => {
                let theta = (rng.next_f64() * 4.0 - 2.0) * PI;
                circuit.add_operation(Operation::gate(
                    &Gate::Ry(Angle::from_radians(theta)),
                    vec![QubitId(q0)],
                ))?;
            }
            10 => {
                let theta = (rng.next_f64() * 4.0 - 2.0) * PI;
                circuit.add_operation(Operation::gate(
                    &Gate::Rz(Angle::from_radians(theta)),
                    vec![QubitId(q0)],
                ))?;
            }
            11 => {
                // CX
                let mut q1 = (rng.next_u64() % (num_qubits - 1) as u64) as usize;
                if q1 >= q0 {
                    q1 += 1;
                }
                circuit
                    .add_operation(Operation::gate(&Gate::CX, vec![QubitId(q0), QubitId(q1)]))?;
            }
            12 => {
                // CZ
                let mut q1 = (rng.next_u64() % (num_qubits - 1) as u64) as usize;
                if q1 >= q0 {
                    q1 += 1;
                }
                circuit
                    .add_operation(Operation::gate(&Gate::CZ, vec![QubitId(q0), QubitId(q1)]))?;
            }
            13 => {
                // SWAP
                let mut q1 = (rng.next_u64() % (num_qubits - 1) as u64) as usize;
                if q1 >= q0 {
                    q1 += 1;
                }
                circuit
                    .add_operation(Operation::gate(&Gate::SWAP, vec![QubitId(q0), QubitId(q1)]))?;
            }
            _ => unreachable!(),
        }
    }

    // Optionally set global phase
    if rng.next_u64() % 2 == 0 {
        let global_phi = (rng.next_f64() * 2.0 - 1.0) * PI;
        circuit.set_global_phase(ParameterExpr::constant(global_phi));
    }

    Ok(circuit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::sampling::SplitMix64;

    #[test]
    fn test_generate_random_statevector_is_normalized() {
        let mut rng = SplitMix64::new(42);
        for n in 1..=4 {
            let state = generate_random_statevector(n, &mut rng).unwrap();
            assert_eq!(state.num_qubits(), n);
            assert!(state.is_normalized(1e-12));
        }
    }

    #[test]
    fn test_generate_random_circuit_validates() {
        let mut rng = SplitMix64::new(12345);
        for n in 1..=3 {
            let circ = generate_random_unitary_circuit("random", n, 10, &mut rng).unwrap();
            assert!(circ.validate().is_ok());
        }
    }
}
