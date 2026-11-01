OPENQASM 3.0;
include "stdgates.inc";

qubit[5] q_arr;
qubit single_q;
bit[5] c_arr;
bit single_c;

x q_arr[0];
x single_q;
c_arr[0] = measure q_arr[0];
single_c = measure single_q;
