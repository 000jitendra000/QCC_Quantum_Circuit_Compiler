OPENQASM 3.0;
include "stdgates.inc";

qubit[2] p;

gphase(5.497787143782138);
rz(3.141592653589793) p[0];
sx p[0];
rz(4.71238898038469) p[0];
sx p[0];
rz(4.71238898038469) p[0];
