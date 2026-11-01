//! Operation representations within QccIR.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::{ClassicalBitId, QubitId};
use crate::quantum::gate::Gate;

/// Wire effects declaring quantum mutations and classical reads/writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WireEffects {
    /// Quantum wires whose state is evolved/mutated by this operation.
    pub quantum_writes: Vec<QubitId>,
    /// Classical wires read by this operation (RAW inducing).
    pub classical_reads: Vec<ClassicalBitId>,
    /// Classical wires written by this operation (WAW and WAR inducing).
    pub classical_writes: Vec<ClassicalBitId>,
    /// Whether this operation acts as a synchronization barrier across its quantum wires.
    pub is_barrier: bool,
}

/// Node operations within a QccIR basic-block dependency DAG.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    /// Unitary quantum gate application.
    Gate {
        /// Lowercase gate mnemonic (e.g. "h", "cx", "rx").
        gate_name: String,
        /// Ordered operand qubits. For CX, operand 0 is control, operand 1 is target.
        qubits: Vec<QubitId>,
        /// Gate parameters.
        parameters: Vec<ParameterExpr>,
    },
    /// Projective quantum measurement writing to a classical bit destination.
    Measure {
        /// Quantum source qubit measured.
        qubit: QubitId,
        /// Classical destination bit written.
        destination: ClassicalBitId,
    },
    /// Non-unitary active state reset ($|0\rangle$).
    Reset {
        /// Qubit wire reset.
        qubit: QubitId,
    },
    /// Synchronization barrier preventing reordering across designated qubits.
    Barrier {
        /// Qubit wires synchronized.
        qubits: Vec<QubitId>,
    },
    /// Classical computation with declared read and write registers.
    Classical {
        /// Identifier or description of classical operation.
        name: String,
        /// Classical bits read.
        reads: Vec<ClassicalBitId>,
        /// Classical bits written.
        writes: Vec<ClassicalBitId>,
    },
}

impl Operation {
    /// Constructs a gate operation from a canonical Phase 2 [`Gate`].
    pub fn gate(gate: &Gate, qubits: Vec<QubitId>) -> Self {
        let gate_name = gate.name().to_string();
        let parameters = match gate {
            Gate::Rx(theta) | Gate::Ry(theta) | Gate::Rz(theta) => {
                vec![ParameterExpr::constant(theta.radians())]
            }
            _ => Vec::new(),
        };
        Self::Gate {
            gate_name,
            qubits,
            parameters,
        }
    }

    /// Constructs a gate operation with custom parameters.
    pub fn custom_gate(
        gate_name: impl Into<String>,
        qubits: Vec<QubitId>,
        parameters: Vec<ParameterExpr>,
    ) -> Self {
        Self::Gate {
            gate_name: gate_name.into(),
            qubits,
            parameters,
        }
    }

    /// Constructs a measurement operation.
    pub fn measure(qubit: QubitId, destination: ClassicalBitId) -> Self {
        Self::Measure { qubit, destination }
    }

    /// Constructs a reset operation.
    pub fn reset(qubit: QubitId) -> Self {
        Self::Reset { qubit }
    }

    /// Constructs a barrier operation.
    pub fn barrier(qubits: Vec<QubitId>) -> Self {
        Self::Barrier { qubits }
    }

    /// Constructs a classical operation with declared reads and writes.
    pub fn classical(
        name: impl Into<String>,
        reads: Vec<ClassicalBitId>,
        writes: Vec<ClassicalBitId>,
    ) -> Self {
        Self::Classical {
            name: name.into(),
            reads,
            writes,
        }
    }

    /// Returns the wire effects induced by this operation for dependency graph construction.
    pub fn wire_effects(&self) -> WireEffects {
        match self {
            Self::Gate { qubits, .. } => WireEffects {
                quantum_writes: qubits.clone(),
                classical_reads: Vec::new(),
                classical_writes: Vec::new(),
                is_barrier: false,
            },
            Self::Measure { qubit, destination } => WireEffects {
                quantum_writes: vec![*qubit],
                classical_reads: Vec::new(),
                classical_writes: vec![*destination],
                is_barrier: false,
            },
            Self::Reset { qubit } => WireEffects {
                quantum_writes: vec![*qubit],
                classical_reads: Vec::new(),
                classical_writes: Vec::new(),
                is_barrier: false,
            },
            Self::Barrier { qubits } => WireEffects {
                quantum_writes: qubits.clone(),
                classical_reads: Vec::new(),
                classical_writes: Vec::new(),
                is_barrier: true,
            },
            Self::Classical { reads, writes, .. } => WireEffects {
                quantum_writes: Vec::new(),
                classical_reads: reads.clone(),
                classical_writes: writes.clone(),
                is_barrier: false,
            },
        }
    }

    /// Returns a slice of the qubits touched by this operation, if any.
    pub fn qubits(&self) -> &[QubitId] {
        match self {
            Self::Gate { qubits, .. } | Self::Barrier { qubits } => qubits,
            Self::Measure { qubit, .. } | Self::Reset { qubit } => std::slice::from_ref(qubit),
            Self::Classical { .. } => &[],
        }
    }

    /// Returns the operation's display name.
    pub fn name(&self) -> &str {
        match self {
            Self::Gate { gate_name, .. } => gate_name,
            Self::Measure { .. } => "measure",
            Self::Reset { .. } => "reset",
            Self::Barrier { .. } => "barrier",
            Self::Classical { name, .. } => name,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gate {
                gate_name,
                qubits,
                parameters,
            } => {
                write!(f, "{gate_name}")?;
                if !parameters.is_empty() {
                    write!(f, "(")?;
                    for (i, p) in parameters.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{p}")?;
                    }
                    write!(f, ")")?;
                }
                for q in qubits {
                    write!(f, " {q}")?;
                }
                Ok(())
            }
            Self::Measure { qubit, destination } => {
                write!(f, "measure {qubit} -> {destination}")
            }
            Self::Reset { qubit } => write!(f, "reset {qubit}"),
            Self::Barrier { qubits } => {
                write!(f, "barrier")?;
                for q in qubits {
                    write!(f, " {q}")?;
                }
                Ok(())
            }
            Self::Classical {
                name,
                reads,
                writes,
            } => {
                write!(f, "classical {name}")?;
                if !reads.is_empty() {
                    write!(f, " reads [")?;
                    for (i, r) in reads.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{r}")?;
                    }
                    write!(f, "]")?;
                }
                if !writes.is_empty() {
                    write!(f, " writes [")?;
                    for (i, w) in writes.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{w}")?;
                    }
                    write!(f, "]")?;
                }
                Ok(())
            }
        }
    }
}
