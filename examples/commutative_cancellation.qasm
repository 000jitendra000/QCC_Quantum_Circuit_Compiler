OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

// Commutative cancellation opportunity:
// Two identical CX(q[0], q[1]) operations separated by an intervening
// Rz rotation on control wire q[0] (which commutes with CX control).
// When --enable-commutative-cancellation is enabled, both CX gates cancel.
// When disabled (default), both CX gates remain.
cx q[0], q[1];
rz(0.5) q[0];
cx q[0], q[1];

// Entangling operation on remaining wire
cx q[1], q[2];
