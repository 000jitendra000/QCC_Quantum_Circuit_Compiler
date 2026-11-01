# QCC Quantum Semantics and Mathematical Conventions

This document freezes the formal mathematical conventions, qubit indexing rules, matrix representations, and semantic contracts enforced across the **QCC (Quantum Circuit Compiler)** codebase.

---

## 1. Qubit Indexing and Statevector Semantics

### 1.1 Little-Endian Indexing Convention
QCC adheres strictly to **little-endian statevector indexing**:
- **Logical qubit $q_0$ is the least-significant bit (LSB)** with numerical bit weight $2^0 = 1$.
- **Logical qubit $q_{n-1}$ is the most-significant bit (MSB)** with numerical bit weight $2^{n-1}$.
- Computational basis states in Dirac notation are displayed as:
  $$|q_{n-1} \, q_{n-2} \dots q_1 \, q_0\rangle$$
  The **rightmost bit is always $q_0$**.
- The corresponding basis integer index $i \in \{0, 1, \dots, 2^n - 1\}$ is:
  $$i = \sum_{k=0}^{n-1} q_k \cdot 2^k$$

### 1.2 Two-Qubit Basis State Mapping

| Dirac State $|q_1 q_0\rangle$ | Statevector Index ($i$) | Binary ($q_1 q_0$) | Computational Meaning |
|:---:|:---:|:---:|---|
| $|00\rangle$ | 0 | `00` | $q_1 = 0, \; q_0 = 0$ |
| $|01\rangle$ | 1 | `01` | $q_1 = 0, \; q_0 = 1$ |
| $|10\rangle$ | 2 | `10` | $q_1 = 1, \; q_0 = 0$ |
| $|11\rangle$ | 3 | `11` | $q_1 = 1, \; q_0 = 1$ |

### 1.3 Statevectors as Column Vectors
Quantum pure states are represented as column vectors in Hilbert space $\mathcal{H} = \mathbb{C}^{2^n}$:
$$|\psi\rangle = \begin{pmatrix} \alpha_0 \\ \alpha_1 \\ \vdots \\ \alpha_{2^n - 1} \end{pmatrix}, \quad \sum_{i=0}^{2^n - 1} |\alpha_i|^2 = 1.0$$

### 1.4 Chronological Multiplication Order (Left Multiplication)
Operations act on statevectors via **left multiplication**:
$$|\psi_{\text{final}}\rangle = U_k \dots U_2 U_1 |\psi_{\text{initial}}\rangle$$

If operation $A$ executes first and operation $B$ executes second in circuit chronological order:
$$\text{Combined Operator} = B \cdot A$$

### 1.5 Tensor Product (Kronecker) Ordering
For an $n$-qubit register in little-endian order $|q_{n-1} \dots q_1 q_0\rangle$:
$$|q_{n-1} \dots q_1 q_0\rangle = |q_{n-1}\rangle \otimes \dots \otimes |q_1\rangle \otimes |q_0\rangle$$

Notice that $q_0$ occupies the rightmost (innermost) factor.
Consequently, when single-qubit operator $U_0$ acts on $q_0$ and $U_1$ acts on $q_1$, the joint operator acting on basis vector $|q_1 q_0\rangle$ in standard matrix Kronecker product ordering is:
$$U_{\text{joint}} = U_1 \otimes U_0$$

### 1.6 Classical Measurement Result Bitstring Formatting
Classical bit formatting is **explicitly decoupled** from quantum statevector index ordering.
When measuring an $m$-bit classical register $c[0 \dots m-1]$:
- Under **`MsbFirst`** (the default standard matching OpenQASM 3 / Qiskit):
  The bitstring is formatted as $c[m-1] \dots c[1] c[0]$, where **$c[0]$ is the rightmost character**.
- Under **`LsbFirst`**:
  The bitstring is formatted as $c[0] c[1] \dots c[m-1]$, where **$c[0]$ is the leftmost character**.

---

## 2. Circuit-Level Global Phase

Every circuit carries an explicit scalar global phase factor $e^{i\phi}$ ($\phi \in [0, 2\pi)$):
$$U_{\text{circuit}} = e^{i\phi} \cdot U_{\text{operations}}$$

### Rules for Global Phase
1. **Never Discard Intermediate Phase**: A phase global to an uncontrolled block becomes an observable relative phase if that block is subsequently placed under coherent control.
2. **Decomposition Phase Extraction**: All decomposition and synthesis passes must return the synthesized gate sequence together with the scalar phase delta $\Delta\phi$.
3. **Single Common Global Phase for Equivalence**: Two unitary circuits $U_A$ and $U_B$ are equivalent if and only if there exists a **single scalar phase** $e^{i\phi}$ such that $U_A = e^{i\phi} U_B$. Choosing an independent phase per basis state or column is strictly forbidden.

---

## 3. Operation Categories & Wire Effects

| Operation | Quantum Wire Effects | Classical Wire Effects | Category | Semantic Class |
|---|---|---|---|---|
| Unitary Gates ($H, CX, R_z$) | Read/Write target qubits | None | `Unitary` | `UnitaryGate` |
| `measure q -> c` | Read/Collapse $q$ | Write $c$ | `NonUnitary` | `Measurement` |
| `reset q` | Read/Write $q$ | None | `NonUnitary` | `Reset` |
| `barrier(q...)` | Ordering boundary on $q...$ | None | `NonUnitary` | `Barrier` |
| Classical Control | None (unless gate touches $q$) | Read condition bits | `NonUnitary` | `ClassicalControl` |

---

## 4. Canonical Initial Gate Catalog

### 4.1 Fixed Single-Qubit Gates

| Gate | Name | Arity | Parameters | Mathematical Matrix | Inverse | Self-Inverse? |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **X** | `"x"` | 1 | 0 | $\begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix}$ | $X$ | Yes |
| **Y** | `"y"` | 1 | 0 | $\begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix}$ | $Y$ | Yes |
| **Z** | `"z"` | 1 | 0 | $\begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix}$ | $Z$ | Yes |
| **H** | `"h"` | 1 | 0 | $\frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix}$ | $H$ | Yes |
| **S** | `"s"` | 1 | 0 | $\begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix}$ | $S^\dagger$ (`Sdg`) | No |
| **Sdg** | `"sdg"` | 1 | 0 | $\begin{pmatrix} 1 & 0 \\ 0 & -i \end{pmatrix}$ | $S$ | No |
| **T** | `"t"` | 1 | 0 | $\begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix} = \begin{pmatrix} 1 & 0 \\ 0 & \frac{1+i}{\sqrt{2}} \end{pmatrix}$ | $T^\dagger$ (`Tdg`) | No |
| **Tdg** | `"tdg"` | 1 | 0 | $\begin{pmatrix} 1 & 0 \\ 0 & e^{-i\pi/4} \end{pmatrix} = \begin{pmatrix} 1 & 0 \\ 0 & \frac{1-i}{\sqrt{2}} \end{pmatrix}$ | $T$ | No |

### 4.2 Parameterized Single-Qubit Rotations

| Gate | Name | Arity | Parameters | Mathematical Matrix Definition | Inverse |
|:---:|:---:|:---:|:---:|:---:|:---:|
| **$R_x(\theta)$** | `"rx"` | 1 | 1 | $\exp\left(-i \frac{\theta}{2} X\right) = \begin{pmatrix} \cos(\theta/2) & -i \sin(\theta/2) \\ -i \sin(\theta/2) & \cos(\theta/2) \end{pmatrix}$ | $R_x(-\theta)$ |
| **$R_y(\theta)$** | `"ry"` | 1 | 1 | $\exp\left(-i \frac{\theta}{2} Y\right) = \begin{pmatrix} \cos(\theta/2) & -\sin(\theta/2) \\ \sin(\theta/2) & \cos(\theta/2) \end{pmatrix}$ | $R_y(-\theta)$ |
| **$R_z(\theta)$** | `"rz"` | 1 | 1 | $\exp\left(-i \frac{\theta}{2} Z\right) = \begin{pmatrix} e^{-i\theta/2} & 0 \\ 0 & e^{i\theta/2} \end{pmatrix}$ | $R_z(-\theta)$ |

#### Rotation Identities
1. $R(0) = I$.
2. $R(\theta)^{-1} = R(-\theta)$.
3. $R(\theta_1) R(\theta_2) = R(\theta_1 + \theta_2)$.
4. $R(2\pi) = -I = e^{i\pi} I$ (global phase $\pi$).
5. $R_x(\pi) = -i X, \quad R_y(\pi) = -i Y, \quad R_z(\pi) = -i Z$ (scalar global phase $-i = e^{-i\pi/2}$).

### 4.3 Two-Qubit Entangling Gates
Basis ordered as $\{|00\rangle, |01\rangle, |10\rangle, |11\rangle\}$ where $q_0$ is LSB:

#### Controlled-NOT ($CX$)
- **Operand 0 is Control ($q_c$)**, **Operand 1 is Target ($q_t$)**.
- When $q_0 = \text{ctrl}, q_1 = \text{targ}$:
  $$CX(q_0, q_1) = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \end{pmatrix}$$
- When $q_1 = \text{ctrl}, q_0 = \text{targ}$:
  $$CX(q_1, q_0) = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \end{pmatrix}$$
- Self-inverse: $CX \cdot CX = I_4$.

#### Controlled-Z ($CZ$)
- Applies phase $-1$ if and only if both control and target are in state $|1\rangle$.
- Diagonal matrix:
  $$CZ = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 0 & 0 & -1 \end{pmatrix}$$
- **Symmetric**: $CZ(q_0, q_1) = CZ(q_1, q_0)$.
- Self-inverse: $CZ \cdot CZ = I_4$.

#### SWAP
- Exchanges the quantum states of two qubits: $|q_1 q_0\rangle \to |q_0 q_1\rangle$.
  $$SWAP = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \end{pmatrix}$$
- **Symmetric**: $SWAP(q_0, q_1) = SWAP(q_1, q_0)$.
- Self-inverse: $SWAP \cdot SWAP = I_4$.
- Exact decomposition: $SWAP = CX(q_0, q_1) \cdot CX(q_1, q_0) \cdot CX(q_0, q_1)$ in circuit chronological order.
