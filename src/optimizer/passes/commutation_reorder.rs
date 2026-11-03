//! DAG-aware commutation rewriting compiler pass.
//!
//! # Semantics & Dependency Contracts
//!
//! 1. **Bounded Reorder Windows**:
//!    - Candidate adjacent pairs are identified within a deterministic, explicitly bounded window
//!      (`window_size`, default: 10).
//!    - Swaps are bounded by a finite rewrite budget (`max_rewrites`, default: 100) and iteration
//!      limit (`max_iterations`, default: 50).
//!
//! 2. **Dependency & Boundary Safety**:
//!    - Only pure unitary quantum [`Operation::Gate`] instances are eligible for commutation.
//!    - Non-unitary operations ([`Operation::Measure`], [`Operation::Reset`]), barriers
//!      ([`Operation::Barrier`]), and classical computations ([`Operation::Classical`]) strictly
//!      prevent reordering.
//!    - All quantum wire ordering and classical RAW/WAR/WAW dependencies are strictly preserved.
//!    - Operations are never moved across measurement, reset, barrier, or classical hazard boundaries.
//!
//! 3. **Commutation Registry Authority**:
//!    - Every candidate reorder is validated against the mathematically verified Phase 33
//!      [`CommutationRegistry`].
//!    - A reorder is approved only if `check_commutation` returns [`CommutationResult::Commutes`](crate::commutation::CommutationResult::Commutes).
//!
//! 4. **Transactional Rewriting**:
//!    - All graph transformations are performed atomically using [`CircuitTransaction`].
//!    - Relative execution order is updated and dependency edges are reconstructed by
//!      [`CircuitGraph::rebuild_from_ordered_nodes`](crate::ir::graph::CircuitGraph::rebuild_from_ordered_nodes).
//!    - Circuit acyclicity and structural invariants are validated upon transaction commit.
//!    - On validation failure or error, the transaction rolls back cleanly, leaving the circuit unchanged.
//!
//! 5. **Anti-Oscillation & Determinism**:
//!    - Track previously swapped operation pairs in `swapped_pairs: HashSet<(OperationId, OperationId)>`.
//!    - A pair `(A, B)` that was previously swapped is strictly forbidden from being swapped back
//!      `(B, A)` within the same pass run, eliminating infinite oscillation.
//!    - Deterministic topological ordering guarantees identical results across repeated runs.
//!
//! 6. **Global Phase & Semantics**:
//!    - Unitary commutation preserves exact matrix equivalence ($B \cdot A = A \cdot B$),
//!      carrying zero net change in circuit global phase.

use std::collections::{HashMap, HashSet};

use crate::commutation::pauli::are_same_axis_rotations;
use crate::commutation::CommutationRegistry;
use crate::ir::circuit::Circuit;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::CircuitTransaction;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::optimizer::algebra::are_inverses_with_config;

/// Default maximum lookahead window for commutation reordering.
pub const DEFAULT_REORDER_WINDOW: usize = 10;

/// Default maximum total number of rewrites in a single pass execution.
pub const DEFAULT_MAX_REWRITES: usize = 100;

/// Default maximum number of outer sweep iterations.
pub const DEFAULT_MAX_ITERATIONS: usize = 50;

/// Default tolerance threshold for rotation parameter comparisons.
pub const DEFAULT_REORDER_TOLERANCE: f64 = 1e-12;

/// Strategy governing which commuting pairs are selected for reordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderStrategy {
    /// Opportunity-driven: reorders a commuting pair if doing so moves an operation closer
    /// to an inverse, rotation-merging, or fusion partner within the bounded window.
    ExposeOpportunities,
    /// Canonical ordering: moves single-qubit gates before commuting multi-qubit gates.
    SingleQubitFirst,
    /// Canonical ordering: moves multi-qubit gates before commuting single-qubit gates.
    TwoQubitFirst,
    /// Unconditional: reorders any adjacent commuting pair (strictly guarded by anti-oscillation).
    AllCommuting,
}

/// Configuration options for the DAG-aware commutation rewriting pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CommutationReorderConfig {
    /// Maximum lookahead window in topological order when searching for candidate pairs (default: `10`).
    pub window_size: usize,
    /// Maximum total number of rewrites (swaps) allowed across the pass execution (default: `100`).
    pub max_rewrites: usize,
    /// Maximum number of outer sweep iterations (default: `50`).
    pub max_iterations: usize,
    /// Angular comparison tolerance for parameter evaluations (default: `1e-12`).
    pub tolerance: f64,
    /// Strategy for choosing which commuting pairs to reorder.
    pub strategy: ReorderStrategy,
    /// Runtime symbol environment for evaluating bound parameter expressions.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for CommutationReorderConfig {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_REORDER_WINDOW,
            max_rewrites: DEFAULT_MAX_REWRITES,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            tolerance: DEFAULT_REORDER_TOLERANCE,
            strategy: ReorderStrategy::ExposeOpportunities,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry statistics recorded during commutation reordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommutationReorderStats {
    /// Total adjacent candidate pairs examined in the DAG across all iterations.
    pub candidate_pairs_examined: usize,
    /// Total candidate pairs that passed commutation registry checks.
    pub commutation_checks_approved: usize,
    /// Total candidate pairs rejected by commutation checks (or non-unitary boundaries).
    pub commutation_checks_rejected: usize,
    /// Total rewrites (adjacent swaps) applied and committed.
    pub rewrites_applied: usize,
    /// Total potential rewrites prevented by the anti-oscillation guard.
    pub rewrites_prevented_by_anti_oscillation: usize,
    /// Total outer sweep iterations performed.
    pub iterations: usize,
    /// Whether the pass terminated because the rewrite budget (`max_rewrites`) was exhausted.
    pub budget_exhausted: bool,
}

/// A compiler pass that reorders commuting operations in the QccIR dependency DAG
/// to expose downstream optimization opportunities (inverse cancellation, rotation merging, fusion).
#[derive(Clone, Debug)]
pub struct CommutationReorderPass {
    config: CommutationReorderConfig,
    domain: SemanticDomain,
    registry: CommutationRegistry,
    last_stats: Option<CommutationReorderStats>,
}

impl Default for CommutationReorderPass {
    fn default() -> Self {
        Self::new()
    }
}

impl CommutationReorderPass {
    /// Creates a new commutation reordering pass with default configuration.
    pub fn new() -> Self {
        Self {
            config: CommutationReorderConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            registry: CommutationRegistry::new(),
            last_stats: None,
        }
    }

    /// Creates a commutation reordering pass with custom configuration options.
    pub fn with_config(config: CommutationReorderConfig) -> Self {
        Self {
            config,
            domain: SemanticDomain::GeneralStructural,
            registry: CommutationRegistry::new(),
            last_stats: None,
        }
    }

    /// Sets the semantic domain for this pass.
    pub fn with_domain(mut self, domain: SemanticDomain) -> Self {
        self.domain = domain;
        self
    }

    /// Sets the reorder strategy.
    pub fn with_strategy(mut self, strategy: ReorderStrategy) -> Self {
        self.config.strategy = strategy;
        self
    }

    /// Sets the lookahead window size.
    pub fn with_window_size(mut self, window_size: usize) -> Self {
        self.config.window_size = window_size;
        self
    }

    /// Sets the maximum number of rewrites budget.
    pub fn with_max_rewrites(mut self, max_rewrites: usize) -> Self {
        self.config.max_rewrites = max_rewrites;
        self
    }

    /// Sets the maximum number of iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Sets the runtime symbol evaluation environment.
    pub fn with_symbol_env(mut self, symbol_env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = symbol_env;
        self
    }

    /// Returns a reference to the active pass configuration.
    #[inline]
    pub fn config(&self) -> &CommutationReorderConfig {
        &self.config
    }

    /// Returns telemetry statistics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<CommutationReorderStats> {
        self.last_stats
    }
}

impl CompilerPass for CommutationReorderPass {
    fn id(&self) -> PassId {
        PassId::new("commutation_reorder")
    }

    fn description(&self) -> &'static str {
        "Reorders commuting operations in the QccIR DAG within bounded windows to expose optimization opportunities."
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
        let mut stats = CommutationReorderStats::default();
        let mut swapped_pairs: HashSet<(OperationId, OperationId)> = HashSet::new();

        let mut tx = CircuitTransaction::begin(circuit);
        let mut iteration = 0;
        let mut modified = false;

        'outer: while iteration < self.config.max_iterations {
            iteration += 1;
            let mut made_progress = false;

            let ordered_nodes =
                tx.circuit()
                    .graph
                    .ordered_nodes()
                    .map_err(|source| PassError::Ir {
                        pass_id: self.id(),
                        source,
                    })?;

            let n_nodes = ordered_nodes.len();
            if n_nodes < 2 {
                break;
            }

            for i in 0..(n_nodes - 1) {
                if stats.rewrites_applied >= self.config.max_rewrites {
                    stats.budget_exhausted = true;
                    break 'outer;
                }

                let node_a = &ordered_nodes[i];
                let node_b = &ordered_nodes[i + 1];

                // 1. Strict unitary gate boundary check
                if !matches!(node_a.operation, Operation::Gate { .. })
                    || !matches!(node_b.operation, Operation::Gate { .. })
                {
                    continue;
                }

                // 2. Anti-oscillation guard: prevent undoing a previous swap
                if swapped_pairs.contains(&(node_b.id, node_a.id)) {
                    stats.rewrites_prevented_by_anti_oscillation += 1;
                    continue;
                }

                // Skip if this exact swap was already recorded
                if swapped_pairs.contains(&(node_a.id, node_b.id)) {
                    continue;
                }

                stats.candidate_pairs_examined += 1;

                // 3. Phase 33 Commutation Registry authority check
                let comm_res = self.registry.check_commutation(
                    &node_a.operation,
                    &node_b.operation,
                    &self.config.symbol_env,
                );

                if !comm_res.is_commuting() {
                    stats.commutation_checks_rejected += 1;
                    continue;
                }

                stats.commutation_checks_approved += 1;

                // 4. Strategy evaluation
                let approve_reorder = match self.config.strategy {
                    ReorderStrategy::AllCommuting => true,
                    ReorderStrategy::SingleQubitFirst => {
                        node_a.operation.qubits().len() > 1 && node_b.operation.qubits().len() == 1
                    }
                    ReorderStrategy::TwoQubitFirst => {
                        node_a.operation.qubits().len() == 1 && node_b.operation.qubits().len() > 1
                    }
                    ReorderStrategy::ExposeOpportunities => should_swap_to_expose_opportunity(
                        &ordered_nodes,
                        i,
                        self.config.window_size,
                        self.config.tolerance,
                        &self.config.symbol_env,
                        &self.registry,
                    ),
                };

                if approve_reorder {
                    // 5. Transactional graph swap
                    tx.swap_adjacent_operations(node_a.id, node_b.id)
                        .map_err(|err| PassError::ExecutionFailed {
                            pass_id: self.id(),
                            reason: format!(
                                "failed to swap operations {:?} and {:?}: {err}",
                                node_a.id, node_b.id
                            ),
                        })?;

                    swapped_pairs.insert((node_a.id, node_b.id));
                    stats.rewrites_applied += 1;
                    modified = true;
                    made_progress = true;

                    // Break inner loop to reload fresh topological ordering for next rewrite
                    break;
                }
            }

            if !made_progress {
                // Fixed point reached: no further commuting rewrites approved
                break;
            }
        }

        stats.iterations = iteration;

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
            "reordered {} commuting pairs across {} iterations ({} candidates examined, {} approved, {} oscillation prevented{})",
            stats.rewrites_applied,
            stats.iterations,
            stats.candidate_pairs_examined,
            stats.commutation_checks_approved,
            stats.rewrites_prevented_by_anti_oscillation,
            if stats.budget_exhausted {
                "; budget exhausted"
            } else {
                ""
            }
        );

        Ok(PassResult::modified(summary))
    }
}

/// Evaluates whether swapping adjacent operations at index `i` ($A$) and `i+1` ($B$)
/// moves either operation closer to a matching optimization partner within `window_size`.
fn should_swap_to_expose_opportunity(
    nodes: &[NodeData],
    i: usize,
    window_size: usize,
    tolerance: f64,
    symbol_env: &HashMap<String, f64>,
    registry: &CommutationRegistry,
) -> bool {
    let node_a = &nodes[i];
    let node_b = &nodes[i + 1];

    let qubits_a = node_a.operation.qubits();
    let qubits_b = node_b.operation.qubits();

    // If A and B already form an optimization pair, swapping them does not expose a new opportunity.
    if forms_optimization_pair(&node_a.operation, &node_b.operation, tolerance, symbol_env) {
        return false;
    }

    let n = nodes.len();

    // 1. Forward scan: Can A move past B to get closer to a downstream partner C?
    let forward_limit = (i + 2 + window_size).min(n);
    if i + 2 < forward_limit {
        for node_c in &nodes[(i + 2)..forward_limit] {
            // Stop forward scan along wires blocked by barriers or non-unitary operations
            if !matches!(node_c.operation, Operation::Gate { .. }) {
                let is_global_barrier = matches!(
                    &node_c.operation,
                    Operation::Barrier { qubits } if qubits.is_empty()
                );
                let shares_qubit = is_global_barrier
                    || node_c
                        .operation
                        .qubits()
                        .iter()
                        .any(|q| qubits_a.contains(q));
                if shares_qubit {
                    break;
                }
                continue;
            }

            let shares_qubit = node_c
                .operation
                .qubits()
                .iter()
                .any(|q| qubits_a.contains(q));

            if shares_qubit {
                // Check if node_c forms an optimization pair with node_a
                if forms_optimization_pair(
                    &node_a.operation,
                    &node_c.operation,
                    tolerance,
                    symbol_env,
                ) {
                    return true;
                }

                // If not an optimization pair, node_a can only reach further downstream
                // if node_a commutes with node_c. Intervening non-commuting operations block traversal.
                let comm =
                    registry.check_commutation(&node_a.operation, &node_c.operation, symbol_env);
                if !comm.is_commuting() {
                    break;
                }
            }
        }
    }

    // 2. Backward scan: Can B move before A to get closer to an upstream partner D?
    let backward_start = i.saturating_sub(window_size);
    if backward_start < i {
        for node_d in nodes[backward_start..i].iter().rev() {
            // Stop backward scan along wires blocked by barriers or non-unitary operations
            if !matches!(node_d.operation, Operation::Gate { .. }) {
                let is_global_barrier = matches!(
                    &node_d.operation,
                    Operation::Barrier { qubits } if qubits.is_empty()
                );
                let shares_qubit = is_global_barrier
                    || node_d
                        .operation
                        .qubits()
                        .iter()
                        .any(|q| qubits_b.contains(q));
                if shares_qubit {
                    break;
                }
                continue;
            }

            let shares_qubit = node_d
                .operation
                .qubits()
                .iter()
                .any(|q| qubits_b.contains(q));

            if shares_qubit {
                // Check if node_d forms an optimization pair with node_b
                if forms_optimization_pair(
                    &node_b.operation,
                    &node_d.operation,
                    tolerance,
                    symbol_env,
                ) {
                    return true;
                }

                // If not an optimization pair, node_b can only reach further upstream
                // if node_b commutes with node_d. Intervening non-commuting operations block traversal.
                let comm =
                    registry.check_commutation(&node_d.operation, &node_b.operation, symbol_env);
                if !comm.is_commuting() {
                    break;
                }
            }
        }
    }

    false
}

/// Identifies whether two operations form an optimization candidate (inverse pair,
/// same-axis rotation merge, identical two-qubit self-inverse, or single-qubit fusion).
fn forms_optimization_pair(
    op1: &Operation,
    op2: &Operation,
    tolerance: f64,
    symbol_env: &HashMap<String, f64>,
) -> bool {
    let q1 = op1.qubits();
    let q2 = op2.qubits();
    if q1.is_empty() || q2.is_empty() {
        return false;
    }

    // 1. Exact ordered inverse pair (Phase 29 inverse cancellation candidate)
    if are_inverses_with_config(op1, op2, tolerance, symbol_env) {
        return true;
    }

    // 2. Same-axis rotation on same qubit (Phase 30 rotation merging candidate)
    if are_same_axis_rotations(op1.name(), op2.name()) && q1 == q2 {
        return true;
    }

    // 3. Two-qubit identical self-inverse / diagonal pair (CX-CX, CZ-CZ, SWAP-SWAP on identical qubits)
    if q1 == q2 && op1.name().eq_ignore_ascii_case(op2.name()) {
        let lower = op1.name().to_ascii_lowercase();
        if matches!(lower.as_str(), "cx" | "cz" | "swap") {
            return true;
        }
    }

    // 4. Single-qubit fusion pair (Phase 32 one-qubit gate fusion candidate)
    if q1.len() == 1 && q2.len() == 1 && q1[0] == q2[0] {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commutation_reorder_pass_defaults_and_metadata() {
        let pass = CommutationReorderPass::new();
        assert_eq!(pass.id(), PassId::new("commutation_reorder"));
        assert_eq!(pass.semantic_domain(), SemanticDomain::GeneralStructural);
        assert_eq!(pass.mutation_scope(), PassMutation::CircuitTopology);
        assert!(!pass.description().is_empty());

        assert_eq!(pass.config().window_size, DEFAULT_REORDER_WINDOW);
        assert_eq!(pass.config().max_rewrites, DEFAULT_MAX_REWRITES);
        assert_eq!(pass.config().max_iterations, DEFAULT_MAX_ITERATIONS);
        assert_eq!(pass.config().strategy, ReorderStrategy::ExposeOpportunities);
        assert!(pass.last_stats().is_none());
    }

    #[test]
    fn test_commutation_reorder_builder_setters() {
        let mut env = HashMap::new();
        env.insert("theta".to_string(), 1.23);

        let pass = CommutationReorderPass::new()
            .with_domain(SemanticDomain::UnitaryOnly)
            .with_strategy(ReorderStrategy::SingleQubitFirst)
            .with_window_size(25)
            .with_max_rewrites(200)
            .with_max_iterations(75)
            .with_symbol_env(env.clone());

        assert_eq!(pass.semantic_domain(), SemanticDomain::UnitaryOnly);
        assert_eq!(pass.config().strategy, ReorderStrategy::SingleQubitFirst);
        assert_eq!(pass.config().window_size, 25);
        assert_eq!(pass.config().max_rewrites, 200);
        assert_eq!(pass.config().max_iterations, 75);
        assert_eq!(pass.config().symbol_env.get("theta"), Some(&1.23));
    }

    #[test]
    fn test_empty_circuit_pass_execution() {
        let mut circuit = Circuit::new("empty");
        let mut pass = CommutationReorderPass::new();
        let mut ctx = PassContext::new();

        let res = pass.run(&mut circuit, &mut ctx).unwrap();
        assert!(!res.modified);
        let stats = pass.last_stats().unwrap();
        assert_eq!(stats.rewrites_applied, 0);
        assert_eq!(stats.candidate_pairs_examined, 0);
    }
}
