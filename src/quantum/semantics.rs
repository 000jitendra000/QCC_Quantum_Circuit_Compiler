use std::fmt;

/// Defines the endianness convention for qubit indexing in QCC.
///
/// Under little-endian ordering:
/// - Logical qubit $q[0]$ is the least-significant bit (LSB) with bit weight $2^0 = 1$.
/// - Logical qubit $q[n-1]$ is the most-significant bit (MSB) with bit weight $2^{n-1}$.
/// - Computational basis states are formatted in Dirac notation as $|q_{n-1} \dots q_1 q_0\rangle$.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Endianness {
    /// Logical $q[0]$ is the least-significant bit (LSB).
    LittleEndian,
}

/// Computes the integer basis index from individual qubit bit values under little-endian convention.
///
/// `bits[0]` corresponds to $q[0]$ (weight 1), `bits[1]` to $q[1]$ (weight 2), etc.:
/// $$i = \sum_{k=0}^{n-1} q_k \cdot 2^k$$
pub fn bits_to_basis_index(bits: &[u8]) -> usize {
    let mut index = 0usize;
    for (k, &bit) in bits.iter().enumerate() {
        assert!(bit <= 1, "bit value must be 0 or 1, got {bit}");
        if bit == 1 {
            index |= 1 << k;
        }
    }
    index
}

/// Extracts individual qubit bit values from an integer basis index under little-endian convention.
///
/// For an $n$-qubit register, the returned vector has length $n$ where `vec[0]` is $q[0]$ (LSB)
/// and `vec[n-1]` is $q[n-1]$ (MSB).
pub fn basis_index_to_bits(index: usize, num_qubits: usize) -> Vec<u8> {
    let mut bits = Vec::with_capacity(num_qubits);
    for k in 0..num_qubits {
        bits.push(((index >> k) & 1) as u8);
    }
    bits
}

/// Formats a computational basis state in standard Dirac notation.
///
/// In little-endian convention, the rightmost displayed character is $q[0]$ (LSB)
/// and the leftmost displayed character is $q[n-1]$ (MSB):
/// $$|q_{n-1} \, q_{n-2} \dots q_1 \, q_0\rangle$$
///
/// # Examples
/// For $n = 2$ qubits:
/// - Index 0 ($q_1=0, q_0=0$) $\implies$ `"|00>"`
/// - Index 1 ($q_1=0, q_0=1$) $\implies$ `"|01>"`
/// - Index 2 ($q_1=1, q_0=0$) $\implies$ `"|10>"`
/// - Index 3 ($q_1=1, q_0=1$) $\implies$ `"|11>"`
pub fn format_basis_state(index: usize, num_qubits: usize) -> String {
    assert!(
        num_qubits < usize::BITS as usize,
        "qubit count {num_qubits} exceeds addressable integer size"
    );
    assert!(
        index < (1 << num_qubits),
        "index {index} out of bounds for {num_qubits} qubits"
    );

    let mut s = String::with_capacity(num_qubits + 2);
    s.push('|');
    // Display bits from MSB (q[n-1]) down to LSB (q[0])
    for k in (0..num_qubits).rev() {
        let bit = (index >> k) & 1;
        s.push(if bit == 1 { '1' } else { '0' });
    }
    s.push('>');
    s
}

/// Parses a Dirac basis label like `"|01>"` into its computational basis index and qubit count.
pub fn parse_basis_state(label: &str) -> Result<(usize, usize), String> {
    let trimmed = label.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('>') {
        return Err(format!("invalid Dirac basis label: `{label}`"));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let num_qubits = inner.len();
    if num_qubits == 0 {
        return Err("empty basis label".to_string());
    }

    let mut index = 0usize;
    // Inner characters are displayed MSB to LSB
    for (pos, ch) in inner.chars().enumerate() {
        let bit = match ch {
            '0' => 0usize,
            '1' => 1usize,
            _ => return Err(format!("invalid basis bit `{ch}` in `{label}`")),
        };
        let bit_index = num_qubits - 1 - pos;
        index |= bit << bit_index;
    }
    Ok((index, num_qubits))
}

/// Ordering convention for classical measurement bitstrings.
///
/// Classical bit formatting is decoupled from quantum statevector index ordering:
/// - `MsbFirst` (standard OpenQASM convention): string is formatted as `c[m-1]...c[1]c[0]`
///   where $c[0]$ is the rightmost character.
/// - `LsbFirst`: string is formatted as `c[0]c[1]...c[m-1]` where $c[0]$ is the leftmost character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClassicalBitOrder {
    /// Most-significant bit first (rightmost character is $c[0]$).
    MsbFirst,
    /// Least-significant bit first (leftmost character is $c[0]$).
    LsbFirst,
}

/// Formats classical measurement result bits into a display string under the specified ordering.
pub fn format_measurement_bits(bits: &[u8], order: ClassicalBitOrder) -> String {
    for &b in bits {
        assert!(b <= 1, "classical bit must be 0 or 1, got {b}");
    }
    match order {
        ClassicalBitOrder::MsbFirst => {
            // Display c[m-1] down to c[0]
            bits.iter()
                .rev()
                .map(|&b| if b == 1 { '1' } else { '0' })
                .collect()
        }
        ClassicalBitOrder::LsbFirst => {
            // Display c[0] to c[m-1]
            bits.iter()
                .map(|&b| if b == 1 { '1' } else { '0' })
                .collect()
        }
    }
}

/// Controlled-gate operand roles.
///
/// In controlled-NOT ($CX$) operations:
/// - **Operand 0** is the **Control** qubit ($q_{\text{ctrl}}$).
/// - **Operand 1** is the **Target** qubit ($q_{\text{targ}}$).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperandRole {
    /// Control qubit: determines whether the target operation is activated.
    Control,
    /// Target qubit: undergoes conditional transformation.
    Target,
}

/// Simulates the classical action of a controlled-NOT ($CX$) gate on computational basis bits.
///
/// - `ctrl`: bit value of Operand 0 (control).
/// - `targ`: bit value of Operand 1 (target).
///
/// Returns `(ctrl_out, targ_out)` where `targ_out = targ ^ ctrl`.
#[inline]
pub fn cx_basis_action(ctrl: u8, targ: u8) -> (u8, u8) {
    assert!(ctrl <= 1 && targ <= 1, "bits must be 0 or 1");
    (ctrl, targ ^ ctrl)
}

/// Top-level classification of quantum circuit operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperationCategory {
    /// Reversible, norm-preserving unitary transformation ($U^\dagger U = I$).
    Unitary,
    /// Non-unitary, irreversible transformation (measurement collapse, reset, noise channel).
    NonUnitary,
}

/// Fine-grained semantic classification of circuit operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticClass {
    /// Reversible unitary gate (e.g. $H$, $CX$, $R_z$).
    UnitaryGate,
    /// Projective measurement onto computational basis, collapsing state and writing classical destination.
    Measurement,
    /// Non-unitary state reset driving target qubit unconditionally to $|0\rangle$.
    Reset,
    /// Scheduling and dependency boundary that preserves execution order without altering quantum state.
    Barrier,
    /// Classically conditioned operation or classical arithmetic.
    ClassicalControl,
}

impl SemanticClass {
    /// Returns the top-level category ([`OperationCategory`]) for this semantic class.
    #[inline]
    pub fn category(&self) -> OperationCategory {
        match self {
            Self::UnitaryGate => OperationCategory::Unitary,
            Self::Measurement | Self::Reset | Self::Barrier | Self::ClassicalControl => {
                OperationCategory::NonUnitary
            }
        }
    }

    /// Returns `true` if this operation preserves unitary state evolution.
    #[inline]
    pub fn is_unitary(&self) -> bool {
        matches!(self, Self::UnitaryGate)
    }

    /// Returns `true` if this operation writes to a classical bit destination.
    #[inline]
    pub fn writes_classical(&self) -> bool {
        matches!(self, Self::Measurement)
    }

    /// Returns `true` if this operation acts as an execution ordering barrier without transforming state.
    #[inline]
    pub fn is_barrier(&self) -> bool {
        matches!(self, Self::Barrier)
    }
}

impl fmt::Display for SemanticClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnitaryGate => write!(f, "UnitaryGate"),
            Self::Measurement => write!(f, "Measurement"),
            Self::Reset => write!(f, "Reset"),
            Self::Barrier => write!(f, "Barrier"),
            Self::ClassicalControl => write!(f, "ClassicalControl"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_to_index_little_endian() {
        // q0 = 1, q1 = 0, q2 = 1 => 1*1 + 0*2 + 1*4 = 5
        assert_eq!(bits_to_basis_index(&[1, 0, 1]), 5);
        assert_eq!(basis_index_to_bits(5, 3), vec![1, 0, 1]);
    }

    #[test]
    fn test_format_basis_state_little_endian() {
        // 2-qubit basis state mapping
        assert_eq!(format_basis_state(0, 2), "|00>"); // q1=0, q0=0
        assert_eq!(format_basis_state(1, 2), "|01>"); // q1=0, q0=1 (rightmost is q0)
        assert_eq!(format_basis_state(2, 2), "|10>"); // q1=1, q0=0
        assert_eq!(format_basis_state(3, 2), "|11>"); // q1=1, q0=1
    }

    #[test]
    fn test_parse_basis_state() {
        assert_eq!(parse_basis_state("|01>").unwrap(), (1, 2));
        assert_eq!(parse_basis_state("|10>").unwrap(), (2, 2));
        assert_eq!(parse_basis_state("|101>").unwrap(), (5, 3));
    }

    #[test]
    fn test_classical_bitstring_formatting() {
        // c[0] = 1, c[1] = 0
        let bits = vec![1, 0];
        // MsbFirst: c[1] followed by c[0] => "01"
        assert_eq!(
            format_measurement_bits(&bits, ClassicalBitOrder::MsbFirst),
            "01"
        );
        // LsbFirst: c[0] followed by c[1] => "10"
        assert_eq!(
            format_measurement_bits(&bits, ClassicalBitOrder::LsbFirst),
            "10"
        );
    }

    #[test]
    fn test_cx_operand_order_semantics() {
        // Operand 0 is control, Operand 1 is target
        // (ctrl=1, targ=0) => (ctrl=1, targ=1)
        assert_eq!(cx_basis_action(1, 0), (1, 1));
        // (ctrl=0, targ=1) => (ctrl=0, targ=1) (no change)
        assert_eq!(cx_basis_action(0, 1), (0, 1));
    }

    #[test]
    fn test_semantic_classification() {
        assert!(SemanticClass::UnitaryGate.is_unitary());
        assert!(!SemanticClass::Measurement.is_unitary());
        assert!(SemanticClass::Measurement.writes_classical());
        assert!(SemanticClass::Reset.category() == OperationCategory::NonUnitary);
        assert!(SemanticClass::Barrier.is_barrier());
    }
}
