OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;

rx(pi / 2) q[0];
ry(pi) q[0];
rz(2 * pi - 0.5) q[0];
p(0.785398) q[1];
gphase(pi / 4);
