OPENQASM 3.0;
include "stdgates.inc";

qubit[5] qreg;
qubit single_q;
bit[5] creg;
bit single_c;

x qreg[0];
x single_q;
creg[0] = measure qreg[0];
single_c = measure single_q;
