//! Semantic and structural validation for QccIR circuits and dependency DAGs.

use std::collections::HashSet;

use crate::ir::circuit::Circuit;
use crate::ir::edge::DependencyKind;
use crate::ir::error::IrValidationError;
use crate::ir::operation::Operation;
use crate::ir::wire::WireId;

/// Looks up the expected arity and parameter count for canonical gate names.
///
/// Returns `Some((expected_arity, expected_param_count))` for known gates, or `None`
/// for uncatalogued or custom gates.
#[inline]
pub fn canonical_gate_signature(gate_name: &str) -> Option<(usize, usize)> {
    match gate_name.to_ascii_lowercase().as_str() {
        "x" | "y" | "z" | "h" | "s" | "sdg" | "t" | "tdg" | "id" | "i" => Some((1, 0)),
        "rx" | "ry" | "rz" => Some((1, 1)),
        "cx" | "cz" | "swap" => Some((2, 0)),
        _ => None,
    }
}

/// Validates all structural and semantic invariants of a [`Circuit`].
///
/// # Checks Performed
/// 1. **Acyclicity**: Verifies the dependency graph contains no directed cycles.
/// 2. **Register Configuration**: Verifies register name uniqueness and contiguous bit allocation.
/// 3. **Qubit and Classical Bit Bounds**: Ensures all operations reference valid declared wires.
/// 4. **Gate Arity and Parameter Signatures**: Compares gate operations against canonical specifications.
/// 5. **Distinct Operands**: Ensures multi-qubit gates and barriers do not repeat operand qubits.
/// 6. **Edge Consistency**: Ensures every dependency edge connects operations that legitimately touch the wire,
///    and that classical dependency kinds (RAW, WAR, WAW) reflect actual read/write access.
/// 7. **Topological Schedulability**: Ensures deterministic topological ordering succeeds.
pub fn validate_circuit(circuit: &Circuit) -> Result<(), IrValidationError> {
    // 1. Acyclicity check
    if circuit.graph.is_cyclic() {
        return Err(IrValidationError::CycleDetected);
    }

    // 2. Register configurations
    let mut q_names = HashSet::new();
    let mut current_q = 0;
    for reg in &circuit.quantum_registers {
        if !q_names.insert(&reg.name) {
            return Err(IrValidationError::InvalidRegisterConfiguration {
                reason: format!("duplicate quantum register name '{}'", reg.name),
            });
        }
        if reg.start_qubit != current_q {
            return Err(IrValidationError::InvalidRegisterConfiguration {
                reason: format!(
                    "quantum register '{}' has non-contiguous start index {} (expected {})",
                    reg.name, reg.start_qubit, current_q
                ),
            });
        }
        current_q += reg.size;
    }

    let mut c_names = HashSet::new();
    let mut current_c = 0;
    for reg in &circuit.classical_registers {
        if !c_names.insert(&reg.name) {
            return Err(IrValidationError::InvalidRegisterConfiguration {
                reason: format!("duplicate classical register name '{}'", reg.name),
            });
        }
        if reg.start_bit != current_c {
            return Err(IrValidationError::InvalidRegisterConfiguration {
                reason: format!(
                    "classical register '{}' has non-contiguous start index {} (expected {})",
                    reg.name, reg.start_bit, current_c
                ),
            });
        }
        current_c += reg.size;
    }

    let total_qubits = circuit.num_qubits();
    let total_bits = circuit.num_classical_bits();

    // 3. Node and operation semantic checks
    let mut seen_ids = HashSet::new();
    let mut last_seq: Option<u64> = None;

    for node in circuit.graph.nodes() {
        let op_id = node.id;

        // Sequence monotonicity and ID uniqueness
        if !seen_ids.insert(op_id) {
            return Err(IrValidationError::InvalidOperationIdSequence {
                message: format!("duplicate operation ID {:?}", op_id),
            });
        }

        if let Some(prev_seq) = last_seq {
            if node.sequence_number <= prev_seq {
                return Err(IrValidationError::InvalidOperationIdSequence {
                    message: format!(
                        "non-monotonic sequence number {} <= previous {}",
                        node.sequence_number, prev_seq
                    ),
                });
            }
        }
        last_seq = Some(node.sequence_number);

        match &node.operation {
            Operation::Gate {
                gate_name,
                qubits,
                parameters,
            } => {
                // Check qubit bounds
                for &q in qubits {
                    if q.0 >= total_qubits {
                        return Err(IrValidationError::UndeclaredQubit { op_id, qubit: q.0 });
                    }
                }

                // Check operand distinctness
                let mut distinct_qubits = HashSet::new();
                for &q in qubits {
                    if !distinct_qubits.insert(q) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "gate '{}' in op {:?} references duplicate qubit {}",
                                gate_name, op_id, q
                            ),
                        });
                    }
                }

                // Check signature if canonical
                if let Some((expected_arity, expected_params)) = canonical_gate_signature(gate_name)
                {
                    if qubits.len() != expected_arity {
                        return Err(IrValidationError::GateArityMismatch {
                            op_id,
                            gate: gate_name.clone(),
                            expected: expected_arity,
                            actual: qubits.len(),
                        });
                    }
                    if parameters.len() != expected_params {
                        return Err(IrValidationError::GateParameterMismatch {
                            op_id,
                            gate: gate_name.clone(),
                            expected: expected_params,
                            actual: parameters.len(),
                        });
                    }
                }
            }
            Operation::Measure { qubit, destination } => {
                if qubit.0 >= total_qubits {
                    return Err(IrValidationError::UndeclaredQubit {
                        op_id,
                        qubit: qubit.0,
                    });
                }
                if destination.0 >= total_bits {
                    return Err(IrValidationError::UndeclaredClassicalBit {
                        op_id,
                        bit: destination.0,
                    });
                }
            }
            Operation::Reset { qubit } => {
                if qubit.0 >= total_qubits {
                    return Err(IrValidationError::UndeclaredQubit {
                        op_id,
                        qubit: qubit.0,
                    });
                }
            }
            Operation::Barrier { qubits } => {
                let mut distinct_qubits = HashSet::new();
                for &q in qubits {
                    if q.0 >= total_qubits {
                        return Err(IrValidationError::UndeclaredQubit { op_id, qubit: q.0 });
                    }
                    if !distinct_qubits.insert(q) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "barrier in op {:?} references duplicate qubit {}",
                                op_id, q
                            ),
                        });
                    }
                }
            }
            Operation::Classical { reads, writes, .. } => {
                for &c in reads {
                    if c.0 >= total_bits {
                        return Err(IrValidationError::UndeclaredClassicalBit { op_id, bit: c.0 });
                    }
                }
                for &c in writes {
                    if c.0 >= total_bits {
                        return Err(IrValidationError::UndeclaredClassicalBit { op_id, bit: c.0 });
                    }
                }
            }
        }
    }

    // 4. Edge consistency checks
    for (from_id, to_id, dep) in circuit.graph.all_edges() {
        let from_op = circuit.graph.get_operation(from_id).ok_or_else(|| {
            IrValidationError::WireDependencyInconsistent {
                reason: format!("source operation {:?} does not exist", from_id),
            }
        })?;
        let to_op = circuit.graph.get_operation(to_id).ok_or_else(|| {
            IrValidationError::WireDependencyInconsistent {
                reason: format!("target operation {:?} does not exist", to_id),
            }
        })?;

        let from_effects = from_op.wire_effects();
        let to_effects = to_op.wire_effects();

        match dep.wire {
            WireId::Quantum(q) => {
                if !from_effects.quantum_writes.contains(&q) {
                    return Err(IrValidationError::WireDependencyInconsistent {
                        reason: format!(
                            "edge ({:?} -> {:?}) on quantum wire {} invalid: source does not touch wire",
                            from_id, to_id, q
                        ),
                    });
                }
                if !to_effects.quantum_writes.contains(&q) {
                    return Err(IrValidationError::WireDependencyInconsistent {
                        reason: format!(
                            "edge ({:?} -> {:?}) on quantum wire {} invalid: target does not touch wire",
                            from_id, to_id, q
                        ),
                    });
                }
            }
            WireId::Classical(c) => match dep.kind {
                DependencyKind::ClassicalRaw => {
                    if !from_effects.classical_writes.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "RAW edge ({:?} -> {:?}) on bit {}: source does not write to bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                    if !to_effects.classical_reads.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "RAW edge ({:?} -> {:?}) on bit {}: target does not read bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                }
                DependencyKind::ClassicalWar => {
                    if !from_effects.classical_reads.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "WAR edge ({:?} -> {:?}) on bit {}: source does not read bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                    if !to_effects.classical_writes.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "WAR edge ({:?} -> {:?}) on bit {}: target does not write to bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                }
                DependencyKind::ClassicalWaw => {
                    if !from_effects.classical_writes.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "WAW edge ({:?} -> {:?}) on bit {}: source does not write to bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                    if !to_effects.classical_writes.contains(&c) {
                        return Err(IrValidationError::WireDependencyInconsistent {
                            reason: format!(
                                "WAW edge ({:?} -> {:?}) on bit {}: target does not write to bit",
                                from_id, to_id, c
                            ),
                        });
                    }
                }
                _ => {
                    return Err(IrValidationError::WireDependencyInconsistent {
                        reason: format!(
                            "unexpected dependency kind {:?} on classical wire {}",
                            dep.kind, c
                        ),
                    });
                }
            },
        }
    }

    // 5. Deterministic topological sort check
    circuit
        .graph
        .deterministic_topological_order()
        .map_err(|_| IrValidationError::CycleDetected)?;

    Ok(())
}
