//! State management for physical circuit routing, SWAP insertion, and layout synchronization.
//!
//! Encapsulates the evolving physical [`Circuit`], active [`CircuitLayout`], hardware [`Target`],
//! and [`RouteProvenanceTracker`], guaranteeing that every DAG rewrite:
//! 1. Touches both physical wires and updates data hazard dependencies correctly.
//! 2. Mutates layout mappings in exact lockstep with inserted physical SWAP gates.
//! 3. Retains full causal provenance tracing back to original logical operations.
//! 4. Recomputes analysis invariants and validates graph integrity via `validate_circuit`.

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::validation::validate_circuit;
use crate::ir::wire::QubitId;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::LayoutMap;
use crate::routing::pathfinding::RoutingError;
use crate::routing::provenance::RouteProvenanceTracker;
use crate::routing::swap_insertion::{
    emit_physical_swap_with_provenance, ensure_legal_orientation_with_provenance,
    route_operands_along_path_with_provenance, PathSwapStrategy, SwapInsertionResult,
};
use crate::target::legality::TargetLegalityValidator;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::verification::layout_equivalence::LayoutEquivalenceChecker;
use crate::verification::unitary::EquivalenceVerdict;

/// Encapsulates the evolving state of a circuit undergoing layout routing and SWAP insertion.
#[derive(Clone, Debug)]
pub struct RouteState {
    circuit: Circuit,
    layout: CircuitLayout,
    target: Target,
    provenance: RouteProvenanceTracker,
}

impl RouteState {
    /// Constructs a `RouteState` from an existing physical circuit, layout tracker, and target device.
    pub fn new(
        circuit: Circuit,
        layout: CircuitLayout,
        target: Target,
    ) -> Result<Self, RoutingError> {
        layout.validate()?;
        validate_circuit(&circuit)?;

        let num_physical = target.num_qubits();
        if layout.initial().num_physical() != num_physical || circuit.num_qubits() != num_physical {
            return Err(RoutingError::InsufficientCapacity {
                required: layout.initial().num_physical(),
                capacity: num_physical,
            });
        }

        Ok(Self {
            circuit,
            layout,
            target,
            provenance: RouteProvenanceTracker::new(),
        })
    }

    /// Initializes a new `RouteState` configured to route `logical` onto `target` with `initial_layout`.
    pub fn from_logical(
        logical: &Circuit,
        target: Target,
        initial_layout: LayoutMap,
    ) -> Result<Self, RoutingError> {
        let num_logical = logical.num_qubits();
        let num_physical = target.num_qubits();

        if num_physical < num_logical {
            return Err(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            });
        }

        initial_layout.validate()?;
        if initial_layout.num_logical() != num_logical
            || initial_layout.num_physical() != num_physical
        {
            return Err(RoutingError::InsufficientCapacity {
                required: num_logical,
                capacity: num_physical,
            });
        }

        let mut physical_circuit = Circuit::new(format!("{}_routed", logical.name));
        physical_circuit
            .add_quantum_register("p", num_physical)
            .map_err(|e| RoutingError::Ir(e.to_string()))?;

        for reg in &logical.classical_registers {
            physical_circuit
                .add_classical_register(&reg.name, reg.size)
                .map_err(|e| RoutingError::Ir(e.to_string()))?;
        }
        physical_circuit.set_global_phase(logical.global_phase().clone());

        let layout = CircuitLayout::new(initial_layout);

        Ok(Self {
            circuit: physical_circuit,
            layout,
            target,
            provenance: RouteProvenanceTracker::new(),
        })
    }

    /// Inserts a physical SWAP operation between physical qubits `u` and `v`.
    ///
    /// Updates wire dependencies on both $u$ and $v$, mutates `layout` synchronously,
    /// attributes provenance to `caused_by`, and verifies QccIR DAG consistency.
    pub fn insert_swap(
        &mut self,
        u: PhysicalQubitId,
        v: PhysicalQubitId,
        caused_by: Option<OperationId>,
    ) -> Result<SwapInsertionResult, RoutingError> {
        let step = self.provenance.all_swaps().len();
        let result = emit_physical_swap_with_provenance(
            &self.target,
            &mut self.circuit,
            &mut self.layout,
            u,
            v,
            caused_by,
            step,
            Some(&mut self.provenance),
        )?;

        // Recompute analysis and validate graph postconditions
        validate_circuit(&self.circuit)?;

        Ok(result)
    }

    /// Routes physical operands along a shortest coupling path, inserting SWAPs and updating layout at each step.
    pub fn insert_swaps_along_path(
        &mut self,
        path: &[PhysicalQubitId],
        strategy: PathSwapStrategy,
        caused_by: Option<OperationId>,
    ) -> Result<(PhysicalQubitId, PhysicalQubitId, Vec<SwapInsertionResult>), RoutingError> {
        let (src, dst, results) = route_operands_along_path_with_provenance(
            &self.target,
            &mut self.circuit,
            &mut self.layout,
            path,
            strategy,
            caused_by,
            Some(&mut self.provenance),
        )?;

        validate_circuit(&self.circuit)?;
        Ok((src, dst, results))
    }

    /// Ensures adjacent physical operands `(p_src, p_dst)` satisfy target directionality for `op_name`.
    pub fn ensure_orientation(
        &mut self,
        op_name: &str,
        p_src: PhysicalQubitId,
        p_dst: PhysicalQubitId,
        caused_by: Option<OperationId>,
    ) -> Result<
        (
            PhysicalQubitId,
            PhysicalQubitId,
            Option<SwapInsertionResult>,
        ),
        RoutingError,
    > {
        let (s, d, res) = ensure_legal_orientation_with_provenance(
            &self.target,
            &mut self.circuit,
            &mut self.layout,
            op_name,
            p_src,
            p_dst,
            caused_by,
            Some(&mut self.provenance),
        )?;

        if res.is_some() {
            validate_circuit(&self.circuit)?;
        }
        Ok((s, d, res))
    }

    /// Applies an original logical operation to the physical circuit by mapping its logical qubits
    /// through the active `current_layout`, updating wire dependencies and recording provenance.
    pub fn apply_logical_operation(
        &mut self,
        original_id: OperationId,
        op: &Operation,
    ) -> Result<OperationId, RoutingError> {
        let phys_op = match op {
            Operation::Gate {
                gate_name,
                qubits,
                parameters,
            } => {
                let phys_qubits: Result<Vec<QubitId>, RoutingError> = qubits
                    .iter()
                    .map(|&q| {
                        let p = self.layout.current().get_physical(q).ok_or_else(|| {
                            RoutingError::Layout(format!("logical qubit {q} unmapped in layout"))
                        })?;
                        Ok(QubitId(p.index()))
                    })
                    .collect();
                Operation::Gate {
                    gate_name: gate_name.clone(),
                    qubits: phys_qubits?,
                    parameters: parameters.clone(),
                }
            }
            Operation::Measure { qubit, destination } => {
                let p = self.layout.current().get_physical(*qubit).ok_or_else(|| {
                    RoutingError::Layout(format!("logical qubit {qubit} unmapped in layout"))
                })?;
                Operation::Measure {
                    qubit: QubitId(p.index()),
                    destination: *destination,
                }
            }
            Operation::Reset { qubit } => {
                let p = self.layout.current().get_physical(*qubit).ok_or_else(|| {
                    RoutingError::Layout(format!("logical qubit {qubit} unmapped in layout"))
                })?;
                Operation::Reset {
                    qubit: QubitId(p.index()),
                }
            }
            Operation::Barrier { qubits } => {
                let phys_qubits: Result<Vec<QubitId>, RoutingError> = qubits
                    .iter()
                    .map(|&q| {
                        let p = self.layout.current().get_physical(q).ok_or_else(|| {
                            RoutingError::Layout(format!("logical qubit {q} unmapped in layout"))
                        })?;
                        Ok(QubitId(p.index()))
                    })
                    .collect();
                Operation::Barrier {
                    qubits: phys_qubits?,
                }
            }
            Operation::Classical {
                name,
                reads,
                writes,
            } => Operation::Classical {
                name: name.clone(),
                reads: reads.clone(),
                writes: writes.clone(),
            },
        };

        let phys_id = self
            .circuit
            .add_operation_with_provenance(phys_op, vec![original_id])?;

        self.provenance.record_logical_operation(
            phys_id,
            original_id,
            format!("mapped logical {}", op.name()),
        );

        validate_circuit(&self.circuit)?;
        Ok(phys_id)
    }

    /// Commits the active layout to final layout.
    #[inline]
    pub fn finalize_layout(&mut self) {
        self.layout.commit_final();
    }

    /// Validates graph invariants, layout consistency, and target legality.
    pub fn validate_all(&self) -> Result<(), RoutingError> {
        validate_circuit(&self.circuit)?;
        self.layout.validate()?;

        let validator = TargetLegalityValidator::new(self.target.clone());
        let report = validator
            .check_circuit(&self.circuit)
            .map_err(|e| RoutingError::TargetLegality(e.to_string()))?;

        if !report.is_valid() {
            return Err(RoutingError::TargetLegality(report.summary()));
        }

        Ok(())
    }

    /// Checks layout-aware semantic equivalence against an unrouted logical reference circuit.
    pub fn validate_equivalence(
        &self,
        logical_reference: &Circuit,
    ) -> Result<EquivalenceVerdict, RoutingError> {
        let checker = LayoutEquivalenceChecker::default();
        let verdict = checker
            .check_layout_equivalence(logical_reference, &self.circuit, &self.layout)
            .map_err(|e| RoutingError::TargetLegality(format!("equivalence check error: {e}")))?;

        if !verdict.is_equivalent {
            return Err(RoutingError::TargetLegality(format!(
                "routed circuit failed layout-aware equivalence: {}",
                verdict
                    .worst_mismatch
                    .as_ref()
                    .map(|m| format!("{m:?}"))
                    .unwrap_or_else(|| "amplitude mismatch".to_string())
            )));
        }

        Ok(verdict)
    }

    /// Consumes the `RouteState`, returning the physical circuit, layout, and provenance tracker.
    pub fn into_parts(mut self) -> (Circuit, CircuitLayout, RouteProvenanceTracker) {
        self.layout.commit_final();
        (self.circuit, self.layout, self.provenance)
    }

    /// Reference to the physical circuit.
    #[inline]
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    /// Mutable reference to the physical circuit.
    #[inline]
    pub fn circuit_mut(&mut self) -> &mut Circuit {
        &mut self.circuit
    }

    /// Reference to the multi-stage layout tracker.
    #[inline]
    pub fn layout(&self) -> &CircuitLayout {
        &self.layout
    }

    /// Mutable reference to the multi-stage layout tracker.
    #[inline]
    pub fn layout_mut(&mut self) -> &mut CircuitLayout {
        &mut self.layout
    }

    /// Reference to the hardware compilation target.
    #[inline]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Reference to the routing provenance tracker.
    #[inline]
    pub fn provenance(&self) -> &RouteProvenanceTracker {
        &self.provenance
    }

    /// Reference to the active current layout map ($L_{\text{current}}$).
    #[inline]
    pub fn current_layout(&self) -> &LayoutMap {
        self.layout.current()
    }

    /// Reference to the initial layout map ($L_{\text{initial}}$).
    #[inline]
    pub fn initial_layout(&self) -> &LayoutMap {
        self.layout.initial()
    }

    /// Reference to the final layout map ($L_{\text{final}}$).
    #[inline]
    pub fn final_layout(&self) -> &LayoutMap {
        self.layout.final_layout()
    }
}
