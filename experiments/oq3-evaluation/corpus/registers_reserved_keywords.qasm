OPENQASM 3.0;
include "stdgates.inc";

// Invalid: 'qreg' and 'creg' are reserved OpenQASM 2/3 keywords and cannot be variable identifiers
qubit[5] qreg;
bit[5] creg;
