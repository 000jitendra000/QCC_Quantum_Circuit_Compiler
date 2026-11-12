OPENQASM 3.0;
include "stdgates.inc";

qubit[3] p;

gphase(0.7853981633974483);
rz(1.5707963267948966) p[0];
sx p[0];
rz(1.5707963267948966) p[0];
cx p[0], p[1];
cx p[1], p[2];
cx p[0], p[1];
cx p[1], p[0];
cx p[0], p[1];
cx p[1], p[2];
