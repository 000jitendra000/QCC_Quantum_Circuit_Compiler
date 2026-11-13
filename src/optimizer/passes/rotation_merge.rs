//! Same-axis rotation merging compiler pass.
//!
//! # Semantics & Periodicity Contracts
//!
//! 1. **Same-Axis Rotation Composition**:
//!    - For continuous single-qubit rotations $R_x(\theta)$, $R_y(\theta)$, $R_z(\theta)$ along
//!      axis $\hat{n} \in \{X, Y, Z\}$:
//!      $$R_{\hat{n}}(\theta_2) \cdot R_{\hat{n}}(\theta_1) = R_{\hat{n}}(\theta_1 + \theta_2)$$
//!    - Same-axis rotations on the same qubit wire commute and add their rotation angles.
//!    - Rotations along different axes ($[X, Y] \neq 0$, $[Y, Z] \neq 0$, $[Z, X] \neq 0$) do not
//!      commute and cannot be merged without basis translation or Euler decomposition (Phase 31).
//!
//! 2. **DAG Wire Adjacency & Intervening Operations**:
//!    - Candidate rotation pairs must be directly adjacent on their shared quantum wire in the QccIR DAG.
//!    - Any intervening operation on the same qubit wire (such as a different gate, a multi-qubit gate,
//!      a measurement, a reset, a barrier, or a classical hazard) strictly prevents merging.
//!    - Operations on disjoint qubit wires commute in the DAG and do not block adjacency on the target wire.
//!
//! 3. **$4\pi$ Identity Periodicity & $2\pi$ Global Phase**:
//!    - Single-qubit rotations double-cover $SO(3)$ with period $4\pi$:
//!      $$R_{\hat{n}}(\theta + 4\pi) = R_{\hat{n}}(\theta)$$
//!    - Shifting by an odd multiple of $2\pi$ introduces an overall factor of $-I = e^{i\pi} I$:
//!      $$R_{\hat{n}}(\theta + 2\pi) = -R_{\hat{n}}(\theta) = e^{i\pi} R_{\hat{n}}(\theta)$$
//!    - When normalizing continuous rotation angles into $[0, 2\pi)$, any reduction by an odd multiple
//!      of $2\pi$ **must** contribute a $+\pi$ adjustment to the circuit's explicit scalar global phase.
//!
//! 4. **Identity Elimination**:
//!    - Resulting rotations with angle $\theta \equiv 0 \pmod{4\pi}$ evaluate to the exact identity $I$
//!      and are eliminated with zero global phase impact.
//!    - Resulting rotations with angle $\theta \equiv 2\pi \pmod{4\pi}$ evaluate to $-I = e^{i\pi} I$
//!      and are eliminated with a $+\pi$ global phase adjustment.
//!
//! 5. **Symbolic Parameter Preservation**:
//!    - Parameter expressions with unbound symbols are **never** assumed to be zero or constants.
//!    - When symbols are bound in the runtime environment, they evaluate to concrete numeric angles.
//!    - Unbound symbolic opposites ($p$ and $-p$) safely cancel algebraically to exact zero.
//!    - Under [`SymbolicMergePolicy::CombineSymbolic`], general symbolic expressions combine into
//!      algebraic sums ([`ParameterExpr::Add`]) while preserving symbols intact.
//!    - Under [`SymbolicMergePolicy::PreserveUnresolved`], unresolved symbolic expressions are left
//!      unchanged as separate operations.
//!
//! 6. **Transactional DAG Rewriting**:
//!    - Mutations are executed atomically through [`CircuitTransaction`].
//!    - Merged operations retain provenance tracing back to the merged original operations.
//!    - DAG acyclicity and structural invariants are validated on transaction commit.

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::ir::circuit::Circuit;
use crate::ir::graph::CircuitGraph;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::rewrite::CircuitTransaction;
use crate::ir::wire::{QubitId, WireId};
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::optimizer::algebra::{compose_rotation_angles, simplify_parameter_expr};
use crate::quantum::angle::{normalize_rotation_angle, RotationNormalizationMode};

/// Default angular comparison tolerance for rotation parameter comparisons.
pub const DEFAULT_ROTATION_TOLERANCE: f64 = 1e-12;

/// Default maximum number of fixed-point iterations.
pub const DEFAULT_MAX_ITERATIONS: usize = 1000;

/// Identifies the spatial rotation axis of a single-qubit rotation gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RotationAxis {
    /// Pauli-X rotation axis ($R_x$).
    X,
    /// Pauli-Y rotation axis ($R_y$).
    Y,
    /// Pauli-Z rotation axis ($R_z$).
    Z,
}

impl RotationAxis {
    /// Returns the canonical gate name string corresponding to this axis.
    #[inline]
    pub const fn gate_name(&self) -> &'static str {
        match self {
            Self::X => "rx",
            Self::Y => "ry",
            Self::Z => "rz",
        }
    }

    /// Parses a gate name string into a `RotationAxis` if it is a supported rotation.
    pub fn from_gate_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "rx" => Some(Self::X),
            "ry" => Some(Self::Y),
            "rz" => Some(Self::Z),
            _ => None,
        }
    }
}

/// Strategy for handling rotation gate pairs containing unbound symbolic parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SymbolicMergePolicy {
    /// Unresolved symbolic parameters remain unchanged as separate operations.
    ///
    /// Matches the specification definition of done: "Supported rotations merge correctly
    /// and unresolved symbolic cases remain unchanged."
    #[default]
    PreserveUnresolved,
    /// Combines symbolic expressions using algebraic addition ([`ParameterExpr::Add`]),
    /// preserving symbolic variables without normalizing or assuming zero.
    CombineSymbolic,
}

/// Configuration options for the same-axis rotation merge compiler pass.
#[derive(Clone, Debug, PartialEq)]
pub struct RotationMergeConfig {
    /// Angular comparison tolerance for floating-point rotation parameters.
    pub tolerance: f64,
    /// Maximum number of fixed-point iterations until convergence.
    pub max_iterations: usize,
    /// Whether to eliminate resulting exact zero identities ($\theta \equiv 0 \pmod{4\pi}$).
    pub eliminate_exact_zero: bool,
    /// Whether to eliminate resulting phase zero identities ($\theta \equiv 2\pi \pmod{4\pi}$) with $+\pi$ phase adjustment.
    pub eliminate_phase_zero: bool,
    /// Angle normalization mode: canonical $[0, 2\pi)$ with phase or exact $[0, 4\pi)$.
    pub normalization_mode: RotationNormalizationMode,
    /// Policy for handling parameter expressions with unbound symbols.
    pub symbolic_policy: SymbolicMergePolicy,
    /// Optional runtime symbol environment to evaluate bound parameter symbols.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for RotationMergeConfig {
    fn default() -> Self {
        Self {
            tolerance: DEFAULT_ROTATION_TOLERANCE,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            eliminate_exact_zero: true,
            eliminate_phase_zero: true,
            normalization_mode: RotationNormalizationMode::TwoPiWithPhase,
            symbolic_policy: SymbolicMergePolicy::PreserveUnresolved,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry statistics recorded during same-axis rotation merging.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotationMergeStats {
    /// Total candidate adjacent rotation pairs examined in the DAG across all iterations.
    pub candidate_pairs_examined: usize,
    /// Total rotation pairs successfully merged.
    pub rotations_merged: usize,
    /// Rotation pairs merged into a combined non-zero rotation.
    pub non_zero_rotations_fused: usize,
    /// Exact zero identities eliminated ($\theta \equiv 0 \pmod{4\pi}$).
    pub exact_zero_identities_eliminated: usize,
    /// Phase zero identities eliminated ($\theta \equiv 2\pi \pmod{4\pi}$) with $+\pi$ global phase adjustment.
    pub phase_zero_identities_eliminated: usize,
    /// Symbolic rotation pairs merged algebraically or into combined expressions.
    pub symbolic_pairs_merged: usize,
    /// Candidate pairs rejected (different axis, different qubit, intervening op, or unsupported symbols).
    pub candidate_pairs_rejected: usize,
    /// Total operations removed from the DAG during the pass.
    pub operations_removed: usize,
    /// Total new fused operations added to the DAG during the pass.
    pub operations_added: usize,
    /// Global phase adjustments applied to the circuit.
    pub global_phase_adjustments: usize,
    /// Total fixed-point iterations performed.
    pub iterations: usize,
}

impl RotationMergeStats {
    /// Returns the net reduction in total circuit operations.
    #[inline]
    pub fn net_gate_reduction(&self) -> usize {
        self.operations_removed
            .saturating_sub(self.operations_added)
    }

    /// Returns the total number of resulting identities eliminated.
    #[inline]
    pub fn total_identities_eliminated(&self) -> usize {
        self.exact_zero_identities_eliminated + self.phase_zero_identities_eliminated
    }
}

/// A compiler pass that combines adjacent $R_x$, $R_y$, or $R_z$ operations on the same qubit wire in QccIR circuits.
///
/// Preserves exact circuit quantum semantics, including explicit circuit global phase.
#[derive(Clone, Debug)]
pub struct RotationMergePass {
    config: RotationMergeConfig,
    domain: SemanticDomain,
    last_stats: Option<RotationMergeStats>,
}

impl Default for RotationMergePass {
    fn default() -> Self {
        Self::new()
    }
}

impl RotationMergePass {
    /// Creates a new rotation merge pass with default configuration.
    pub fn new() -> Self {
        Self {
            config: RotationMergeConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates a rotation merge pass with custom configuration options.
    pub fn with_config(config: RotationMergeConfig) -> Self {
        Self {
            config,
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Sets the semantic domain for this pass.
    pub fn with_domain(mut self, domain: SemanticDomain) -> Self {
        self.domain = domain;
        self
    }

    /// Sets the angular tolerance for continuous rotation parameter comparisons.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.config.tolerance = tolerance;
        self
    }

    /// Sets the maximum number of fixed-point iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Sets the policy for merging symbolic parameters.
    pub fn with_symbolic_policy(mut self, policy: SymbolicMergePolicy) -> Self {
        self.config.symbolic_policy = policy;
        self
    }

    /// Sets the runtime symbol evaluation environment.
    pub fn with_symbol_env(mut self, symbol_env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = symbol_env;
        self
    }

    /// Sets the angle normalization mode.
    pub fn with_normalization_mode(mut self, mode: RotationNormalizationMode) -> Self {
        self.config.normalization_mode = mode;
        self
    }

    /// Configures whether to eliminate resulting exact zero identities.
    pub fn with_eliminate_exact_zero(mut self, eliminate: bool) -> Self {
        self.config.eliminate_exact_zero = eliminate;
        self
    }

    /// Configures whether to eliminate resulting phase zero identities ($2\pi$ rotations).
    pub fn with_eliminate_phase_zero(mut self, eliminate: bool) -> Self {
        self.config.eliminate_phase_zero = eliminate;
        self
    }

    /// Returns a reference to the active configuration.
    #[inline]
    pub fn config(&self) -> &RotationMergeConfig {
        &self.config
    }

    /// Returns telemetry statistics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<RotationMergeStats> {
        self.last_stats
    }
}

/// Action to perform for a candidate rotation pair.
enum MergeAction {
    /// Eliminate both operations as an exact zero identity ($\theta \equiv 0 \pmod{4\pi}$).
    EliminateExactZero,
    /// Eliminate both operations as a phase zero identity ($\theta \equiv 2\pi \pmod{4\pi}$) with $+\pi$ phase adjustment.
    EliminatePhaseZero,
    /// Fuse both operations into a single combined rotation with the specified parameter and phase adjustment.
    Fuse {
        fused_op: Operation,
        phase_adjustment: f64,
        is_symbolic: bool,
    },
}

impl CompilerPass for RotationMergePass {
    fn id(&self) -> PassId {
        PassId::new("rotation_merge")
    }

    fn description(&self) -> &'static str {
        "Combines adjacent same-axis Rx, Ry, or Rz operations on the same qubit while preserving quantum semantics and global phase."
    }

    fn config_summary(&self) -> std::collections::BTreeMap<String, serde_json::Value> {
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "tolerance".to_string(),
            serde_json::json!(self.config.tolerance),
        );
        map.insert(
            "symbolic_policy".to_string(),
            serde_json::json!(format!("{:?}", self.config.symbolic_policy)),
        );
        map
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
        let mut stats = RotationMergeStats::default();
        let mut rejected_pairs: HashSet<(OperationId, OperationId)> = HashSet::new();

        let mut tx = CircuitTransaction::begin(circuit);
        let mut iteration = 0;
        let mut modified = false;

        while iteration < self.config.max_iterations {
            iteration += 1;

            let ordered_nodes =
                tx.circuit()
                    .graph
                    .ordered_nodes()
                    .map_err(|source| PassError::Ir {
                        pass_id: self.id(),
                        source,
                    })?;

            let mut used_ids: HashSet<OperationId> = HashSet::new();
            let mut actions: Vec<(OperationId, OperationId, MergeAction)> = Vec::new();

            for node in &ordered_nodes {
                if used_ids.contains(&node.id) {
                    continue;
                }

                let (axis1, q1, param1) = match extract_rotation(&node.operation) {
                    Some(res) => res,
                    None => continue,
                };

                // Find direct wire-adjacent successor on the same quantum wire
                let partner_id = match find_adjacent_wire_partner(&tx.circuit().graph, node, q1) {
                    Some(id) => id,
                    None => continue,
                };

                if used_ids.contains(&partner_id) {
                    continue;
                }

                let partner_node = match tx.circuit().graph.get_node(partner_id) {
                    Some(n) => n,
                    None => continue,
                };

                // Check that partner is a rotation on the SAME axis and SAME qubit
                let (axis2, q2, param2) = match extract_rotation(&partner_node.operation) {
                    Some(res) => res,
                    None => {
                        rejected_pairs.insert((node.id, partner_id));
                        continue;
                    }
                };

                if axis1 != axis2 || q1 != q2 {
                    rejected_pairs.insert((node.id, partner_id));
                    continue;
                }

                // Both nodes are adjacent same-axis rotations on the same wire
                let action = match self.plan_merge(axis1, q1, param1, param2) {
                    Some(act) => act,
                    None => {
                        // Unresolved symbolic or unsupported combination under current policy
                        rejected_pairs.insert((node.id, partner_id));
                        continue;
                    }
                };

                used_ids.insert(node.id);
                used_ids.insert(partner_id);
                actions.push((node.id, partner_id, action));
            }

            if actions.is_empty() {
                break;
            }

            modified = true;
            for (id_a, id_b, action) in actions {
                stats.rotations_merged += 1;

                match action {
                    MergeAction::EliminateExactZero => {
                        tx.remove_operation(id_a)
                            .map_err(|err| PassError::ExecutionFailed {
                                pass_id: self.id(),
                                reason: format!("failed to remove operation {id_a}: {err}"),
                            })?;
                        tx.remove_operation(id_b)
                            .map_err(|err| PassError::ExecutionFailed {
                                pass_id: self.id(),
                                reason: format!("failed to remove operation {id_b}: {err}"),
                            })?;
                        stats.exact_zero_identities_eliminated += 1;
                        stats.operations_removed += 2;
                    }
                    MergeAction::EliminatePhaseZero => {
                        tx.remove_operation(id_a)
                            .map_err(|err| PassError::ExecutionFailed {
                                pass_id: self.id(),
                                reason: format!("failed to remove operation {id_a}: {err}"),
                            })?;
                        tx.remove_operation(id_b)
                            .map_err(|err| PassError::ExecutionFailed {
                                pass_id: self.id(),
                                reason: format!("failed to remove operation {id_b}: {err}"),
                            })?;
                        tx.add_global_phase(ParameterExpr::Constant(PI));
                        stats.phase_zero_identities_eliminated += 1;
                        stats.operations_removed += 2;
                        stats.global_phase_adjustments += 1;
                    }
                    MergeAction::Fuse {
                        fused_op,
                        phase_adjustment,
                        is_symbolic,
                    } => {
                        tx.fuse_operations(&[id_a, id_b], fused_op).map_err(|err| {
                            PassError::ExecutionFailed {
                                pass_id: self.id(),
                                reason: format!(
                                    "failed to fuse operations {id_a} and {id_b}: {err}"
                                ),
                            }
                        })?;
                        if phase_adjustment.abs() > 1e-15 {
                            tx.add_global_phase(ParameterExpr::Constant(phase_adjustment));
                            stats.global_phase_adjustments += 1;
                        }
                        if is_symbolic {
                            stats.symbolic_pairs_merged += 1;
                        }
                        stats.non_zero_rotations_fused += 1;
                        stats.operations_removed += 2;
                        stats.operations_added += 1;
                    }
                }
            }
        }

        stats.iterations = iteration;
        stats.candidate_pairs_rejected = rejected_pairs.len();
        stats.candidate_pairs_examined = stats.rotations_merged + stats.candidate_pairs_rejected;

        if !modified {
            self.last_stats = Some(stats);
            return Ok(PassResult::unmodified());
        }

        tx.commit().map_err(|err| PassError::ExecutionFailed {
            pass_id: self.id(),
            reason: format!("transaction commit failed: {err}"),
        })?;

        self.last_stats = Some(stats);
        Ok(PassResult::modified(format!(
            "merged {} rotation pairs ({} fused, {} exact zero eliminated, {} phase zero eliminated)",
            stats.rotations_merged,
            stats.non_zero_rotations_fused,
            stats.exact_zero_identities_eliminated,
            stats.phase_zero_identities_eliminated,
        )))
    }
}

impl RotationMergePass {
    /// Plans the merge action for two adjacent rotation parameters along `axis` on `qubit`.
    ///
    /// Returns `None` if the parameters cannot be safely combined (e.g. unresolved symbols
    /// under `PreserveUnresolved`).
    fn plan_merge(
        &self,
        axis: RotationAxis,
        qubit: QubitId,
        param1: &ParameterExpr,
        param2: &ParameterExpr,
    ) -> Option<MergeAction> {
        // Case 1: Both parameters evaluate numerically in the current environment
        if let (Ok(val1), Ok(val2)) = (
            param1.eval(&self.config.symbol_env),
            param2.eval(&self.config.symbol_env),
        ) {
            if !val1.is_finite() || !val2.is_finite() {
                return None;
            }
            let sum = val1 + val2;
            let norm = normalize_rotation_angle(
                sum,
                self.config.tolerance,
                self.config.normalization_mode,
            );

            if norm.is_exact_zero() {
                if self.config.eliminate_exact_zero {
                    return Some(MergeAction::EliminateExactZero);
                } else {
                    let fused_op = Operation::custom_gate(
                        axis.gate_name(),
                        vec![qubit],
                        vec![ParameterExpr::Constant(0.0)],
                    );
                    return Some(MergeAction::Fuse {
                        fused_op,
                        phase_adjustment: 0.0,
                        is_symbolic: false,
                    });
                }
            }

            if norm.is_phase_zero() {
                if self.config.eliminate_phase_zero {
                    return Some(MergeAction::EliminatePhaseZero);
                } else {
                    let fused_op = Operation::custom_gate(
                        axis.gate_name(),
                        vec![qubit],
                        vec![ParameterExpr::Constant(norm.angle)],
                    );
                    return Some(MergeAction::Fuse {
                        fused_op,
                        phase_adjustment: norm.global_phase_correction,
                        is_symbolic: false,
                    });
                }
            }

            let fused_op = Operation::custom_gate(
                axis.gate_name(),
                vec![qubit],
                vec![ParameterExpr::Constant(norm.angle)],
            );
            return Some(MergeAction::Fuse {
                fused_op,
                phase_adjustment: norm.global_phase_correction,
                is_symbolic: false,
            });
        }

        // Case 2: Symbolic parameters containing unbound variables
        // Reject invalid / unsupported expressions (e.g. division by zero)
        if contains_division_by_zero(param1) || contains_division_by_zero(param2) {
            return None;
        }

        // Check exact algebraic cancellation: p1 + p2 == 0
        if are_algebraic_opposites(param1, param2) {
            if self.config.eliminate_exact_zero {
                return Some(MergeAction::EliminateExactZero);
            } else {
                let fused_op = Operation::custom_gate(
                    axis.gate_name(),
                    vec![qubit],
                    vec![ParameterExpr::Constant(0.0)],
                );
                return Some(MergeAction::Fuse {
                    fused_op,
                    phase_adjustment: 0.0,
                    is_symbolic: true,
                });
            }
        }

        // If not opposites, handle according to symbolic policy
        match self.config.symbolic_policy {
            SymbolicMergePolicy::CombineSymbolic => {
                let combined_param = compose_rotation_angles(param1, param2);
                let fused_op =
                    Operation::custom_gate(axis.gate_name(), vec![qubit], vec![combined_param]);
                Some(MergeAction::Fuse {
                    fused_op,
                    phase_adjustment: 0.0,
                    is_symbolic: true,
                })
            }
            SymbolicMergePolicy::PreserveUnresolved => {
                // Unresolved symbolic combinations remain unchanged
                None
            }
        }
    }
}

/// Extracts rotation parameters from an operation if it is a single-qubit rotation ($R_x$, $R_y$, or $R_z$).
fn extract_rotation(op: &Operation) -> Option<(RotationAxis, QubitId, &ParameterExpr)> {
    match op {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => {
            if qubits.len() != 1 || parameters.len() != 1 {
                return None;
            }
            let axis = RotationAxis::from_gate_name(gate_name)?;
            Some((axis, qubits[0], &parameters[0]))
        }
        _ => None,
    }
}

/// Identifies if `node` has a unique direct successor on wire `WireId::Quantum(qubit)`.
fn find_adjacent_wire_partner(
    graph: &CircuitGraph,
    node: &NodeData,
    qubit: QubitId,
) -> Option<OperationId> {
    let wire = WireId::Quantum(qubit);
    let succs = graph.successors(node.id);
    let mut direct_succ = None;

    for (target_id, dep) in succs {
        if dep.wire == wire {
            if direct_succ.is_some() {
                // Should not occur for quantum linear wires
                return None;
            }
            direct_succ = Some(target_id);
        }
    }

    direct_succ
}

/// Checks if two parameter expressions are exact algebraic opposites ($p$ and $-p$ or $-p$ and $p$).
fn are_algebraic_opposites(p1: &ParameterExpr, p2: &ParameterExpr) -> bool {
    let s1 = simplify_parameter_expr(p1);
    let s2 = simplify_parameter_expr(p2);
    matches!(
        (&s1, &s2),
        (ParameterExpr::Neg(inner), other) | (other, ParameterExpr::Neg(inner))
            if **inner == *other
    )
}

/// Recursively detects whether an expression contains division by zero.
fn contains_division_by_zero(expr: &ParameterExpr) -> bool {
    match expr {
        ParameterExpr::Div(a, b) => {
            if let ParameterExpr::Constant(c) = **b {
                if c.abs() < 1e-15 {
                    return true;
                }
            }
            contains_division_by_zero(a) || contains_division_by_zero(b)
        }
        ParameterExpr::Add(a, b) | ParameterExpr::Sub(a, b) | ParameterExpr::Mul(a, b) => {
            contains_division_by_zero(a) || contains_division_by_zero(b)
        }
        ParameterExpr::Neg(a) => contains_division_by_zero(a),
        _ => false,
    }
}
