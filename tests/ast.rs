//! Integration test runner for the OpenQASM AST model.

#[path = "ast/program.rs"]
mod program;

#[path = "ast/declaration.rs"]
mod declaration;

#[path = "ast/statement.rs"]
mod statement;

#[path = "ast/expression.rs"]
mod expression;

#[path = "ast/golden.rs"]
mod golden;

#[path = "ast/spans.rs"]
mod spans;
