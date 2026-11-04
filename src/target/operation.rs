//! Operation representations and physical qubit identifiers for target devices.
//!
//! Separates hardware-native operation identity from universal logical gate definitions,
//! allowing provider-neutral description of vendor instruction sets (e.g. `cx`, `cz`, `ecr`, `sx`, `fsim`).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::gate_signature::ParameterDomain;
use crate::target::connectivity::OperationConnectivity;
use crate::target::error::TargetError;

/// Strongly typed identifier for a physical device qubit.
///
/// Strictly distinct from logical [`crate::ir::wire::QubitId`] to preserve injective layout mappings
/// and prevent accidental confusion between program variables and hardware sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PhysicalQubitId(pub u32);

impl PhysicalQubitId {
    /// Creates a new physical qubit identifier from a 0-indexed integer.
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the physical qubit index as a `usize`.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for PhysicalQubitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p[{}]", self.0)
    }
}

impl From<u32> for PhysicalQubitId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<usize> for PhysicalQubitId {
    #[inline]
    fn from(id: usize) -> Self {
        Self(id as u32)
    }
}

/// Target-native operation identifier (e.g. `"cx"`, `"cz"`, `"ecr"`, `"sx"`, `"rz"`, `"measure"`).
///
/// Decoupled from the target-independent logical [`crate::gate_signature::GateId`] catalog to
/// represent hardware primitives without requiring compiler-core modifications.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TargetOpId(pub String);

impl TargetOpId {
    /// Creates a new target operation identifier.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the operation mnemonic as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TargetOpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for TargetOpId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for TargetOpId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for TargetOpId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Descriptor of a hardware-native target operation.
///
/// Specifies identity, quantum arity, parameter constraints, per-operation connectivity,
/// and optional default timing and error characteristics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetOperationDescriptor {
    /// Unique target-native operation name.
    pub name: TargetOpId,
    /// Number of physical qubits acted upon by this operation.
    pub quantum_arity: usize,
    /// Expected parameter domain constraints (length equals parameter count).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_domains: Vec<ParameterDomain>,
    /// Physical qubit connectivity and allowed operand tuples.
    pub connectivity: OperationConnectivity,
    /// Optional nominal gate duration (e.g. in nanoseconds).
    ///
    /// Following the Explicit Unknown Principle, `None` denotes an unknown or uncalibrated
    /// duration, **never zero**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Optional nominal gate infidelity ($1 - \text{fidelity} \in [0.0, 1.0]$).
    ///
    /// Following the Explicit Unknown Principle, `None` denotes an unknown error rate,
    /// **never zero**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_estimate: Option<f64>,
    /// Optional human-readable description or hardware documentation note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl TargetOperationDescriptor {
    /// Returns a new fluent builder for constructing a [`TargetOperationDescriptor`].
    pub fn builder(name: impl Into<TargetOpId>, quantum_arity: usize) -> TargetOperationBuilder {
        TargetOperationBuilder::new(name, quantum_arity)
    }

    /// Returns `true` if this is a single-qubit operation.
    #[inline]
    pub const fn is_single_qubit(&self) -> bool {
        self.quantum_arity == 1
    }

    /// Returns `true` if this is a two-qubit operation.
    #[inline]
    pub const fn is_two_qubit(&self) -> bool {
        self.quantum_arity == 2
    }

    /// Returns `true` if this operation requires continuous parameters.
    #[inline]
    pub fn is_parameterized(&self) -> bool {
        !self.parameter_domains.is_empty()
    }

    /// Returns the number of continuous parameters required by this operation.
    #[inline]
    pub fn parameter_count(&self) -> usize {
        self.parameter_domains.len()
    }

    /// Returns `true` if this operation enforces asymmetric operand directionality.
    #[inline]
    pub fn is_directed(&self) -> bool {
        self.connectivity.is_directed()
    }

    /// Checks if a physical operand tuple is valid and permitted for this operation.
    pub fn is_valid_operands(&self, operands: &[PhysicalQubitId]) -> bool {
        if operands.len() != self.quantum_arity {
            return false;
        }
        // Verify no duplicate physical qubits in the operand tuple
        for i in 0..operands.len() {
            for j in (i + 1)..operands.len() {
                if operands[i] == operands[j] {
                    return false;
                }
            }
        }
        self.connectivity.is_valid_operands(operands)
    }

    /// Validates this descriptor against the physical qubit capacity of the host target.
    pub fn validate(&self, target_capacity: usize) -> Result<(), TargetError> {
        let op_name = self.name.as_str();

        if self.quantum_arity == 0 || self.quantum_arity > target_capacity {
            return Err(TargetError::InvalidArity {
                name: op_name.to_string(),
                target: "<descriptor>".to_string(),
                arity: self.quantum_arity,
                capacity: target_capacity,
            });
        }

        if let Some(err) = self.error_estimate {
            if !err.is_finite() || !(0.0..=1.0).contains(&err) {
                return Err(TargetError::InvalidErrorEstimate {
                    value: err,
                    context: format!("operation '{op_name}'"),
                });
            }
        }

        if let Some(dur) = self.duration {
            if !dur.is_finite() || dur < 0.0 {
                return Err(TargetError::InvalidDuration {
                    value: dur,
                    context: format!("operation '{op_name}'"),
                });
            }
        }

        self.connectivity
            .validate(op_name, self.quantum_arity, target_capacity)?;

        Ok(())
    }
}

/// Fluent builder for constructing a [`TargetOperationDescriptor`].
#[derive(Clone, Debug)]
pub struct TargetOperationBuilder {
    name: TargetOpId,
    quantum_arity: usize,
    parameter_domains: Vec<ParameterDomain>,
    connectivity: OperationConnectivity,
    duration: Option<f64>,
    error_estimate: Option<f64>,
    description: Option<String>,
}

impl TargetOperationBuilder {
    /// Creates a new builder with required operation name and quantum arity.
    pub fn new(name: impl Into<TargetOpId>, quantum_arity: usize) -> Self {
        Self {
            name: name.into(),
            quantum_arity,
            parameter_domains: Vec::new(),
            connectivity: OperationConnectivity::AllToAll,
            duration: None,
            error_estimate: None,
            description: None,
        }
    }

    /// Sets parameter domain specifications for this operation.
    pub fn parameter_domains(mut self, domains: impl IntoIterator<Item = ParameterDomain>) -> Self {
        self.parameter_domains = domains.into_iter().collect();
        self
    }

    /// Adds a single parameter domain constraint.
    pub fn add_parameter_domain(mut self, domain: ParameterDomain) -> Self {
        self.parameter_domains.push(domain);
        self
    }

    /// Sets the physical connectivity constraint for this operation.
    pub fn connectivity(mut self, connectivity: OperationConnectivity) -> Self {
        self.connectivity = connectivity;
        self
    }

    /// Sets the nominal duration for this operation.
    pub fn duration(mut self, duration: f64) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Sets the nominal error estimate (infidelity) for this operation.
    pub fn error_estimate(mut self, error_estimate: f64) -> Self {
        self.error_estimate = Some(error_estimate);
        self
    }

    /// Sets a human-readable description for this operation.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builds the validated [`TargetOperationDescriptor`].
    pub fn build(self) -> TargetOperationDescriptor {
        TargetOperationDescriptor {
            name: self.name,
            quantum_arity: self.quantum_arity,
            parameter_domains: self.parameter_domains,
            connectivity: self.connectivity,
            duration: self.duration,
            error_estimate: self.error_estimate,
            description: self.description,
        }
    }
}
