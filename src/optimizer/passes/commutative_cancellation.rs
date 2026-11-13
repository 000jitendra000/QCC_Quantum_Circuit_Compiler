//! Commutative two-qubit ($CX$ and $CZ$) self-inverse gate cancellation compiler pass.
//!
//! # Semantics & Preconditions (Roadmap v3.1 Phase 80, ADR-0005)
//!
//! 1. **Self-Inverse Commutative Cancellation**:
//!    - Discrete self-inverse two-qubit gates ($G^2 = I$):
//!      - Directed Controlled-NOT: $CX(c, t) \cdot CX(c, t) = I_4$.
//!      - Symmetric Controlled-$Z$: $CZ(a, b) \cdot CZ(a, b) = I_4$, where $CZ(a, b) \equiv CZ(b, a)$.
//!    - Cancels non-adjacent pairs across intervening sequences where every intervening operation
//!      individually commutes with $G$.
//!
//! 2. **Gate Identity & Operand Invariants**:
//!    - $CX(c, t)$ requires strict ordered operand identity: operand 0 is control, operand 1 is target.
//!      Reversed operands ($CX(c, t)$ and $CX(t, c)$) do NOT commute or cancel.
//!    - $CZ(a, b)$ is symmetric: operands are normalized to $(\min(a, b), \max(a, b))$ such that
//!      $CZ(a, b)$ and $CZ(b, a)$ represent identical operations.
//!    - Cross-type cancellation ($CX$ with $CZ$) is strictly prohibited.
//!
//! 3. **Commutation Rules**:
//!    - Operations on disjoint wires commute unconditionally.
//!    - On $CX$ control wire $c$: computational-basis diagonal operations ($Z, S, S^\dagger, T, T^\dagger, R_z, P$),
//!      $CZ$, and $CX$ sharing control ($CX(c, t_2)$) commute.
//!    - On $CX$ target wire $t$: $X$-basis operations ($X, R_x$) and $CX$ sharing target ($CX(c_2, t)$) commute.
//!    - On $CZ$ operands: computational-basis diagonal operations ($Z, S, S^\dagger, T, T^\dagger, R_z, P$),
//!      any $CZ$, and $CX$ where operand is control ($CX(q, t_2)$) commute.
//!    - Any intervening operation touching both wires terminates search unless it is the matching identical gate.
//!
//! 4. **Impenetrable Boundaries & Fail-Closed Policy**:
//!    - Non-unitary operations (`Barrier`, `Measure`, `Reset`), classical hazard boundaries (`Classical`),
//!      and uncatalogued/unknown gates strictly block traversal.
//!
//! 5. **Global Phase & Invariants**:
//!    - Unitary self-inverse cancellation carries zero net global phase ($\Delta \phi = 0.0$).
//!    - Mutations execute atomically via [`CircuitTransaction`].
//!    - Unremoved operations retain stable semantic [`OperationId`]s.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::gate_catalog::GateCatalog;
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::rewrite::CircuitTransaction;
use crate::ir::wire::QubitId;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};

/// Default maximum fixed-point iterations for commutative cancellation.
pub const DEFAULT_COMMUTATIVE_CANCELLATION_MAX_ITERATIONS: usize = 100;

/// Default maximum lookahead search depth in topological order.
pub const DEFAULT_COMMUTATIVE_CANCELLATION_MAX_SEARCH_DEPTH: usize = 1000;

/// Reason explaining why a candidate two-qubit gate was rejected from cancellation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommutativeRejectionReason {
    /// Non-unitary boundary (barrier, measure, reset) encountered along search path.
    NonUnitaryOrBarrier,
    /// Classical wire dependency hazard encountered along search path.
    ClassicalHazard,
    /// Intervening operation touches both operand wires without being the matching gate.
    TouchesBothWires,
    /// Intervening operation on control wire does not commute with Z-axis.
    NonCommutingControlOperation,
    /// Intervening operation on target wire does not commute with X-axis.
    NonCommutingTargetOperation,
    /// Intervening operation on CZ operand does not commute with Z-axis.
    NonCommutingCzOperation,
    /// Intervening operation is uncatalogued or unsupported in QCC gate catalog.
    UncataloguedOperation,
    /// Lookahead search depth limit exceeded before reaching matching gate.
    SearchDepthExceeded,
    /// Reached end of circuit without finding matching gate.
    NoMatchingPartnerFound,
}

impl CommutativeRejectionReason {
    /// Returns a human-readable description of the rejection reason.
    pub fn description(&self) -> &'static str {
        match self {
            Self::NonUnitaryOrBarrier => {
                "non-unitary boundary (barrier, measure, reset) blocks commutation"
            }
            Self::ClassicalHazard => "classical wire hazard blocks commutation",
            Self::TouchesBothWires => "intervening operation touches both operand wires",
            Self::NonCommutingControlOperation => {
                "operation on CX control wire does not commute with Z-axis"
            }
            Self::NonCommutingTargetOperation => {
                "operation on CX target wire does not commute with X-axis"
            }
            Self::NonCommutingCzOperation => "operation on CZ wire does not commute with Z-axis",
            Self::UncataloguedOperation => {
                "uncatalogued or unsupported operation cannot be proven to commute"
            }
            Self::SearchDepthExceeded => "lookahead search depth limit exceeded",
            Self::NoMatchingPartnerFound => {
                "reached end of circuit without finding matching partner gate"
            }
        }
    }
}

/// Configuration options for commutative two-qubit cancellation.
#[derive(Clone, Debug, PartialEq)]
pub struct CommutativeCancellationConfig {
    /// Maximum number of fixed-point iterations (default: `100`).
    pub max_iterations: usize,
    /// Maximum lookahead window depth in topological order (default: `1000`).
    pub max_search_depth: usize,
    /// Optional symbol evaluation environment for parameter expressions.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for CommutativeCancellationConfig {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_COMMUTATIVE_CANCELLATION_MAX_ITERATIONS,
            max_search_depth: DEFAULT_COMMUTATIVE_CANCELLATION_MAX_SEARCH_DEPTH,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry statistics recorded during commutative two-qubit cancellation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommutativeCancellationStats {
    /// Total candidate gates examined as candidate first operations.
    pub candidate_pairs_examined: usize,
    /// Total pairs successfully cancelled across all iterations.
    pub cancellation_pairs_accepted: usize,
    /// Total CX pairs cancelled.
    pub cx_pairs_cancelled: usize,
    /// Total CZ pairs cancelled.
    pub cz_pairs_cancelled: usize,
    /// Total candidate gates rejected from cancellation.
    pub candidate_pairs_rejected: usize,
    /// Rejection counts categorized by cause.
    pub rejection_non_commuting_target: usize,
    pub rejection_non_commuting_control: usize,
    pub rejection_non_commuting_cz: usize,
    pub rejection_non_unitary_or_barrier: usize,
    pub rejection_classical_hazard: usize,
    pub rejection_touches_both_wires: usize,
    pub rejection_uncatalogued_operation: usize,
    pub rejection_search_depth_exceeded: usize,
    pub rejection_no_matching_partner: usize,
    /// Total fixed-point iterations performed until convergence.
    pub iterations: usize,
}

impl CommutativeCancellationStats {
    /// Returns the total number of operations eliminated ($2 \times \text{cancellation\_pairs\_accepted}$).
    #[inline]
    pub fn total_operations_removed(&self) -> usize {
        self.cancellation_pairs_accepted * 2
    }

    /// Records a candidate rejection reason in detailed telemetry counters.
    pub fn record_rejection(&mut self, reason: CommutativeRejectionReason) {
        self.candidate_pairs_rejected += 1;
        match reason {
            CommutativeRejectionReason::NonUnitaryOrBarrier => {
                self.rejection_non_unitary_or_barrier += 1;
            }
            CommutativeRejectionReason::ClassicalHazard => {
                self.rejection_classical_hazard += 1;
            }
            CommutativeRejectionReason::TouchesBothWires => {
                self.rejection_touches_both_wires += 1;
            }
            CommutativeRejectionReason::NonCommutingControlOperation => {
                self.rejection_non_commuting_control += 1;
            }
            CommutativeRejectionReason::NonCommutingTargetOperation => {
                self.rejection_non_commuting_target += 1;
            }
            CommutativeRejectionReason::NonCommutingCzOperation => {
                self.rejection_non_commuting_cz += 1;
            }
            CommutativeRejectionReason::UncataloguedOperation => {
                self.rejection_uncatalogued_operation += 1;
            }
            CommutativeRejectionReason::SearchDepthExceeded => {
                self.rejection_search_depth_exceeded += 1;
            }
            CommutativeRejectionReason::NoMatchingPartnerFound => {
                self.rejection_no_matching_partner += 1;
            }
        }
    }
}

/// Internal classification of two-qubit gate operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TwoQubitGateKind {
    Cx(QubitId, QubitId),
    Cz(QubitId, QubitId),
}

/// Compiler pass implementing commutative two-qubit ($CX$ and $CZ$) cancellation.
///
/// Eliminates pairs of identical two-qubit self-inverse gates separated by intervening
/// operations that individually commute with the candidate gate.
#[derive(Clone, Debug)]
pub struct CommutativeTwoQubitCancellationPass {
    config: CommutativeCancellationConfig,
    domain: SemanticDomain,
    last_stats: Option<CommutativeCancellationStats>,
}

impl Default for CommutativeTwoQubitCancellationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl CommutativeTwoQubitCancellationPass {
    /// Creates a new commutative cancellation pass with default settings.
    pub fn new() -> Self {
        Self {
            config: CommutativeCancellationConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates a new commutative cancellation pass with custom configuration options.
    pub fn with_config(config: CommutativeCancellationConfig) -> Self {
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

    /// Sets the maximum number of fixed-point iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Sets the maximum lookahead search depth in topological order.
    pub fn with_max_search_depth(mut self, depth: usize) -> Self {
        self.config.max_search_depth = depth;
        self
    }

    /// Sets the symbol evaluation environment for bound parameters.
    pub fn with_symbol_env(mut self, env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = env;
        self
    }

    /// Returns a reference to the active pass configuration.
    #[inline]
    pub fn config(&self) -> &CommutativeCancellationConfig {
        &self.config
    }

    /// Returns telemetry statistics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<&CommutativeCancellationStats> {
        self.last_stats.as_ref()
    }
}

impl CompilerPass for CommutativeTwoQubitCancellationPass {
    fn id(&self) -> PassId {
        PassId::new("commutative_cancellation")
    }

    fn description(&self) -> &'static str {
        "Cancels non-adjacent two-qubit (CX/CZ) self-inverse operations across intervening commuting operations in QccIR DAGs."
    }

    fn config_summary(&self) -> BTreeMap<String, serde_json::Value> {
        let mut map = BTreeMap::new();
        map.insert(
            "max_iterations".to_string(),
            serde_json::json!(self.config.max_iterations),
        );
        map.insert(
            "max_search_depth".to_string(),
            serde_json::json!(self.config.max_search_depth),
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
        let mut stats = CommutativeCancellationStats::default();
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
            let mut pairs_to_remove: Vec<(OperationId, OperationId, TwoQubitGateKind)> = Vec::new();

            for i in 0..ordered_nodes.len() {
                let node_i = &ordered_nodes[i];
                if used_ids.contains(&node_i.id) {
                    continue;
                }

                let gate_kind = match &node_i.operation {
                    Operation::Gate {
                        gate_name, qubits, ..
                    } => {
                        let lower = gate_name.to_ascii_lowercase();
                        if lower == "cx" && qubits.len() == 2 {
                            TwoQubitGateKind::Cx(qubits[0], qubits[1])
                        } else if lower == "cz" && qubits.len() == 2 {
                            let norm = (
                                std::cmp::min(qubits[0], qubits[1]),
                                std::cmp::max(qubits[0], qubits[1]),
                            );
                            TwoQubitGateKind::Cz(norm.0, norm.1)
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                // Search forward for a matching partner
                stats.candidate_pairs_examined += 1;
                let mut matched_partner: Option<OperationId> = None;
                let mut rejection_reason: Option<CommutativeRejectionReason> = None;

                for (j, node_j) in ordered_nodes.iter().enumerate().skip(i + 1) {
                    if j - i > self.config.max_search_depth {
                        rejection_reason = Some(CommutativeRejectionReason::SearchDepthExceeded);
                        break;
                    }

                    // Check if node_j is the matching partner
                    match gate_kind {
                        TwoQubitGateKind::Cx(c, t) => {
                            if let Operation::Gate {
                                gate_name, qubits, ..
                            } = &node_j.operation
                            {
                                if gate_name.eq_ignore_ascii_case("cx")
                                    && qubits.as_slice() == [c, t]
                                {
                                    if used_ids.contains(&node_j.id) {
                                        // Intervening identical gate already consumed
                                        rejection_reason =
                                            Some(CommutativeRejectionReason::TouchesBothWires);
                                    } else {
                                        matched_partner = Some(node_j.id);
                                    }
                                    break;
                                }
                            }
                            // Otherwise, node_j is an intervening operation
                            if let Err(reason) = check_cx_intervening(&node_j.operation, c, t) {
                                rejection_reason = Some(reason);
                                break;
                            }
                        }
                        TwoQubitGateKind::Cz(q0, q1) => {
                            if let Operation::Gate {
                                gate_name, qubits, ..
                            } = &node_j.operation
                            {
                                if gate_name.eq_ignore_ascii_case("cz") && qubits.len() == 2 {
                                    let norm_j = (
                                        std::cmp::min(qubits[0], qubits[1]),
                                        std::cmp::max(qubits[0], qubits[1]),
                                    );
                                    if norm_j == (q0, q1) {
                                        if used_ids.contains(&node_j.id) {
                                            rejection_reason =
                                                Some(CommutativeRejectionReason::TouchesBothWires);
                                        } else {
                                            matched_partner = Some(node_j.id);
                                        }
                                        break;
                                    }
                                }
                            }
                            if let Err(reason) = check_cz_intervening(&node_j.operation, q0, q1) {
                                rejection_reason = Some(reason);
                                break;
                            }
                        }
                    }
                }

                if let Some(partner_id) = matched_partner {
                    used_ids.insert(node_i.id);
                    used_ids.insert(partner_id);
                    pairs_to_remove.push((node_i.id, partner_id, gate_kind));
                } else {
                    let reason = rejection_reason
                        .unwrap_or(CommutativeRejectionReason::NoMatchingPartnerFound);
                    stats.record_rejection(reason);
                }
            }

            if pairs_to_remove.is_empty() {
                break;
            }

            modified = true;
            for (id1, id2, kind) in pairs_to_remove {
                tx.remove_operation(id1)
                    .map_err(|err| PassError::ExecutionFailed {
                        pass_id: self.id(),
                        reason: format!("failed to remove operation {id1}: {err}"),
                    })?;
                tx.remove_operation(id2)
                    .map_err(|err| PassError::ExecutionFailed {
                        pass_id: self.id(),
                        reason: format!("failed to remove operation {id2}: {err}"),
                    })?;

                stats.cancellation_pairs_accepted += 1;
                match kind {
                    TwoQubitGateKind::Cx(..) => stats.cx_pairs_cancelled += 1,
                    TwoQubitGateKind::Cz(..) => stats.cz_pairs_cancelled += 1,
                }
            }
        }

        stats.iterations = iteration;
        self.last_stats = Some(stats.clone());

        if !modified {
            return Ok(PassResult::unmodified());
        }

        tx.commit().map_err(|err| PassError::ExecutionFailed {
            pass_id: self.id(),
            reason: format!("transaction commit failed: {err}"),
        })?;

        let summary = format!(
            "cancelled {} two-qubit pairs ({} CX, {} CZ); examined {} candidate pairs in {} iterations",
            stats.cancellation_pairs_accepted,
            stats.cx_pairs_cancelled,
            stats.cz_pairs_cancelled,
            stats.candidate_pairs_examined,
            stats.iterations
        );

        Ok(PassResult::modified(summary))
    }
}

/// Evaluates whether an intervening operation commutes with $CX(c, t)$.
///
/// In QCC, for $CX(c, t)$:
/// - Operand 0 ($c$) is control.
/// - Operand 1 ($t$) is target.
fn check_cx_intervening(
    op: &Operation,
    c: QubitId,
    t: QubitId,
) -> Result<(), CommutativeRejectionReason> {
    match op {
        Operation::Barrier { .. } | Operation::Measure { .. } | Operation::Reset { .. } => {
            Err(CommutativeRejectionReason::NonUnitaryOrBarrier)
        }
        Operation::Classical { .. } => Err(CommutativeRejectionReason::ClassicalHazard),
        Operation::Gate {
            gate_name, qubits, ..
        } => {
            // Uncatalogued / unsupported gates fail closed
            if GateCatalog::lookup(gate_name).is_none() {
                return Err(CommutativeRejectionReason::UncataloguedOperation);
            }

            let touches_c = qubits.contains(&c);
            let touches_t = qubits.contains(&t);

            if !touches_c && !touches_t {
                // Disjoint wire with known unitary gate: strictly commutes
                return Ok(());
            }

            if touches_c && touches_t {
                // Intervening operation touches both wires: any 2Q operation on {c, t}
                // other than the matching CX terminates the cancellation path.
                return Err(CommutativeRejectionReason::TouchesBothWires);
            }

            if touches_c {
                // Operation on control wire must commute with Z-axis
                let lower = gate_name.to_ascii_lowercase();
                if qubits.len() == 1 {
                    if matches!(
                        lower.as_str(),
                        "z" | "s" | "sdg" | "t" | "tdg" | "rz" | "p" | "id" | "i"
                    ) {
                        return Ok(());
                    } else {
                        return Err(CommutativeRejectionReason::NonCommutingControlOperation);
                    }
                } else if qubits.len() == 2 {
                    if lower == "cz" {
                        // CZ is diagonal on c, targets are disjoint
                        return Ok(());
                    } else if lower == "cx" && qubits[0] == c {
                        // Shared control CX(c, t2) commutes with CX(c, t)
                        return Ok(());
                    } else {
                        return Err(CommutativeRejectionReason::NonCommutingControlOperation);
                    }
                } else {
                    return Err(CommutativeRejectionReason::NonCommutingControlOperation);
                }
            }

            if touches_t {
                // Operation on target wire must commute with X-axis
                let lower = gate_name.to_ascii_lowercase();
                if qubits.len() == 1 {
                    if matches!(lower.as_str(), "x" | "rx" | "id" | "i") {
                        return Ok(());
                    } else {
                        return Err(CommutativeRejectionReason::NonCommutingTargetOperation);
                    }
                } else if qubits.len() == 2 {
                    if lower == "cx" && qubits[1] == t {
                        // Shared target CX(c2, t) commutes with CX(c, t)
                        return Ok(());
                    } else {
                        return Err(CommutativeRejectionReason::NonCommutingTargetOperation);
                    }
                } else {
                    return Err(CommutativeRejectionReason::NonCommutingTargetOperation);
                }
            }

            Ok(())
        }
    }
}

/// Evaluates whether an intervening operation commutes with $CZ(q_0, q_1)$.
///
/// Because $CZ$ is diagonal in the computational basis, any computational-basis
/// diagonal operation ($Z$-axis) on either operand commutes.
fn check_cz_intervening(
    op: &Operation,
    q0: QubitId,
    q1: QubitId,
) -> Result<(), CommutativeRejectionReason> {
    match op {
        Operation::Barrier { .. } | Operation::Measure { .. } | Operation::Reset { .. } => {
            Err(CommutativeRejectionReason::NonUnitaryOrBarrier)
        }
        Operation::Classical { .. } => Err(CommutativeRejectionReason::ClassicalHazard),
        Operation::Gate {
            gate_name, qubits, ..
        } => {
            if GateCatalog::lookup(gate_name).is_none() {
                return Err(CommutativeRejectionReason::UncataloguedOperation);
            }

            let touches_q0 = qubits.contains(&q0);
            let touches_q1 = qubits.contains(&q1);

            if !touches_q0 && !touches_q1 {
                // Disjoint wire with known unitary gate: strictly commutes
                return Ok(());
            }

            if touches_q0 && touches_q1 {
                // Intervening operation touches both wires
                return Err(CommutativeRejectionReason::TouchesBothWires);
            }

            let lower = gate_name.to_ascii_lowercase();

            // Evaluate each touched operand
            for &q in &[q0, q1] {
                if !qubits.contains(&q) {
                    continue;
                }
                if qubits.len() == 1 {
                    if !matches!(
                        lower.as_str(),
                        "z" | "s" | "sdg" | "t" | "tdg" | "rz" | "p" | "id" | "i"
                    ) {
                        return Err(CommutativeRejectionReason::NonCommutingCzOperation);
                    }
                } else if qubits.len() == 2 {
                    if lower == "cz" {
                        // Any CZ is diagonal in the computational basis and commutes
                        continue;
                    } else if lower == "cx"
                        && qubits[0] == q
                        && !qubits.contains(&if q == q0 { q1 } else { q0 })
                    {
                        // CX where touched qubit is control, and target wire is disjoint from CZ
                        continue;
                    } else {
                        return Err(CommutativeRejectionReason::NonCommutingCzOperation);
                    }
                } else {
                    return Err(CommutativeRejectionReason::NonCommutingCzOperation);
                }
            }

            Ok(())
        }
    }
}
