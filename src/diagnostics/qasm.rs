//! Standardized OpenQASM diagnostic codes and constructor helpers.
//!
//! Provides stable, categorized diagnostic codes (`E0001`–`E0032`) and structured constructor
//! functions to generate consistent, source-located compiler diagnostics.

use crate::diagnostics::diagnostic::Diagnostic;
use crate::diagnostics::span::SourceSpan;

// ============================================================================
// 1. Lexical and Syntactic Error Codes (E0001 - E0009)
// ============================================================================

/// Generic syntax error or unexpected token.
pub const QASM_SYNTAX_ERROR: &str = "E0001";

/// Reserved keyword or obsolete OpenQASM 2 construct (e.g. `qreg`, `creg`).
pub const QASM_RESERVED_KEYWORD: &str = "E0002";

/// Unsupported three-qubit gate (e.g. `ccx`, `cswap`).
pub const QASM_UNSUPPORTED_3Q_GATE: &str = "E0003";

/// Unsupported gate in current milestone.
pub const QASM_UNSUPPORTED_GATE: &str = "E0004";

/// Unsupported language feature (e.g. loops, defcal, timing delays).
pub const QASM_UNSUPPORTED_FEATURE: &str = "E0005";

// ============================================================================
// 2. Declaration and Register Error Codes (E0010 - E0019)
// ============================================================================

/// Duplicate register or symbol declaration.
pub const QASM_DUPLICATE_DECLARATION: &str = "E0010";

/// Zero-size or non-positive register declaration.
pub const QASM_INVALID_REGISTER_SIZE: &str = "E0011";

/// Reference to an undeclared register or identifier.
pub const QASM_UNDECLARED_REGISTER: &str = "E0012";

/// Register kind or type mismatch (e.g. classical bit where qubit expected).
pub const QASM_REGISTER_TYPE_MISMATCH: &str = "E0013";

/// Register index out of declared bounds ($index \ge size$).
pub const QASM_INDEX_OUT_OF_BOUNDS: &str = "E0014";

/// Attempt to index a scalar (non-array) register.
pub const QASM_INDEXED_SCALAR: &str = "E0015";

/// Array register referenced without an index where a scalar bit/qubit is required.
pub const QASM_UNINDEXED_ARRAY: &str = "E0016";

// ============================================================================
// 3. Gate Call and Operation Error Codes (E0020 - E0029)
// ============================================================================

/// Unknown gate name.
pub const QASM_UNKNOWN_GATE: &str = "E0020";

/// Gate parameter arity mismatch (too few, too many, or missing parameter list).
pub const QASM_GATE_PARAM_ARITY: &str = "E0021";

/// Gate qubit operand arity mismatch (too few or too many qubit operands).
pub const QASM_GATE_QUBIT_ARITY: &str = "E0022";

/// Duplicate qubit operand in a multi-qubit gate (e.g. `cx q[0], q[0]`).
pub const QASM_DUPLICATE_QUBIT_OPERAND: &str = "E0023";

/// Parameter domain violation for gate operations with restricted parameter domains.
pub const QASM_INVALID_PARAM_DOMAIN: &str = "E0024";

/// Whole-register measurement size mismatch.
pub const QASM_MEASUREMENT_SIZE_MISMATCH: &str = "E0025";

/// Reset operation applied to an invalid operand (e.g. classical bit).
pub const QASM_INVALID_RESET_OPERAND: &str = "E0026";

// ============================================================================
// 4. Expression and Evaluation Error Codes (E0030 - E0039)
// ============================================================================

/// Division by zero in parameter expression.
pub const QASM_DIVISION_BY_ZERO: &str = "E0030";

/// Unresolved parameter symbol in gate or phase directive.
pub const QASM_UNRESOLVED_SYMBOL: &str = "E0031";

/// Parameter expression evaluated to a non-finite value (NaN or infinity).
pub const QASM_NON_FINITE_PARAM: &str = "E0032";

// ============================================================================
// 5. Lowering Error Codes (E0040 - E0049)
// ============================================================================

/// General AST-to-QccIR lowering failure or graph invariant violation.
pub const QASM_LOWERING_ERROR: &str = "E0040";

// ============================================================================
// 6. Diagnostic Constructor Helpers
// ============================================================================

/// Constructs a diagnostic for a duplicate register declaration (`E0010`).
pub fn duplicate_declaration(
    name: &str,
    span: SourceSpan,
    prev_span: Option<SourceSpan>,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(format!(
        "duplicate register declaration '{name}'; identifier was already declared"
    ))
    .with_code(QASM_DUPLICATE_DECLARATION)
    .with_span(span)
    .with_file_path(file_path);

    if let Some(prev) = prev_span {
        diag = diag.with_note(format!("previously declared at line {}", prev.start_line()));
    }
    diag
}

/// Constructs a diagnostic for an invalid register size of zero (`E0011`).
pub fn invalid_register_size(name: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "register '{name}' has invalid size 0; register size must be positive"
    ))
    .with_code(QASM_INVALID_REGISTER_SIZE)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an undeclared register reference (`E0012`).
pub fn undeclared_register(name: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!("use of undeclared register '{name}'"))
        .with_code(QASM_UNDECLARED_REGISTER)
        .with_span(span)
        .with_file_path(file_path)
}

/// Constructs a diagnostic for a register kind mismatch (`E0013`).
pub fn register_type_mismatch(
    name: &str,
    expected: &str,
    actual: &str,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "register '{name}' is a {actual} register, expected {expected}"
    ))
    .with_code(QASM_REGISTER_TYPE_MISMATCH)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an out-of-bounds register index (`E0014`).
pub fn index_out_of_bounds(
    name: &str,
    index: usize,
    size: usize,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "index {index} out of bounds for register '{name}' of size {size}"
    ))
    .with_code(QASM_INDEX_OUT_OF_BOUNDS)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for attempting to index a scalar register (`E0015`).
pub fn indexed_scalar(name: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot index scalar register '{name}'; declared without '[size]'"
    ))
    .with_code(QASM_INDEXED_SCALAR)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an unindexed array register (`E0016`).
pub fn unindexed_array(name: &str, size: usize, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "array register '{name}' of size {size} must be indexed with '[index]'"
    ))
    .with_code(QASM_UNINDEXED_ARRAY)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an unknown gate name (`E0020`).
pub fn unknown_gate(
    name: &str,
    span: SourceSpan,
    suggestion: Option<&str>,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(format!("unknown gate '{name}'"))
        .with_code(QASM_UNKNOWN_GATE)
        .with_span(span)
        .with_file_path(file_path);

    if let Some(sug) = suggestion {
        diag = diag.with_note(format!(
            "gate names in OpenQASM are lowercase; did you mean '{sug}'?"
        ));
    }
    diag
}

/// Constructs a diagnostic for a gate parameter arity mismatch (`E0021`).
pub fn gate_param_arity(
    name: &str,
    expected: usize,
    actual: usize,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(format!(
        "gate '{name}' takes {expected} parameter(s), but {actual} provided"
    ))
    .with_code(QASM_GATE_PARAM_ARITY)
    .with_span(span)
    .with_file_path(file_path);

    if expected == 0 && actual > 0 {
        diag = diag.with_note(format!(
            "syntax for fixed gate '{name}' is '{name} <qubits>;'"
        ));
    } else if expected > 0 && actual == 0 {
        diag = diag.with_note(format!(
            "syntax for parameterized gate '{name}' is '{name}(<expr>) <qubits>;'"
        ));
    }
    diag
}

/// Constructs a diagnostic for a gate qubit operand arity mismatch (`E0022`).
pub fn gate_qubit_arity(
    name: &str,
    expected: usize,
    actual: usize,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "gate '{name}' takes {expected} qubit operand(s), but {actual} provided"
    ))
    .with_code(QASM_GATE_QUBIT_ARITY)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for duplicate qubit operands in a multi-qubit gate (`E0023`).
pub fn duplicate_qubit_operand(
    gate_name: &str,
    qubit_ref: &str,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "duplicate qubit operand in '{gate_name}' gate: operand refers multiple times to '{qubit_ref}'"
    ))
    .with_code(QASM_DUPLICATE_QUBIT_OPERAND)
    .with_span(span)
    .with_file_path(file_path)
    .with_note("multi-qubit gates must operate on distinct qubits")
}

/// Constructs a diagnostic for an invalid parameter domain or value (`E0024`).
pub fn invalid_parameter_domain(
    msg: impl Into<String>,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(msg)
        .with_code(QASM_INVALID_PARAM_DOMAIN)
        .with_span(span)
        .with_file_path(file_path)
}

/// Constructs a diagnostic for whole-register measurement size mismatch (`E0025`).
pub fn measurement_size_mismatch(
    qubit_name: &str,
    qubit_size: usize,
    bit_name: &str,
    bit_size: usize,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "measurement register size mismatch: quantum register '{qubit_name}' has size {qubit_size}, but classical register '{bit_name}' has size {bit_size}"
    ))
    .with_code(QASM_MEASUREMENT_SIZE_MISMATCH)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for division by zero in parameter expression (`E0030`).
pub fn division_by_zero(span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error("division by zero in parameter expression")
        .with_code(QASM_DIVISION_BY_ZERO)
        .with_span(span)
        .with_file_path(file_path)
}

/// Constructs a diagnostic for an unresolved parameter symbol (`E0031`).
pub fn unresolved_symbol(
    symbol: &str,
    context: &str,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "unresolved parameter '{symbol}' in {context}; free parameters must be bound before lowering"
    ))
    .with_code(QASM_UNRESOLVED_SYMBOL)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for a non-finite parameter evaluation result (`E0032`).
pub fn non_finite_parameter(val: f64, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "parameter expression evaluated to non-finite value '{val}' (NaN/infinity)"
    ))
    .with_code(QASM_NON_FINITE_PARAM)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an obsolete or reserved OpenQASM keyword (`E0002`).
pub fn reserved_keyword(keyword: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    let replacement = match keyword {
        "qreg" => "use 'qubit[size] name;' instead",
        "creg" => "use 'bit[size] name;' instead",
        _ => "use OpenQASM 3 register declarations instead",
    };
    Diagnostic::error(format!(
        "OpenQASM 2 keyword '{keyword}' is not supported in OpenQASM 3; {replacement}"
    ))
    .with_code(QASM_RESERVED_KEYWORD)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an unsupported three-qubit gate (`E0003`).
pub fn unsupported_3q_gate(gate: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "three-qubit gate '{gate}' is not supported in QccIR at this milestone; decomposition into 1- and 2-qubit operations is required"
    ))
    .with_code(QASM_UNSUPPORTED_3Q_GATE)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an unsupported standard gate requiring decomposition (`E0004`).
pub fn unsupported_gate(gate: &str, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "gate '{gate}' is not supported in QccIR at this milestone without decomposition"
    ))
    .with_code(QASM_UNSUPPORTED_GATE)
    .with_span(span)
    .with_file_path(file_path)
    .with_note("supported MVP gates: x, y, z, h, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap")
}

/// Constructs a diagnostic for an unsupported language feature (`E0005`).
pub fn unsupported_feature(
    feature: &str,
    detail: &str,
    span: SourceSpan,
    file_path: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "{feature} ('{detail}') is not supported in QccIR at this milestone"
    ))
    .with_code(QASM_UNSUPPORTED_FEATURE)
    .with_span(span)
    .with_file_path(file_path)
}

/// Constructs a diagnostic for an AST-to-QccIR lowering error (`E0040`).
pub fn lowering_error(msg: impl Into<String>, span: SourceSpan, file_path: &str) -> Diagnostic {
    Diagnostic::error(msg)
        .with_code(QASM_LOWERING_ERROR)
        .with_span(span)
        .with_file_path(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_code_stability_constants() {
        // 1. Lexical and Syntactic
        assert_eq!(QASM_SYNTAX_ERROR, "E0001");
        assert_eq!(QASM_RESERVED_KEYWORD, "E0002");
        assert_eq!(QASM_UNSUPPORTED_3Q_GATE, "E0003");
        assert_eq!(QASM_UNSUPPORTED_GATE, "E0004");
        assert_eq!(QASM_UNSUPPORTED_FEATURE, "E0005");

        // 2. Declaration and Register (Phase 23 invariants)
        assert_eq!(QASM_DUPLICATE_DECLARATION, "E0010");
        assert_eq!(QASM_INVALID_REGISTER_SIZE, "E0011");
        assert_eq!(QASM_UNDECLARED_REGISTER, "E0012");
        assert_eq!(QASM_REGISTER_TYPE_MISMATCH, "E0013");
        assert_eq!(QASM_INDEX_OUT_OF_BOUNDS, "E0014");
        assert_eq!(QASM_INDEXED_SCALAR, "E0015");
        assert_eq!(QASM_UNINDEXED_ARRAY, "E0016");

        // 3. Gate Call and Operation
        assert_eq!(QASM_UNKNOWN_GATE, "E0020");
        assert_eq!(QASM_GATE_PARAM_ARITY, "E0021");
        assert_eq!(QASM_GATE_QUBIT_ARITY, "E0022");
        assert_eq!(QASM_DUPLICATE_QUBIT_OPERAND, "E0023");
        assert_eq!(QASM_INVALID_PARAM_DOMAIN, "E0024");
        assert_eq!(QASM_MEASUREMENT_SIZE_MISMATCH, "E0025");
        assert_eq!(QASM_INVALID_RESET_OPERAND, "E0026");

        // 4. Expression and Evaluation
        assert_eq!(QASM_DIVISION_BY_ZERO, "E0030");
        assert_eq!(QASM_UNRESOLVED_SYMBOL, "E0031");
        assert_eq!(QASM_NON_FINITE_PARAM, "E0032");

        // 5. Lowering
        assert_eq!(QASM_LOWERING_ERROR, "E0040");
    }
}
