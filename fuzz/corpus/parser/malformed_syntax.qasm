OPENQASM 3.0;
qubit[2] q;
h q[0]
cx q[0], ;
rx() q[1];
c = measure;
