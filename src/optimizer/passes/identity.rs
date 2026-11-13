//! Identity-gate elimination and zero-rotation cancellation compiler pass.
//!
//! # Semantics & Periodicity Contracts
//!
//! 1. **Explicit Identity Operations**:
//!    - Operations explicitly named `"id"` or `"i"` represent identity operations ($I$).
//!    - They are eliminated unconditionally with zero global phase impact.
//!
//! 2. **Continuous Rotations ($R_x, R_y, R_z$)**:
//!    - The single-qubit rotation operator along axis $\hat{n}$ is:
//!      $$R_{\hat{n}}(\theta) = \exp\left(-i \frac{\theta}{2} \hat{n} \cdot \vec{\sigma}\right) = \cos\left(\frac{\theta}{2}\right) I - i \sin\left(\frac{\theta}{2}\right) (\hat{n} \cdot \vec{\sigma})$$
//!    - **Exact Zero Rotations** ($\theta \equiv 0 \pmod{4\pi}$):
//!      $\cos(\theta/2) = 1$ and $\sin(\theta/2) = 0 \implies R(\theta) = I$.
//!      These operations are eliminated with zero global phase correction.
//!    - **$2\pi$ Periodic Rotations** ($\theta \equiv 2\pi \pmod{4\pi}$, i.e. $\theta = 2\pi(2k+1)$):
//!      $\cos(\theta/2) = -1$ and $\sin(\theta/2) = 0 \implies R(\theta) = -I = e^{i\pi} I$.
//!      When global-phase-aware elimination is enabled (`eliminate_two_pi_rotations = true`),
//!      these operations are eliminated and $+\pi$ is added to the circuit's explicit global phase.
//!      This ensures exact unitary equivalence under all circumstances.
//!
//! 3. **Symbolic Parameters**:
//!    - Parameter expressions containing unbound symbolic variables are **never** assumed to be zero.
//!    - Only expressions that evaluate cleanly to concrete numeric values are eliminated.
//!
//! 4. **Preservation of Non-Removable Operations**:
//!    - Non-unitary operations (`Measure`, `Reset`), synchronization barriers (`Barrier`),
//!      and classical compute statements (`Classical`) are strictly preserved.
//!
//! 5. **Transactional Rewriting**:
//!    - Graph mutations use [`CircuitTransaction`] to atomically repair wire dependencies,
//!      validate DAG acyclicity, and ensure full rollback on error.

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::gate_catalog::GateCatalog;
use crate::gate_signature::{GateId, ParameterDomain};
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::rewrite::CircuitTransaction;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};

/// Default tolerance threshold for continuous rotation parameter comparisons.
pub const DEFAULT_ANGLE_TOLERANCE: f64 = 1e-12;

/// Classification of a continuous rotation angle's identity status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotationIdentityKind {
    /// Not an identity rotation (matrix is non-trivial).
    NotIdentity,
    /// Exact mathematical identity rotation ($\theta \equiv 0 \pmod{4\pi}$), matrix is $I$, global phase correction is $0$.
    ExactZero,
    /// Global-phase identity rotation ($\theta \equiv 2\pi \pmod{4\pi}$), matrix is $-I = e^{i\pi} I$, requiring $+\pi$ global phase adjustment.
    PhaseZero {
        /// Scalar phase correction in radians to add to the circuit global phase.
        phase_adjustment: f64,
    },
}

/// Classifies a continuous single-qubit rotation angle under $4\pi$ and $2\pi$ periodicity rules.
///
/// Returns [`RotationIdentityKind::ExactZero`] if $\theta$ is within `tolerance` of an integer
/// multiple of $4\pi$, [`RotationIdentityKind::PhaseZero`] if within `tolerance` of an odd
/// multiple of $2\pi$, or [`RotationIdentityKind::NotIdentity`] otherwise.
pub fn classify_rotation_angle(val: f64, tolerance: f64) -> RotationIdentityKind {
    if !val.is_finite() {
        return RotationIdentityKind::NotIdentity;
    }

    let four_pi = 4.0 * PI;
    let two_pi = 2.0 * PI;

    let rem = val.rem_euclid(four_pi);
    let dist_0 = rem.min(four_pi - rem);
    if dist_0 <= tolerance {
        return RotationIdentityKind::ExactZero;
    }

    let dist_2pi = (rem - two_pi).abs();
    if dist_2pi <= tolerance {
        return RotationIdentityKind::PhaseZero {
            phase_adjustment: PI,
        };
    }

    RotationIdentityKind::NotIdentity
}

/// Checks whether an angle is zero modulo $2\pi$ within tolerance (used for phase gates $P / U_1$).
pub fn is_zero_mod_2pi(val: f64, tolerance: f64) -> bool {
    if !val.is_finite() {
        return false;
    }
    let two_pi = 2.0 * PI;
    let rem = val.rem_euclid(two_pi);
    let dist = rem.min(two_pi - rem);
    dist <= tolerance
}

/// Configuration options for identity gate elimination.
#[derive(Clone, Debug, PartialEq)]
pub struct IdentityEliminationConfig {
    /// Angular comparison tolerance for rotation parameters (default: `1e-12`).
    pub tolerance: f64,
    /// Whether to eliminate $2\pi$ rotations by applying a $+\pi$ global phase adjustment (default: `true`).
    pub eliminate_two_pi_rotations: bool,
    /// Optional runtime symbol environment to evaluate bound parameter symbols.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for IdentityEliminationConfig {
    fn default() -> Self {
        Self {
            tolerance: DEFAULT_ANGLE_TOLERANCE,
            eliminate_two_pi_rotations: true,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry statistics recorded during identity gate elimination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdentityEliminationStats {
    /// Total operations examined in the circuit DAG.
    pub operations_examined: usize,
    /// Explicit identity gates removed (`id`, `i`).
    pub explicit_identities_removed: usize,
    /// Exact zero rotations removed ($\theta \equiv 0 \pmod{4\pi}$).
    pub exact_zero_rotations_removed: usize,
    /// Phase zero rotations removed ($\theta \equiv 2\pi \pmod{4\pi}$) with $+\pi$ phase adjustment.
    pub phase_zero_rotations_removed: usize,
    /// Operations preserved (non-identities, non-zero rotations, barriers, resets, measures, classical, unresolved symbols).
    pub operations_preserved: usize,
    /// Global phase adjustments applied to the circuit.
    pub global_phase_adjustments: usize,
}

impl IdentityEliminationStats {
    /// Returns the total number of operations removed across all identity categories.
    #[inline]
    pub fn total_removed(&self) -> usize {
        self.explicit_identities_removed
            + self.exact_zero_rotations_removed
            + self.phase_zero_rotations_removed
    }
}

/// A compiler pass that removes identity operations and proven zero rotations from QccIR circuits.
///
/// Preserves circuit semantics, explicit global phase, and wire dependency invariants.
#[derive(Clone, Debug)]
pub struct IdentityEliminationPass {
    config: IdentityEliminationConfig,
    domain: SemanticDomain,
    last_stats: Option<IdentityEliminationStats>,
}

impl Default for IdentityEliminationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityEliminationPass {
    /// Creates a new identity elimination pass with default configuration.
    pub fn new() -> Self {
        Self {
            config: IdentityEliminationConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates an identity elimination pass with custom configuration options.
    pub fn with_config(config: IdentityEliminationConfig) -> Self {
        Self {
            config,
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates an identity elimination pass that only removes exact zero rotations ($0 \pmod{4\pi}$),
    /// leaving $2\pi$ rotations intact without applying global phase corrections.
    pub fn exact_only() -> Self {
        let config = IdentityEliminationConfig {
            eliminate_two_pi_rotations: false,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Sets the semantic domain for this pass (e.g. [`SemanticDomain::UnitaryOnly`] or [`SemanticDomain::GeneralStructural`]).
    pub fn with_domain(mut self, domain: SemanticDomain) -> Self {
        self.domain = domain;
        self
    }

    /// Sets the floating-point tolerance for angular comparisons.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.config.tolerance = tolerance;
        self
    }

    /// Sets the symbol evaluation environment for bound parameter variables.
    pub fn with_symbol_env(mut self, symbol_env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = symbol_env;
        self
    }

    /// Returns a reference to the active pass configuration.
    #[inline]
    pub fn config(&self) -> &IdentityEliminationConfig {
        &self.config
    }

    /// Returns telemetry statistics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<IdentityEliminationStats> {
        self.last_stats
    }
}

#[derive(Clone, Copy, Debug)]
enum RemovalReason {
    ExplicitIdentity,
    ExactZeroRotation,
    PhaseZeroRotation { phase_adjustment: f64 },
}

impl CompilerPass for IdentityEliminationPass {
    fn id(&self) -> PassId {
        PassId::new("identity_elimination")
    }

    fn description(&self) -> &'static str {
        "Eliminates explicit identity gates and proven zero rotations while preserving circuit semantics and QccIR DAG invariants."
    }

    fn config_summary(&self) -> std::collections::BTreeMap<String, serde_json::Value> {
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "tolerance".to_string(),
            serde_json::json!(self.config.tolerance),
        );
        map.insert(
            "eliminate_two_pi_rotations".to_string(),
            serde_json::json!(self.config.eliminate_two_pi_rotations),
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
        let mut stats = IdentityEliminationStats::default();

        let ordered_nodes = circuit
            .graph
            .ordered_nodes()
            .map_err(|source| PassError::Ir {
                pass_id: self.id(),
                source,
            })?;

        // 1. Identify candidate operations for removal
        let mut removal_candidates: Vec<(OperationId, RemovalReason)> = Vec::new();

        for node in &ordered_nodes {
            stats.operations_examined += 1;

            match &node.operation {
                // Non-removable operations are strictly preserved
                Operation::Barrier { .. }
                | Operation::Reset { .. }
                | Operation::Measure { .. }
                | Operation::Classical { .. } => {
                    stats.operations_preserved += 1;
                }

                Operation::Gate {
                    gate_name,
                    parameters,
                    ..
                } => {
                    if let Some(sig) = GateCatalog::lookup(gate_name) {
                        match sig.id {
                            GateId::Id => {
                                removal_candidates.push((node.id, RemovalReason::ExplicitIdentity));
                            }
                            _ if sig.is_parameterized()
                                && parameters.len() == sig.parameter_count =>
                            {
                                match parameters[0].eval(&self.config.symbol_env) {
                                    Ok(angle) => match sig.parameter_domains[0] {
                                        ParameterDomain::Angle4Pi => {
                                            match classify_rotation_angle(
                                                angle,
                                                self.config.tolerance,
                                            ) {
                                                RotationIdentityKind::ExactZero => {
                                                    removal_candidates.push((
                                                        node.id,
                                                        RemovalReason::ExactZeroRotation,
                                                    ));
                                                }
                                                RotationIdentityKind::PhaseZero {
                                                    phase_adjustment,
                                                } => {
                                                    if self.config.eliminate_two_pi_rotations {
                                                        removal_candidates.push((
                                                            node.id,
                                                            RemovalReason::PhaseZeroRotation {
                                                                phase_adjustment,
                                                            },
                                                        ));
                                                    } else {
                                                        stats.operations_preserved += 1;
                                                    }
                                                }
                                                RotationIdentityKind::NotIdentity => {
                                                    stats.operations_preserved += 1;
                                                }
                                            }
                                        }
                                        ParameterDomain::Angle2Pi => {
                                            if is_zero_mod_2pi(angle, self.config.tolerance) {
                                                removal_candidates.push((
                                                    node.id,
                                                    RemovalReason::ExactZeroRotation,
                                                ));
                                            } else {
                                                stats.operations_preserved += 1;
                                            }
                                        }
                                        ParameterDomain::UnconstrainedReal => {
                                            stats.operations_preserved += 1;
                                        }
                                    },
                                    Err(_) => {
                                        // Unresolved symbolic parameter: NEVER treat as zero
                                        stats.operations_preserved += 1;
                                    }
                                }
                            }
                            _ => {
                                stats.operations_preserved += 1;
                            }
                        }
                    } else {
                        // All other quantum gates are preserved
                        stats.operations_preserved += 1;
                    }
                }
            }
        }

        // 2. If no identity operations found, return unmodified result
        if removal_candidates.is_empty() {
            self.last_stats = Some(stats);
            return Ok(PassResult::unmodified());
        }

        // 3. Perform atomic transactional removal
        let mut tx = CircuitTransaction::begin(circuit);

        for (op_id, reason) in removal_candidates {
            tx.remove_operation(op_id)
                .map_err(|err| PassError::ExecutionFailed {
                    pass_id: self.id(),
                    reason: format!("failed to remove operation {op_id}: {err}"),
                })?;

            match reason {
                RemovalReason::ExplicitIdentity => {
                    stats.explicit_identities_removed += 1;
                }
                RemovalReason::ExactZeroRotation => {
                    stats.exact_zero_rotations_removed += 1;
                }
                RemovalReason::PhaseZeroRotation { phase_adjustment } => {
                    stats.phase_zero_rotations_removed += 1;
                    tx.add_global_phase(ParameterExpr::constant(phase_adjustment));
                    stats.global_phase_adjustments += 1;
                }
            }
        }

        tx.commit().map_err(|err| PassError::ExecutionFailed {
            pass_id: self.id(),
            reason: format!("transaction commit failed: {err}"),
        })?;

        self.last_stats = Some(stats);

        let summary = format!(
            "removed {} identity operations ({} explicit, {} exact zero rotations, {} phase zero rotations); preserved {}; applied {} global phase adjustments",
            stats.total_removed(),
            stats.explicit_identities_removed,
            stats.exact_zero_rotations_removed,
            stats.phase_zero_rotations_removed,
            stats.operations_preserved,
            stats.global_phase_adjustments
        );

        Ok(PassResult::modified(summary))
    }
}
