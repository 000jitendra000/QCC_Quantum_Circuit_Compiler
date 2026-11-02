OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;
bit[2] c;

// Prepare state to teleport on q[0]
rx(0.3) q[0];
ry(0.7) q[0];

// Create EPR pair on q[1] and q[2]
h q[1];
cx q[1], q[2];
barrier q[0], q[1], q[2];

// Bell basis measurement on q[0] and q[1]
cx q[0], q[1];
h q[0];
c[0] = measure q[0];
c[1] = measure q[1];
