//! OpenQASM 3 frontend adapter boundary, source buffer, lexer, AST, and diagnostics.
//!
//! This module isolates parser implementations from compiler core (QccIR and optimizer).
//! Compiler pipelines interact exclusively through the abstract [`Frontend`] trait,
//! which consumes a [`SourceBuffer`] and produces a typed, span-annotated [`AstProgram`].

pub mod adapter;
pub mod ast;
pub mod errors;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod token;

pub use adapter::{Frontend, OpenQasmFrontend};
pub use ast::{
    AstBarrier, AstBitRef, AstExprKind, AstExpression, AstGateCall, AstGlobalPhase, AstInclude,
    AstMeasure, AstProgram, AstQubitRef, AstRegisterDecl, AstRegisterKind, AstReset, AstStatement,
    AstUnsupported, AstVersion,
};
pub use errors::{FrontendError, FrontendResult};
pub use lexer::{LexResult, Lexer, LexerLimits};
pub use parser::{
    find_similar_gate, lookup_gate_signature, BitId, ClassicalRegisterEntry, ClassicalRegisterId,
    DeclarationParser, GateParser, GateSignature, OperandParser, QuantumRegisterEntry,
    QuantumRegisterId, QubitId, RegisterEntry, RegisterError, RegisterKind, RegisterTable,
    ResolvedBit, ResolvedQubit, SUPPORTED_GATES,
};
pub use source::SourceBuffer;
pub use token::{Token, TokenKind};
