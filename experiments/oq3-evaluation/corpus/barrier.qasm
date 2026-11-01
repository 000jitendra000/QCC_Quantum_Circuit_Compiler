OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

h q[0];
h q[1];
// Global barrier
barrier;
// Selective barrier
barrier q[0], q[1];
cx q[0], q[1];

