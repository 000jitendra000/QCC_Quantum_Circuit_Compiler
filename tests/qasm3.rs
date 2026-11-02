//! Integration test runner for OpenQASM 3 frontend tests.

#[path = "qasm3/declarations.rs"]
mod declarations;

#[path = "qasm3/gates.rs"]
mod gates;

#[path = "qasm3/expressions.rs"]
mod expressions;

#[path = "qasm3/non_unitary.rs"]
mod non_unitary;

#[path = "qasm3/semantic.rs"]
mod semantic;

#[path = "qasm3/unsupported/mod.rs"]
mod unsupported;

#[path = "qasm3/lowering.rs"]
mod lowering;
