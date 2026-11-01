//! CompilerPass trait definition, pass identifiers, results, and semantic domains.

use std::fmt;

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::opt::context::{AnalysisId, PassContext, PreservedAnalyses};
use crate::opt::errors::PassError;

/// Human-readable, stable identifier for a compiler pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PassId(pub &'static str);

impl PassId {
    /// Creates a new static pass identifier.
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

impl fmt::Display for PassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Operational semantic domain of a compiler pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticDomain {
    /// Pass operates exclusively on pure unitary circuits (rejects measurements, resets, and classical computation).
    UnitaryOnly,
    /// Pass handles circuits with terminal or mid-circuit measurements, resets, or classical hazards.
    NonUnitary,
    /// Pass performs general structural canonicalization or scheduling transformations applicable across all circuits.
    GeneralStructural,
    /// Pass inspects circuit properties without mutating any graph elements.
    AnalysisOnly,
}

impl fmt::Display for SemanticDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnitaryOnly => write!(f, "UnitaryOnly"),
            Self::NonUnitary => write!(f, "NonUnitary"),
            Self::GeneralStructural => write!(f, "GeneralStructural"),
            Self::AnalysisOnly => write!(f, "AnalysisOnly"),
        }
    }
}

/// Declared mutation scope of a compiler pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PassMutation {
    /// Pass performs no mutations on the circuit DAG.
    None,
    /// Pass modifies circuit metadata or parameters without altering graph topology.
    MetadataOnly,
    /// Pass mutates circuit operations, gates, wires, or graph topology.
    CircuitTopology,
}

/// Outcome of executing an individual pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassResult {
    /// `true` if the pass mutated the circuit intermediate representation.
    pub modified: bool,
    /// Set of analyses preserved by this pass execution.
    pub preserved_analyses: PreservedAnalyses,
    /// Human-readable summary of transformations applied or diagnostic insights.
    pub summary: String,
}

impl PassResult {
    /// Creates a new `PassResult`.
    pub fn new(
        modified: bool,
        preserved_analyses: PreservedAnalyses,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            modified,
            preserved_analyses,
            summary: summary.into(),
        }
    }

    /// Convenience constructor for passes that made no changes and preserve all analyses.
    pub fn unmodified() -> Self {
        Self {
            modified: false,
            preserved_analyses: PreservedAnalyses::all(),
            summary: "circuit unchanged".to_string(),
        }
    }

    /// Convenience constructor for passes that mutated the circuit and preserve no analyses.
    pub fn modified(summary: impl Into<String>) -> Self {
        Self {
            modified: true,
            preserved_analyses: PreservedAnalyses::none(),
            summary: summary.into(),
        }
    }
}

/// Core trait implemented by all compiler optimization, translation, and analysis passes.
pub trait CompilerPass: Send + Sync {
    /// Stable, human-readable pass identifier.
    fn id(&self) -> PassId;

    /// Short human-readable description of what the pass does.
    fn description(&self) -> &'static str {
        ""
    }

    /// Semantic domain of this pass.
    fn semantic_domain(&self) -> SemanticDomain;

    /// Declared mutation scope of this pass.
    fn mutation_scope(&self) -> PassMutation {
        match self.semantic_domain() {
            SemanticDomain::AnalysisOnly => PassMutation::None,
            _ => PassMutation::CircuitTopology,
        }
    }

    /// Ordered list of analysis IDs required to be valid before this pass executes.
    fn required_analyses(&self) -> Vec<AnalysisId> {
        Vec::new()
    }

    /// Checks preconditions before running the pass.
    ///
    /// The default implementation validates circuit structural integrity and verifies
    /// that circuits processed by [`SemanticDomain::UnitaryOnly`] passes contain no measurements,
    /// resets, or classical operations.
    fn check_preconditions(
        &self,
        circuit: &Circuit,
        _context: &PassContext,
    ) -> Result<(), PassError> {
        circuit
            .validate()
            .map_err(|source| PassError::IrValidation {
                pass_id: self.id(),
                source,
            })?;

        if self.semantic_domain() == SemanticDomain::UnitaryOnly {
            for node in circuit.graph.nodes() {
                match &node.operation {
                    Operation::Measure { qubit, destination } => {
                        return Err(PassError::PreconditionViolation {
                            pass_id: self.id(),
                            reason: format!(
                                "circuit '{}' contains measurement (q[{}] -> c[{}]) rejected by UnitaryOnly pass",
                                circuit.name, qubit.0, destination.0
                            ),
                        });
                    }
                    Operation::Reset { qubit } => {
                        return Err(PassError::PreconditionViolation {
                            pass_id: self.id(),
                            reason: format!(
                                "circuit '{}' contains reset on q[{}] rejected by UnitaryOnly pass",
                                circuit.name, qubit.0
                            ),
                        });
                    }
                    Operation::Classical { .. } => {
                        return Err(PassError::PreconditionViolation {
                            pass_id: self.id(),
                            reason: format!(
                                "circuit '{}' contains classical operation rejected by UnitaryOnly pass",
                                circuit.name
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Executes the pass on the provided circuit program.
    fn run(
        &mut self,
        circuit: &mut Circuit,
        context: &mut PassContext,
    ) -> Result<PassResult, PassError>;

    /// Checks postconditions after pass execution.
    ///
    /// The default implementation runs full QccIR structural validation and enforces
    /// that [`SemanticDomain::UnitaryOnly`] passes did not introduce non-unitary operations.
    fn check_postconditions(
        &self,
        circuit: &Circuit,
        _context: &PassContext,
    ) -> Result<(), PassError> {
        circuit
            .validate()
            .map_err(|source| PassError::IrValidation {
                pass_id: self.id(),
                source,
            })?;

        if self.semantic_domain() == SemanticDomain::UnitaryOnly {
            for node in circuit.graph.nodes() {
                match &node.operation {
                    Operation::Measure { .. }
                    | Operation::Reset { .. }
                    | Operation::Classical { .. } => {
                        return Err(PassError::PostconditionViolation {
                            pass_id: self.id(),
                            reason: format!(
                                "pass introduced non-unitary operation {:?} violating UnitaryOnly postcondition",
                                node.operation
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
