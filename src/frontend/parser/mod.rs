//! OpenQASM 3 parser implementations, declaration parsers, gate parsers, and register symbol tables.
//!
//! # Submodules
//!
//! - [`declarations`]: Parsing routines for version directives, includes, and register declarations.
//! - [`expressions`]: Operator precedence parsing and evaluation for parameter expressions.
//! - [`gates`]: Parsing routines, signatures, arity validation, and error recovery for standard gates.
//! - [`operands`]: Parsing routines for qubit and bit references and comma-separated operand lists.
//! - [`registers`]: Static register symbol table, typed IDs, and operand bounds resolution.

pub mod declarations;
pub mod expressions;
pub mod gates;
pub mod operands;
pub mod registers;

pub use declarations::DeclarationParser;
pub use expressions::{
    evaluate_expression, is_literal_zero, ExpressionEvaluationError, ExpressionParser,
};
pub use gates::{
    find_similar_gate, lookup_gate_signature, GateParser, GateSignature, SUPPORTED_GATES,
};
pub use operands::OperandParser;
pub use registers::{
    BitId, ClassicalRegisterEntry, ClassicalRegisterId, QuantumRegisterEntry, QuantumRegisterId,
    QubitId, RegisterEntry, RegisterError, RegisterKind, RegisterTable, ResolvedBit, ResolvedQubit,
};
