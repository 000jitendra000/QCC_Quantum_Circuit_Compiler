OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

// Redundant inverse gates eliminated by the logical optimizer
x q[0];
x q[0];

// Entangling gates on a linear topology (0 - 1 - 2)
h q[0];
cx q[0], q[1];
cx q[1], q[2];
cx q[0], q[2];

