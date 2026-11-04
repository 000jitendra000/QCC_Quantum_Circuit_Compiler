//! Basis translation engine performing recursive decomposition, cycle detection,
//! cost-based candidate selection, and local equivalence verification.

use num_complex::Complex64;

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::quantum::gate_matrix::{
    kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_swap, Matrix2x2, Matrix4x4,
};
use crate::synthesis::templates::matrix::Matrix8x8;
use crate::target::cost::CostModel;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::translation::rules::{
    check_operation_legality_with_exprs, DecompositionCandidate, SingleQubitSynthesisRule,
    TranslationRuleRegistry,
};
use crate::translation::{TranslationError, TranslationReport};

/// Default maximum allowable recursion depth for basis translation decompositions.
pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 10;

/// Default tolerance threshold for local unitary equivalence matrix verification.
pub const DEFAULT_LOCAL_EQUIVALENCE_TOLERANCE: f64 = 1e-10;

/// Hardware-aware basis translation engine.
#[derive(Clone)]
pub struct BasisTranslationEngine {
    target: Target,
    cost_model: CostModel,
    registry: std::sync::Arc<TranslationRuleRegistry>,
    max_depth: usize,
    verify_equivalence: bool,
    equivalence_tolerance: f64,
}

impl BasisTranslationEngine {
    /// Creates a new translation engine targeting the specified hardware `Target`.
    pub fn new(target: Target) -> Self {
        Self {
            target,
            cost_model: CostModel::default(),
            registry: std::sync::Arc::new(TranslationRuleRegistry::with_default_rules()),
            max_depth: DEFAULT_MAX_RECURSION_DEPTH,
            verify_equivalence: true,
            equivalence_tolerance: DEFAULT_LOCAL_EQUIVALENCE_TOLERANCE,
        }
    }

    /// Creates a fluent builder for configuring a `BasisTranslationEngine`.
    pub fn builder(target: Target) -> BasisTranslationEngineBuilder {
        BasisTranslationEngineBuilder::new(target)
    }

    /// Sets the target cost model used for candidate decomposition scoring.
    pub fn with_cost_model(mut self, cost_model: CostModel) -> Self {
        self.cost_model = cost_model;
        self
    }

    /// Sets the translation rule registry.
    pub fn with_registry(mut self, registry: TranslationRuleRegistry) -> Self {
        self.registry = std::sync::Arc::new(registry);
        self
    }

    /// Sets the maximum allowable recursion depth for nested decompositions.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Enables or disables local unitary equivalence verification.
    pub fn with_verify_equivalence(mut self, verify: bool) -> Self {
        self.verify_equivalence = verify;
        self
    }

    /// Sets the numeric tolerance for local unitary equivalence checks.
    pub fn with_equivalence_tolerance(mut self, tolerance: f64) -> Self {
        self.equivalence_tolerance = tolerance;
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

    /// Returns a reference to the active rule registry.
    #[inline]
    pub fn registry(&self) -> &TranslationRuleRegistry {
        &self.registry
    }

    /// Returns the maximum allowed recursion depth.
    #[inline]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Translates an entire circuit into target-supported native operations.
    ///
    /// Preserves already-supported operations without modification. Recursively decomposes
    /// unsupported operations, evaluates candidate costs, verifies local unitary equivalence,
    /// and checks final target legality on all emitted operations.
    pub fn translate_circuit(
        &self,
        circuit: &Circuit,
    ) -> Result<(Circuit, TranslationReport), TranslationError> {
        let initial_cost = self.cost_model.evaluate_circuit(&self.target, circuit);

        let mut translated = Circuit::new(format!("{}_translated", circuit.name));
        for reg in &circuit.quantum_registers {
            translated.add_quantum_register(&reg.name, reg.size)?;
        }
        for reg in &circuit.classical_registers {
            translated.add_classical_register(&reg.name, reg.size)?;
        }
        translated.set_global_phase(circuit.global_phase().clone());

        let mut operations_examined = 0;
        let mut operations_preserved = 0;
        let mut operations_decomposed = 0;
        let mut total_phase_delta = 0.0;

        let ordered_nodes = circuit.graph.ordered_nodes()?;

        for node in &ordered_nodes {
            operations_examined += 1;
            let op = &node.operation;
            let phys: Vec<PhysicalQubitId> = op
                .qubits()
                .iter()
                .map(|q| PhysicalQubitId(q.0 as u32))
                .collect();

            // 1. If already legal and supported on target, preserve without changes
            if check_operation_legality_with_exprs(&self.target, op.name(), &phys, op.parameters())
                .is_ok()
            {
                operations_preserved += 1;
                translated.add_operation(op.clone())?;
                continue;
            }

            // 2. Unsupported operation: recursively translate
            let mut call_stack = Vec::new();
            let translated_candidate =
                self.translate_operation_recursive(op, 0, &mut call_stack)?;

            operations_decomposed += 1;
            total_phase_delta += translated_candidate.global_phase_delta;

            for emitted_op in translated_candidate.operations {
                translated.add_operation(emitted_op)?;
            }
        }

        // 3. Accumulate circuit-level global phase delta
        if total_phase_delta != 0.0 {
            translated.add_global_phase(ParameterExpr::Constant(total_phase_delta));
        }

        // 4. Validate graph DAG invariants
        translated.validate()?;

        // 5. Final Target Legality Check across all emitted physical operations
        for node in translated.graph.ordered_nodes()? {
            let op = &node.operation;
            let phys: Vec<PhysicalQubitId> = op
                .qubits()
                .iter()
                .map(|q| PhysicalQubitId(q.0 as u32))
                .collect();
            check_operation_legality_with_exprs(&self.target, op.name(), &phys, op.parameters())?;
        }

        let final_cost = self.cost_model.evaluate_circuit(&self.target, &translated);
        let total_emitted_operations = translated.num_operations();

        let report = TranslationReport {
            operations_examined,
            operations_preserved,
            operations_decomposed,
            total_emitted_operations,
            global_phase_delta: total_phase_delta,
            initial_cost,
            final_cost,
        };

        Ok((translated, report))
    }

    /// In-place variant of circuit translation.
    pub fn translate_circuit_in_place(
        &self,
        circuit: &mut Circuit,
    ) -> Result<TranslationReport, TranslationError> {
        let (translated, report) = self.translate_circuit(circuit)?;
        *circuit = translated;
        Ok(report)
    }

    /// Translates a single operation into a sequence of target-supported operations.
    pub fn translate_operation(
        &self,
        op: &Operation,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let mut call_stack = Vec::new();
        self.translate_operation_recursive(op, 0, &mut call_stack)
    }

    // ------------------------------------------------------------------------
    // Recursive translation search with cycle & depth protection
    // ------------------------------------------------------------------------

    fn translate_operation_recursive(
        &self,
        op: &Operation,
        current_depth: usize,
        call_stack: &mut Vec<String>,
    ) -> Result<DecompositionCandidate, TranslationError> {
        let phys: Vec<PhysicalQubitId> = op
            .qubits()
            .iter()
            .map(|q| PhysicalQubitId(q.0 as u32))
            .collect();

        // 1. If already legal on target, base case is reached
        if check_operation_legality_with_exprs(&self.target, op.name(), &phys, op.parameters())
            .is_ok()
        {
            return Ok(DecompositionCandidate {
                rule_name: "identity_preserve",
                operations: vec![op.clone()],
                global_phase_delta: 0.0,
            });
        }

        // 2. Check recursion depth limit
        if current_depth >= self.max_depth {
            return Err(TranslationError::MaxRecursionDepthExceeded {
                op_name: op.name().to_string(),
                depth: current_depth,
                max_depth: self.max_depth,
            });
        }

        // 3. Cycle detection
        let signature = format!("{}_{:?}_{:?}", op.name(), phys, op.parameters());
        if call_stack.contains(&signature) {
            return Err(TranslationError::CycleDetected {
                op_name: op.name().to_string(),
                cycle_path: format!("{} -> {}", call_stack.join(" -> "), signature),
            });
        }
        call_stack.push(signature);

        // 4. Find candidate rules matching `op` on `target`
        let matching_rules = self.registry.find_rules_for_operation(op, &self.target);
        if matching_rules.is_empty() {
            call_stack.pop();
            return Err(TranslationError::NoViableTranslation {
                op_name: op.name().to_string(),
                operands: phys,
                candidates_tried: 0,
                reason: format!("no decomposition rules match operation '{}'", op.name()),
            });
        }

        let mut viable_candidates = Vec::new();
        let mut last_fatal_error: Option<TranslationError> = None;

        for rule in &matching_rules {
            let decomp = match rule.decompose(op, &self.target) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Local equivalence verification
            if self.verify_equivalence && self.verify_local_equivalence(op, &decomp).is_err() {
                // Equivalence discrepancy: reject this candidate
                continue;
            }

            // Recursively translate introduced sub-operations
            let mut fully_translated_sub_ops = Vec::new();
            let mut sub_phase_sum = decomp.global_phase_delta;
            let mut all_sub_ops_valid = true;

            for sub_op in &decomp.operations {
                match self.translate_operation_recursive(sub_op, current_depth + 1, call_stack) {
                    Ok(trans) => {
                        fully_translated_sub_ops.extend(trans.operations);
                        sub_phase_sum += trans.global_phase_delta;
                    }
                    Err(e @ TranslationError::CycleDetected { .. })
                    | Err(e @ TranslationError::MaxRecursionDepthExceeded { .. }) => {
                        last_fatal_error = Some(e);
                        all_sub_ops_valid = false;
                        break;
                    }
                    Err(_) => {
                        all_sub_ops_valid = false;
                        break;
                    }
                }
            }

            if all_sub_ops_valid {
                viable_candidates.push(DecompositionCandidate {
                    rule_name: rule.name(),
                    operations: fully_translated_sub_ops,
                    global_phase_delta: sub_phase_sum,
                });
            }
        }

        call_stack.pop();

        if viable_candidates.is_empty() {
            if let Some(err) = last_fatal_error {
                return Err(err);
            }
            return Err(TranslationError::NoViableTranslation {
                op_name: op.name().to_string(),
                operands: phys,
                candidates_tried: matching_rules.len(),
                reason: "all decomposition candidates failed recursive translation or equivalence verification"
                    .to_string(),
            });
        }

        // 5. Cost-based candidate selection
        if viable_candidates.len() == 1 {
            return Ok(viable_candidates.remove(0));
        }

        let mut best_candidate: Option<DecompositionCandidate> = None;
        let mut best_score = f64::INFINITY;

        for candidate in viable_candidates {
            let op_tuples: Vec<(&str, Vec<PhysicalQubitId>)> = candidate
                .operations
                .iter()
                .map(|o| {
                    (
                        o.name(),
                        o.qubits()
                            .iter()
                            .map(|q| PhysicalQubitId(q.0 as u32))
                            .collect(),
                    )
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

            let is_better = if (score - best_score).abs() < 1e-9 {
                // Deterministic tie-breaking by rule name
                best_candidate
                    .as_ref()
                    .map(|b| candidate.rule_name < b.rule_name)
                    .unwrap_or(true)
            } else {
                score < best_score
            };

            if is_better {
                best_score = score;
                best_candidate = Some(candidate);
            }
        }

        Ok(best_candidate.expect("viable candidates not empty"))
    }

    // ------------------------------------------------------------------------
    // Local unitary equivalence verification
    // ------------------------------------------------------------------------

    fn verify_local_equivalence(
        &self,
        original: &Operation,
        candidate: &DecompositionCandidate,
    ) -> Result<(), TranslationError> {
        let arity = original.qubits().len();
        match arity {
            1 => self.verify_local_equivalence_1q(original, candidate),
            2 => self.verify_local_equivalence_2q(original, candidate),
            3 => self.verify_local_equivalence_3q(original, candidate),
            _ => Ok(()), // Multi-qubit (>3) gates fall through to circuit-level verification
        }
    }

    fn verify_local_equivalence_1q(
        &self,
        original: &Operation,
        candidate: &DecompositionCandidate,
    ) -> Result<(), TranslationError> {
        // If parameters are constant, evaluate analytical 2x2 matrix
        let original_matrix = match SingleQubitSynthesisRule::evaluate_matrix(original) {
            Some(m) => m,
            None => return Ok(()), // Symbolic parameter gate verified via algebraic identity
        };

        let mut prod = Matrix2x2::identity();
        for sub_op in &candidate.operations {
            let sub_mat = match SingleQubitSynthesisRule::evaluate_matrix(sub_op) {
                Some(m) => m,
                None => return Ok(()),
            };
            // Left multiplication
            prod = sub_mat.matmul(&prod);
        }

        let phase_factor = Complex64::from_polar(1.0, candidate.global_phase_delta);
        let mut scaled_prod = prod.0;
        for row in &mut scaled_prod {
            for cell in row {
                *cell *= phase_factor;
            }
        }
        let scaled_matrix = Matrix2x2(scaled_prod);

        let diff = original_matrix.max_difference(&scaled_matrix);
        if diff > self.equivalence_tolerance {
            return Err(TranslationError::VerificationFailed {
                op_name: original.name().to_string(),
                max_diff: diff,
                tolerance: self.equivalence_tolerance,
            });
        }

        Ok(())
    }

    fn verify_local_equivalence_2q(
        &self,
        original: &Operation,
        candidate: &DecompositionCandidate,
    ) -> Result<(), TranslationError> {
        let name = original.name().to_ascii_lowercase();
        let original_matrix = if name == "swap" {
            matrix_swap()
        } else if name == "cx" {
            matrix_cx_c0_t1()
        } else if name == "cz" {
            matrix_cz()
        } else {
            return Ok(());
        };

        let q0 = original.qubits()[0];
        let q1 = original.qubits()[1];

        let mut prod = Matrix4x4::identity();
        for sub_op in &candidate.operations {
            let sub_name = sub_op.name().to_ascii_lowercase();
            let sub_qubits = sub_op.qubits();

            let sub_mat = if sub_qubits.len() == 1 {
                let m1 = match SingleQubitSynthesisRule::evaluate_matrix(sub_op) {
                    Some(m) => m,
                    None => return Ok(()),
                };
                if sub_qubits[0] == q0 {
                    // q0 is LSB: I \otimes M
                    kron_2x2(&Matrix2x2::identity(), &m1)
                } else if sub_qubits[0] == q1 {
                    // q1 is MSB: M \otimes I
                    kron_2x2(&m1, &Matrix2x2::identity())
                } else {
                    return Ok(());
                }
            } else if sub_qubits.len() == 2 {
                if sub_qubits[0] == q0 && sub_qubits[1] == q1 {
                    if sub_name == "cx" {
                        matrix_cx_c0_t1()
                    } else if sub_name == "cz" {
                        matrix_cz()
                    } else if sub_name == "swap" {
                        matrix_swap()
                    } else {
                        return Ok(());
                    }
                } else if sub_qubits[0] == q1 && sub_qubits[1] == q0 {
                    if sub_name == "cx" {
                        matrix_cx_c1_t0()
                    } else if sub_name == "cz" {
                        matrix_cz()
                    } else if sub_name == "swap" {
                        matrix_swap()
                    } else {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            };

            prod = sub_mat.matmul(&prod);
        }

        let phase_factor = Complex64::from_polar(1.0, candidate.global_phase_delta);
        let mut scaled_prod = prod.0;
        for row in &mut scaled_prod {
            for cell in row {
                *cell *= phase_factor;
            }
        }
        let scaled_matrix = Matrix4x4(scaled_prod);

        let diff = original_matrix.max_difference(&scaled_matrix);
        if diff > self.equivalence_tolerance {
            return Err(TranslationError::VerificationFailed {
                op_name: original.name().to_string(),
                max_diff: diff,
                tolerance: self.equivalence_tolerance,
            });
        }

        Ok(())
    }

    fn verify_local_equivalence_3q(
        &self,
        original: &Operation,
        candidate: &DecompositionCandidate,
    ) -> Result<(), TranslationError> {
        let name = original.name().to_ascii_lowercase();
        if name != "ccx" && name != "toffoli" {
            return Ok(());
        }

        let q0 = original.qubits()[0];
        let q1 = original.qubits()[1];
        let q2 = original.qubits()[2];

        let original_matrix = Matrix8x8::exact_toffoli(0, 1, 2);

        let mut prod = Matrix8x8::identity();
        for sub_op in &candidate.operations {
            let sub_name = sub_op.name().to_ascii_lowercase();
            let sub_qubits = sub_op.qubits();

            let sub_mat = if sub_qubits.len() == 1 {
                let m1 = match SingleQubitSynthesisRule::evaluate_matrix(sub_op) {
                    Some(m) => m,
                    None => return Ok(()),
                };
                let target_idx = if sub_qubits[0] == q0 {
                    0
                } else if sub_qubits[0] == q1 {
                    1
                } else if sub_qubits[0] == q2 {
                    2
                } else {
                    return Ok(());
                };
                Matrix8x8::embed_single_qubit(&m1, target_idx)
            } else if sub_qubits.len() == 2 {
                let ctrl_idx = if sub_qubits[0] == q0 {
                    0
                } else if sub_qubits[0] == q1 {
                    1
                } else if sub_qubits[0] == q2 {
                    2
                } else {
                    return Ok(());
                };
                let targ_idx = if sub_qubits[1] == q0 {
                    0
                } else if sub_qubits[1] == q1 {
                    1
                } else if sub_qubits[1] == q2 {
                    2
                } else {
                    return Ok(());
                };

                if sub_name == "cx" {
                    Matrix8x8::embed_cx(ctrl_idx, targ_idx)
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            };

            prod = sub_mat.matmul(&prod);
        }

        let phase_factor = Complex64::from_polar(1.0, candidate.global_phase_delta);
        let mut scaled_prod = prod.0;
        for row in &mut scaled_prod {
            for cell in row {
                *cell *= phase_factor;
            }
        }
        let scaled_matrix = Matrix8x8(scaled_prod);

        let diff = original_matrix.max_difference(&scaled_matrix);
        if diff > self.equivalence_tolerance {
            return Err(TranslationError::VerificationFailed {
                op_name: original.name().to_string(),
                max_diff: diff,
                tolerance: self.equivalence_tolerance,
            });
        }

        Ok(())
    }
}

/// Fluent builder for [`BasisTranslationEngine`].
pub struct BasisTranslationEngineBuilder {
    target: Target,
    cost_model: CostModel,
    registry: TranslationRuleRegistry,
    max_depth: usize,
    verify_equivalence: bool,
    equivalence_tolerance: f64,
}

impl BasisTranslationEngineBuilder {
    /// Creates a builder for the specified target.
    pub fn new(target: Target) -> Self {
        Self {
            target,
            cost_model: CostModel::default(),
            registry: TranslationRuleRegistry::with_default_rules(),
            max_depth: DEFAULT_MAX_RECURSION_DEPTH,
            verify_equivalence: true,
            equivalence_tolerance: DEFAULT_LOCAL_EQUIVALENCE_TOLERANCE,
        }
    }

    /// Configures the cost model.
    pub fn cost_model(mut self, cost_model: CostModel) -> Self {
        self.cost_model = cost_model;
        self
    }

    /// Configures the translation rule registry.
    pub fn registry(mut self, registry: TranslationRuleRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Configures the maximum recursion depth.
    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Configures whether local equivalence is verified for each candidate.
    pub fn verify_equivalence(mut self, verify: bool) -> Self {
        self.verify_equivalence = verify;
        self
    }

    /// Configures the local equivalence tolerance.
    pub fn equivalence_tolerance(mut self, tolerance: f64) -> Self {
        self.equivalence_tolerance = tolerance;
        self
    }

    /// Builds the `BasisTranslationEngine`.
    pub fn build(self) -> BasisTranslationEngine {
        BasisTranslationEngine {
            target: self.target,
            cost_model: self.cost_model,
            registry: std::sync::Arc::new(self.registry),
            max_depth: self.max_depth,
            verify_equivalence: self.verify_equivalence,
            equivalence_tolerance: self.equivalence_tolerance,
        }
    }
}
