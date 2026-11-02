OPENQASM 3.0;
qubit[2] q;
gate custom_g a, b {
    h a;
    cx a, b;
}
if (c == 1) {
    h q[0];
}
