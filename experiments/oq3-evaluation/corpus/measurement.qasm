OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
bit[2] c;

// Assignment syntax
c[0] = measure q[0];

// Arrow syntax (legacy / alt)
measure q[1] -> c[1];

