//! Single-qubit hardware instruction basis families for synthesis.
//!
//! Classifies native single-qubit instruction sets supported by target devices,
//! enabling capability-driven synthesis without hardcoded hardware assumptions.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Hardware-native instruction basis families for single-qubit synthesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SingleQubitBasis {
    /// Generic Z-Y-Z rotation basis: $R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$.
    /// Requires target operations: `rz` (1 parameter) and `ry` (1 parameter).
    Zyz,
    /// Generic Z-X-Z rotation basis: $R_z(\phi) \cdot R_x(\theta) \cdot R_z(\lambda)$.
    /// Requires target operations: `rz` (1 parameter) and `rx` (1 parameter).
    Zxz,
    /// Target-native RZ/SX microwave/virtual transmon basis:
    /// $R_z(\phi + \pi) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \cdot R_z(\lambda)$.
    /// Requires target operations: `rz` (1 parameter) and `sx` (0 parameters).
    RzSx,
    /// OpenQASM 3 / IBM U3 single-qubit basis: $U_3(\theta, \phi, \lambda)$.
    /// Requires target operation: `u3` or `u` (3 parameters).
    U3,
}

impl fmt::Display for SingleQubitBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zyz => write!(f, "Z-Y-Z"),
            Self::Zxz => write!(f, "Z-X-Z"),
            Self::RzSx => write!(f, "RZ-SX"),
            Self::U3 => write!(f, "U3"),
        }
    }
}

impl SingleQubitBasis {
    /// Returns the required operation names and expected parameter counts for this basis.
    pub fn required_operations(&self) -> &'static [(&'static str, usize)] {
        match self {
            Self::Zyz => &[("rz", 1), ("ry", 1)],
            Self::Zxz => &[("rz", 1), ("rx", 1)],
            Self::RzSx => &[("rz", 1), ("sx", 0)],
            Self::U3 => &[("u3", 3)], // Also accepts "u" via fallback check
        }
    }

    /// Checks whether `target` natively supports this basis on `qubit`.
    pub fn is_supported_by(&self, target: &Target, qubit: PhysicalQubitId) -> bool {
        if qubit.index() >= target.num_qubits() {
            return false;
        }

        match self {
            Self::Zyz => {
                target_supports_1q_gate(target, "rz", qubit, 1)
                    && target_supports_1q_gate(target, "ry", qubit, 1)
            }
            Self::Zxz => {
                target_supports_1q_gate(target, "rz", qubit, 1)
                    && target_supports_1q_gate(target, "rx", qubit, 1)
            }
            Self::RzSx => {
                target_supports_1q_gate(target, "rz", qubit, 1)
                    && target_supports_1q_gate(target, "sx", qubit, 0)
            }
            Self::U3 => {
                target_supports_1q_gate(target, "u3", qubit, 3)
                    || target_supports_1q_gate(target, "u", qubit, 3)
            }
        }
    }
}

/// Helper checking if `target` declares a 1-qubit gate `op_name` on `qubit` with `expected_params` parameters.
fn target_supports_1q_gate(
    target: &Target,
    op_name: &str,
    qubit: PhysicalQubitId,
    expected_params: usize,
) -> bool {
    if let Some(desc) = target.operation(op_name) {
        desc.quantum_arity == 1
            && desc.parameter_count() == expected_params
            && desc.is_valid_operands(&[qubit])
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate_signature::ParameterDomain;
    use crate::target::connectivity::OperationConnectivity;
    use crate::target::operation::TargetOperationDescriptor;

    #[test]
    fn test_basis_support_queries() {
        let p0 = PhysicalQubitId::new(0);
        let mut target = Target::new("test_target", 2);

        assert!(!SingleQubitBasis::RzSx.is_supported_by(&target, p0));
        assert!(!SingleQubitBasis::Zyz.is_supported_by(&target, p0));
        assert!(!SingleQubitBasis::Zxz.is_supported_by(&target, p0));
        assert!(!SingleQubitBasis::U3.is_supported_by(&target, p0));

        // Add rz
        target.add_operation(
            TargetOperationDescriptor::builder("rz", 1)
                .add_parameter_domain(ParameterDomain::Angle4Pi)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .build(),
        );

        // Add sx
        target.add_operation(
            TargetOperationDescriptor::builder("sx", 1)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .build(),
        );

        assert!(SingleQubitBasis::RzSx.is_supported_by(&target, p0));
        assert!(!SingleQubitBasis::Zyz.is_supported_by(&target, p0));
        assert!(!SingleQubitBasis::Zxz.is_supported_by(&target, p0));

        // Add ry
        target.add_operation(
            TargetOperationDescriptor::builder("ry", 1)
                .add_parameter_domain(ParameterDomain::Angle4Pi)
                .connectivity(OperationConnectivity::all_single_qubits(2))
                .build(),
        );
        assert!(SingleQubitBasis::Zyz.is_supported_by(&target, p0));

        // Out of bounds qubit
        let p99 = PhysicalQubitId::new(99);
        assert!(!SingleQubitBasis::RzSx.is_supported_by(&target, p99));
        assert!(!SingleQubitBasis::Zyz.is_supported_by(&target, p99));
    }
}
