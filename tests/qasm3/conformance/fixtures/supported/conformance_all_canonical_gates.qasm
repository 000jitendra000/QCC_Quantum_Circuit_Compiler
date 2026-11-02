OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

x q[0];
y q[1];
z q[2];
h q[0];
s q[1];
sdg q[2];
t q[0];
tdg q[1];
rx(0.5) q[2];
ry(1.0) q[0];
rz(1.5) q[1];
cx q[0], q[1];
cz q[1], q[2];
swap q[0], q[2];
barrier q[0], q[1], q[2];

