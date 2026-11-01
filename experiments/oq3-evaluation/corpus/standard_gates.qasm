OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

x q[0];
y q[0];
z q[0];
h q[0];
s q[0];
sdg q[0];
t q[0];
tdg q[0];

cx q[0], q[1];
cz q[0], q[1];
swap q[1], q[2];

