OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;

// Malformed: duplicate operands on two-qubit gate
cx q[0], q[0];
// Malformed: out of bounds index (semantic or syntactic check)
h q[9999];
