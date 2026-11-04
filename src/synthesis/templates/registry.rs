//! Registry of verified fixed decomposition templates.
//!
//! Enforces the Core Definition of Done: only templates that pass rigorous mathematical
//! verification against independently built reference matrices may enter the enabled registry.

use std::collections::BTreeMap;

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::synthesis::templates::cx_cz::{CxToCzTemplate, CzToCxTemplate};
use crate::synthesis::templates::swap::SwapToCxTemplate;
use crate::synthesis::templates::toffoli::ToffoliToCxTemplate;
use crate::synthesis::templates::{DecompositionTemplate, TemplateError, TemplateId};

/// Registry of verified decomposition templates enabled for compilation and synthesis.
pub struct TemplateRegistry {
    templates: BTreeMap<TemplateId, Box<dyn DecompositionTemplate>>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::with_verified_templates().expect("built-in templates must pass verification")
    }
}

impl TemplateRegistry {
    /// Creates an empty template registry.
    pub fn new() -> Self {
        Self {
            templates: BTreeMap::new(),
        }
    }

    /// Initializes a registry pre-loaded with all canonical verified templates.
    ///
    /// Every template is verified against independent matrix models before registration.
    pub fn with_verified_templates() -> Result<Self, TemplateError> {
        let mut registry = Self::new();

        registry.register(Box::new(SwapToCxTemplate::new()))?;
        registry.register(Box::new(CxToCzTemplate::new()))?;
        registry.register(Box::new(CzToCxTemplate::new()))?;
        registry.register(Box::new(ToffoliToCxTemplate::new()))?;

        Ok(registry)
    }

    /// Registers a decomposition template after verifying its mathematical correctness.
    ///
    /// # Errors
    /// Returns [`TemplateError::VerificationFailed`] if the template does not pass
    /// full matrix verification against the reference unitary operator.
    pub fn register(
        &mut self,
        template: Box<dyn DecompositionTemplate>,
    ) -> Result<(), TemplateError> {
        template.verify_matrix()?;
        self.templates.insert(template.id(), template);
        Ok(())
    }

    /// Returns a reference to the decomposition template for `id`, if registered.
    pub fn get(&self, id: TemplateId) -> Option<&dyn DecompositionTemplate> {
        self.templates.get(&id).map(|b| b.as_ref())
    }

    /// Returns a reference to a decomposition template by its mnemonic name.
    pub fn get_by_name(&self, name: &str) -> Option<&dyn DecompositionTemplate> {
        self.templates
            .values()
            .find(|t| t.name().eq_ignore_ascii_case(name))
            .map(|b| b.as_ref())
    }

    /// Returns all registered template IDs that can decompose the specified gate mnemonic.
    pub fn find_for_gate(&self, gate_name: &str) -> Vec<TemplateId> {
        let normalized = gate_name.to_ascii_lowercase();
        self.templates
            .iter()
            .filter(|(_, t)| {
                t.target_gate().eq_ignore_ascii_case(&normalized)
                    || (normalized == "toffoli" && t.target_gate() == "ccx")
            })
            .map(|(&id, _)| id)
            .collect()
    }

    /// Returns the total count of registered and enabled decomposition templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Returns `true` if no templates are registered.
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Transactionally applies the specified template to decompose `target_id` in `circuit`.
    pub fn apply_template(
        &self,
        circuit: &mut Circuit,
        target_id: OperationId,
        template_id: TemplateId,
    ) -> Result<Vec<OperationId>, TemplateError> {
        let template =
            self.get(template_id)
                .ok_or_else(|| TemplateError::UnsupportedOperation {
                    template: format!("{template_id}"),
                    operation: format!("unregistered template {template_id}"),
                })?;

        template.apply(circuit, target_id)
    }

    /// Transactionally applies a template matching `template_name` to decompose `target_id`.
    pub fn apply_by_name(
        &self,
        circuit: &mut Circuit,
        target_id: OperationId,
        template_name: &str,
    ) -> Result<Vec<OperationId>, TemplateError> {
        let template =
            self.get_by_name(template_name)
                .ok_or_else(|| TemplateError::UnsupportedOperation {
                    template: template_name.to_string(),
                    operation: format!("unregistered template {template_name}"),
                })?;

        template.apply(circuit, target_id)
    }
}
