//! Typed Abstract Syntax Tree (AST) representing the supported OpenQASM 3 subset.
//!
//! All AST nodes are owned exclusively by the frontend boundary and retain exact
//! [`SourceSpan`](crate::diagnostics::SourceSpan) coordinates for diagnostic provenance without leaking parser-specific types
//! or internal [`QccIR`](crate::ir::Circuit) representation details.
//!
//! # Submodules
//!
//! - [`program`]: Root compilation unit, version directives, and file inclusions.
//! - [`declaration`]: Quantum and classical register declarations.
//! - [`statement`]: Gate applications, measurements, resets, barriers, and global phase.
//! - [`expression`]: Arithmetic parameter expressions and operator trees.

pub mod declaration;
pub mod expression;
pub mod program;
pub mod statement;

pub use declaration::{AstRegisterDecl, AstRegisterKind};
pub use expression::{AstExprKind, AstExpression};
pub use program::{AstInclude, AstProgram, AstVersion};
pub use statement::{
    AstBarrier, AstBitRef, AstGateCall, AstGlobalPhase, AstMeasure, AstQubitRef, AstReset,
    AstStatement, AstUnsupported,
};

use std::fmt::Write;

impl AstProgram {
    /// Formats this AST into a deterministic, structural multi-line representation
    /// including exact source spans, suitable for AST inspection and golden testing.
    pub fn to_structural_string(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "Program [{}]", self.span);

        if let Some(v) = &self.version {
            let _ = writeln!(out, "  Version: {} [{}]", v.specifier, v.span);
        }

        for inc in &self.includes {
            let _ = writeln!(out, "  Include: \"{}\" [{}]", inc.path, inc.span);
        }

        for stmt in &self.statements {
            format_statement(&mut out, stmt, 2);
        }

        out
    }
}

fn format_statement(out: &mut String, stmt: &AstStatement, indent: usize) {
    let pad = " ".repeat(indent);
    match stmt {
        AstStatement::RegisterDecl(decl) => {
            if let Some(size) = decl.size {
                let _ = writeln!(
                    out,
                    "{pad}RegisterDecl({}): {}[{size}] [{}]",
                    decl.kind, decl.name, decl.span
                );
            } else {
                let _ = writeln!(
                    out,
                    "{pad}RegisterDecl({}): {} [{}]",
                    decl.kind, decl.name, decl.span
                );
            }
        }
        AstStatement::GateCall(call) => {
            let _ = writeln!(out, "{pad}GateCall: {} [{}]", call.name, call.span);
            for param in &call.parameters {
                let _ = writeln!(out, "{pad}  Param: {param} [{}]", param.span);
            }
            for q in &call.qubits {
                let _ = writeln!(out, "{pad}  Qubit: {q} [{}]", q.span);
            }
        }
        AstStatement::Measure(m) => {
            let _ = writeln!(out, "{pad}Measure [{}]", m.span);
            let _ = writeln!(out, "{pad}  Qubit: {} [{}]", m.qubit, m.qubit.span);
            let _ = writeln!(out, "{pad}  Bit: {} [{}]", m.bit, m.bit.span);
        }
        AstStatement::Reset(r) => {
            let _ = writeln!(out, "{pad}Reset [{}]", r.span);
            let _ = writeln!(out, "{pad}  Qubit: {} [{}]", r.qubit, r.qubit.span);
        }
        AstStatement::Barrier(b) => {
            if b.is_global() {
                let _ = writeln!(out, "{pad}Barrier (global) [{}]", b.span);
            } else {
                let _ = writeln!(out, "{pad}Barrier [{}]", b.span);
                for q in &b.qubits {
                    let _ = writeln!(out, "{pad}  Qubit: {q} [{}]", q.span);
                }
            }
        }
        AstStatement::GlobalPhase(gp) => {
            let _ = writeln!(out, "{pad}GlobalPhase [{}]", gp.span);
            let _ = writeln!(out, "{pad}  Angle: {} [{}]", gp.angle, gp.angle.span);
        }
        AstStatement::Unsupported(u) => {
            let _ = writeln!(
                out,
                "{pad}Unsupported({}): \"{}\" [{}]",
                u.kind, u.raw, u.span
            );
        }
    }
}
