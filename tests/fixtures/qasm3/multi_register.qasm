OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
qubit[3] r;
bit[1] c;
bit[2] d;

h q[0];
x r[1];
cx q[1], r[2];
barrier q, r;
c[0] = measure q[0];
d[0] = measure r[0];
d[1] = measure r[2];

