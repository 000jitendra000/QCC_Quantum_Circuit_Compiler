//! OpenQASM 3 emission backend.
//!
//! Provides deterministic export from QccIR dependency DAGs to standard OpenQASM 3.0 source text.

pub mod emitter;
pub mod errors;
pub mod format;

pub use emitter::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
};
pub use errors::OpenQasmError;
pub use format::{
    format_bit_ref, format_float, format_parameter_expr, format_qubit_ref, is_valid_identifier,
    OPENQASM_KEYWORDS,
};
