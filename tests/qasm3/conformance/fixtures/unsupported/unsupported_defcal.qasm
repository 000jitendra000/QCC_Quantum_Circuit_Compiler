OPENQASM 3.0;
defcal rx(pi) q {
    waveform drag = drag_gaussian();
}
qubit[1] q;
rx(pi) q[0];

