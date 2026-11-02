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
pub mod semantic;
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
    evaluate_expression, find_similar_gate, is_literal_zero, lookup_gate_signature, BarrierParser,
    BitId, ClassicalRegisterEntry, ClassicalRegisterId, DeclarationParser,
    ExpressionEvaluationError, ExpressionParser, GateParser, GateSignature, MeasurementParser,
    OperandParser, QuantumRegisterEntry, QuantumRegisterId, QubitId, RegisterEntry, RegisterError,
    RegisterKind, RegisterTable, ResetParser, ResolvedBit, ResolvedQubit, SUPPORTED_GATES,
};
pub use semantic::{SemanticAnalyzer, SemanticChecker, SymbolTable};
pub use source::SourceBuffer;
pub use token::{Token, TokenKind};
