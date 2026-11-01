//! Backend targets and format emitters for QCC.
//!
//! Submodules support exporting QccIR circuits to external target representations,
//! including OpenQASM 3.

pub mod openqasm;

pub use openqasm::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
    OpenQasmError,
};
