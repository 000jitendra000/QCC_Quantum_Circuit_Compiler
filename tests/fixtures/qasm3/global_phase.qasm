OPENQASM 3.0;
include "stdgates.inc";

qubit[1] q;

gphase(pi / 4);
h q[0];
gphase(pi / 4);
s q[0];

