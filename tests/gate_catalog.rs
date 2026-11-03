//! Comprehensive tests for the canonical logical gate catalog (Phase 36).
//!
//! Validates:
//! 1. Unique IDs, canonical names, and alias non-overlap.
//! 2. Signature consistency (arity, parameter count, operand roles, domains, symmetry).
//! 3. Operation category (all canonical logical gates are strictly Unitary).
//! 4. Inverse metadata correctness and pairing reciprocity.
//! 5. Matrix registry cross-checks (unitarity, inverse composition, Kronecker semantics).
//! 6. Strict separation of target-native instructions from universal logical gates.
//! 7. Integration with validation (`canonical_gate_signature`) and algebra (`is_self_inverse`, `invert_gate_name`).
//! 8. Property-based testing of parameterized rotations and phase gates.

use std::collections::HashSet;
use std::f64::consts::{FRAC_PI_4, PI};

use proptest::prelude::*;
use qcc::frontend::{Frontend, OpenQasmFrontend, SourceBuffer};
use qcc::gate_catalog::{GateCatalog, CANONICAL_GATES, KNOWN_TARGET_NATIVE_CANDIDATES};
use qcc::gate_signature::{GateClassification, GateId, InverseKind, ParameterDomain};
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::validation::canonical_gate_signature;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::optimizer::algebra::inverses::{invert_gate_name, invert_operation, is_self_inverse};
use qcc::quantum::gate::Gate;
use qcc::quantum::gate_matrix::{Matrix2x2, Matrix4x4};
use qcc::quantum::parameter::Angle;
use qcc::quantum::semantics::{OperandRole, OperationCategory, SemanticClass};

const TOLERANCE: f64 = 1e-12;

// ============================================================================
// 1. Unique IDs, Names, and Aliases
// ============================================================================

#[test]
fn test_canonical_gate_count_and_completeness() {
    assert_eq!(
        CANONICAL_GATES.len(),
        16,
        "QCC canonical logical gate catalog must contain exactly 16 gates"
    );

    let all = GateCatalog::all_gates();
    assert_eq!(all.len(), 16);
}

#[test]
fn test_canonical_gate_id_uniqueness() {
    let mut seen_ids = HashSet::new();
    let mut seen_names = HashSet::new();

    for sig in CANONICAL_GATES {
        assert!(
            seen_ids.insert(sig.id),
            "duplicate GateId in catalog: {:?}",
            sig.id
        );
        assert!(
            seen_names.insert(sig.name),
            "duplicate canonical gate name: {}",
            sig.name
        );
        assert_eq!(
            sig.id.name(),
            sig.name,
            "GateId::name() must match GateSignature::name"
        );
        assert_eq!(
            sig.id.to_string(),
            sig.name,
            "Display for GateId must format as canonical name"
        );
    }
}

#[test]
fn test_aliases_are_disjoint_and_do_not_collide_with_canonical_names() {
    let mut all_canonical_names = HashSet::new();
    for sig in CANONICAL_GATES {
        all_canonical_names.insert(sig.name);
    }

    let mut seen_aliases = HashSet::new();
    for sig in CANONICAL_GATES {
        for &alias in sig.aliases {
            assert!(
                !all_canonical_names.contains(alias),
                "alias '{}' for gate '{}' collides with another gate's canonical name",
                alias,
                sig.name
            );
            assert!(
                seen_aliases.insert(alias),
                "alias '{}' for gate '{}' is duplicated across gates",
                alias,
                sig.name
            );
        }
    }
}

// ============================================================================
// 2. Signature Internal Consistency
// ============================================================================

#[test]
fn test_signature_internal_consistency() {
    for sig in CANONICAL_GATES {
        // Arity consistency
        assert!(
            sig.quantum_arity == 1 || sig.quantum_arity == 2,
            "gate '{}' has unexpected quantum arity {}",
            sig.name,
            sig.quantum_arity
        );
        assert_eq!(
            sig.operand_roles.len(),
            sig.quantum_arity,
            "gate '{}' operand_roles length ({}) must equal quantum_arity ({})",
            sig.name,
            sig.operand_roles.len(),
            sig.quantum_arity
        );
        assert_eq!(
            sig.is_single_qubit(),
            sig.quantum_arity == 1,
            "gate '{}' is_single_qubit mismatch",
            sig.name
        );
        assert_eq!(
            sig.is_two_qubit(),
            sig.quantum_arity == 2,
            "gate '{}' is_two_qubit mismatch",
            sig.name
        );

        // Parameter consistency
        assert_eq!(
            sig.parameter_domains.len(),
            sig.parameter_count,
            "gate '{}' parameter_domains length ({}) must equal parameter_count ({})",
            sig.name,
            sig.parameter_domains.len(),
            sig.parameter_count
        );
        assert_eq!(
            sig.is_parameterized(),
            sig.parameter_count > 0,
            "gate '{}' is_parameterized mismatch",
            sig.name
        );

        // Operational category
        assert_eq!(
            sig.category,
            OperationCategory::Unitary,
            "all canonical logical gates must be Unitary"
        );
        assert!(sig.is_unitary());

        // Single-qubit operand role
        if sig.quantum_arity == 1 {
            assert_eq!(
                sig.operand_roles[0],
                OperandRole::Target,
                "single-qubit gate '{}' must have Target operand role",
                sig.name
            );
            assert!(
                sig.is_symmetric,
                "single-qubit gate '{}' is trivially symmetric",
                sig.name
            );
        }
    }
}

#[test]
fn test_two_qubit_gate_operand_roles_and_symmetry() {
    let cx = GateCatalog::get(GateId::CX);
    assert_eq!(cx.quantum_arity, 2);
    assert_eq!(
        cx.operand_roles,
        &[OperandRole::Control, OperandRole::Target]
    );
    assert!(
        !cx.is_symmetric,
        "CX must be asymmetric (control vs target)"
    );

    let cz = GateCatalog::get(GateId::CZ);
    assert_eq!(cz.quantum_arity, 2);
    assert_eq!(
        cz.operand_roles,
        &[OperandRole::Target, OperandRole::Target]
    );
    assert!(cz.is_symmetric, "CZ must be symmetric");

    let swap = GateCatalog::get(GateId::SWAP);
    assert_eq!(swap.quantum_arity, 2);
    assert_eq!(
        swap.operand_roles,
        &[OperandRole::Target, OperandRole::Target]
    );
    assert!(swap.is_symmetric, "SWAP must be symmetric");
}

#[test]
fn test_parameter_domains() {
    let rx = GateCatalog::get(GateId::Rx);
    assert_eq!(rx.parameter_domains, &[ParameterDomain::Angle4Pi]);

    let ry = GateCatalog::get(GateId::Ry);
    assert_eq!(ry.parameter_domains, &[ParameterDomain::Angle4Pi]);

    let rz = GateCatalog::get(GateId::Rz);
    assert_eq!(rz.parameter_domains, &[ParameterDomain::Angle4Pi]);

    let p = GateCatalog::get(GateId::P);
    assert_eq!(p.parameter_domains, &[ParameterDomain::Angle2Pi]);

    for sig in GateCatalog::fixed_gates() {
        assert_eq!(
            sig.parameter_count, 0,
            "fixed gate '{}' must have 0 parameters",
            sig.name
        );
        assert!(
            sig.parameter_domains.is_empty(),
            "fixed gate '{}' must have empty parameter domains",
            sig.name
        );
    }
}

#[test]
fn test_unitary_category_recording_and_non_unitary_rejection() {
    // 1. Every gate in CANONICAL_GATES is explicitly recorded as Unitary
    for sig in CANONICAL_GATES {
        assert_eq!(
            sig.category,
            OperationCategory::Unitary,
            "logical gate '{}' must have OperationCategory::Unitary",
            sig.name
        );
        assert!(
            sig.is_unitary(),
            "sig.is_unitary() must return true for '{}'",
            sig.name
        );
        assert!(
            GateCatalog::is_unitary(sig.name),
            "GateCatalog::is_unitary('{}') must be true",
            sig.name
        );
    }

    // 2. Gate aliases also report as unitary
    for sig in CANONICAL_GATES {
        for &alias in sig.aliases {
            assert!(
                GateCatalog::is_unitary(alias),
                "GateCatalog::is_unitary('{}') (alias for '{}') must be true",
                alias,
                sig.name
            );
        }
    }

    // 3. Non-unitary operations are strictly excluded from the gate catalog
    let non_unitary_operations = ["measure", "reset", "barrier", "classical"];
    for non_unitary in non_unitary_operations {
        assert!(
            !GateCatalog::is_unitary(non_unitary),
            "non-unitary operation '{}' must not report as unitary in GateCatalog",
            non_unitary
        );
        assert!(
            !GateCatalog::is_supported(non_unitary),
            "non-unitary operation '{}' must not be in GateCatalog",
            non_unitary
        );
        assert!(
            !GateCatalog::is_canonical_name(non_unitary),
            "non-unitary operation '{}' must not be a canonical gate name",
            non_unitary
        );
        assert!(
            GateCatalog::lookup(non_unitary).is_none(),
            "GateCatalog::lookup('{}') must return None",
            non_unitary
        );
        assert_eq!(
            GateCatalog::classify(non_unitary),
            GateClassification::Unknown,
            "GateCatalog::classify('{}') must not be Logical or TargetNative",
            non_unitary
        );
    }
}

#[test]
fn test_ir_operation_semantic_classification_and_category() {
    // 1. Operation::Gate -> UnitaryGate, Unitary
    let gate_op = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![QubitId(0), QubitId(1)],
        parameters: vec![],
    };
    assert_eq!(gate_op.category(), OperationCategory::Unitary);
    assert_eq!(gate_op.semantic_class(), SemanticClass::UnitaryGate);
    assert!(gate_op.is_unitary());

    // 2. Operation::Measure -> Measurement, NonUnitary
    let measure_op = Operation::Measure {
        qubit: QubitId(0),
        destination: ClassicalBitId(0),
    };
    assert_eq!(measure_op.category(), OperationCategory::NonUnitary);
    assert_eq!(measure_op.semantic_class(), SemanticClass::Measurement);
    assert!(!measure_op.is_unitary());

    // 3. Operation::Reset -> Reset, NonUnitary
    let reset_op = Operation::Reset { qubit: QubitId(0) };
    assert_eq!(reset_op.category(), OperationCategory::NonUnitary);
    assert_eq!(reset_op.semantic_class(), SemanticClass::Reset);
    assert!(!reset_op.is_unitary());

    // 4. Operation::Barrier -> Barrier, NonUnitary
    let barrier_op = Operation::Barrier {
        qubits: vec![QubitId(0), QubitId(1)],
    };
    assert_eq!(barrier_op.category(), OperationCategory::NonUnitary);
    assert_eq!(barrier_op.semantic_class(), SemanticClass::Barrier);
    assert!(!barrier_op.is_unitary());

    // 5. Operation::Classical -> ClassicalControl, NonUnitary
    let classical_op = Operation::Classical {
        name: "test_branch".to_string(),
        reads: vec![ClassicalBitId(0)],
        writes: vec![ClassicalBitId(1)],
    };
    assert_eq!(classical_op.category(), OperationCategory::NonUnitary);
    assert_eq!(
        classical_op.semantic_class(),
        SemanticClass::ClassicalControl
    );
    assert!(!classical_op.is_unitary());
}

// ============================================================================
// 3. Inverse Metadata Correctness
// ============================================================================

#[test]
fn test_self_inverse_gates() {
    let expected_self_inverses = ["id", "x", "y", "z", "h", "cx", "cz", "swap"];

    for name in expected_self_inverses {
        let sig = GateCatalog::lookup(name).expect("self-inverse gate must exist in catalog");
        assert!(
            sig.is_self_inverse(),
            "gate '{}' must have InverseKind::SelfInverse",
            name
        );
        assert!(
            GateCatalog::is_self_inverse(name),
            "GateCatalog::is_self_inverse('{}') must be true",
            name
        );
        assert_eq!(
            GateCatalog::invert_gate_name(name),
            Some(sig.name),
            "self-inverse gate '{}' must invert to its canonical name",
            name
        );
    }
}

#[test]
fn test_paired_inverse_reciprocity() {
    let paired_gates = [
        (GateId::S, GateId::Sdg),
        (GateId::Sdg, GateId::S),
        (GateId::T, GateId::Tdg),
        (GateId::Tdg, GateId::T),
    ];

    for (a_id, b_id) in paired_gates {
        let sig_a = GateCatalog::get(a_id);
        let sig_b = GateCatalog::get(b_id);

        assert_eq!(
            sig_a.inverse_kind,
            InverseKind::Paired(b_id),
            "gate '{}' must be paired with '{:?}'",
            sig_a.name,
            b_id
        );
        assert_eq!(
            sig_b.inverse_kind,
            InverseKind::Paired(a_id),
            "gate '{}' must be paired with '{:?}'",
            sig_b.name,
            a_id
        );

        assert_eq!(
            GateCatalog::invert_gate_name(sig_a.name),
            Some(sig_b.name),
            "invert_gate_name('{}') must return '{}'",
            sig_a.name,
            sig_b.name
        );
    }
}

#[test]
fn test_parameter_negation_inverses() {
    let param_gates = [GateId::Rx, GateId::Ry, GateId::Rz, GateId::P];

    for id in param_gates {
        let sig = GateCatalog::get(id);
        assert_eq!(
            sig.inverse_kind,
            InverseKind::ParameterNegation,
            "parameterized gate '{}' must have InverseKind::ParameterNegation",
            sig.name
        );
        assert_eq!(
            GateCatalog::invert_gate_name(sig.name),
            Some(sig.name),
            "parameterized gate '{}' must invert to itself under parameter negation",
            sig.name
        );
    }
}

// ============================================================================
// 4. Matrix Registry Cross-Checks
// ============================================================================

#[test]
fn test_single_qubit_matrices_are_unitary() {
    for sig in GateCatalog::single_qubit_gates() {
        let test_params = match sig.parameter_count {
            0 => vec![],
            1 => vec![FRAC_PI_4],
            _ => vec![0.0; sig.parameter_count],
        };

        let mat = GateCatalog::matrix_for_gate_1q(sig.id, &test_params)
            .unwrap_or_else(|| panic!("missing matrix for 1Q gate '{}'", sig.name));

        assert!(
            mat.is_unitary(TOLERANCE),
            "matrix for gate '{}' must be unitary (error: U†U != I)",
            sig.name
        );
    }
}

#[test]
fn test_two_qubit_matrices_are_unitary() {
    for sig in GateCatalog::two_qubit_gates() {
        let mat = GateCatalog::matrix_for_gate_2q(sig.id)
            .unwrap_or_else(|| panic!("missing matrix for 2Q gate '{}'", sig.name));

        assert!(
            mat.is_unitary(TOLERANCE),
            "matrix for gate '{}' must be unitary (error: U†U != I)",
            sig.name
        );
    }
}

#[test]
fn test_matrix_inverse_composition_exactness() {
    // 1. Self-inverses: U * U = I
    for sig in CANONICAL_GATES
        .iter()
        .filter(|s| s.inverse_kind == InverseKind::SelfInverse)
    {
        if sig.is_single_qubit() {
            let u = GateCatalog::matrix_for_gate_1q(sig.id, &[]).unwrap();
            let prod = u.matmul(&u);
            assert!(
                prod.approx_eq(&Matrix2x2::identity(), TOLERANCE),
                "self-inverse gate '{}' squared must equal identity",
                sig.name
            );
        } else {
            let u = GateCatalog::matrix_for_gate_2q(sig.id).unwrap();
            let prod = u.matmul(&u);
            assert!(
                prod.approx_eq(&Matrix4x4::identity(), TOLERANCE),
                "self-inverse 2Q gate '{}' squared must equal identity",
                sig.name
            );
        }
    }

    // 2. Paired inverses: U_A * U_B = I
    let s = GateCatalog::matrix_for_gate_1q(GateId::S, &[]).unwrap();
    let sdg = GateCatalog::matrix_for_gate_1q(GateId::Sdg, &[]).unwrap();
    assert!(
        s.matmul(&sdg).approx_eq(&Matrix2x2::identity(), TOLERANCE),
        "S * Sdg must equal identity"
    );
    assert!(
        sdg.matmul(&s).approx_eq(&Matrix2x2::identity(), TOLERANCE),
        "Sdg * S must equal identity"
    );

    let t = GateCatalog::matrix_for_gate_1q(GateId::T, &[]).unwrap();
    let tdg = GateCatalog::matrix_for_gate_1q(GateId::Tdg, &[]).unwrap();
    assert!(
        t.matmul(&tdg).approx_eq(&Matrix2x2::identity(), TOLERANCE),
        "T * Tdg must equal identity"
    );
    assert!(
        tdg.matmul(&t).approx_eq(&Matrix2x2::identity(), TOLERANCE),
        "Tdg * T must equal identity"
    );

    // 3. Parameterized gates: U(theta) * U(-theta) = I
    let theta = 1.2345;
    for &id in &[GateId::Rx, GateId::Ry, GateId::Rz, GateId::P] {
        let u = GateCatalog::matrix_for_gate_1q(id, &[theta]).unwrap();
        let u_inv = GateCatalog::matrix_for_gate_1q(id, &[-theta]).unwrap();
        assert!(
            u.matmul(&u_inv)
                .approx_eq(&Matrix2x2::identity(), TOLERANCE),
            "Gate '{:?}(theta) * {:?}(-theta)' must equal identity",
            id,
            id
        );
    }
}

#[test]
fn test_cx_control_target_orientation_asymmetry() {
    let cx_c0_t1 = GateCatalog::matrix_for_gate_2q(GateId::CX).unwrap();
    let cx_c1_t0 = qcc::quantum::gate_matrix::matrix_cx_c1_t0();

    assert!(
        !cx_c0_t1.approx_eq(&cx_c1_t0, 1e-4),
        "CX(0, 1) and CX(1, 0) must be mathematically distinct matrices"
    );
}

#[test]
fn test_cz_and_swap_symmetry_under_qubit_swap() {
    let cz = GateCatalog::matrix_for_gate_2q(GateId::CZ).unwrap();
    let swap = GateCatalog::matrix_for_gate_2q(GateId::SWAP).unwrap();

    // SWAP * CZ * SWAP must equal CZ (qubit permutation invariance)
    let permuted_cz = swap.matmul(&cz).matmul(&swap);
    assert!(
        permuted_cz.approx_eq(&cz, TOLERANCE),
        "CZ must be symmetric under SWAP conjugation: SWAP * CZ * SWAP == CZ"
    );

    // SWAP * SWAP * SWAP must equal SWAP
    let permuted_swap = swap.matmul(&swap).matmul(&swap);
    assert!(
        permuted_swap.approx_eq(&swap, TOLERANCE),
        "SWAP is self-inverse and symmetric"
    );
}

#[test]
fn test_all_16_canonical_gate_ids_have_unambiguous_matrices() {
    let all_gate_ids = [
        // 1Q Fixed
        GateId::Id,
        GateId::X,
        GateId::Y,
        GateId::Z,
        GateId::H,
        GateId::S,
        GateId::Sdg,
        GateId::T,
        GateId::Tdg,
        // 1Q Parameterized
        GateId::Rx,
        GateId::Ry,
        GateId::Rz,
        GateId::P,
        // 2Q Fixed
        GateId::CX,
        GateId::CZ,
        GateId::SWAP,
    ];

    assert_eq!(
        all_gate_ids.len(),
        16,
        "Exactly 16 canonical gate IDs must be tested"
    );

    let mut single_qubit_count = 0;
    let mut two_qubit_count = 0;

    for &id in &all_gate_ids {
        let sig = GateCatalog::get(id);

        if sig.is_single_qubit() {
            single_qubit_count += 1;
            // 1Q gates must evaluate via matrix_for_gate_1q
            let test_params = if sig.is_parameterized() {
                vec![FRAC_PI_4]
            } else {
                vec![]
            };
            let mat_1q = GateCatalog::matrix_for_gate_1q(id, &test_params);
            assert!(
                mat_1q.is_some(),
                "1Q gate '{:?}' must return Some from matrix_for_gate_1q",
                id
            );
            assert!(
                mat_1q.unwrap().is_unitary(TOLERANCE),
                "1Q gate '{:?}' matrix must be unitary",
                id
            );
            // 1Q gates must return None from matrix_for_gate_2q
            assert!(
                GateCatalog::matrix_for_gate_2q(id).is_none(),
                "1Q gate '{:?}' must return None from matrix_for_gate_2q",
                id
            );
        } else if sig.is_two_qubit() {
            two_qubit_count += 1;
            // 2Q gates must evaluate via matrix_for_gate_2q
            let mat_2q = GateCatalog::matrix_for_gate_2q(id);
            assert!(
                mat_2q.is_some(),
                "2Q gate '{:?}' must return Some from matrix_for_gate_2q",
                id
            );
            assert!(
                mat_2q.unwrap().is_unitary(TOLERANCE),
                "2Q gate '{:?}' matrix must be unitary",
                id
            );
            // 2Q gates must return None from matrix_for_gate_1q
            assert!(
                GateCatalog::matrix_for_gate_1q(id, &[]).is_none(),
                "2Q gate '{:?}' must return None from matrix_for_gate_1q",
                id
            );
        } else {
            panic!("Unexpected quantum arity for gate {:?}", id);
        }
    }

    assert_eq!(single_qubit_count, 13, "Expected 13 single-qubit gates");
    assert_eq!(two_qubit_count, 3, "Expected 3 two-qubit gates");
}

// ============================================================================
// 5. Hardware Separation & Target-Native Rejection
// ============================================================================

#[test]
fn test_target_native_instructions_are_rejected_from_logical_catalog() {
    for &candidate in KNOWN_TARGET_NATIVE_CANDIDATES {
        assert!(
            GateCatalog::lookup(candidate).is_none(),
            "target-native instruction '{}' must NOT be in the logical gate catalog",
            candidate
        );
        assert!(
            !GateCatalog::is_canonical_name(candidate),
            "target-native instruction '{}' must NOT be recognized as a canonical name",
            candidate
        );
        assert!(
            GateCatalog::is_target_native_candidate(candidate),
            "target-native instruction '{}' must be identified by is_target_native_candidate",
            candidate
        );
    }
}

#[test]
fn test_logical_gates_are_not_flagged_as_target_native() {
    for sig in CANONICAL_GATES {
        assert!(
            !GateCatalog::is_target_native_candidate(sig.name),
            "canonical logical gate '{}' must not be flagged as target-native candidate",
            sig.name
        );
    }
}

// ============================================================================
// 6. Alias and Case-Insensitive Lookup
// ============================================================================

#[test]
fn test_case_insensitive_lookup() {
    let test_cases = [
        ("X", GateId::X),
        ("x", GateId::X),
        ("H", GateId::H),
        ("h", GateId::H),
        ("CX", GateId::CX),
        ("cx", GateId::CX),
        ("cnot", GateId::CX),
        ("CNOT", GateId::CX),
        ("ID", GateId::Id),
        ("id", GateId::Id),
        ("i", GateId::Id),
        ("I", GateId::Id),
        ("P", GateId::P),
        ("p", GateId::P),
        ("phase", GateId::P),
        ("PHASE", GateId::P),
        ("u1", GateId::P),
        ("U1", GateId::P),
        ("sdg", GateId::Sdg),
        ("SDG", GateId::Sdg),
        ("s_dag", GateId::Sdg),
        ("sdag", GateId::Sdg),
        ("tdg", GateId::Tdg),
        ("TDG", GateId::Tdg),
        ("t_dag", GateId::Tdg),
        ("tdag", GateId::Tdg),
    ];

    for (query, expected_id) in test_cases {
        let sig = GateCatalog::lookup(query)
            .unwrap_or_else(|| panic!("failed to lookup valid gate/alias '{query}'"));
        assert_eq!(
            sig.id, expected_id,
            "query '{query}' resolved to {:?}, expected {:?}",
            sig.id, expected_id
        );
    }
}

#[test]
fn test_unknown_and_malformed_lookups() {
    let invalid = [
        "",
        "   ",
        "not_a_gate",
        "qubit",
        "measure",
        "barrier",
        "123",
    ];
    for name in invalid {
        assert!(
            GateCatalog::lookup(name).is_none(),
            "invalid name '{name}' must return None from lookup"
        );
        assert!(
            !GateCatalog::is_supported(name),
            "invalid name '{name}' must return false from is_supported"
        );
    }
}

// ============================================================================
// 7. Catalog Iterators
// ============================================================================

#[test]
fn test_catalog_subdivision_iterators() {
    let single_1q: Vec<_> = GateCatalog::single_qubit_gates().collect();
    let two_2q: Vec<_> = GateCatalog::two_qubit_gates().collect();
    let param: Vec<_> = GateCatalog::parameterized_gates().collect();
    let fixed: Vec<_> = GateCatalog::fixed_gates().collect();

    assert_eq!(single_1q.len(), 13, "must have 13 single-qubit gates");
    assert_eq!(two_2q.len(), 3, "must have 3 two-qubit gates");
    assert_eq!(param.len(), 4, "must have 4 parameterized gates");
    assert_eq!(fixed.len(), 12, "must have 12 fixed gates");

    assert_eq!(single_1q.len() + two_2q.len(), CANONICAL_GATES.len());
    assert_eq!(param.len() + fixed.len(), CANONICAL_GATES.len());
}

// ============================================================================
// 8. Integration with Existing Validation and Inverses Modules
// ============================================================================

#[test]
fn test_ir_validation_canonical_gate_signature_integration() {
    for sig in CANONICAL_GATES {
        let result = canonical_gate_signature(sig.name);
        assert_eq!(
            result,
            Some((sig.quantum_arity, sig.parameter_count)),
            "ir::validation::canonical_gate_signature('{}') mismatch",
            sig.name
        );

        for &alias in sig.aliases {
            let alias_result = canonical_gate_signature(alias);
            assert_eq!(
                alias_result,
                Some((sig.quantum_arity, sig.parameter_count)),
                "canonical_gate_signature for alias '{}' of '{}' mismatch",
                alias,
                sig.name
            );
        }
    }

    // Invalid or target-native instruction returns None
    assert_eq!(canonical_gate_signature("sx"), None);
    assert_eq!(canonical_gate_signature("ecr"), None);
    assert_eq!(canonical_gate_signature("unknown"), None);
}

#[test]
fn test_optimizer_algebra_inverses_integration() {
    for sig in CANONICAL_GATES {
        assert_eq!(
            is_self_inverse(sig.name),
            sig.is_self_inverse(),
            "optimizer::algebra::inverses::is_self_inverse('{}') mismatch",
            sig.name
        );

        let expected_inv = match sig.inverse_kind {
            InverseKind::SelfInverse | InverseKind::ParameterNegation => Some(sig.name),
            InverseKind::Paired(id) => Some(GateCatalog::get(id).name),
            InverseKind::None => None,
        };

        assert_eq!(
            invert_gate_name(sig.name),
            expected_inv,
            "optimizer::algebra::inverses::invert_gate_name('{}') mismatch",
            sig.name
        );
    }
}

#[test]
fn test_gate_enum_bridge() {
    let gates_and_ids = [
        (Gate::X, GateId::X),
        (Gate::Y, GateId::Y),
        (Gate::Z, GateId::Z),
        (Gate::H, GateId::H),
        (Gate::S, GateId::S),
        (Gate::Sdg, GateId::Sdg),
        (Gate::T, GateId::T),
        (Gate::Tdg, GateId::Tdg),
        (Gate::Rx(Angle::zero()), GateId::Rx),
        (Gate::Ry(Angle::zero()), GateId::Ry),
        (Gate::Rz(Angle::zero()), GateId::Rz),
        (Gate::CX, GateId::CX),
        (Gate::CZ, GateId::CZ),
        (Gate::SWAP, GateId::SWAP),
    ];

    for (gate, expected_id) in gates_and_ids {
        assert_eq!(gate.gate_id(), expected_id);
    }
}

// ============================================================================
// 10. Audit Verification & Architecture Boundary Tests
// ============================================================================

#[test]
fn test_gate_classification_api() {
    // 1. Logical gates and aliases
    assert_eq!(
        GateCatalog::classify("x"),
        GateClassification::Logical(GateId::X)
    );
    assert_eq!(
        GateCatalog::classify("CX"),
        GateClassification::Logical(GateId::CX)
    );
    assert_eq!(
        GateCatalog::classify("cnot"),
        GateClassification::Logical(GateId::CX)
    );
    assert_eq!(
        GateCatalog::classify("CNOT"),
        GateClassification::Logical(GateId::CX)
    );
    assert_eq!(
        GateCatalog::classify("p"),
        GateClassification::Logical(GateId::P)
    );
    assert_eq!(
        GateCatalog::classify("u1"),
        GateClassification::Logical(GateId::P)
    );
    assert_eq!(
        GateCatalog::classify("phase"),
        GateClassification::Logical(GateId::P)
    );
    assert_eq!(
        GateCatalog::classify("id"),
        GateClassification::Logical(GateId::Id)
    );
    assert_eq!(
        GateCatalog::classify("i"),
        GateClassification::Logical(GateId::Id)
    );
    assert_eq!(
        GateCatalog::classify("sdg"),
        GateClassification::Logical(GateId::Sdg)
    );
    assert_eq!(
        GateCatalog::classify("sdag"),
        GateClassification::Logical(GateId::Sdg)
    );

    // 2. Target-native primitives
    let target_natives = [
        "sx", "sy", "sxdg", "sydg", "rzx", "rzz", "ecr", "iswap", "fsim", "cz_pulse",
    ];
    for name in target_natives {
        assert_eq!(
            GateCatalog::classify(name),
            GateClassification::TargetNative,
            "expected '{}' to classify as TargetNative",
            name
        );
        assert!(!GateCatalog::is_logical(name));
    }

    // 3. Unknown names are strictly Unknown, NEVER classified as TargetNative
    let unknown_names = [
        "foo",
        "user_defined_gate",
        "custom_op",
        "bar",
        "123",
        "",
        "   ",
    ];
    for name in unknown_names {
        assert_eq!(
            GateCatalog::classify(name),
            GateClassification::Unknown,
            "expected '{}' to classify as Unknown",
            name
        );
        assert!(!GateCatalog::is_logical(name));
    }
}

#[test]
fn test_frontend_rejection_of_catalog_aliases_and_unsupported_gates() {
    let frontend = OpenQasmFrontend::new();

    // Catalog aliases that are not valid OpenQASM 3 standard gates
    let unknown_alias_cases = [
        (
            "cnot_test.qasm",
            "OPENQASM 3.0;\nqubit[2] q;\ncnot q[0], q[1];\n",
            "cnot",
        ),
        ("i_test.qasm", "OPENQASM 3.0;\nqubit[1] q;\ni q[0];\n", "i"),
        (
            "phase_test.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nphase(0.5) q[0];\n",
            "phase",
        ),
        (
            "sdag_test.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nsdag q[0];\n",
            "sdag",
        ),
    ];

    for (file, src, gate) in unknown_alias_cases {
        let buf = SourceBuffer::new(file, src);
        let res = frontend.parse(&buf);
        assert!(
            !res.is_ok(),
            "frontend must reject internal alias '{gate}' as unknown gate"
        );
        let diags: Vec<_> = res.errors().collect();
        assert!(!diags.is_empty());
        assert_eq!(
            diags[0].code.as_deref(),
            Some("E0020"),
            "expected unknown gate diagnostic E0020 for '{gate}'"
        );
    }

    // Standard OpenQASM gates that require basis decomposition (must return E0004)
    let unsupported_cases = [
        (
            "u1_test.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\nu1(0.5) q[0];\n",
            "u1",
        ),
        (
            "p_test.qasm",
            "OPENQASM 3.0;\nqubit[1] q;\np(0.5) q[0];\n",
            "p",
        ),
    ];

    for (file, src, gate) in unsupported_cases {
        let buf = SourceBuffer::new(file, src);
        let res = frontend.parse(&buf);
        assert!(
            !res.is_ok(),
            "frontend must reject '{gate}' as unsupported decomposition-required gate"
        );
        let diags: Vec<_> = res.errors().collect();
        assert!(!diags.is_empty());
        assert_eq!(
            diags[0].code.as_deref(),
            Some("E0004"),
            "expected unsupported gate diagnostic E0004 for '{gate}'"
        );
    }
}

#[test]
fn test_invert_operation_integration() {
    use qcc::ir::wire::QubitId;

    // 1. Invert Identity
    let id_op = Operation::Gate {
        gate_name: "id".to_string(),
        qubits: vec![QubitId(0)],
        parameters: vec![],
    };
    let inv_id = invert_operation(&id_op).unwrap();
    assert_eq!(inv_id.name(), "id");

    // 2. Invert P gate
    let p_op = Operation::Gate {
        gate_name: "p".to_string(),
        qubits: vec![QubitId(0)],
        parameters: vec![ParameterExpr::constant(0.75)],
    };
    let inv_p = invert_operation(&p_op).unwrap();
    assert_eq!(inv_p.name(), "p");
    assert_eq!(inv_p.parameters(), &[ParameterExpr::constant(-0.75)]);

    // 3. Invert alias "u1" -> produces canonical "p"
    let u1_op = Operation::Gate {
        gate_name: "u1".to_string(),
        qubits: vec![QubitId(0)],
        parameters: vec![ParameterExpr::constant(1.23)],
    };
    let inv_u1 = invert_operation(&u1_op).unwrap();
    assert_eq!(inv_u1.name(), "p");
    assert_eq!(inv_u1.parameters(), &[ParameterExpr::constant(-1.23)]);

    // 4. Invert paired gate S -> Sdg
    let s_op = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![QubitId(1)],
        parameters: vec![],
    };
    let inv_s = invert_operation(&s_op).unwrap();
    assert_eq!(inv_s.name(), "sdg");

    // 5. Invert alias "cnot" -> produces canonical "cx"
    let cnot_op = Operation::Gate {
        gate_name: "cnot".to_string(),
        qubits: vec![QubitId(0), QubitId(1)],
        parameters: vec![],
    };
    let inv_cnot = invert_operation(&cnot_op).unwrap();
    assert_eq!(inv_cnot.name(), "cx");
}

#[test]
fn test_rotation_4pi_vs_phase_2pi_domain_distinction() {
    use num_complex::Complex64;

    // 1. At theta = 2*pi:
    // Rz(2*pi) = exp(-i*pi * Z) = -I != I
    let rz_2pi = GateCatalog::matrix_for_gate_1q(GateId::Rz, &[2.0 * PI]).unwrap();
    assert!(
        !rz_2pi.approx_eq(&Matrix2x2::identity(), 1e-10),
        "Rz(2*pi) must equal -I, NOT +I (spinor periodicity is 4*pi)"
    );
    let minus_identity = Matrix2x2([
        [Complex64::new(-1.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(-1.0, 0.0)],
    ]);
    assert!(
        rz_2pi.approx_eq(&minus_identity, 1e-10),
        "Rz(2*pi) must equal -I"
    );

    // P(2*pi) = diag(1, exp(i*2*pi)) = diag(1, 1) = +I exact identity!
    let p_2pi = GateCatalog::matrix_for_gate_1q(GateId::P, &[2.0 * PI]).unwrap();
    assert!(
        p_2pi.approx_eq(&Matrix2x2::identity(), 1e-10),
        "P(2*pi) must equal +I exact identity (phase periodicity is 2*pi)"
    );

    // 2. At theta = 4*pi:
    // Rz(4*pi) = +I
    let rz_4pi = GateCatalog::matrix_for_gate_1q(GateId::Rz, &[4.0 * PI]).unwrap();
    assert!(
        rz_4pi.approx_eq(&Matrix2x2::identity(), 1e-10),
        "Rz(4*pi) must equal +I"
    );

    // 3. Mathematical relationship: P(lambda) = exp(i*lambda/2) * Rz(lambda)
    let lambda = 1.3579;
    let p_mat = GateCatalog::matrix_for_gate_1q(GateId::P, &[lambda]).unwrap();
    let rz_mat = GateCatalog::matrix_for_gate_1q(GateId::Rz, &[lambda]).unwrap();

    // Multiply Rz by phase exp(i * lambda / 2)
    let global_factor = Complex64::from_polar(1.0, lambda / 2.0);
    let rz_with_phase = Matrix2x2([
        [
            rz_mat.0[0][0] * global_factor,
            rz_mat.0[0][1] * global_factor,
        ],
        [
            rz_mat.0[1][0] * global_factor,
            rz_mat.0[1][1] * global_factor,
        ],
    ]);
    assert!(
        p_mat.approx_eq(&rz_with_phase, 1e-10),
        "P(lambda) must equal exp(i*lambda/2) * Rz(lambda)"
    );
}

proptest! {
    #[test]
    fn prop_rotations_unitarity_for_arbitrary_angles(theta in -100.0f64..100.0f64) {
        let rx = GateCatalog::matrix_for_gate_1q(GateId::Rx, &[theta]).unwrap();
        let ry = GateCatalog::matrix_for_gate_1q(GateId::Ry, &[theta]).unwrap();
        let rz = GateCatalog::matrix_for_gate_1q(GateId::Rz, &[theta]).unwrap();
        let p = GateCatalog::matrix_for_gate_1q(GateId::P, &[theta]).unwrap();

        prop_assert!(rx.is_unitary(1e-10));
        prop_assert!(ry.is_unitary(1e-10));
        prop_assert!(rz.is_unitary(1e-10));
        prop_assert!(p.is_unitary(1e-10));
    }

    #[test]
    fn prop_rotations_inverse_composition(theta in -100.0f64..100.0f64) {
        for &id in &[GateId::Rx, GateId::Ry, GateId::Rz, GateId::P] {
            let u = GateCatalog::matrix_for_gate_1q(id, &[theta]).unwrap();
            let u_inv = GateCatalog::matrix_for_gate_1q(id, &[-theta]).unwrap();
            let prod = u.matmul(&u_inv);
            prop_assert!(prod.approx_eq(&Matrix2x2::identity(), 1e-10));
        }
    }

    #[test]
    fn prop_phase_gate_periodicity_2pi(lambda in -50.0f64..50.0f64) {
        let p1 = GateCatalog::matrix_for_gate_1q(GateId::P, &[lambda]).unwrap();
        let p2 = GateCatalog::matrix_for_gate_1q(GateId::P, &[lambda + 2.0 * PI]).unwrap();
        prop_assert!(p1.approx_eq(&p2, 1e-10));
    }
}
