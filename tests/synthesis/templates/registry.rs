//! Integration tests for the template registry and verified admission policy.

use qcc::builder::CircuitBuilder;
use qcc::ir::operation::Operation;
use qcc::ir::wire::QubitId;
use qcc::quantum::Gate;
use qcc::synthesis::templates::registry::TemplateRegistry;
use qcc::synthesis::templates::swap::SwapToCxTemplate;
use qcc::synthesis::templates::{DecompositionTemplate, Precondition, TemplateError, TemplateId};
use qcc::verification::check_equivalence;

#[test]
fn test_default_registry_contents() {
    let registry = TemplateRegistry::with_verified_templates().unwrap();
    assert_eq!(registry.len(), 4);

    assert!(registry.get(TemplateId::SwapToCx).is_some());
    assert!(registry.get(TemplateId::CxToCz).is_some());
    assert!(registry.get(TemplateId::CzToCx).is_some());
    assert!(registry.get(TemplateId::ToffoliToCx).is_some());

    assert!(registry.get_by_name("swap_to_cx").is_some());
    assert!(registry.get_by_name("cx_to_cz").is_some());
    assert!(registry.get_by_name("cz_to_cx").is_some());
    assert!(registry.get_by_name("toffoli_to_cx").is_some());

    assert!(!registry.find_for_gate("swap").is_empty());
    assert!(!registry.find_for_gate("cx").is_empty());
    assert!(!registry.find_for_gate("cz").is_empty());
    assert!(!registry.find_for_gate("ccx").is_empty());
    assert!(!registry.find_for_gate("toffoli").is_empty());

    // Case-insensitivity
    assert!(!registry.find_for_gate("SWAP").is_empty());
    assert!(!registry.find_for_gate("Toffoli").is_empty());
    assert!(!registry.find_for_gate("CCX").is_empty());
}

#[derive(Debug, Clone)]
struct BogusFailingTemplate;

impl DecompositionTemplate for BogusFailingTemplate {
    fn id(&self) -> TemplateId {
        TemplateId::SwapToCx
    }

    fn name(&self) -> &'static str {
        "bogus_failing"
    }

    fn target_gate(&self) -> &'static str {
        "x"
    }

    fn quantum_arity(&self) -> usize {
        1
    }

    fn phase_contribution(&self) -> f64 {
        0.0
    }

    fn preconditions(&self) -> &'static [Precondition] {
        &[]
    }

    fn expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError> {
        Ok(vec![Operation::gate(&Gate::H, vec![operands[0]])])
    }

    fn verify_matrix(&self) -> Result<(), TemplateError> {
        Err(TemplateError::VerificationFailed {
            template: self.name().to_string(),
            max_diff: 1.0,
            tolerance: 1e-12,
        })
    }
}

#[test]
fn test_registry_admission_rejects_unverified_template() {
    let mut registry = TemplateRegistry::new();
    let bogus = BogusFailingTemplate;

    let res = registry.register(Box::new(bogus));
    match res {
        Err(TemplateError::VerificationFailed { template, .. }) => {
            assert_eq!(template, "bogus_failing");
        }
        other => panic!("Expected VerificationFailed, got {:?}", other),
    }

    assert!(registry.is_empty());
}

#[test]
fn test_registry_admission_accepts_verified_template() {
    let mut registry = TemplateRegistry::new();
    assert!(registry.is_empty());

    let swap_template = SwapToCxTemplate::new();
    registry.register(Box::new(swap_template)).unwrap();

    assert_eq!(registry.len(), 1);
    assert!(!registry.find_for_gate("swap").is_empty());
}

#[test]
fn test_registry_apply_by_name() {
    let registry = TemplateRegistry::with_verified_templates().unwrap();

    let mut builder = CircuitBuilder::new("reg_apply_swap");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let swap_op = builder.swap(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();
    let orig = circuit.clone();

    let replacements = registry
        .apply_by_name(&mut circuit, swap_op, "swap_to_cx")
        .unwrap();

    assert_eq!(replacements.len(), 3);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_registry_apply_unknown_name() {
    let registry = TemplateRegistry::with_verified_templates().unwrap();

    let mut builder = CircuitBuilder::new("reg_unknown");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let h_op = builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let res = registry.apply_by_name(&mut circuit, h_op, "non_existent_gate");

    match res {
        Err(TemplateError::UnsupportedOperation { template, .. }) => {
            assert_eq!(template, "non_existent_gate");
        }
        other => panic!("Expected UnsupportedOperation, got {:?}", other),
    }
}

#[test]
fn test_registry_apply_mismatched_target_gate() {
    let registry = TemplateRegistry::with_verified_templates().unwrap();

    let mut builder = CircuitBuilder::new("reg_mismatch");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let h_op = builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    // Attempting to apply "swap_to_cx" template on an H node
    let res = registry.apply_by_name(&mut circuit, h_op, "swap_to_cx");

    match res {
        Err(TemplateError::UnsupportedOperation {
            template,
            operation,
        }) => {
            assert_eq!(template, "swap_to_cx");
            assert_eq!(operation, "h");
        }
        other => panic!("Expected UnsupportedOperation, got {:?}", other),
    }
}
