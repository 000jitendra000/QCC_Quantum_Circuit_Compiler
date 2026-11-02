//! Inverse-gate cancellation compiler pass.
//!
//! # Semantics & Dependency Contracts
//!
//! 1. **Exact Mathematical Inverses**:
//!    - Discrete self-inverse gates ($U^2 = I$): single-qubit `H`, `X`, `Y`, `Z`; two-qubit `CX`, `CZ`, `SWAP`.
//!    - Discrete adjoint pairs ($U \cdot U^\dagger = I$): `S`-`Sdg`, `Sdg`-`S`, `T`-`Tdg`, `Tdg`-`T`.
//!    - Parameterized continuous rotation pairs: $R_k(\theta) \cdot R_k(-\theta) = I$ along axis $k \in \{x, y, z\}$.
//!      Rotation parameters cancel to identity when $(\theta_1 + \theta_2) \equiv 0 \pmod{4\pi}$.
//!      Note that $R_k(\pi) \cdot R_k(\pi) = R_k(2\pi) = -I \neq I$, which requires a global phase correction
//!      and is therefore not an exact matrix inverse.
//!    - General rotation merging ($\theta_1 + \theta_2 \neq 0 \pmod{4\pi}$) is deferred to Phase 30.
//!
//! 2. **Exact Ordered Operands**:
//!    - For asymmetric directional gates such as `CX(c, t)`, operand order is strictly preserved:
//!      `CX(a, b)` and `CX(a, b)` cancel, but `CX(a, b)` and `CX(b, a)` **never** cancel ($CX(a, b) \cdot CX(b, a) \neq I$).
//!    - For symmetric gates (`CZ`, `SWAP`), operand permutations are recognized as valid inverses:
//!      `CZ(a, b)` and `CZ(b, a)` cancel.
//!
//! 3. **Multi-Wire Dependency Safety**:
//!    - Multi-qubit candidate pairs must be wire-adjacent along **all** shared qubit wires simultaneously.
//!    - An intervening operation on even a single shared wire prevents cancellation.
//!    - Operations on disjoint wires commute in the DAG and do not block wire-adjacency on independent wires.
//!
//! 4. **Intervening Dependency Boundaries**:
//!    - Non-unitary operations (`Measure`, `Reset`), synchronization barriers (`Barrier`),
//!      and classical operations (`Classical`) strictly prevent cancellation.
//!
//! 5. **Transactional Rewriting & Invariants**:
//!    - Graph mutations are performed atomically using [`CircuitTransaction`].
//!    - Dependency wires are reconnected automatically and DAG acyclicity is validated upon commit.
//!    - Circuit global phase is strictly preserved (unitary cancellation carries zero net phase change).
//!    - Unremoved operations retain their stable semantic [`OperationId`]s and provenance.

use std::collections::{HashMap, HashSet};

use crate::ir::circuit::Circuit;
use crate::ir::graph::CircuitGraph;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::CircuitTransaction;
use crate::ir::wire::WireId;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::optimizer::algebra::are_inverses_with_config;

/// Default tolerance threshold for continuous rotation parameter comparisons.
pub const DEFAULT_CANCELLATION_TOLERANCE: f64 = 1e-12;

/// Default maximum number of fixed-point iterations.
pub const DEFAULT_MAX_ITERATIONS: usize = 1000;

/// Classification of a cancelled gate pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateCategory {
    /// Single-qubit discrete gate pair (e.g. H-H, X-X, S-Sdg, T-Tdg).
    SingleQubit,
    /// Two-qubit gate pair (e.g. CX-CX, CZ-CZ, SWAP-SWAP).
    TwoQubit,
    /// Parameterized rotation pair (e.g. Rx-Rx, Ry-Ry, Rz-Rz, P-P).
    Rotation,
}

/// Configuration options for inverse-gate cancellation.
#[derive(Clone, Debug, PartialEq)]
pub struct InverseCancellationConfig {
    /// Angular comparison tolerance for rotation parameters (default: `1e-12`).
    pub tolerance: f64,
    /// Maximum number of cancellation loop iterations until fixed point (default: `1000`).
    pub max_iterations: usize,
    /// Optional runtime symbol environment to evaluate bound parameter symbols.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for InverseCancellationConfig {
    fn default() -> Self {
        Self {
            tolerance: DEFAULT_CANCELLATION_TOLERANCE,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry statistics recorded during inverse-gate cancellation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InverseCancellationStats {
    /// Total candidate adjacent pairs examined in the DAG across all iterations.
    pub candidate_pairs_examined: usize,
    /// Total inverse pairs successfully cancelled.
    pub inverse_pairs_cancelled: usize,
    /// Total candidate pairs rejected (adjacent on wires but not mathematical inverses).
    pub candidate_pairs_rejected: usize,
    /// Total single-qubit discrete inverse pairs cancelled.
    pub single_qubit_pairs_cancelled: usize,
    /// Total two-qubit inverse pairs cancelled.
    pub two_qubit_pairs_cancelled: usize,
    /// Total parameterized rotation inverse pairs cancelled.
    pub rotation_pairs_cancelled: usize,
    /// Total fixed-point iterations performed.
    pub iterations: usize,
}

impl InverseCancellationStats {
    /// Returns the total number of operations removed across all cancelled pairs ($2 \times \text{pairs}$).
    #[inline]
    pub fn total_operations_removed(&self) -> usize {
        self.inverse_pairs_cancelled * 2
    }
}

/// A compiler pass that cancels adjacent inverse operations on the same dependency wires in QccIR circuits.
///
/// Preserves circuit semantics, explicit circuit global phase, and wire dependency invariants.
#[derive(Clone, Debug)]
pub struct InverseCancellationPass {
    config: InverseCancellationConfig,
    domain: SemanticDomain,
    last_stats: Option<InverseCancellationStats>,
}

impl Default for InverseCancellationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl InverseCancellationPass {
    /// Creates a new inverse cancellation pass with default configuration.
    pub fn new() -> Self {
        Self {
            config: InverseCancellationConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates an inverse cancellation pass with custom configuration options.
    pub fn with_config(config: InverseCancellationConfig) -> Self {
        Self {
            config,
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Sets the semantic domain for this pass (e.g. [`SemanticDomain::UnitaryOnly`] or [`SemanticDomain::GeneralStructural`]).
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

    /// Sets the symbol evaluation environment for bound parameter variables.
    pub fn with_symbol_env(mut self, symbol_env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = symbol_env;
        self
    }

    /// Returns a reference to the active pass configuration.
    #[inline]
    pub fn config(&self) -> &InverseCancellationConfig {
        &self.config
    }

    /// Returns telemetry statistics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<InverseCancellationStats> {
        self.last_stats
    }
}

impl CompilerPass for InverseCancellationPass {
    fn id(&self) -> PassId {
        PassId::new("inverse_cancellation")
    }

    fn description(&self) -> &'static str {
        "Cancels adjacent inverse operations on the same dependency wires while preserving circuit semantics and QccIR DAG invariants."
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
        let mut stats = InverseCancellationStats::default();
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
            let mut pairs_to_remove: Vec<(OperationId, OperationId, GateCategory)> = Vec::new();

            for node in &ordered_nodes {
                if used_ids.contains(&node.id) {
                    continue;
                }

                // Only quantum gate operations can have unitary inverses
                if !matches!(node.operation, Operation::Gate { .. }) {
                    continue;
                }

                // Check for a wire-adjacent candidate partner
                if let Some(partner_id) = find_adjacent_partner(&tx.circuit().graph, node) {
                    if used_ids.contains(&partner_id) {
                        continue;
                    }

                    let partner_node = match tx.circuit().graph.get_node(partner_id) {
                        Some(n) => n,
                        None => continue,
                    };

                    if !matches!(partner_node.operation, Operation::Gate { .. }) {
                        continue;
                    }

                    // Test exact mathematical inverse relationship
                    let is_inv = are_inverses_with_config(
                        &node.operation,
                        &partner_node.operation,
                        self.config.tolerance,
                        &self.config.symbol_env,
                    );

                    if is_inv {
                        used_ids.insert(node.id);
                        used_ids.insert(partner_id);

                        let category = categorize_gate(&node.operation);
                        pairs_to_remove.push((node.id, partner_id, category));
                    } else {
                        rejected_pairs.insert((node.id, partner_id));
                    }
                }
            }

            if pairs_to_remove.is_empty() {
                break;
            }

            modified = true;
            for (id_a, id_b, cat) in pairs_to_remove {
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

                stats.inverse_pairs_cancelled += 1;
                match cat {
                    GateCategory::SingleQubit => stats.single_qubit_pairs_cancelled += 1,
                    GateCategory::TwoQubit => stats.two_qubit_pairs_cancelled += 1,
                    GateCategory::Rotation => stats.rotation_pairs_cancelled += 1,
                }
            }
        }

        stats.iterations = iteration;
        stats.candidate_pairs_rejected = rejected_pairs.len();
        stats.candidate_pairs_examined =
            stats.inverse_pairs_cancelled + stats.candidate_pairs_rejected;

        if !modified {
            self.last_stats = Some(stats);
            return Ok(PassResult::unmodified());
        }

        tx.commit().map_err(|err| PassError::ExecutionFailed {
            pass_id: self.id(),
            reason: format!("transaction commit failed: {err}"),
        })?;

        self.last_stats = Some(stats);

        let summary = format!(
            "cancelled {} inverse pairs ({} single-qubit, {} two-qubit, {} rotation); examined {} candidate pairs in {} iterations",
            stats.inverse_pairs_cancelled,
            stats.single_qubit_pairs_cancelled,
            stats.two_qubit_pairs_cancelled,
            stats.rotation_pairs_cancelled,
            stats.candidate_pairs_examined,
            stats.iterations
        );

        Ok(PassResult::modified(summary))
    }
}

/// Identifies if `node` has a unique, direct successor that is wire-adjacent along all its qubit wires.
///
/// Returns `Some(partner_id)` only when:
/// 1. `node` acts on at least one qubit wire.
/// 2. For every qubit wire of `node`, there is a direct outgoing edge to `partner_id`.
/// 3. `partner` acts on the exact same number of qubits, all of which match `node`'s qubits.
fn find_adjacent_partner(graph: &CircuitGraph, node: &NodeData) -> Option<OperationId> {
    let qubits = node.operation.qubits();
    if qubits.is_empty() {
        return None;
    }

    // Direct successor on the first qubit wire
    let q0 = qubits[0];
    let succs = graph.successors(node.id);
    let first_succ = succs
        .iter()
        .find(|(_, dep)| dep.wire == WireId::Quantum(q0))
        .map(|(target_id, _)| *target_id)?;

    // Check that all other qubit wires also connect directly to `first_succ`
    for &q in &qubits[1..] {
        let has_direct_edge = succs
            .iter()
            .any(|(target_id, dep)| *target_id == first_succ && dep.wire == WireId::Quantum(q));
        if !has_direct_edge {
            return None;
        }
    }

    // Inspect candidate partner node
    let partner = graph.get_node(first_succ)?;
    let partner_qubits = partner.operation.qubits();

    // Must act on the exact same number of qubits
    if partner_qubits.len() != qubits.len() {
        return None;
    }

    // All qubits must be shared (exact operand ordering or permutation handled by `are_inverses`)
    for pq in partner_qubits {
        if !qubits.contains(pq) {
            return None;
        }
    }

    Some(first_succ)
}

/// Categorizes a gate operation for detailed telemetry accounting.
fn categorize_gate(op: &Operation) -> GateCategory {
    match op {
        Operation::Gate {
            gate_name, qubits, ..
        } => {
            let lower = gate_name.to_ascii_lowercase();
            if matches!(lower.as_str(), "rx" | "ry" | "rz" | "p" | "u1" | "phase") {
                GateCategory::Rotation
            } else if qubits.len() == 1 {
                GateCategory::SingleQubit
            } else {
                GateCategory::TwoQubit
            }
        }
        _ => GateCategory::SingleQubit,
    }
}
