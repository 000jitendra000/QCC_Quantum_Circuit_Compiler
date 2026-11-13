//! Audit reports, circuit metrics, and telemetry for compiler optimization pipelines.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::compiler::pass_diagnostic::PassDiagnosticReport;
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::opt::pass::PassId;
use crate::optimizer::config::OptimizationLevel;

/// Comprehensive quantitative metrics of a quantum circuit DAG.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitMetrics {
    /// Total number of operations (gates, barriers, measurements, resets, classical).
    pub total_operations: usize,
    /// Total number of unitary quantum gates.
    pub gate_count: usize,
    /// Total number of single-qubit quantum gates.
    pub single_qubit_gates: usize,
    /// Total number of two-qubit quantum gates.
    pub two_qubit_gates: usize,
    /// Total number of multi-qubit (>= 2 qubits) quantum gates.
    pub multi_qubit_gates: usize,
    /// Total number of non-unitary operations (measurement, reset, classical).
    pub non_unitary_operations: usize,
    /// Total number of barrier operations (compiler scheduling boundaries; semantically unitary identity).
    pub barrier_operations: usize,
    /// Critical path circuit depth in the dependency DAG.
    pub circuit_depth: usize,
    /// Exact count of each gate mnemonic in deterministic alphabetical order.
    pub gate_histogram: BTreeMap<String, usize>,
    /// Circuit-level global phase.
    pub global_phase: ParameterExpr,
}

impl CircuitMetrics {
    /// Computes circuit metrics from the given QccIR [`Circuit`].
    pub fn compute(circuit: &Circuit) -> Self {
        let mut gate_count = 0;
        let mut single_qubit_gates = 0;
        let mut two_qubit_gates = 0;
        let mut multi_qubit_gates = 0;
        let mut non_unitary_operations = 0;
        let mut barrier_operations = 0;
        let mut gate_histogram: BTreeMap<String, usize> = BTreeMap::new();

        for node in circuit.graph.nodes() {
            match &node.operation {
                Operation::Gate { .. } => {
                    gate_count += 1;
                    let n_qubits = node.operation.qubits().len();
                    if n_qubits == 1 {
                        single_qubit_gates += 1;
                    } else if n_qubits == 2 {
                        two_qubit_gates += 1;
                        multi_qubit_gates += 1;
                    } else if n_qubits > 2 {
                        multi_qubit_gates += 1;
                    }

                    let mnemonic = node.operation.name().to_ascii_lowercase();
                    *gate_histogram.entry(mnemonic).or_default() += 1;
                }
                Operation::Barrier { .. } => {
                    barrier_operations += 1;
                }
                Operation::Measure { .. }
                | Operation::Reset { .. }
                | Operation::Classical { .. } => {
                    non_unitary_operations += 1;
                }
            }
        }

        let circuit_depth = compute_circuit_depth(circuit);

        Self {
            total_operations: circuit.num_operations(),
            gate_count,
            single_qubit_gates,
            two_qubit_gates,
            multi_qubit_gates,
            non_unitary_operations,
            barrier_operations,
            circuit_depth,
            gate_histogram,
            global_phase: circuit.global_phase().clone(),
        }
    }

    /// Returns `true` if the circuit contains zero non-unitary operations (i.e. is purely unitary).
    ///
    /// Barriers are semantically the identity operator $I$ and do not render a circuit non-unitary.
    #[inline]
    pub fn is_unitary(&self) -> bool {
        self.non_unitary_operations == 0
    }

    /// Returns `true` if the circuit is eligible for unitary equivalence checking:
    /// it contains at least 1 declared qubit and zero non-unitary operations (`Measure`, `Reset`, `Classical`).
    #[inline]
    pub fn is_unitary_eligible(&self, circuit: &Circuit) -> bool {
        circuit.num_qubits() > 0 && self.is_unitary()
    }
}

/// Computes the critical path depth in a circuit's dependency DAG.
///
/// Depth is defined as the length of the longest path in the dependency graph
/// where each operation node represents 1 sequential execution step:
/// $$\text{depth}(v) = 1 + \max_{u \in \text{pred}(v)} \text{depth}(u)$$
pub fn compute_circuit_depth(circuit: &Circuit) -> usize {
    let order = match circuit.graph.deterministic_topological_order() {
        Ok(order) => order,
        Err(_) => return 0,
    };
    if order.is_empty() {
        return 0;
    }

    let mut depths: HashMap<OperationId, usize> = HashMap::with_capacity(order.len());
    let mut max_depth = 0;

    for id in order {
        let max_pred = circuit
            .graph
            .predecessors(id)
            .into_iter()
            .map(|(pred_id, _)| depths.get(&pred_id).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);

        let node_depth = max_pred + 1;
        depths.insert(id, node_depth);
        if node_depth > max_depth {
            max_depth = node_depth;
        }
    }

    max_depth
}

/// Termination reason for an optimizer pipeline execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PipelineTermination {
    /// Fixed-point reached: an iteration produced zero modifications across all passes.
    Converged,
    /// Maximum configured iterations limit was reached before reaching a fixed point.
    IterationLimitReached,
    /// Single sweep execution completed (e.g. O1).
    SinglePassCompleted,
    /// No-op pipeline execution completed (O0).
    NoOp,
}

impl fmt::Display for PipelineTermination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Converged => write!(f, "Converged"),
            Self::IterationLimitReached => write!(f, "IterationLimitReached"),
            Self::SinglePassCompleted => write!(f, "SinglePassCompleted"),
            Self::NoOp => write!(f, "NoOp"),
        }
    }
}

/// Audit report for an individual compiler pass execution within the pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct PassExecutionReport {
    /// Pass identifier.
    pub pass_id: PassId,
    /// Outer iteration index (1-indexed).
    pub iteration: usize,
    /// Overall step index across the pipeline run (0-indexed).
    pub step_index: usize,
    /// Whether this pass modified the circuit.
    pub modified: bool,
    /// Human-readable transformation summary from the pass.
    pub summary: String,
    /// Metrics before this pass executed.
    pub initial_metrics: CircuitMetrics,
    /// Metrics after this pass executed.
    pub final_metrics: CircuitMetrics,
    /// Measured execution duration in nanoseconds (nondeterministic telemetry).
    pub duration_ns: Option<u128>,
    /// Whether unitary equivalence was verified against the pre-pass state.
    /// `None` indicates skipped (e.g. non-unitary circuit or disabled).
    pub equivalence_verified: Option<bool>,
}

impl PassExecutionReport {
    /// Returns the net change in total operations produced by this pass.
    #[inline]
    pub fn delta_operations(&self) -> isize {
        self.final_metrics.total_operations as isize
            - self.initial_metrics.total_operations as isize
    }

    /// Returns the net change in circuit depth produced by this pass.
    #[inline]
    pub fn delta_depth(&self) -> isize {
        self.final_metrics.circuit_depth as isize - self.initial_metrics.circuit_depth as isize
    }
}

/// Complete, deterministic optimization summary report.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerReport {
    /// Name of the circuit that was optimized.
    pub circuit_name: String,
    /// Optimization aggressiveness level configured.
    pub level: OptimizationLevel,
    /// Circuit metrics prior to optimization.
    pub initial_metrics: CircuitMetrics,
    /// Circuit metrics after optimization pipeline completed.
    pub final_metrics: CircuitMetrics,
    /// Chronological list of every individual pass executed.
    pub passes: Vec<PassExecutionReport>,
    /// Total outer fixed-point iterations performed.
    pub iterations_executed: usize,
    /// Pipeline termination condition.
    pub termination: PipelineTermination,
    /// Number of pass executions that mutated the circuit.
    pub total_passes_modified: usize,
    /// Total number of pass executions in the run.
    pub total_passes_executed: usize,
    /// Overall unitary equivalence verification status against original circuit.
    /// `None` if circuit was non-unitary or verification disabled; `Some(true)` if verified.
    pub unitary_equivalence_verified: Option<bool>,
    /// Total execution duration in nanoseconds (nondeterministic telemetry).
    pub total_duration_ns: Option<u128>,
    /// Detailed per-pass diagnostic reports conforming to Phase 71 schema.
    pub pass_diagnostics: Vec<PassDiagnosticReport>,
}

impl OptimizerReport {
    /// Returns the net change in total operations across the entire optimization pipeline.
    #[inline]
    pub fn delta_operations(&self) -> isize {
        self.final_metrics.total_operations as isize
            - self.initial_metrics.total_operations as isize
    }

    /// Returns the net change in circuit depth across the entire optimization pipeline.
    #[inline]
    pub fn delta_depth(&self) -> isize {
        self.final_metrics.circuit_depth as isize - self.initial_metrics.circuit_depth as isize
    }

    /// Returns whether the circuit's explicit global phase was altered during optimization.
    #[inline]
    pub fn phase_changed(&self) -> bool {
        self.final_metrics.global_phase != self.initial_metrics.global_phase
    }

    /// Formats a deterministic summary string suitable for reproducible golden assertions.
    ///
    /// Excludes wall-clock timestamps or durations to guarantee identical strings across runs.
    pub fn to_deterministic_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Optimization Report: {}\n", self.circuit_name));
        out.push_str(&format!("Level: {}\n", self.level));
        out.push_str(&format!("Termination: {}\n", self.termination));
        out.push_str(&format!("Iterations: {}\n", self.iterations_executed));
        out.push_str(&format!(
            "Passes Executed: {} (Modified: {})\n",
            self.total_passes_executed, self.total_passes_modified
        ));
        out.push_str(&format!(
            "Equivalence Verified: {}\n",
            match self.unitary_equivalence_verified {
                Some(true) => "true",
                Some(false) => "false",
                None => "skipped (non-unitary or disabled)",
            }
        ));
        out.push_str(&format!(
            "Operations: {} -> {} (delta: {})\n",
            self.initial_metrics.total_operations,
            self.final_metrics.total_operations,
            self.delta_operations()
        ));
        out.push_str(&format!(
            "Depth: {} -> {} (delta: {})\n",
            self.initial_metrics.circuit_depth,
            self.final_metrics.circuit_depth,
            self.delta_depth()
        ));
        out.push_str(&format!(
            "Global Phase: {} -> {}\n",
            self.initial_metrics.global_phase, self.final_metrics.global_phase
        ));

        out.push_str("Pass Trace:\n");
        for p in &self.passes {
            out.push_str(&format!(
                "  [{}] iter={} pass={} modified={} ops_delta={} depth_delta={} summary=\"{}\"\n",
                p.step_index,
                p.iteration,
                p.pass_id,
                p.modified,
                p.delta_operations(),
                p.delta_depth(),
                p.summary
            ));
        }

        out
    }

    /// Formats a human-readable visual summary table.
    pub fn display_summary(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(&format!(
            "                    QCC Optimizer Pipeline Summary: {}\n",
            self.circuit_name
        ));
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(&format!("Optimization Level: {}\n", self.level));
        out.push_str(&format!(
            "Termination:        {} (in {} iteration{})\n",
            self.termination,
            self.iterations_executed,
            if self.iterations_executed == 1 {
                ""
            } else {
                "s"
            }
        ));
        out.push_str(&format!(
            "Passes Executed:    {} ({} modified circuit)\n",
            self.total_passes_executed, self.total_passes_modified
        ));
        out.push_str(&format!(
            "Equivalence:        {}\n",
            match self.unitary_equivalence_verified {
                Some(true) => "Verified (exact unitary match under common global phase)",
                Some(false) => "FAILED (unitary mismatch)",
                None => "Skipped (circuit contains non-unitary operations or check disabled)",
            }
        ));

        out.push_str(
            "\n--------------------------------------------------------------------------------\n",
        );
        out.push_str("Circuit Metrics                  Initial         Final           Delta\n");
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        let op_pct = if self.initial_metrics.total_operations > 0 {
            format!(
                " ({:+.1}%)",
                (self.delta_operations() as f64 / self.initial_metrics.total_operations as f64)
                    * 100.0
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "Total Operations                 {:<15} {:<15} {}{}\n",
            self.initial_metrics.total_operations,
            self.final_metrics.total_operations,
            self.delta_operations(),
            op_pct
        ));

        let depth_pct = if self.initial_metrics.circuit_depth > 0 {
            format!(
                " ({:+.1}%)",
                (self.delta_depth() as f64 / self.initial_metrics.circuit_depth as f64) * 100.0
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "Circuit Depth                    {:<15} {:<15} {}{}\n",
            self.initial_metrics.circuit_depth,
            self.final_metrics.circuit_depth,
            self.delta_depth(),
            depth_pct
        ));

        out.push_str(&format!(
            "Single-Qubit Gates               {:<15} {:<15} {}\n",
            self.initial_metrics.single_qubit_gates,
            self.final_metrics.single_qubit_gates,
            self.final_metrics.single_qubit_gates as isize
                - self.initial_metrics.single_qubit_gates as isize
        ));
        out.push_str(&format!(
            "Multi-Qubit Gates                {:<15} {:<15} {}\n",
            self.initial_metrics.multi_qubit_gates,
            self.final_metrics.multi_qubit_gates,
            self.final_metrics.multi_qubit_gates as isize
                - self.initial_metrics.multi_qubit_gates as isize
        ));
        out.push_str(&format!(
            "Non-Unitary Ops                  {:<15} {:<15} {}\n",
            self.initial_metrics.non_unitary_operations,
            self.final_metrics.non_unitary_operations,
            self.final_metrics.non_unitary_operations as isize
                - self.initial_metrics.non_unitary_operations as isize
        ));
        if self.initial_metrics.barrier_operations > 0 || self.final_metrics.barrier_operations > 0
        {
            out.push_str(&format!(
                "Barriers                         {:<15} {:<15} {}\n",
                self.initial_metrics.barrier_operations,
                self.final_metrics.barrier_operations,
                self.final_metrics.barrier_operations as isize
                    - self.initial_metrics.barrier_operations as isize
            ));
        }
        out.push_str(&format!(
            "Global Phase                     {:<15} {:<15}\n",
            format!("{:.6}", self.initial_metrics.global_phase),
            format!("{:.6}", self.final_metrics.global_phase)
        ));

        // Gate histogram changes
        if !self.initial_metrics.gate_histogram.is_empty()
            || !self.final_metrics.gate_histogram.is_empty()
        {
            out.push_str("\nGate Histogram:\n");
            let mut all_gates = BTreeMap::new();
            for (k, &v) in &self.initial_metrics.gate_histogram {
                all_gates.entry(k.clone()).or_insert((0, 0)).0 = v;
            }
            for (k, &v) in &self.final_metrics.gate_histogram {
                all_gates.entry(k.clone()).or_insert((0, 0)).1 = v;
            }
            for (gate, (init, fin)) in all_gates {
                let delta = fin as isize - init as isize;
                let delta_str = if delta != 0 {
                    format!(" ({:+})", delta)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "  {:<6} {:<3} -> {:<3}{}\n",
                    gate, init, fin, delta_str
                ));
            }
        }
        out.push_str(
            "================================================================================\n",
        );

        out
    }
}

impl fmt::Display for OptimizerReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_summary())
    }
}
