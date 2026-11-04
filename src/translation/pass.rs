//! Compiler pass implementation for hardware-aware basis translation.

use crate::ir::circuit::Circuit;
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::target::model::Target;
use crate::translation::engine::BasisTranslationEngine;
use crate::translation::TranslationReport;

/// A compiler pass that translates a circuit's operations into a target's native basis.
///
/// Wraps a [`BasisTranslationEngine`] and integrates with the QCC pass manager.
pub struct BasisTranslationPass {
    engine: BasisTranslationEngine,
    domain: SemanticDomain,
    last_report: Option<TranslationReport>,
}

impl BasisTranslationPass {
    /// Creates a new basis translation pass targeting the given `target`.
    pub fn new(target: Target) -> Self {
        Self {
            engine: BasisTranslationEngine::new(target),
            domain: SemanticDomain::GeneralStructural,
            last_report: None,
        }
    }

    /// Creates a pass wrapping a pre-configured [`BasisTranslationEngine`].
    pub fn with_engine(engine: BasisTranslationEngine) -> Self {
        Self {
            engine,
            domain: SemanticDomain::GeneralStructural,
            last_report: None,
        }
    }

    /// Sets the semantic domain for this pass.
    pub fn with_domain(mut self, domain: SemanticDomain) -> Self {
        self.domain = domain;
        self
    }

    /// Returns the translation report from the most recent pass run, if any.
    pub fn last_report(&self) -> Option<&TranslationReport> {
        self.last_report.as_ref()
    }

    /// Returns a reference to the underlying translation engine.
    pub fn engine(&self) -> &BasisTranslationEngine {
        &self.engine
    }
}

impl CompilerPass for BasisTranslationPass {
    fn id(&self) -> PassId {
        PassId::new("basis_translation")
    }

    fn description(&self) -> &'static str {
        "Translates logical circuit operations into target-supported native operations using verified decomposition rules and hardware cost models."
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
        let report = self
            .engine
            .translate_circuit_in_place(circuit)
            .map_err(|err| PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: format!("basis translation failed: {err}"),
            })?;

        let modified = report.operations_decomposed > 0;
        let summary = format!(
            "examined {} ops, preserved {}, decomposed {} into {} native ops (phase delta: {:.4} rad)",
            report.operations_examined,
            report.operations_preserved,
            report.operations_decomposed,
            report.total_emitted_operations,
            report.global_phase_delta
        );

        self.last_report = Some(report);

        if modified {
            Ok(PassResult::modified(summary))
        } else {
            Ok(PassResult::unmodified())
        }
    }
}
