//! Single-qubit gate fusion compiler pass.
//!
//! # Semantics & Invariants
//!
//! 1. **Column-Vector & Execution Order**:
//!    - Operations act on statevectors by left multiplication.
//!    - For a sequential sequence of operations $N_1, N_2, \dots, N_k$ on a wire, the composite
//!      operator is:
//!      $$M = U_k \cdot U_{k-1} \cdot \dots \cdot U_2 \cdot U_1$$
//!    - Evaluated as $M \leftarrow U_i \cdot M$ for each node $N_i$ from $i = 1$ to $k$.
//!
//! 2. **Wire Adjacency & DAG Boundaries**:
//!    - Fusion windows are strictly formed along direct dependency edges on `WireId::Quantum(q)`.
//!    - Never reorder operations or commute operations across other gates to construct windows.
//!    - Multi-qubit gates ($CX, CZ, SWAP$), measurements, resets, barriers, classical hazards,
//!      and unresolved symbolic parameters strictly terminate fusion windows.
//!
//! 3. **Convexity & Provenance**:
//!    - Any consecutive subsegment of pure single-qubit gates on a single wire is strictly convex
//!      in the DAG because every node in the window accesses only that single wire.
//!    - Resynthesized operations record the complete set of original operation IDs as provenance.
//!
//! 4. **Phase Preservation via Euler Decomposition**:
//!    - Factorizes $M = e^{i\alpha} V$ with $V \in SU(2)$.
//!    - Circuit global phase is explicitly updated with $\alpha$.
//!    - Identity rotations are simplified ($R(0) = I$, $R(2\pi) = -I = e^{i\pi} I$).

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::decomposition::euler::{decompose_euler, EulerBasis, EulerDecomposition};
use crate::ir::circuit::Circuit;
use crate::ir::graph::CircuitGraph;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::rewrite::CircuitTransaction;
use crate::ir::wire::{QubitId, WireId};
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::quantum::angle::Angle;
use crate::quantum::gate_matrix::{
    matrix_h, matrix_rx, matrix_ry, matrix_rz, matrix_s, matrix_sdg, matrix_t, matrix_tdg,
    matrix_x, matrix_y, matrix_z, Matrix2x2,
};

/// Default angular and residual tolerance for one-qubit gate fusion.
pub const DEFAULT_FUSION_TOLERANCE: f64 = 1e-12;

/// Default maximum number of fixed-point iterations.
pub const DEFAULT_MAX_ITERATIONS: usize = 1000;

/// Configuration options for the one-qubit gate fusion compiler pass.
#[derive(Clone, Debug, PartialEq)]
pub struct OneQubitFusionConfig {
    /// Numerical comparison tolerance for matrix residual and angles.
    pub tolerance: f64,
    /// Maximum number of fixed-point iterations.
    pub max_iterations: usize,
    /// Target Euler decomposition basis (default $Z\text{-}Y\text{-}Z$).
    pub basis: EulerBasis,
    /// Minimum consecutive gates on a wire to form a fusion window (default 2).
    pub min_window_size: usize,
    /// Maximum consecutive gates on a wire to fuse in a single window (None for maximal).
    pub max_window_size: Option<usize>,
    /// Whether to eliminate identity rotations from resynthesized Euler sequences.
    pub eliminate_identities: bool,
    /// Whether to allow gate count increase when canonicalizing short sequences.
    pub allow_gate_count_increase: bool,
    /// Runtime symbol evaluation environment.
    pub symbol_env: HashMap<String, f64>,
}

impl Default for OneQubitFusionConfig {
    fn default() -> Self {
        Self {
            tolerance: DEFAULT_FUSION_TOLERANCE,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            basis: EulerBasis::Zyz,
            min_window_size: 2,
            max_window_size: None,
            eliminate_identities: true,
            allow_gate_count_increase: true,
            symbol_env: HashMap::new(),
        }
    }
}

/// Telemetry metrics recorded during one-qubit gate fusion execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OneQubitFusionStats {
    /// Candidate fusion windows examined.
    pub windows_examined: usize,
    /// Windows successfully fused and replaced.
    pub windows_fused: usize,
    /// Total operations removed across all fused windows.
    pub operations_removed: usize,
    /// Total replacement operations inserted across all fused windows.
    pub operations_added: usize,
    /// Circuit global phase adjustments applied.
    pub global_phase_adjustments: usize,
    /// Windows rejected (e.g. non-unitary, already canonical, or disallowing increase).
    pub windows_rejected: usize,
    /// Fixed-point iterations executed.
    pub iterations: usize,
}

impl OneQubitFusionStats {
    /// Returns the net reduction in operation count (`operations_removed - operations_added`).
    #[inline]
    pub fn net_operation_reduction(&self) -> isize {
        self.operations_removed as isize - self.operations_added as isize
    }
}

/// A compiler pass that fuses adjacent one-qubit operations on the same wire into
/// canonical Euler rotation sequences.
#[derive(Clone, Debug)]
pub struct OneQubitFusionPass {
    config: OneQubitFusionConfig,
    domain: SemanticDomain,
    last_stats: Option<OneQubitFusionStats>,
}

impl Default for OneQubitFusionPass {
    fn default() -> Self {
        Self::new()
    }
}

impl OneQubitFusionPass {
    /// Creates a new one-qubit gate fusion pass with default settings.
    pub fn new() -> Self {
        Self {
            config: OneQubitFusionConfig::default(),
            domain: SemanticDomain::GeneralStructural,
            last_stats: None,
        }
    }

    /// Creates a fusion pass with custom configuration options.
    pub fn with_config(config: OneQubitFusionConfig) -> Self {
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

    /// Sets the comparison tolerance.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.config.tolerance = tolerance;
        self
    }

    /// Sets the maximum number of fixed-point iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Sets the Euler decomposition basis ($Z\text{-}Y\text{-}Z$ or $Z\text{-}X\text{-}Z$).
    pub fn with_basis(mut self, basis: EulerBasis) -> Self {
        self.config.basis = basis;
        self
    }

    /// Sets the minimum window size for fusion.
    pub fn with_min_window_size(mut self, min_window_size: usize) -> Self {
        self.config.min_window_size = min_window_size;
        self
    }

    /// Sets the maximum window size for fusion.
    pub fn with_max_window_size(mut self, max_window_size: Option<usize>) -> Self {
        self.config.max_window_size = max_window_size;
        self
    }

    /// Configures whether to eliminate identity rotations from resynthesized sequences.
    pub fn with_eliminate_identities(mut self, eliminate: bool) -> Self {
        self.config.eliminate_identities = eliminate;
        self
    }

    /// Configures whether to allow gate count increase when canonicalizing short sequences.
    pub fn with_allow_gate_count_increase(mut self, allow: bool) -> Self {
        self.config.allow_gate_count_increase = allow;
        self
    }

    /// Sets the runtime symbol evaluation environment.
    pub fn with_symbol_env(mut self, symbol_env: HashMap<String, f64>) -> Self {
        self.config.symbol_env = symbol_env;
        self
    }

    /// Returns a reference to the active configuration.
    #[inline]
    pub fn config(&self) -> &OneQubitFusionConfig {
        &self.config
    }

    /// Returns telemetry metrics from the most recent pass execution, if any.
    #[inline]
    pub fn last_stats(&self) -> Option<OneQubitFusionStats> {
        self.last_stats
    }
}

impl CompilerPass for OneQubitFusionPass {
    fn id(&self) -> PassId {
        PassId::new("one_qubit_fusion")
    }

    fn description(&self) -> &'static str {
        "Fuses adjacent one-qubit operations on the same wire into equivalent Euler rotations while preserving quantum semantics and global phase."
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
        let mut stats = OneQubitFusionStats::default();
        let mut rejected_windows: HashSet<Vec<OperationId>> = HashSet::new();

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
            let mut actions: Vec<(Vec<OperationId>, Vec<Operation>, f64)> = Vec::new();

            for node in &ordered_nodes {
                if used_ids.contains(&node.id) {
                    continue;
                }

                let (qubit, first_mat) =
                    match extract_single_qubit_matrix(&node.operation, &self.config.symbol_env) {
                        Some(res) => res,
                        None => continue,
                    };

                // Trace consecutive single-qubit operations along WireId::Quantum(qubit)
                let mut window_ids = vec![node.id];
                let mut window_ops = vec![node.operation.clone()];
                let mut window_mats = vec![first_mat];
                let mut curr_id = node.id;

                loop {
                    if let Some(max_sz) = self.config.max_window_size {
                        if window_ids.len() >= max_sz {
                            break;
                        }
                    }

                    let succ_id =
                        match find_adjacent_wire_successor(&tx.circuit().graph, curr_id, qubit) {
                            Some(id) => id,
                            None => break,
                        };

                    if used_ids.contains(&succ_id) {
                        break;
                    }

                    let succ_node = match tx.circuit().graph.get_node(succ_id) {
                        Some(n) => n,
                        None => break,
                    };

                    let (succ_q, succ_mat) = match extract_single_qubit_matrix(
                        &succ_node.operation,
                        &self.config.symbol_env,
                    ) {
                        Some(res) => res,
                        None => break,
                    };

                    if succ_q != qubit {
                        break;
                    }

                    window_ids.push(succ_id);
                    window_ops.push(succ_node.operation.clone());
                    window_mats.push(succ_mat);
                    curr_id = succ_id;
                }

                if window_ids.len() < self.config.min_window_size {
                    continue;
                }

                if rejected_windows.contains(&window_ids) {
                    for &id in &window_ids {
                        used_ids.insert(id);
                    }
                    continue;
                }

                for &id in &window_ids {
                    used_ids.insert(id);
                }

                stats.windows_examined += 1;

                // Multiply local matrices in execution order: M = U_k * ... * U_1
                let mut composite = Matrix2x2::identity();
                for mat in &window_mats {
                    composite = mat.matmul(&composite);
                }

                // Decompose into Euler form
                let decomp =
                    match decompose_euler(&composite, self.config.basis, self.config.tolerance) {
                        Ok(d) => d,
                        Err(_) => {
                            stats.windows_rejected += 1;
                            rejected_windows.insert(window_ids);
                            continue;
                        }
                    };

                // Resynthesize Euler operations
                let (replacement_ops, extra_phase) = resynthesize_euler(
                    &decomp,
                    qubit,
                    self.config.eliminate_identities,
                    self.config.tolerance,
                );
                let total_phase = decomp.global_phase.angle().radians() + extra_phase;

                // Check gate count constraint if increase is disallowed
                if !self.config.allow_gate_count_increase
                    && replacement_ops.len() > window_ops.len()
                {
                    stats.windows_rejected += 1;
                    rejected_windows.insert(window_ids);
                    continue;
                }

                // Check if already in canonical form to prevent infinite loops
                if are_equivalent_operation_sequences(
                    &window_ops,
                    &replacement_ops,
                    total_phase,
                    self.config.tolerance,
                    &self.config.symbol_env,
                ) {
                    stats.windows_rejected += 1;
                    rejected_windows.insert(window_ids);
                    continue;
                }

                actions.push((window_ids, replacement_ops, total_phase));
            }

            if actions.is_empty() {
                break;
            }

            for (target_ids, replacement_ops, total_phase) in actions {
                let rem_count = target_ids.len();
                let add_count = replacement_ops.len();

                tx.replace_sequence(&target_ids, replacement_ops)
                    .map_err(|err| PassError::ExecutionFailed {
                        pass_id: self.id(),
                        reason: format!("replace_sequence failed: {err}"),
                    })?;

                let norm_phase = total_phase.rem_euclid(2.0 * PI);
                let is_zero_phase = norm_phase <= self.config.tolerance
                    || (2.0 * PI - norm_phase) <= self.config.tolerance;
                if !is_zero_phase {
                    tx.add_global_phase(ParameterExpr::constant(norm_phase));
                    stats.global_phase_adjustments += 1;
                }

                stats.windows_fused += 1;
                stats.operations_removed += rem_count;
                stats.operations_added += add_count;
                modified = true;
            }
        }

        stats.iterations = iteration;
        self.last_stats = Some(stats);

        if modified {
            tx.commit().map_err(|err| PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: format!("transaction commit failed: {err}"),
            })?;
            let summary = format!(
                "fused {} windows (removed {} ops, added {} ops, {} phase adjustments in {} iterations)",
                stats.windows_fused,
                stats.operations_removed,
                stats.operations_added,
                stats.global_phase_adjustments,
                stats.iterations
            );
            Ok(PassResult::modified(summary))
        } else {
            tx.rollback();
            Ok(PassResult::unmodified())
        }
    }
}

/// Identifies if `node_id` has a direct successor on wire `WireId::Quantum(qubit)`.
fn find_adjacent_wire_successor(
    graph: &CircuitGraph,
    node_id: OperationId,
    qubit: QubitId,
) -> Option<OperationId> {
    let wire = WireId::Quantum(qubit);
    let mut direct_succ = None;
    for (succ_id, dep) in graph.successors(node_id) {
        if dep.wire == wire {
            if direct_succ.is_some() {
                // Quantum wires have linear single-reader/single-writer dependencies
                return None;
            }
            direct_succ = Some(succ_id);
        }
    }
    direct_succ
}

/// Extracts the single-qubit unitary matrix and target qubit for an operation.
fn extract_single_qubit_matrix(
    op: &Operation,
    env: &HashMap<String, f64>,
) -> Option<(QubitId, Matrix2x2)> {
    match op {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => {
            if qubits.len() != 1 {
                return None;
            }
            let q = qubits[0];
            match gate_name.to_ascii_lowercase().as_str() {
                "id" | "i" => {
                    if parameters.is_empty() {
                        Some((q, Matrix2x2::identity()))
                    } else {
                        None
                    }
                }
                "x" => {
                    if parameters.is_empty() {
                        Some((q, matrix_x()))
                    } else {
                        None
                    }
                }
                "y" => {
                    if parameters.is_empty() {
                        Some((q, matrix_y()))
                    } else {
                        None
                    }
                }
                "z" => {
                    if parameters.is_empty() {
                        Some((q, matrix_z()))
                    } else {
                        None
                    }
                }
                "h" => {
                    if parameters.is_empty() {
                        Some((q, matrix_h()))
                    } else {
                        None
                    }
                }
                "s" => {
                    if parameters.is_empty() {
                        Some((q, matrix_s()))
                    } else {
                        None
                    }
                }
                "sdg" => {
                    if parameters.is_empty() {
                        Some((q, matrix_sdg()))
                    } else {
                        None
                    }
                }
                "t" => {
                    if parameters.is_empty() {
                        Some((q, matrix_t()))
                    } else {
                        None
                    }
                }
                "tdg" => {
                    if parameters.is_empty() {
                        Some((q, matrix_tdg()))
                    } else {
                        None
                    }
                }
                "rx" => {
                    if parameters.len() == 1 {
                        let val = parameters[0].eval(env).ok()?;
                        if !val.is_finite() {
                            return None;
                        }
                        Some((q, matrix_rx(Angle::from_radians(val))))
                    } else {
                        None
                    }
                }
                "ry" => {
                    if parameters.len() == 1 {
                        let val = parameters[0].eval(env).ok()?;
                        if !val.is_finite() {
                            return None;
                        }
                        Some((q, matrix_ry(Angle::from_radians(val))))
                    } else {
                        None
                    }
                }
                "rz" => {
                    if parameters.len() == 1 {
                        let val = parameters[0].eval(env).ok()?;
                        if !val.is_finite() {
                            return None;
                        }
                        Some((q, matrix_rz(Angle::from_radians(val))))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Resynthesizes an [`EulerDecomposition`] into circuit operations with optional identity elimination.
fn resynthesize_euler(
    decomp: &EulerDecomposition,
    qubit: QubitId,
    eliminate_identities: bool,
    tolerance: f64,
) -> (Vec<Operation>, f64) {
    if !eliminate_identities {
        return (decomp.to_operations(qubit), 0.0);
    }

    let mut ops = Vec::new();
    let mut extra_phase = 0.0;

    let middle_gate = match decomp.basis {
        EulerBasis::Zyz => "ry",
        EulerBasis::Zxz => "rx",
    };

    let lambda = decomp.lambda.radians();
    let theta = decomp.theta.radians();
    let phi = decomp.phi.radians();

    // 1. First gate in execution order: Rz(lambda)
    if is_near_exact_zero(lambda, tolerance) {
        // Rz(0) = I -> omit
    } else if is_near_phase_zero(lambda, tolerance) {
        // Rz(2pi) = -I -> omit and add +pi phase
        extra_phase += PI;
    } else {
        ops.push(Operation::custom_gate(
            "rz",
            vec![qubit],
            vec![ParameterExpr::constant(lambda)],
        ));
    }

    // 2. Middle gate in execution order: Ry(theta) or Rx(theta)
    // In EulerDecomposition, theta is in [0, pi]
    if is_near_exact_zero(theta, tolerance) {
        // Ry(0) = I -> omit
    } else {
        ops.push(Operation::custom_gate(
            middle_gate,
            vec![qubit],
            vec![ParameterExpr::constant(theta)],
        ));
    }

    // 3. Third gate in execution order: Rz(phi)
    if is_near_exact_zero(phi, tolerance) {
        // Rz(0) = I -> omit
    } else if is_near_phase_zero(phi, tolerance) {
        // Rz(2pi) = -I -> omit and add +pi phase
        extra_phase += PI;
    } else {
        ops.push(Operation::custom_gate(
            "rz",
            vec![qubit],
            vec![ParameterExpr::constant(phi)],
        ));
    }

    (ops, extra_phase)
}

/// Checks if an angle is zero modulo 4*pi.
fn is_near_exact_zero(angle: f64, tolerance: f64) -> bool {
    let rem = angle.rem_euclid(4.0 * PI);
    rem <= tolerance || (4.0 * PI - rem) <= tolerance
}

/// Checks if an angle is 2*pi modulo 4*pi.
fn is_near_phase_zero(angle: f64, tolerance: f64) -> bool {
    let rem = angle.rem_euclid(4.0 * PI);
    (rem - 2.0 * PI).abs() <= tolerance
}

/// Determines whether the original sequence of operations is already equivalent to the replacement.
fn are_equivalent_operation_sequences(
    orig: &[Operation],
    replacement: &[Operation],
    extra_phase: f64,
    tolerance: f64,
    symbol_env: &HashMap<String, f64>,
) -> bool {
    let norm_phase = extra_phase.rem_euclid(2.0 * PI);
    let is_zero_phase = norm_phase <= tolerance || (2.0 * PI - norm_phase) <= tolerance;
    if !is_zero_phase {
        return false;
    }

    if orig.len() != replacement.len() {
        return false;
    }

    for (op1, op2) in orig.iter().zip(replacement.iter()) {
        match (op1, op2) {
            (
                Operation::Gate {
                    gate_name: name1,
                    qubits: q1,
                    parameters: p1,
                },
                Operation::Gate {
                    gate_name: name2,
                    qubits: q2,
                    parameters: p2,
                },
            ) => {
                if !name1.eq_ignore_ascii_case(name2) || q1 != q2 || p1.len() != p2.len() {
                    return false;
                }
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    let v1 = match param1.eval(symbol_env) {
                        Ok(v) => v,
                        Err(_) => return false,
                    };
                    let v2 = match param2.eval(symbol_env) {
                        Ok(v) => v,
                        Err(_) => return false,
                    };
                    let diff = (v1 - v2).rem_euclid(4.0 * PI);
                    let is_zero_diff = diff <= tolerance || (4.0 * PI - diff) <= tolerance;
                    if !is_zero_diff {
                        return false;
                    }
                }
            }
            _ => return false,
        }
    }

    true
}
