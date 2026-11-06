// ============================================================================
// QCC Optimization Showcase Circuit
//
// A comprehensive benchmark circuit designed to demonstrate the complete suite
// of compiler optimizations implemented across Phases 1 through 59 of QCC:
//
// 1. Identity Gate Elimination:
//    Zero-angle rotations and identity gates (id, rz(0.0)) are safely removed.
//
// 2. Self-Inverse & Adjoint Pair Cancellation:
//    Self-inverse gates (X-X, Y-Y, Z-Z, H-H, CX-CX) and adjoint pairs
//    (S-Sdg, T-Tdg) cancel out to identity across single qubits and pairs.
//
// 3. Rotation Merging & Modular 2*pi Cancellation:
//    Consecutive same-axis rotations combine by algebraic angle addition:
//    - pi/8 + pi/8 + pi/4 merges into a single pi/2 rotation.
//    - pi/2 + 3*pi/2 = 2*pi evaluates to identity and is completely removed.
//
// 4. Commutation-Aware Gate Reordering & Cancellation:
//    Gates commute across non-conflicting dependency DAG edges:
//    - RZ commutes across CX control wires to cancel downstream inverse gates.
//    - X commutes across CX target wires to cancel downstream inverse gates.
//
// 5. SABRE Routing on Limited Coupling Topologies:
//    Long-range entangling operations across a 5-qubit linear chain (0-1-2-3-4)
//    are routed using bidirectional SABRE heuristics and minimal physical SWAPs.
//
// 6. Global Phase & Layout-Aware Semantic Verification:
//    Circuit global phase is strictly preserved, and unitary equivalence is
//    verified by the built-in O(2^n) statevector simulator under ADR-0001.
// ============================================================================

OPENQASM 3.0;
include "stdgates.inc";

qubit[5] q;

// ----------------------------------------------------------------------------
// Stage 1: Identity Gate Elimination
// Expectation: All 5 gates are pruned during optimization.
// ----------------------------------------------------------------------------
id q[0];
rz(0.0) q[1];
id q[2];
rz(0.0) q[3];
id q[4];

// ----------------------------------------------------------------------------
// Stage 2: Self-Inverse and Adjoint Pair Cancellations
// Expectation: All 14 gates cancel out to identity and disappear.
// ----------------------------------------------------------------------------
x q[0];
x q[0];

y q[1];
y q[1];

z q[2];
z q[2];

h q[3];
h q[3];

s q[4];
sdg q[4];

t q[0];
tdg q[0];

cx q[1], q[2];
cx q[1], q[2];

// ----------------------------------------------------------------------------
// Stage 3: Same-Axis Rotation Merging & Angle Addition
// Expectation:
// - Three RZ rotations on q[0] (pi/8 + pi/8 + pi/4) merge into one RZ(pi/2).
// - Two RZ rotations on q[1] (pi/2 + 3*pi/2 = 2*pi) cancel to identity.
// ----------------------------------------------------------------------------
rz(0.39269908169872414) q[0];
rz(0.39269908169872414) q[0];
rz(0.78539816339744830) q[0];

rz(1.5707963267948966) q[1];
rz(4.7123889803846900) q[1];

// ----------------------------------------------------------------------------
// Stage 4: Commutation-Aware Gate Reordering
// Expectation:
// - Outer RZ gates on q[2] commute through CX control to meet and cancel.
// - Outer X gates on q[3] commute through CX target to meet and cancel.
// - Only the central CX operation survives.
// ----------------------------------------------------------------------------
rz(0.75) q[2];
x q[3];
cx q[2], q[3];
rz(-0.75) q[2];
x q[3];

// ----------------------------------------------------------------------------
// Stage 5: Multi-Qubit Entanglement & SABRE Routing
// Target Topology: Linear chain 0 - 1 - 2 - 3 - 4
// Expectation:
// - Direct nearest-neighbor interactions execute natively.
// - Long-range interactions (0 to 4, 1 to 4, 0 to 3) cannot be executed directly;
//   the SABRE router inserts minimal physical SWAP gates to satisfy coupling.
// ----------------------------------------------------------------------------
h q[0];
h q[2];
h q[4];

// Nearest-neighbor interactions (native on linear chain)
cx q[0], q[1];
cx q[1], q[2];
cx q[2], q[3];
cx q[3], q[4];

// Long-distance interactions (require SABRE routing and physical SWAPs)
cx q[0], q[4];
cx q[1], q[4];
cx q[0], q[3];
cx q[1], q[3];
