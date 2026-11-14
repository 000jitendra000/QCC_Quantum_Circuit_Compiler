//! Logical SWAP gate elision into circuit layout permutation (ADR-0006).
//!
//! Replaces explicit logical `swap` operations with virtual wire permutation tracking,
//! absorbing output permutations into the compiler's [`CircuitLayout`] ($L_{\text{final}}$)
//! and eliminating the $3 \times \text{SWAP}$ CX synthesis cost.

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::error::IrError;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::{LayoutError, LayoutMap};

/// Tracks the virtual wire permutation induced by elided logical SWAP operations.
///
/// Encapsulates the bijective mapping between logical program qubits and active
/// virtual circuit wires across a circuit's sequential execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElidedSwapPermutation {
    num_qubits: usize,
    /// `logical_to_wire[q]` is the active wire currently carrying logical qubit `q`.
    logical_to_wire: Vec<usize>,
    /// `wire_to_logical[w]` is the logical qubit currently carried on wire `w`.
    wire_to_logical: Vec<usize>,
    /// Total number of logical SWAP gates elided.
    swaps_elided: usize,
}

impl ElidedSwapPermutation {
    /// Constructs an identity permutation for `num_qubits` wires.
    pub fn identity(num_qubits: usize) -> Self {
        let logical_to_wire: Vec<usize> = (0..num_qubits).collect();
        let wire_to_logical: Vec<usize> = (0..num_qubits).collect();
        Self {
            num_qubits,
            logical_to_wire,
            wire_to_logical,
            swaps_elided: 0,
        }
    }

    /// Returns the number of qubits tracked by this permutation.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns the total number of explicit logical SWAP gates elided.
    #[inline]
    pub fn swaps_elided(&self) -> usize {
        self.swaps_elided
    }

    /// Returns `true` if this permutation is the trivial identity map.
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.logical_to_wire
            .iter()
            .enumerate()
            .all(|(i, &w)| i == w)
    }

    /// Returns a slice of the logical-to-wire mapping.
    #[inline]
    pub fn logical_to_wire(&self) -> &[usize] {
        &self.logical_to_wire
    }

    /// Returns a slice of the wire-to-logical mapping.
    #[inline]
    pub fn wire_to_logical(&self) -> &[usize] {
        &self.wire_to_logical
    }

    /// Returns the active virtual wire carrying logical qubit `q`.
    #[inline]
    pub fn wire_of(&self, logical_qubit: usize) -> Option<usize> {
        self.logical_to_wire.get(logical_qubit).copied()
    }

    /// Returns the logical qubit currently carried on virtual wire `w`.
    #[inline]
    pub fn logical_of(&self, wire: usize) -> Option<usize> {
        self.wire_to_logical.get(wire).copied()
    }

    /// Applies a logical swap between diagram wires `q_a` and `q_b`.
    ///
    /// Updates the active virtual wire permutation in place and increments `swaps_elided`.
    pub fn apply_swap(&mut self, q_a: usize, q_b: usize) -> Result<(), IrError> {
        if q_a >= self.num_qubits {
            return Err(IrError::InvalidQubit {
                qubit: q_a,
                total: self.num_qubits,
            });
        }
        if q_b >= self.num_qubits {
            return Err(IrError::InvalidQubit {
                qubit: q_b,
                total: self.num_qubits,
            });
        }

        if q_a != q_b {
            self.logical_to_wire.swap(q_a, q_b);
            for (l, &w) in self.logical_to_wire.iter().enumerate() {
                self.wire_to_logical[w] = l;
            }
        }
        self.swaps_elided += 1;
        Ok(())
    }

    /// Composes this logical SWAP elision permutation with a routed [`CircuitLayout`].
    ///
    /// Preserves initial layout unchanged:
    /// $$L_{\text{initial}}(q) = L_{\text{router\_initial}}(q)$$
    ///
    /// Maps final layout through the elided wire permutation:
    /// $$L_{\text{final}}(q) = L_{\text{router\_final}}(\pi_{\text{elided}}(q))$$
    ///
    /// Unused physical ancilla qubits are preserved identically from the router,
    /// strictly enforcing ADR-0001 clean ancilla zero-state preservation.
    pub fn compose_with_circuit_layout(
        &self,
        router_layout: &CircuitLayout,
    ) -> Result<CircuitLayout, LayoutError> {
        let initial = router_layout.initial().clone();
        let router_final = router_layout.final_layout();
        let num_logical = router_layout.initial().num_logical();
        let num_physical = router_layout.initial().num_physical();

        let mut composed_final = LayoutMap::new(num_logical, num_physical);
        for q in 0..num_logical {
            let q_id = QubitId(q);
            if let Some(target_wire) = self.wire_of(q) {
                if let Some(p) = router_final.get_physical(QubitId(target_wire)) {
                    composed_final.assign(q_id, p)?;
                }
            } else if let Some(p) = router_final.get_physical(q_id) {
                composed_final.assign(q_id, p)?;
            }
        }
        composed_final.validate()?;

        CircuitLayout::with_layouts(initial, composed_final.clone(), composed_final)
    }
}

/// Elides all explicit logical `swap` operations from a circuit DAG, returning the
/// rewritten circuit and the accumulated wire permutation.
///
/// Rewrites quantum operands of subsequent operations (`Gate`, `Measure`, `Reset`, `Barrier`)
/// to their active virtual wires while preserving classical bit destinations, global phase,
/// and exact RAW/WAR/WAW hazard semantics.
pub fn elide_logical_swaps(circuit: &Circuit) -> Result<(Circuit, ElidedSwapPermutation), IrError> {
    let num_qubits = circuit.num_qubits();
    let mut permutation = ElidedSwapPermutation::identity(num_qubits);

    let mut elided = Circuit::new(&circuit.name);
    elided.global_phase = circuit.global_phase.clone();
    elided.quantum_registers = circuit.quantum_registers.clone();
    elided.classical_registers = circuit.classical_registers.clone();
    elided.metadata = circuit.metadata.clone();

    for node in circuit.graph.ordered_nodes()? {
        match &node.operation {
            Operation::Gate {
                gate_name, qubits, ..
            } if gate_name.eq_ignore_ascii_case("swap") => {
                if qubits.len() != 2 {
                    return Err(IrError::GateArityMismatch {
                        gate: "swap".to_string(),
                        expected: 2,
                        actual: qubits.len(),
                    });
                }
                let q_a = qubits[0].0;
                let q_b = qubits[1].0;
                permutation.apply_swap(q_a, q_b)?;
            }
            Operation::Gate {
                gate_name,
                qubits,
                parameters,
            } => {
                let rewritten_qubits = qubits
                    .iter()
                    .map(|q| {
                        let w = permutation.wire_of(q.0).ok_or(IrError::InvalidQubit {
                            qubit: q.0,
                            total: num_qubits,
                        })?;
                        Ok(QubitId(w))
                    })
                    .collect::<Result<Vec<_>, IrError>>()?;

                let rewritten_op = Operation::Gate {
                    gate_name: gate_name.clone(),
                    qubits: rewritten_qubits,
                    parameters: parameters.clone(),
                };
                let prov = vec![node.id];
                elided.add_operation_with_provenance(rewritten_op, prov)?;
            }
            Operation::Measure { qubit, destination } => {
                let w = permutation.wire_of(qubit.0).ok_or(IrError::InvalidQubit {
                    qubit: qubit.0,
                    total: num_qubits,
                })?;
                let rewritten_op = Operation::Measure {
                    qubit: QubitId(w),
                    destination: *destination,
                };
                let prov = vec![node.id];
                elided.add_operation_with_provenance(rewritten_op, prov)?;
            }
            Operation::Reset { qubit } => {
                let w = permutation.wire_of(qubit.0).ok_or(IrError::InvalidQubit {
                    qubit: qubit.0,
                    total: num_qubits,
                })?;
                let rewritten_op = Operation::Reset { qubit: QubitId(w) };
                let prov = vec![node.id];
                elided.add_operation_with_provenance(rewritten_op, prov)?;
            }
            Operation::Barrier { qubits } => {
                let rewritten_qubits = qubits
                    .iter()
                    .map(|q| {
                        let w = permutation.wire_of(q.0).ok_or(IrError::InvalidQubit {
                            qubit: q.0,
                            total: num_qubits,
                        })?;
                        Ok(QubitId(w))
                    })
                    .collect::<Result<Vec<_>, IrError>>()?;
                let rewritten_op = Operation::Barrier {
                    qubits: rewritten_qubits,
                };
                let prov = vec![node.id];
                elided.add_operation_with_provenance(rewritten_op, prov)?;
            }
            Operation::Classical {
                name,
                reads,
                writes,
            } => {
                let rewritten_op = Operation::Classical {
                    name: name.clone(),
                    reads: reads.clone(),
                    writes: writes.clone(),
                };
                let prov = vec![node.id];
                elided.add_operation_with_provenance(rewritten_op, prov)?;
            }
        }
    }

    elided.validate().map_err(IrError::Validation)?;
    Ok((elided, permutation))
}
