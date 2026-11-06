// ============================================================================
// Quantum Fourier Transform (QFT) — 4-Qubit Architecture Showcase
//
// The Quantum Fourier Transform (QFT) is the core algorithmic subroutine in:
// - Shor's Polynomial-Time Factoring Algorithm
// - Quantum Phase Estimation (QPE)
// - Discrete Logarithm & Hidden Subgroup Problems
//
// Mathematical Definition:
// |j> -> (1 / sqrt(2^n)) * sum_{k=0}^{2^n - 1} e^(2*pi*i*j*k / 2^n) |k>
//
// Why QFT is the Gold-Standard Compiler Benchmark:
// 1. All-to-All Entanglement: Controlled-phase rotations connect every pair of
//    qubits (q0-q1, q0-q2, q0-q3, q1-q2, q1-q3, q2-q3).
// 2. Hardware Topology Stress-Test: On a physical linear chain (0 - 1 - 2 - 3),
//    non-adjacent connections cannot execute directly. The SABRE router must
//    evaluate lookahead heuristics and insert physical SWAPs to satisfy coupling constraints.
// 3. Phase Algebra & Rotation Merging: Controlled phase angles halve with each
//    qubit step (pi/2, pi/4, pi/8), exercising single-qubit Euler synthesis
//    and rotation merging into hardware-native RZ and SX primitives.
// 4. Bit-Reversal Permutation: The terminal SWAP gates reverse the output
//    endianness, interacting with dynamic layout tracking (L_initial -> L_final).
// 5. Layout-Aware Verification: QFT is pure unitary, allowing QCC's statevector
//    engine to verify exact mathematical equivalence under ADR-0001.
// ============================================================================

OPENQASM 3.0;
include "stdgates.inc";

qubit[4] q;

// ----------------------------------------------------------------------------
// Stage 1: Most Significant Qubit (q[0]) Superposition & Controlled Rotations
// ----------------------------------------------------------------------------
h q[0];

// Controlled-R2 (pi/2) phase rotation between q[1] and q[0]
rz(0.7853981633974483) q[1];
rz(0.7853981633974483) q[0];
cx q[1], q[0];
rz(-0.7853981633974483) q[0];
cx q[1], q[0];

// Controlled-R3 (pi/4) phase rotation between q[2] and q[0] (non-adjacent!)
rz(0.39269908169872414) q[2];
rz(0.39269908169872414) q[0];
cx q[2], q[0];
rz(-0.39269908169872414) q[0];
cx q[2], q[0];

// Controlled-R4 (pi/8) phase rotation between q[3] and q[0] (distance 3!)
rz(0.19634954084936207) q[3];
rz(0.19634954084936207) q[0];
cx q[3], q[0];
rz(-0.19634954084936207) q[0];
cx q[3], q[0];

// ----------------------------------------------------------------------------
// Stage 2: Qubit q[1] Superposition & Controlled Rotations
// ----------------------------------------------------------------------------
h q[1];

// Controlled-R2 (pi/2) phase rotation between q[2] and q[1]
rz(0.7853981633974483) q[2];
rz(0.7853981633974483) q[1];
cx q[2], q[1];
rz(-0.7853981633974483) q[1];
cx q[2], q[1];

// Controlled-R3 (pi/4) phase rotation between q[3] and q[1] (non-adjacent!)
rz(0.39269908169872414) q[3];
rz(0.39269908169872414) q[1];
cx q[3], q[1];
rz(-0.39269908169872414) q[1];
cx q[3], q[1];

// ----------------------------------------------------------------------------
// Stage 3: Qubit q[2] Superposition & Controlled Rotations
// ----------------------------------------------------------------------------
h q[2];

// Controlled-R2 (pi/2) phase rotation between q[3] and q[2]
rz(0.7853981633974483) q[3];
rz(0.7853981633974483) q[2];
cx q[3], q[2];
rz(-0.7853981633974483) q[2];
cx q[3], q[2];

// ----------------------------------------------------------------------------
// Stage 4: Least Significant Qubit (q[3]) Superposition
// ----------------------------------------------------------------------------
h q[3];

// ----------------------------------------------------------------------------
// Stage 5: Bit-Reversal Permutation
// Reverses qubit order so that outputs match standard binary index order.
// ----------------------------------------------------------------------------
swap q[0], q[3];
swap q[1], q[2];
