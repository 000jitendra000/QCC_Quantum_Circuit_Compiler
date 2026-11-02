OPENQASM 3.0;
include "stdgates.inc";
qubit[2] q;
bit[2] c;
h q[0];
cx q[0], q[1];
rx(pi / 4) q[0];
ry(0.5) q[1];
rz(-pi / 2) q[0];
c[0] = measure q[0];
c[1] = measure q[1];

