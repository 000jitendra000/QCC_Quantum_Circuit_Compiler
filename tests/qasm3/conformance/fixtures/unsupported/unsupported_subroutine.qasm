OPENQASM 3.0;
def my_subroutine(qubit[2] q) {
    h q[0];
    cx q[0], q[1];
}
qubit[2] q;
my_subroutine(q);

