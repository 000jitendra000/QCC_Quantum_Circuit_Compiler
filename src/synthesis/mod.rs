//! Quantum circuit synthesis, decomposition templates, and basis translation engines.
//!
//! Provides mathematically verified decomposition templates (SWAP, CX/CZ, Toffoli)
//! and synthesis primitives for quantum circuit compilation.

pub mod single_qubit;
pub mod templates;

pub use single_qubit::{
    normalize_angle_with_spinor_shift, SingleQubitSynthesis, SingleQubitSynthesizer,
    SynthesisError, SynthesizedOneQubitSequence,
};
pub use templates::{
    CxToCzTemplate, CzToCxTemplate, DecompositionTemplate, Matrix8x8, Precondition,
    SwapToCxTemplate, TemplateError, TemplateId, TemplateRegistry, ToffoliToCxTemplate,
};
