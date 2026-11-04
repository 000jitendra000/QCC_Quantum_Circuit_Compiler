//! Decomposition rules and extensible rule registry for basis translation.
//!
//! Provides mathematically verified transformations from high-level logical gates to
//! hardware-supported target gates, with full support for symbolic expressions and local equivalence.

use std::f64::consts::PI;

use crate::gate_catalog::GateCatalog;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::quantum::angle::Angle;
use crate::quantum::gate_matrix::{matrix_sx, matrix_u3, Matrix2x2};
use crate::synthesis::single_qubit::SingleQubitSynthesizer;
use crate::synthesis::templates::{
    CxToCzTemplate, CzToCxTemplate, DecompositionTemplate, SwapToCxTemplate, ToffoliToCxTemplate,
};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::target::query::LegalityError;
use crate::translation::TranslationError;

/// Helper to validate legality of an operation with possibly symbolic parameters.
pub fn check_operation_legality_with_exprs(
    target: &Target,
    op_name: &str,
    operands: &[PhysicalQubitId],
    parameters: &[ParameterExpr],
) -> Result<(), LegalityError> {
    let const_params: Option<Vec<f64>> = parameters.iter().map(|p| p.as_constant()).collect();
    if let Some(params_f64) = const_params {
        target.check_operation_legality(op_name, operands, &params_f64)
    } else {
        target.check_operand_legality(op_name, operands)?;
        if let Some(desc) = target.operation(op_name) {
            if desc.parameter_count() != parameters.len() {
                return Err(LegalityError::ParameterCountMismatch {
                    operation: op_name.to_string(),
                    expected: desc.parameter_count(),
                    actual: parameters.len(),
                });
            }
        }
        Ok(())
    }
}

/// A candidate decomposition sequence produced by applying a translation rule.
#[derive(Clone, Debug)]
pub struct DecompositionCandidate {
    /// Identifying name of the rule that generated this decomposition.
    pub rule_name: &'static str,
    /// Decomposed operations in execution order.
    pub operations: Vec<Operation>,
    /// Global phase added by this decomposition in radians $[0, 2\pi)$.
    pub global_phase_delta: f64,
}

/// Pluggable interface for basis translation and decomposition rules.
pub trait TranslationRule: Send + Sync {
    /// Stable canonical rule identifier.
    fn name(&self) -> &'static str;

    /// Gate mnemonic decomposed by this rule (e.g. `"swap"`, `"cx"`, `"cz"`, `"ccx"`, `"rx"`, etc.).
    fn target_gate(&self) -> &'static str;

    /// Expected quantum arity of the operation.
    fn quantum_arity(&self) -> usize;

    /// Returns `true` if this rule can decompose `op` on `target`.
    fn matches(&self, op: &Operation, target: &Target) -> bool;

    /// Decomposes `op` into a candidate sequence.
    fn decompose(
        &self,
        op: &Operation,
        target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError>;
}

// ----------------------------------------------------------------------------
// Built-in Rules
// ----------------------------------------------------------------------------

/// Decomposes `SWAP(a, b)` into three `CX` gates: `CX(a, b); CX(b, a); CX(a, b)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwapToCxRule;

impl TranslationRule for SwapToCxRule {
    fn name(&self) -> &'static str {
        "swap_to_cx_forward"
    }

    fn target_gate(&self) -> &'static str {
        "swap"
    }

    fn quantum_arity(&self) -> usize {
        2
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("swap") || op.qubits().len() != 2 {
            return false;
        }
        let phys = [
            PhysicalQubitId(op.qubits()[0].0 as u32),
            PhysicalQubitId(op.qubits()[1].0 as u32),
        ];
        // Only decompose if SWAP is not already legal on target
        check_operation_legality_with_exprs(target, "swap", &phys, op.parameters()).is_err()
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let template = SwapToCxTemplate;
        let operations = template.expand(op.qubits())?;
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: template.phase_contribution(),
        })
    }
}

/// Decomposes `SWAP(a, b)` with reversed orientation: `CX(b, a); CX(a, b); CX(b, a)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwapToCxReversedRule;

impl TranslationRule for SwapToCxReversedRule {
    fn name(&self) -> &'static str {
        "swap_to_cx_reversed"
    }

    fn target_gate(&self) -> &'static str {
        "swap"
    }

    fn quantum_arity(&self) -> usize {
        2
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("swap") || op.qubits().len() != 2 {
            return false;
        }
        let phys = [
            PhysicalQubitId(op.qubits()[0].0 as u32),
            PhysicalQubitId(op.qubits()[1].0 as u32),
        ];
        check_operation_legality_with_exprs(target, "swap", &phys, op.parameters()).is_err()
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let a = op.qubits()[0];
        let b = op.qubits()[1];
        let operations = vec![
            Operation::custom_gate("cx", vec![b, a], vec![]),
            Operation::custom_gate("cx", vec![a, b], vec![]),
            Operation::custom_gate("cx", vec![b, a], vec![]),
        ];
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: 0.0,
        })
    }
}

/// Decomposes `CX(c, t)` into `H(t); CZ(c, t); H(t)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CxToCzRule;

impl TranslationRule for CxToCzRule {
    fn name(&self) -> &'static str {
        "cx_to_cz"
    }

    fn target_gate(&self) -> &'static str {
        "cx"
    }

    fn quantum_arity(&self) -> usize {
        2
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("cx") || op.qubits().len() != 2 {
            return false;
        }
        let phys = [
            PhysicalQubitId(op.qubits()[0].0 as u32),
            PhysicalQubitId(op.qubits()[1].0 as u32),
        ];
        // Decompose if CX is not legal on this pair and target has CZ
        check_operation_legality_with_exprs(target, "cx", &phys, op.parameters()).is_err()
            && target.is_operation_supported("cz")
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let template = CxToCzTemplate;
        let operations = template.expand(op.qubits())?;
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: template.phase_contribution(),
        })
    }
}

/// Decomposes `CZ(c, t)` into `H(t); CX(c, t); H(t)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CzToCxRule;

impl TranslationRule for CzToCxRule {
    fn name(&self) -> &'static str {
        "cz_to_cx"
    }

    fn target_gate(&self) -> &'static str {
        "cz"
    }

    fn quantum_arity(&self) -> usize {
        2
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("cz") || op.qubits().len() != 2 {
            return false;
        }
        let phys = [
            PhysicalQubitId(op.qubits()[0].0 as u32),
            PhysicalQubitId(op.qubits()[1].0 as u32),
        ];
        check_operation_legality_with_exprs(target, "cz", &phys, op.parameters()).is_err()
            && target.is_operation_supported("cx")
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let template = CzToCxTemplate;
        let operations = template.expand(op.qubits())?;
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: template.phase_contribution(),
        })
    }
}

/// Decomposes `CCX` / `Toffoli` into 15 elementary gates (6 CX, 2 H, 4 T, 3 Tdg).
#[derive(Clone, Copy, Debug, Default)]
pub struct ToffoliToCxRule;

impl TranslationRule for ToffoliToCxRule {
    fn name(&self) -> &'static str {
        "toffoli_to_cx"
    }

    fn target_gate(&self) -> &'static str {
        "ccx"
    }

    fn quantum_arity(&self) -> usize {
        3
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        let name = op.name();
        if (!name.eq_ignore_ascii_case("ccx") && !name.eq_ignore_ascii_case("toffoli"))
            || op.qubits().len() != 3
        {
            return false;
        }
        let phys = [
            PhysicalQubitId(op.qubits()[0].0 as u32),
            PhysicalQubitId(op.qubits()[1].0 as u32),
            PhysicalQubitId(op.qubits()[2].0 as u32),
        ];
        check_operation_legality_with_exprs(target, name, &phys, op.parameters()).is_err()
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let template = ToffoliToCxTemplate;
        let operations = template.expand(op.qubits())?;
        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: template.phase_contribution(),
        })
    }
}

/// Synthesizes single-qubit unitaries with constant parameters into the target's native 1Q basis.
#[derive(Clone, Copy, Debug, Default)]
pub struct SingleQubitSynthesisRule;

impl SingleQubitSynthesisRule {
    /// Evaluates the concrete $2 \times 2$ unitary matrix for single-qubit operation `op`.
    pub fn evaluate_matrix(op: &Operation) -> Option<Matrix2x2> {
        let name = op.name().to_ascii_lowercase();
        let params: Vec<f64> = op
            .parameters()
            .iter()
            .map(|p| p.as_constant())
            .collect::<Option<Vec<f64>>>()?;

        if let Some(sig) = GateCatalog::lookup(&name) {
            GateCatalog::matrix_for_gate_1q(sig.id, &params)
        } else if name == "sx" {
            Some(matrix_sx())
        } else if name == "u3" || name == "u" {
            if params.len() == 3 {
                Some(matrix_u3(
                    Angle::from_radians(params[0]),
                    Angle::from_radians(params[1]),
                    Angle::from_radians(params[2]),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl TranslationRule for SingleQubitSynthesisRule {
    fn name(&self) -> &'static str {
        "single_qubit_synthesis"
    }

    fn target_gate(&self) -> &'static str {
        "*"
    }

    fn quantum_arity(&self) -> usize {
        1
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if op.qubits().len() != 1 {
            return false;
        }
        let phys = PhysicalQubitId(op.qubits()[0].0 as u32);

        // Check if already legal on target
        if check_operation_legality_with_exprs(target, op.name(), &[phys], op.parameters()).is_ok()
        {
            return false;
        }

        // Must have at least one supported single-qubit basis on this qubit
        if target.preferred_single_qubit_basis(phys).is_none() {
            return false;
        }

        // Must be able to evaluate concrete 2x2 matrix
        Self::evaluate_matrix(op).is_some()
    }

    fn decompose(
        &self,
        op: &Operation,
        target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let phys = PhysicalQubitId(op.qubits()[0].0 as u32);
        let matrix =
            Self::evaluate_matrix(op).ok_or_else(|| TranslationError::NoViableTranslation {
                op_name: op.name().to_string(),
                operands: vec![phys],
                candidates_tried: 1,
                reason: "cannot evaluate concrete 2x2 unitary matrix".to_string(),
            })?;

        let synthesizer = SingleQubitSynthesizer::default();
        let seq = synthesizer.synthesize(&matrix, phys, target)?;

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations: seq.operations().to_vec(),
            global_phase_delta: seq.global_phase().angle().radians(),
        })
    }
}

/// Decomposes `Rx(theta)` preserving symbolic parameter expressions.
///
/// On `RZ-SX` targets:
/// $$R_x(\theta) = e^{i\pi/2} \left( R_z(\pi/2) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \cdot R_z(\pi/2) \right)$$
#[derive(Clone, Copy, Debug, Default)]
pub struct SymbolicRxRule;

impl TranslationRule for SymbolicRxRule {
    fn name(&self) -> &'static str {
        "symbolic_rx_to_rz_sx"
    }

    fn target_gate(&self) -> &'static str {
        "rx"
    }

    fn quantum_arity(&self) -> usize {
        1
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("rx")
            || op.qubits().len() != 1
            || op.parameters().is_empty()
        {
            return false;
        }
        let phys = PhysicalQubitId(op.qubits()[0].0 as u32);
        check_operation_legality_with_exprs(target, "rx", &[phys], op.parameters()).is_err()
            && target.is_operation_supported("rz")
            && target.is_operation_supported("sx")
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let q = op.qubits()[0];
        let theta = &op.parameters()[0];

        let half_pi = ParameterExpr::Constant(PI / 2.0);
        let theta_plus_pi = ParameterExpr::Add(
            Box::new(theta.clone()),
            Box::new(ParameterExpr::Constant(PI)),
        );

        // Execution order (left multiplication B @ A):
        // 1. Rz(pi/2)
        // 2. SX
        // 3. Rz(theta + pi)
        // 4. SX
        // 5. Rz(pi/2)
        let operations = vec![
            Operation::custom_gate("rz", vec![q], vec![half_pi.clone()]),
            Operation::custom_gate("sx", vec![q], vec![]),
            Operation::custom_gate("rz", vec![q], vec![theta_plus_pi]),
            Operation::custom_gate("sx", vec![q], vec![]),
            Operation::custom_gate("rz", vec![q], vec![half_pi]),
        ];

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: PI / 2.0,
        })
    }
}

/// Decomposes `Ry(theta)` preserving symbolic parameter expressions.
///
/// On `RZ-SX` targets:
/// $$R_y(\theta) = e^{i\pi/2} \left( R_z(\pi) \cdot SX \cdot R_z(\theta + \pi) \cdot SX \right)$$
#[derive(Clone, Copy, Debug, Default)]
pub struct SymbolicRyRule;

impl TranslationRule for SymbolicRyRule {
    fn name(&self) -> &'static str {
        "symbolic_ry_to_rz_sx"
    }

    fn target_gate(&self) -> &'static str {
        "ry"
    }

    fn quantum_arity(&self) -> usize {
        1
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        if !op.name().eq_ignore_ascii_case("ry")
            || op.qubits().len() != 1
            || op.parameters().is_empty()
        {
            return false;
        }
        let phys = PhysicalQubitId(op.qubits()[0].0 as u32);
        check_operation_legality_with_exprs(target, "ry", &[phys], op.parameters()).is_err()
            && target.is_operation_supported("rz")
            && target.is_operation_supported("sx")
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let q = op.qubits()[0];
        let theta = &op.parameters()[0];

        let pi_param = ParameterExpr::Constant(PI);
        let theta_plus_pi = ParameterExpr::Add(
            Box::new(theta.clone()),
            Box::new(ParameterExpr::Constant(PI)),
        );

        // Execution order:
        // 1. SX
        // 2. Rz(theta + pi)
        // 3. SX
        // 4. Rz(pi)
        let operations = vec![
            Operation::custom_gate("sx", vec![q], vec![]),
            Operation::custom_gate("rz", vec![q], vec![theta_plus_pi]),
            Operation::custom_gate("sx", vec![q], vec![]),
            Operation::custom_gate("rz", vec![q], vec![pi_param]),
        ];

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: PI / 2.0,
        })
    }
}

/// Decomposes `P(lambda)` / `U1(lambda)` into `Rz(lambda)` with global phase $\lambda/2$.
#[derive(Clone, Copy, Debug, Default)]
pub struct SymbolicPhaseRule;

impl TranslationRule for SymbolicPhaseRule {
    fn name(&self) -> &'static str {
        "symbolic_phase_to_rz"
    }

    fn target_gate(&self) -> &'static str {
        "p"
    }

    fn quantum_arity(&self) -> usize {
        1
    }

    fn matches(&self, op: &Operation, target: &Target) -> bool {
        let name = op.name();
        if (!name.eq_ignore_ascii_case("p") && !name.eq_ignore_ascii_case("u1"))
            || op.qubits().len() != 1
            || op.parameters().is_empty()
        {
            return false;
        }
        let phys = PhysicalQubitId(op.qubits()[0].0 as u32);
        check_operation_legality_with_exprs(target, name, &[phys], op.parameters()).is_err()
            && target.is_operation_supported("rz")
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let q = op.qubits()[0];
        let lambda = &op.parameters()[0];
        let operations = vec![Operation::custom_gate("rz", vec![q], vec![lambda.clone()])];

        let phase_delta = if let Some(val) = lambda.as_constant() {
            Angle::from_radians(val / 2.0).radians()
        } else {
            0.0
        };

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: phase_delta,
        })
    }
}

// ----------------------------------------------------------------------------
// Rule Registry
// ----------------------------------------------------------------------------

/// Registry of verified basis translation and decomposition rules.
pub struct TranslationRuleRegistry {
    rules: Vec<Box<dyn TranslationRule>>,
}

impl Default for TranslationRuleRegistry {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl TranslationRuleRegistry {
    /// Creates an empty rule registry.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates a registry pre-populated with all verified standard rules.
    pub fn with_default_rules() -> Self {
        let mut reg = Self::empty();
        reg.register(Box::new(SwapToCxRule));
        reg.register(Box::new(SwapToCxReversedRule));
        reg.register(Box::new(CxToCzRule));
        reg.register(Box::new(CzToCxRule));
        reg.register(Box::new(ToffoliToCxRule));
        reg.register(Box::new(SingleQubitSynthesisRule));
        reg.register(Box::new(SymbolicRxRule));
        reg.register(Box::new(SymbolicRyRule));
        reg.register(Box::new(SymbolicPhaseRule));
        reg
    }

    /// Registers a custom translation rule.
    pub fn register(&mut self, rule: Box<dyn TranslationRule>) {
        self.rules.push(rule);
    }

    /// Returns all registered rules that match `op` on `target`.
    pub fn find_rules_for_operation(
        &self,
        op: &Operation,
        target: &Target,
    ) -> Vec<&dyn TranslationRule> {
        self.rules
            .iter()
            .filter(|rule| rule.matches(op, target))
            .map(|rule| rule.as_ref())
            .collect()
    }

    /// Returns the number of registered rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns `true` if no rules are registered.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}
