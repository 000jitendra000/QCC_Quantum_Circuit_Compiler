//! Two-qubit direction correction rules, candidate selection, and verification.
//!
//! Corrects direction-sensitive entangling operations (such as $CX$) to satisfy hardware
//! connectivity and operand-direction constraints using verified Hadamard conjugation:
//!
//! $$CX(c, t) = (H_c \otimes H_t) \cdot CX(t, c) \cdot (H_c \otimes H_t)$$
//!
//! # Invariants
//! - **Coupling-Edge vs Control-Target**: Hardware coupling direction $(u, v)$ dictates allowed
//!   invocation order, while operand 0 is control and operand 1 is target in QccIR. Reversing the
//!   physical edge does NOT mean swapping control and target; Hadamards preserve the original semantics.
//! - **Symmetric Operation Preservation**: Symmetric operations ($CZ$, $SWAP$) are not subjected to
//!   unnecessary direction corrections or Hadamard insertions.
//! - **Explicit Failure**: If no valid coupling exists between the qubits in either orientation,
//!   compilation fails explicitly with [`DirectionError::NoViableDirection`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::quantum::gate_matrix::{kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_h};
use crate::target::cost::{CircuitCost, CostModel};
use crate::target::direction::{DirectionError, DirectionStatus};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::translation::engine::{BasisTranslationEngine, DEFAULT_LOCAL_EQUIVALENCE_TOLERANCE};
use crate::translation::rules::{
    check_operation_legality_with_exprs, DecompositionCandidate, TranslationRule,
};
use crate::translation::TranslationError;

// ============================================================================
// Matrix Verification Helpers
// ============================================================================

/// Verifies that the Hadamard-conjugated reverse CX matrix strictly equals $CX(0, 1)$ ($c=0, t=1$).
///
/// In little-endian indexing where $q_0$ is LSB:
/// $$(H \otimes H) \cdot CX(1, 0) \cdot (H \otimes H) = CX(0, 1)$$
pub fn verify_cx_reversal_matrix_c0_t1(tolerance: f64) -> Result<(), TranslationError> {
    let h = matrix_h();
    let h_both = kron_2x2(&h, &h);
    let cx_rev = matrix_cx_c1_t0();
    let prod = h_both.matmul(&cx_rev).matmul(&h_both);
    let expected = matrix_cx_c0_t1();

    let diff = expected.max_difference(&prod);
    if diff > tolerance {
        return Err(TranslationError::VerificationFailed {
            op_name: "cx_reversal_c0_t1".to_string(),
            max_diff: diff,
            tolerance,
        });
    }
    Ok(())
}

/// Verifies that the Hadamard-conjugated reverse CX matrix strictly equals $CX(1, 0)$ ($c=1, t=0$).
///
/// In little-endian indexing where $q_0$ is LSB:
/// $$(H \otimes H) \cdot CX(0, 1) \cdot (H \otimes H) = CX(1, 0)$$
pub fn verify_cx_reversal_matrix_c1_t0(tolerance: f64) -> Result<(), TranslationError> {
    let h = matrix_h();
    let h_both = kron_2x2(&h, &h);
    let cx_fwd = matrix_cx_c0_t1();
    let prod = h_both.matmul(&cx_fwd).matmul(&h_both);
    let expected = matrix_cx_c1_t0();

    let diff = expected.max_difference(&prod);
    if diff > tolerance {
        return Err(TranslationError::VerificationFailed {
            op_name: "cx_reversal_c1_t0".to_string(),
            max_diff: diff,
            tolerance,
        });
    }
    Ok(())
}

// ============================================================================
// Standard Direction Correction Rules
// ============================================================================

/// Canonical CX direction reversal rule using local Hadamards.
///
/// Replaces $CX(u, v)$ with $H(u); H(v); CX(v, u); H(u); H(v)$ when the target hardware
/// only permits the directed edge $(v, u)$.
#[derive(Clone, Copy, Debug, Default)]
pub struct CxDirectionCorrectionRule;

impl CxDirectionCorrectionRule {
    /// Numerical tolerance for unitary equivalence checks.
    pub const TOLERANCE: f64 = DEFAULT_LOCAL_EQUIVALENCE_TOLERANCE;

    /// Emits the raw Hadamard-conjugated sequence for reversing CX direction.
    pub fn expand_sequence(ctrl: QubitId, targ: QubitId) -> Vec<Operation> {
        vec![
            Operation::custom_gate("h", vec![ctrl], vec![]),
            Operation::custom_gate("h", vec![targ], vec![]),
            Operation::custom_gate("cx", vec![targ, ctrl], vec![]),
            Operation::custom_gate("h", vec![ctrl], vec![]),
            Operation::custom_gate("h", vec![targ], vec![]),
        ]
    }
}

impl TranslationRule for CxDirectionCorrectionRule {
    fn name(&self) -> &'static str {
        "cx_direction_reversal"
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

        let u = PhysicalQubitId(op.qubits()[0].0 as u32);
        let v = PhysicalQubitId(op.qubits()[1].0 as u32);

        // Matches only if forward direction is illegal, but reversed direction is legal
        target.check_direction("cx", u, v) == DirectionStatus::NeedsReversal
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let ctrl = op.qubits()[0];
        let targ = op.qubits()[1];
        let operations = Self::expand_sequence(ctrl, targ);

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: 0.0,
        })
    }
}

/// Commutation rule for symmetric $CZ$ gates on directed hardware targets.
///
/// Because $CZ(u, v) = CZ(v, u)$ is mathematically symmetric, if a directed target permits
/// $(v, u)$ but not $(u, v)$, the operands can be commuted directly without inserting Hadamards.
#[derive(Clone, Copy, Debug, Default)]
pub struct CzDirectionCommutationRule;

impl TranslationRule for CzDirectionCommutationRule {
    fn name(&self) -> &'static str {
        "cz_direction_commutation"
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

        let u = PhysicalQubitId(op.qubits()[0].0 as u32);
        let v = PhysicalQubitId(op.qubits()[1].0 as u32);

        // Only matches if forward tuple is illegal and reversed tuple is legal
        target.check_direction("cz", u, v) == DirectionStatus::NeedsReversal
    }

    fn decompose(
        &self,
        op: &Operation,
        _target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let u = op.qubits()[0];
        let v = op.qubits()[1];
        let operations = vec![Operation::custom_gate("cz", vec![v, u], vec![])];

        Ok(DecompositionCandidate {
            rule_name: self.name(),
            operations,
            global_phase_delta: 0.0,
        })
    }
}

// ============================================================================
// Target-Specific Direction Correction Rule Trait & Registry
// ============================================================================

/// Trait for target-specific direction correction alternatives.
pub trait TargetDirectionRule: Send + Sync {
    /// Canonical rule identifier.
    fn name(&self) -> &'static str;

    /// Gate mnemonic corrected by this rule (e.g. `"cx"`).
    fn target_gate(&self) -> &'static str;

    /// Returns `true` if this rule can correct `op` on `target`.
    fn matches(&self, op: &Operation, target: &Target) -> bool;

    /// Generates the correction decomposition candidate.
    fn correct(
        &self,
        op: &Operation,
        target: &Target,
    ) -> Result<DecompositionCandidate, TranslationError>;
}

/// Registry holding direction correction rules, including standard and target-specific alternatives.
pub struct DirectionCorrectionRegistry {
    rules: Vec<Arc<dyn TranslationRule>>,
}

impl Default for DirectionCorrectionRegistry {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl DirectionCorrectionRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Pre-populates the registry with the verified standard CX reversal and CZ commutation rules.
    pub fn with_default_rules() -> Self {
        let mut reg = Self::empty();
        reg.register(Arc::new(CxDirectionCorrectionRule));
        reg.register(Arc::new(CzDirectionCommutationRule));
        reg
    }

    /// Registers a translation rule into the registry.
    pub fn register(&mut self, rule: Arc<dyn TranslationRule>) {
        self.rules.push(rule);
    }

    /// Registers a target-specific direction correction rule.
    pub fn register_alternative(&mut self, rule: Arc<dyn TargetDirectionRule>) {
        struct TargetDirectionRuleAdapter(Arc<dyn TargetDirectionRule>);

        impl TranslationRule for TargetDirectionRuleAdapter {
            fn name(&self) -> &'static str {
                self.0.name()
            }

            fn target_gate(&self) -> &'static str {
                self.0.target_gate()
            }

            fn quantum_arity(&self) -> usize {
                2
            }

            fn matches(&self, op: &Operation, target: &Target) -> bool {
                self.0.matches(op, target)
            }

            fn decompose(
                &self,
                op: &Operation,
                target: &Target,
            ) -> Result<DecompositionCandidate, TranslationError> {
                self.0.correct(op, target)
            }
        }

        self.rules.push(Arc::new(TargetDirectionRuleAdapter(rule)));
    }

    /// Finds all matching direction correction rules for an operation on a target.
    pub fn find_rules_for_operation(
        &self,
        op: &Operation,
        target: &Target,
    ) -> Vec<Arc<dyn TranslationRule>> {
        self.rules
            .iter()
            .filter(|r| r.matches(op, target))
            .cloned()
            .collect()
    }
}

// ============================================================================
// TwoQubitDirectionCorrector & Pass
// ============================================================================

/// Telemetry report produced after running two-qubit direction correction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectionCorrectionReport {
    /// Number of operations analyzed.
    pub operations_examined: usize,
    /// Number of operations that were already directional/legal and preserved.
    pub operations_preserved: usize,
    /// Number of direction-sensitive operations corrected.
    pub operations_corrected: usize,
    /// Total count of operations emitted in output circuit.
    pub total_emitted_operations: usize,
    /// Evaluated circuit cost before correction.
    pub initial_cost: CircuitCost,
    /// Evaluated circuit cost after correction.
    pub final_cost: CircuitCost,
}

/// Standalone compiler engine for correcting two-qubit direction constraints.
pub struct TwoQubitDirectionCorrector {
    target: Target,
    cost_model: CostModel,
    registry: Arc<DirectionCorrectionRegistry>,
    basis_engine: BasisTranslationEngine,
}

impl TwoQubitDirectionCorrector {
    /// Creates a new direction corrector for the specified `Target`.
    pub fn new(target: Target) -> Self {
        let basis_engine = BasisTranslationEngine::new(target.clone());
        Self {
            target,
            cost_model: CostModel::default(),
            registry: Arc::new(DirectionCorrectionRegistry::with_default_rules()),
            basis_engine,
        }
    }

    /// Sets the cost model used to score candidate correction alternatives.
    pub fn with_cost_model(mut self, cost_model: CostModel) -> Self {
        self.cost_model = cost_model;
        self
    }

    /// Sets the direction correction rule registry.
    pub fn with_registry(mut self, registry: DirectionCorrectionRegistry) -> Self {
        self.registry = Arc::new(registry);
        self
    }

    /// Returns a reference to the compilation target.
    #[inline]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Returns a reference to the active cost model.
    #[inline]
    pub fn cost_model(&self) -> &CostModel {
        &self.cost_model
    }

    /// Corrects all direction-sensitive operations in `circuit` to satisfy `target` connectivity.
    ///
    /// Preserves already legal operations. If an illegal orientation has a valid reversal,
    /// evaluates candidates using the cost model, lowers introduced single-qubit gates
    /// into native target gates if necessary, and validates target legality.
    ///
    /// # Errors
    /// Returns [`DirectionError::NoViableDirection`] if an operation cannot be satisfied in either
    /// orientation, or if target legality fails.
    pub fn correct_circuit(
        &self,
        circuit: &Circuit,
    ) -> Result<(Circuit, DirectionCorrectionReport), DirectionError> {
        let initial_cost = self.cost_model.evaluate_circuit(&self.target, circuit);

        let mut corrected = Circuit::new(format!("{}_dir_corrected", circuit.name));
        for reg in &circuit.quantum_registers {
            corrected
                .add_quantum_register(&reg.name, reg.size)
                .map_err(|e| {
                    DirectionError::Legality(format!("failed to add quantum register: {e}"))
                })?;
        }
        for reg in &circuit.classical_registers {
            corrected
                .add_classical_register(&reg.name, reg.size)
                .map_err(|e| {
                    DirectionError::Legality(format!("failed to add classical register: {e}"))
                })?;
        }
        corrected.set_global_phase(circuit.global_phase().clone());

        let mut operations_examined = 0;
        let mut operations_preserved = 0;
        let mut operations_corrected = 0;

        let ordered_nodes = circuit
            .graph
            .ordered_nodes()
            .map_err(|e| DirectionError::Legality(format!("failed to order circuit nodes: {e}")))?;

        for node in &ordered_nodes {
            operations_examined += 1;
            let op = &node.operation;
            let phys: Vec<PhysicalQubitId> = op
                .qubits()
                .iter()
                .map(|q| PhysicalQubitId(q.0 as u32))
                .collect();

            // 1. If single-qubit, non-unitary, or already legal on target: preserve untouched
            if op.qubits().len() != 2
                || check_operation_legality_with_exprs(
                    &self.target,
                    op.name(),
                    &phys,
                    op.parameters(),
                )
                .is_ok()
            {
                operations_preserved += 1;
                corrected.add_operation(op.clone()).map_err(|e| {
                    DirectionError::Legality(format!("failed to add preserved operation: {e}"))
                })?;
                continue;
            }

            let u = phys[0];
            let v = phys[1];

            // 2. Check if hardware supports this operation between (u, v) in either orientation
            let dir_status = self.target.check_direction(op.name(), u, v);
            if dir_status == DirectionStatus::NoCoupling {
                return Err(DirectionError::NoViableDirection {
                    operation: op.name().to_string(),
                    u,
                    v,
                    target: self.target.name.clone(),
                    reason:
                        "no physical coupling edge exists between these qubits in either direction"
                            .to_string(),
                });
            }

            // 3. Find matching direction correction rules
            let matching_rules = self.registry.find_rules_for_operation(op, &self.target);
            if matching_rules.is_empty() {
                return Err(DirectionError::NoViableDirection {
                    operation: op.name().to_string(),
                    u,
                    v,
                    target: self.target.name.clone(),
                    reason: format!(
                        "operation '{op_name}' direction is illegal and no correction rule matches (status: {dir_status:?})",
                        op_name = op.name()
                    ),
                });
            }

            // 4. Generate candidate sequences and evaluate costs
            let mut candidates = Vec::new();
            for rule in &matching_rules {
                let cand = rule.decompose(op, &self.target).map_err(|e| {
                    DirectionError::NoViableDirection {
                        operation: op.name().to_string(),
                        u,
                        v,
                        target: self.target.name.clone(),
                        reason: format!("rule '{}' failed to decompose: {e}", rule.name()),
                    }
                })?;

                // If candidate contains gates not supported natively on target (e.g. H on RzSx machine),
                // lower them via the basis engine so emitted operations are valid target operations.
                let mut lowered_ops = Vec::new();
                for sub_op in &cand.operations {
                    let sub_phys: Vec<PhysicalQubitId> = sub_op
                        .qubits()
                        .iter()
                        .map(|q| PhysicalQubitId(q.0 as u32))
                        .collect();
                    if check_operation_legality_with_exprs(
                        &self.target,
                        sub_op.name(),
                        &sub_phys,
                        sub_op.parameters(),
                    )
                    .is_ok()
                    {
                        lowered_ops.push(sub_op.clone());
                    } else {
                        let sub_cand = self.basis_engine.translate_operation(sub_op).map_err(|e| {
                            DirectionError::NoViableDirection {
                                operation: op.name().to_string(),
                                u,
                                v,
                                target: self.target.name.clone(),
                                reason: format!("failed to lower sub-operation '{sub_name}' in direction sequence: {e}", sub_name = sub_op.name()),
                            }
                        })?;
                        lowered_ops.extend(sub_cand.operations);
                    }
                }

                let op_tuples: Vec<(&str, Vec<PhysicalQubitId>)> = lowered_ops
                    .iter()
                    .map(|sub_op| {
                        let sub_phys: Vec<PhysicalQubitId> = sub_op
                            .qubits()
                            .iter()
                            .map(|q| PhysicalQubitId(q.0 as u32))
                            .collect();
                        (sub_op.name(), sub_phys)
                    })
                    .collect();

                let op_refs: Vec<(&str, &[PhysicalQubitId])> = op_tuples
                    .iter()
                    .map(|(name, q_slice)| (*name, q_slice.as_slice()))
                    .collect();

                let seq_cost = self
                    .cost_model
                    .evaluate_physical_sequence(&self.target, op_refs);
                let fallback_score = (seq_cost.total_gates as f64)
                    + 10.0 * (seq_cost.two_qubit_gates as f64)
                    + 2.0 * (seq_cost.depth as f64)
                    + 5.0 * (seq_cost.two_qubit_depth as f64);
                let score = self
                    .cost_model
                    .calculate_score(&seq_cost)
                    .unwrap_or(fallback_score);

                candidates.push((rule.name(), lowered_ops, score));
            }

            // 5. Select candidate with lowest cost (deterministic tie-breaking by rule_name)
            candidates.sort_by(|a, b| {
                a.2.partial_cmp(&b.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(b.0))
            });

            let (_best_rule, best_ops, _best_score) = candidates.remove(0);
            operations_corrected += 1;

            for emitted_op in best_ops {
                corrected.add_operation(emitted_op).map_err(|e| {
                    DirectionError::Legality(format!("failed to add corrected operation: {e}"))
                })?;
            }
        }

        // 6. Validate DAG invariants
        corrected
            .validate()
            .map_err(|e| DirectionError::Legality(format!("DAG validation failed: {e}")))?;

        // 7. Validate target legality on all emitted operations
        for node in corrected.graph.ordered_nodes().map_err(|e| {
            DirectionError::Legality(format!("failed to traverse nodes for legality: {e}"))
        })? {
            let op = &node.operation;
            let phys: Vec<PhysicalQubitId> = op
                .qubits()
                .iter()
                .map(|q| PhysicalQubitId(q.0 as u32))
                .collect();
            check_operation_legality_with_exprs(&self.target, op.name(), &phys, op.parameters())
                .map_err(|e| {
                    DirectionError::Legality(format!(
                        "operation '{op_name}' failed target legality check after direction correction: {e}",
                        op_name = op.name()
                    ))
                })?;
        }

        let final_cost = self.cost_model.evaluate_circuit(&self.target, &corrected);
        let total_emitted_operations = corrected.num_operations();

        let report = DirectionCorrectionReport {
            operations_examined,
            operations_preserved,
            operations_corrected,
            total_emitted_operations,
            initial_cost,
            final_cost,
        };

        Ok((corrected, report))
    }

    /// In-place variant of direction correction.
    pub fn correct_circuit_in_place(
        &self,
        circuit: &mut Circuit,
    ) -> Result<DirectionCorrectionReport, DirectionError> {
        let (corrected, report) = self.correct_circuit(circuit)?;
        *circuit = corrected;
        Ok(report)
    }
}

// ============================================================================
// TwoQubitDirectionPass (PassManager Integration)
// ============================================================================

/// Compiler pass executing two-qubit direction correction within a [`crate::opt::pass_manager::PassManager`].
pub struct TwoQubitDirectionPass {
    corrector: TwoQubitDirectionCorrector,
    domain: SemanticDomain,
    last_report: Option<DirectionCorrectionReport>,
}

impl TwoQubitDirectionPass {
    /// Creates a new `TwoQubitDirectionPass` for `target`.
    pub fn new(target: Target) -> Self {
        Self {
            corrector: TwoQubitDirectionCorrector::new(target),
            domain: SemanticDomain::GeneralStructural,
            last_report: None,
        }
    }

    /// Configures the pass with a custom cost model.
    pub fn with_cost_model(mut self, cost_model: CostModel) -> Self {
        self.corrector = self.corrector.with_cost_model(cost_model);
        self
    }

    /// Returns the last direction correction report, if any.
    pub fn last_report(&self) -> Option<&DirectionCorrectionReport> {
        self.last_report.as_ref()
    }
}

impl CompilerPass for TwoQubitDirectionPass {
    fn id(&self) -> PassId {
        PassId::new("two_qubit_direction")
    }

    fn description(&self) -> &'static str {
        "Corrects direction-sensitive entangling operations to satisfy hardware target connectivity constraints using verified Hadamard identities."
    }

    fn semantic_domain(&self) -> SemanticDomain {
        self.domain
    }

    fn mutation_scope(&self) -> PassMutation {
        PassMutation::CircuitTopology
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let report = self
            .corrector
            .correct_circuit_in_place(circuit)
            .map_err(|err| PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: format!("direction correction failed: {err}"),
            })?;

        let modified = report.operations_corrected > 0;
        let summary = format!(
            "examined {} ops, preserved {}, corrected {} (total emitted: {})",
            report.operations_examined,
            report.operations_preserved,
            report.operations_corrected,
            report.total_emitted_operations
        );

        self.last_report = Some(report);

        if modified {
            Ok(PassResult::modified(summary))
        } else {
            Ok(PassResult::unmodified())
        }
    }
}
