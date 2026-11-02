OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;

rx(pi / 4) q[0];
ry(pi / 2) q[1];
rz(3 * pi / 4) q[0];
rx(0.5) q[1];
cx q[0], q[1];
rz(-pi / 2) q[1];
