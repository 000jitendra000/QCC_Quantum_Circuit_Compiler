OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
bit[2] c;

// Dynamic while loop
int[32] count = 0;
while (count < 5) {
    count += 1;
}

// Arbitrary timing / pulse calibration
defcal rx(angle theta) q[0] {
    // Pulse waveform implementation
}

// Extern function call
extern custom_oracle(qubit[2]) -> bit;

