//! Target capability queries, legality validation, and metric lookup.
//!
//! Provides uniform query interfaces for all compiler stages, strictly preserving
//! the distinction between measured empirical metrics, estimated nominal metrics,
//! and absent data (the Explicit Unknown Principle).

use crate::gate_catalog::GateCatalog;
use crate::gate_signature::{GateId, ParameterDomain};
use crate::target::calibration::Provenance;
use crate::target::metrics::MetricValue;
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};

/// Structured errors encountered during target operation or parameter legality validation.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LegalityError {
    /// The specified operation is not supported by the target.
    #[error("target does not support operation '{0}'")]
    UnsupportedOperation(String),

    /// The number of physical qubit operands does not match the operation's declared quantum arity.
    #[error("arity mismatch for operation '{operation}': expected {expected}, got {actual}")]
    ArityMismatch {
        /// Target operation name.
        operation: String,
        /// Expected quantum arity.
        expected: usize,
        /// Actual operand count supplied.
        actual: usize,
    },

    /// A physical qubit operand index exceeds the physical qubit capacity of the target device.
    #[error("physical qubit {qubit} is out of bounds for target with capacity {capacity}")]
    QubitOutOfBounds {
        /// Out-of-bounds physical qubit.
        qubit: PhysicalQubitId,
        /// Target device physical capacity.
        capacity: usize,
    },

    /// The operand tuple contains duplicate physical qubits (e.g. `cx(p[0], p[0])`).
    #[error("duplicate operand qubit {qubit} in operand tuple {operands:?}")]
    DuplicateOperandQubit {
        /// Duplicate physical qubit.
        qubit: PhysicalQubitId,
        /// Entire operand tuple.
        operands: Vec<PhysicalQubitId>,
    },

    /// The physical operand tuple violates the target's coupling graph or directionality constraints.
    #[error(
        "illegal coupling or directionality for operation '{operation}' on operands {operands:?}"
    )]
    IllegalCoupling {
        /// Target operation name.
        operation: String,
        /// Illegal physical operand tuple.
        operands: Vec<PhysicalQubitId>,
    },

    /// The number of supplied parameters does not match the operation's declared parameter domain count.
    #[error(
        "parameter count mismatch for operation '{operation}': expected {expected}, got {actual}"
    )]
    ParameterCountMismatch {
        /// Target operation name.
        operation: String,
        /// Expected parameter count.
        expected: usize,
        /// Actual parameter count supplied.
        actual: usize,
    },

    /// A supplied continuous parameter is non-finite (`NaN` or `+/-inf`).
    #[error("non-finite parameter value at index {index}: {value}")]
    NonFiniteParameter {
        /// Parameter index in the argument list.
        index: usize,
        /// Non-finite value encountered.
        value: f64,
    },

    /// A parameter violates domain constraints.
    #[error("parameter at index {index} violates domain {domain}: {value}")]
    ParameterDomainViolation {
        /// Parameter index.
        index: usize,
        /// Expected parameter domain.
        domain: ParameterDomain,
        /// Rejected parameter value.
        value: f64,
    },
}

/// Helper function to classify a scalar metric with provenance.
fn resolve_metric(
    val: Option<f64>,
    item_prov: Option<&Provenance>,
    target_prov: Option<&Provenance>,
) -> MetricValue<f64> {
    match val {
        Some(v) => {
            if let Some(prov) = item_prov {
                MetricValue::Measured {
                    value: v,
                    provenance: prov.clone(),
                }
            } else if let Some(prov) = target_prov {
                MetricValue::Measured {
                    value: v,
                    provenance: prov.clone(),
                }
            } else {
                MetricValue::Estimated {
                    value: v,
                    provenance: None,
                }
            }
        }
        None => MetricValue::Absent,
    }
}

/// Uniform target query interface implemented by targets and target simulators.
pub trait TargetQuery {
    /// Returns `true` if this target supports an operation with the given mnemonic.
    fn supports_operation(&self, op_name: &str) -> bool;

    /// Returns `true` if this target natively supports the canonical logical gate.
    fn supports_logical_gate(&self, gate_id: GateId) -> bool;

    /// Validates physical operand tuple legality against target capacity, arity, duplicates, and coupling.
    fn check_operand_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Result<(), LegalityError>;

    /// Validates parameter legality against parameter count, finiteness, and parameter domain constraints.
    fn check_parameter_legality(&self, op_name: &str, params: &[f64]) -> Result<(), LegalityError>;

    /// Validates full legality (both operands and parameters) of an operation invocation.
    fn check_operation_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
        params: &[f64],
    ) -> Result<(), LegalityError>;

    /// Convenience predicate returning `true` if the operation invocation is strictly legal.
    fn is_legal(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> bool;

    /// Queries the execution duration of an operation on a physical operand tuple with tri-state certainty.
    fn query_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>;

    /// Queries the error estimate (infidelity) of an operation on a physical operand tuple with tri-state certainty.
    fn query_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64>;

    /// Queries the T1 relaxation time of a physical qubit with tri-state certainty.
    fn query_qubit_t1(&self, qubit: PhysicalQubitId) -> MetricValue<f64>;

    /// Queries the T2 dephasing time of a physical qubit with tri-state certainty.
    fn query_qubit_t2(&self, qubit: PhysicalQubitId) -> MetricValue<f64>;

    /// Queries the readout error probability of a physical qubit with tri-state certainty.
    fn query_readout_error(&self, qubit: PhysicalQubitId) -> MetricValue<f64>;

    /// Queries the readout pulse duration of a physical qubit with tri-state certainty.
    fn query_readout_duration(&self, qubit: PhysicalQubitId) -> MetricValue<f64>;

    /// Queries the operational frequency of a physical qubit with tri-state certainty.
    fn query_qubit_frequency(&self, qubit: PhysicalQubitId) -> MetricValue<f64>;
}

impl TargetQuery for Target {
    fn supports_operation(&self, op_name: &str) -> bool {
        self.operations.contains_key(&TargetOpId::new(op_name))
    }

    fn supports_logical_gate(&self, gate_id: GateId) -> bool {
        let canonical_name = gate_id.name();
        if self.supports_operation(canonical_name) {
            return true;
        }

        let sig = GateCatalog::get(gate_id);
        for &alias in sig.aliases {
            if self.supports_operation(alias) {
                return true;
            }
        }

        // Check if any declared target op maps to this gate_id via catalog lookup
        for op_id in self.operations.keys() {
            if let Some(entry) = GateCatalog::lookup(op_id.as_str()) {
                if entry.id == gate_id {
                    return true;
                }
            }
        }

        false
    }

    fn check_operand_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Result<(), LegalityError> {
        let desc = self
            .operation(op_name)
            .ok_or_else(|| LegalityError::UnsupportedOperation(op_name.to_string()))?;

        if operands.len() != desc.quantum_arity {
            return Err(LegalityError::ArityMismatch {
                operation: op_name.to_string(),
                expected: desc.quantum_arity,
                actual: operands.len(),
            });
        }

        for &q in operands {
            if q.index() >= self.num_qubits {
                return Err(LegalityError::QubitOutOfBounds {
                    qubit: q,
                    capacity: self.num_qubits,
                });
            }
        }

        for i in 0..operands.len() {
            for j in (i + 1)..operands.len() {
                if operands[i] == operands[j] {
                    return Err(LegalityError::DuplicateOperandQubit {
                        qubit: operands[i],
                        operands: operands.to_vec(),
                    });
                }
            }
        }

        if !desc.connectivity.is_valid_operands(operands) {
            return Err(LegalityError::IllegalCoupling {
                operation: op_name.to_string(),
                operands: operands.to_vec(),
            });
        }

        Ok(())
    }

    fn check_parameter_legality(&self, op_name: &str, params: &[f64]) -> Result<(), LegalityError> {
        let desc = self
            .operation(op_name)
            .ok_or_else(|| LegalityError::UnsupportedOperation(op_name.to_string()))?;

        if params.len() != desc.parameter_domains.len() {
            return Err(LegalityError::ParameterCountMismatch {
                operation: op_name.to_string(),
                expected: desc.parameter_domains.len(),
                actual: params.len(),
            });
        }

        for (i, (&param, &domain)) in params.iter().zip(&desc.parameter_domains).enumerate() {
            if !param.is_finite() {
                return Err(LegalityError::NonFiniteParameter {
                    index: i,
                    value: param,
                });
            }

            match domain {
                ParameterDomain::Angle4Pi
                | ParameterDomain::Angle2Pi
                | ParameterDomain::UnconstrainedReal => {
                    // All finite floating-point values are mathematically valid in these periodic/unconstrained domains.
                }
            }
        }

        Ok(())
    }

    fn check_operation_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
        params: &[f64],
    ) -> Result<(), LegalityError> {
        self.check_operand_legality(op_name, operands)?;
        self.check_parameter_legality(op_name, params)?;
        Ok(())
    }

    fn is_legal(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> bool {
        self.check_operation_legality(op_name, operands, params)
            .is_ok()
    }

    fn query_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64> {
        let op_id = TargetOpId::new(op_name);
        if let Some(cal) = &self.calibration {
            if let Some(tuple_cal) = cal.get_tuple(&op_id, operands) {
                if tuple_cal.duration.is_some() {
                    return resolve_metric(
                        tuple_cal.duration,
                        tuple_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }

        if let Some(desc) = self.operation(op_name) {
            if let Some(dur) = desc.duration {
                return MetricValue::Estimated {
                    value: dur,
                    provenance: self.provenance.clone(),
                };
            }
        }

        MetricValue::Absent
    }

    fn query_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64> {
        let op_id = TargetOpId::new(op_name);
        if let Some(cal) = &self.calibration {
            if let Some(tuple_cal) = cal.get_tuple(&op_id, operands) {
                if tuple_cal.error_estimate.is_some() {
                    return resolve_metric(
                        tuple_cal.error_estimate,
                        tuple_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }

        if let Some(desc) = self.operation(op_name) {
            if let Some(err) = desc.error_estimate {
                return MetricValue::Estimated {
                    value: err,
                    provenance: self.provenance.clone(),
                };
            }
        }

        MetricValue::Absent
    }

    fn query_qubit_t1(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        if qubit.index() >= self.num_qubits {
            return MetricValue::Absent;
        }
        if let Some(cal) = &self.calibration {
            if let Some(q_cal) = cal.get_qubit(qubit) {
                if q_cal.t1.is_some() {
                    return resolve_metric(
                        q_cal.t1,
                        q_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }
        MetricValue::Absent
    }

    fn query_qubit_t2(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        if qubit.index() >= self.num_qubits {
            return MetricValue::Absent;
        }
        if let Some(cal) = &self.calibration {
            if let Some(q_cal) = cal.get_qubit(qubit) {
                if q_cal.t2.is_some() {
                    return resolve_metric(
                        q_cal.t2,
                        q_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }
        MetricValue::Absent
    }

    fn query_readout_error(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        if qubit.index() >= self.num_qubits {
            return MetricValue::Absent;
        }
        if let Some(cal) = &self.calibration {
            if let Some(q_cal) = cal.get_qubit(qubit) {
                if q_cal.readout_error.is_some() {
                    return resolve_metric(
                        q_cal.readout_error,
                        q_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }
        MetricValue::Absent
    }

    fn query_readout_duration(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        if qubit.index() >= self.num_qubits {
            return MetricValue::Absent;
        }
        if let Some(cal) = &self.calibration {
            if let Some(q_cal) = cal.get_qubit(qubit) {
                if q_cal.readout_duration.is_some() {
                    return resolve_metric(
                        q_cal.readout_duration,
                        q_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }
        MetricValue::Absent
    }

    fn query_qubit_frequency(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        if qubit.index() >= self.num_qubits {
            return MetricValue::Absent;
        }
        if let Some(cal) = &self.calibration {
            if let Some(q_cal) = cal.get_qubit(qubit) {
                if q_cal.frequency.is_some() {
                    return resolve_metric(
                        q_cal.frequency,
                        q_cal.provenance.as_ref(),
                        self.provenance.as_ref(),
                    );
                }
            }
        }
        MetricValue::Absent
    }
}

// Inherent convenience methods on Target
impl Target {
    /// Returns `true` if this target supports the operation mnemonic.
    #[inline]
    pub fn supports_operation(&self, op_name: &str) -> bool {
        TargetQuery::supports_operation(self, op_name)
    }

    /// Returns `true` if this target supports the canonical logical gate.
    #[inline]
    pub fn supports_logical_gate(&self, gate_id: GateId) -> bool {
        TargetQuery::supports_logical_gate(self, gate_id)
    }

    /// Validates physical operand tuple legality.
    #[inline]
    pub fn check_operand_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Result<(), LegalityError> {
        TargetQuery::check_operand_legality(self, op_name, operands)
    }

    /// Validates continuous parameter legality.
    #[inline]
    pub fn check_parameter_legality(
        &self,
        op_name: &str,
        params: &[f64],
    ) -> Result<(), LegalityError> {
        TargetQuery::check_parameter_legality(self, op_name, params)
    }

    /// Validates full legality of an operation invocation.
    #[inline]
    pub fn check_operation_legality(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
        params: &[f64],
    ) -> Result<(), LegalityError> {
        TargetQuery::check_operation_legality(self, op_name, operands, params)
    }

    /// Returns `true` if an operation invocation is legal.
    #[inline]
    pub fn is_legal(&self, op_name: &str, operands: &[PhysicalQubitId], params: &[f64]) -> bool {
        TargetQuery::is_legal(self, op_name, operands, params)
    }

    /// Queries operation duration with tri-state certainty.
    #[inline]
    pub fn query_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64> {
        TargetQuery::query_duration(self, op_name, operands)
    }

    /// Queries operation error estimate with tri-state certainty.
    #[inline]
    pub fn query_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> MetricValue<f64> {
        TargetQuery::query_error(self, op_name, operands)
    }

    /// Queries T1 relaxation time for a physical qubit with tri-state certainty.
    #[inline]
    pub fn query_qubit_t1(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        TargetQuery::query_qubit_t1(self, qubit)
    }

    /// Queries T2 dephasing time for a physical qubit with tri-state certainty.
    #[inline]
    pub fn query_qubit_t2(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        TargetQuery::query_qubit_t2(self, qubit)
    }

    /// Queries readout error probability for a physical qubit with tri-state certainty.
    #[inline]
    pub fn query_readout_error(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        TargetQuery::query_readout_error(self, qubit)
    }

    /// Queries readout duration for a physical qubit with tri-state certainty.
    #[inline]
    pub fn query_readout_duration(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        TargetQuery::query_readout_duration(self, qubit)
    }

    /// Queries qubit operational frequency with tri-state certainty.
    #[inline]
    pub fn query_qubit_frequency(&self, qubit: PhysicalQubitId) -> MetricValue<f64> {
        TargetQuery::query_qubit_frequency(self, qubit)
    }
}
