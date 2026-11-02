# QCC Command-Line Interface Reference

The **QCC (Quantum Circuit Compiler)** command-line interface provides compilation, inspection, and verification tools for quantum circuits written in OpenQASM 3.

---

## 1. Stable Exit Codes

Every invocation of `qcc` returns a deterministic integer exit code:

| Exit Code | Constant | Meaning | Typical Causes |
|:---:|---|---|---|
| `0` | `SUCCESS` | Success | Compilation or inspection completed with zero errors; help or version flag displayed. |
| `1` | `CLI_ERROR` | Operational Failure | Invalid command-line flags, missing argument, unreadable input file, or file write failure. |
| `2` | `COMPILE_ERROR` | Source Code Error | Syntax error (`E0001`), out-of-bounds index (`E0014`), undeclared register (`E0012`), division by zero (`E0030`), or arity mismatch (`E0021`). |
| `3` | `UNSUPPORTED_ERROR` | Fail-Closed Subset Error | Unsupported construct detected (`E0002`–`E0006`, `E0026`, `E0027`), such as conditionals (`if`), loops, subroutines, or 3-qubit gates. |

---

## 2. Command Overview

```text
Usage: qcc [OPTIONS] [COMMAND]

Commands:
  compile  Compile an OpenQASM 3 file to QccIR and emit compiled output
  inspect  Inspect an OpenQASM 3 file and display structural/semantic details

Options:
  -V, --version  Print version information and exit
  -h, --help     Print help information and exit
```

---

## 3. `qcc compile`

Compiles supported OpenQASM 3 source code into a validated intermediate representation ([`Circuit`]) and emits canonical OpenQASM 3 or QccIR text.

```text
Usage: qcc compile [OPTIONS] <FILE>

Arguments:
  <FILE>                       Source file to compile (use '-' for stdin)

Options:
  -o, --output <FILE>          Write output to <FILE> instead of stdout
      --format <FMT>           Output format: 'openqasm' (default) or 'qccir'
      --global-phase <POLICY>  Global phase policy: 'auto' (default), 'always', or 'reject-nonzero'
      --measure-syntax <SYN>   Measurement syntax: 'assign' (default) or 'arrow'
  -b, --bind <NAME=VAL>        Bind parameter symbol to a numerical value
  -h, --help                   Print help information and exit
```

### Examples

#### Basic Compilation to stdout
```bash
qcc compile circuits/bell.qasm
```
Output:
```qasm
OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
```

#### Compilation to an Output File with Arrow Measurement Syntax
```bash
qcc compile --measure-syntax arrow -o out.qasm circuits/bell.qasm
```

#### Compilation with Parameter Bindings
```bash
qcc compile -b theta=0.7853981633974483 circuits/rotation.qasm
```

#### Reading from Standard Input
```bash
cat circuits/bell.qasm | qcc compile -
```

---

## 4. `qcc inspect`

Inspects OpenQASM 3 source code without bypassing semantic analysis, displaying structural and circuit metrics.

```text
Usage: qcc inspect [OPTIONS] <FILE>

Arguments:
  <FILE>                 Source file to inspect (use '-' for stdin)

Options:
      --summary          Display circuit summary and topological trace (default)
      --ast              Display parsed Abstract Syntax Tree (AST)
      --ir               Display lowered QccIR dependency graph
      --registers        Display declared quantum and classical registers
  -b, --bind <NAME=VAL>  Bind parameter symbol to a numerical value
  -h, --help             Print help information and exit
```

### Examples

#### Default Circuit Summary
```bash
qcc inspect circuits/bell.qasm
```
Output:
```text
============================================================
 QCC Circuit Inspection: bell_state
============================================================
Source File:          circuits/bell.qasm
Total Qubits:         2
Total Classical Bits: 2
Global Phase:         Constant(0.0)
Dependency Edges:     3

Declared Quantum Registers:
  'q' [size: 2, qubits: 0..2]
Declared Classical Registers:
  'c' [size: 2, bits: 0..2]

Topological Operation Sequence:
  Step  1: [node #NodeIndex(0)] h q[0]
  Step  2: [node #NodeIndex(1)] cx q[0] q[1]
  Step  3: [node #NodeIndex(2)] measure q[0] -> c[0]
  Step  4: [node #NodeIndex(3)] measure q[1] -> c[1]
============================================================
```

#### Inspecting Declared Registers
```bash
qcc inspect --registers circuits/multi_register.qasm
```
Output:
```text
=== Declared Registers (circuits/multi_register.qasm) ===
Quantum Registers:
  q (size: 2, array, base qubit: 0)
  r (size: 3, array, base qubit: 2)
Classical Registers:
  c (size: 1, array, base bit: 0)
  d (size: 2, array, base bit: 1)
```

---

## 5. Diagnostic Rendering

Diagnostics are emitted to `stderr` with exact source locations:
```text
<FILE>:<LINE>:<START_COL>-<END_COL>: <LEVEL>[<CODE>]: <MESSAGE>
  note: <NOTE>
```

### Examples

#### Syntax Error (Exit Code 2)
```bash
qcc compile invalid.qasm
# invalid.qasm:3:5-18: error[E0030]: division by zero in parameter expression
# Exit code: 2
```

#### Unsupported Construct (Exit Code 3)
```bash
qcc compile conditionals.qasm
# conditionals.qasm:2:1-4: error[E0005]: conditionals ('if') are not supported in QccIR at this milestone
#   note: dynamic classical control flow is deferred to Milestone M7.
# Exit code: 3
```

#### Operational Error (Exit Code 1)
```bash
qcc compile missing.qasm
# error: failed to read input file 'missing.qasm': No such file or directory (os error 2)
# Exit code: 1
```

