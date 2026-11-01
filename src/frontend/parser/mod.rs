//! OpenQASM 3 parser implementations, declaration parsers, and register symbol tables.
//!
//! # Submodules
//!
//! - [`declarations`]: Parsing routines for version directives, includes, and register declarations.
//! - [`registers`]: Static register symbol table, typed IDs, and operand bounds resolution.

pub mod declarations;
pub mod registers;

pub use declarations::DeclarationParser;
pub use registers::{
    BitId, ClassicalRegisterEntry, ClassicalRegisterId, QuantumRegisterEntry, QuantumRegisterId,
    QubitId, RegisterEntry, RegisterError, RegisterKind, RegisterTable, ResolvedBit, ResolvedQubit,
};
