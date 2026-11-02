OPENQASM 3.0;
gate bell_prep a, b {
    h a;
    cx a, b;
}
qubit[2] q;
bell_prep q[0], q[1];
