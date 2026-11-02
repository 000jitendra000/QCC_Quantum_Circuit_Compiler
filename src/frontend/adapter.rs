//! OpenQASM 3 frontend adapter boundary implementation.
//!
//! Provides the parser-independent [`Frontend`] trait and the canonical [`OpenQasmFrontend`]
//! implementation wrapping an internal, lightweight recursive-descent parser for the supported
//! OpenQASM 3 subset.

use crate::diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan};
use crate::frontend::ast::{
    AstExpression, AstGlobalPhase, AstInclude, AstProgram, AstRegisterDecl, AstRegisterKind,
    AstStatement, AstUnsupported, AstVersion,
};
use crate::frontend::errors::FrontendResult;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::barrier::BarrierParser;
use crate::frontend::parser::expressions::ExpressionParser;
use crate::frontend::parser::gates::GateParser;
use crate::frontend::parser::measurement::MeasurementParser;
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::parser::reset::ResetParser;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Abstract frontend contract isolating parser implementation from compiler core.
///
/// Any source language (e.g. OpenQASM 3, future OpenQASM dialects, or test mocks)
/// can implement this trait without coupling compiler core or QccIR to parser internals.
pub trait Frontend {
    /// Returns the descriptive identifier of this frontend implementation.
    fn name(&self) -> &'static str;

    /// Parses the provided source buffer into a validated AST program,
    /// returning any diagnostics encountered during lexical and syntactic analysis.
    fn parse(&self, buffer: &SourceBuffer) -> FrontendResult<AstProgram>;
}

/// Canonical OpenQASM 3 frontend adapter.
///
/// Encapsulates all lexical analysis, grammar parsing, and diagnostic translation
/// for QCC's supported OpenQASM 3 subset.
#[derive(Clone, Debug, Default)]
pub struct OpenQasmFrontend;

impl OpenQasmFrontend {
    /// Creates a new [`OpenQasmFrontend`] instance.
    pub fn new() -> Self {
        Self
    }

    /// Parses the provided source buffer into a validated AST program and symbol table.
    pub fn parse_with_registers(
        &self,
        buffer: &SourceBuffer,
    ) -> (FrontendResult<AstProgram>, RegisterTable) {
        let (tokens, lex_diags) = Lexer::tokenize(buffer);
        let mut parser = Parser::new(buffer, tokens, lex_diags);
        let res = parser.parse_program();
        (res, parser.register_table)
    }

    /// Parses the source buffer into an AST program and performs comprehensive semantic
    /// analysis, returning both the AST and resolved symbol table on success.
    pub fn parse_and_analyze(
        &self,
        buffer: &SourceBuffer,
        parameter_bindings: Option<std::collections::HashMap<String, f64>>,
    ) -> FrontendResult<(AstProgram, crate::frontend::semantic::SymbolTable)> {
        let parse_res = self.parse(buffer);
        let mut diagnostics = parse_res.diagnostics;

        if let Some(program) = parse_res.output {
            let analyzer = if let Some(bindings) = parameter_bindings {
                crate::frontend::semantic::SemanticAnalyzer::with_bindings(buffer, bindings)
            } else {
                crate::frontend::semantic::SemanticAnalyzer::new(buffer)
            };

            match analyzer.analyze(&program) {
                Ok(symbols) => FrontendResult::with_diagnostics((program, symbols), diagnostics),
                Err(sem_diags) => {
                    diagnostics.extend(sem_diags);
                    FrontendResult::failure(diagnostics)
                }
            }
        } else {
            FrontendResult::failure(diagnostics)
        }
    }
}

impl Frontend for OpenQasmFrontend {
    fn name(&self) -> &'static str {
        "openqasm3"
    }

    fn parse(&self, buffer: &SourceBuffer) -> FrontendResult<AstProgram> {
        let (tokens, lex_diags) = Lexer::tokenize(buffer);
        let mut parser = Parser::new(buffer, tokens, lex_diags);
        parser.parse_program()
    }
}

// ============================================================================
// Internal Private Parser (Consumes Standalone Lexer Token Stream)
// ============================================================================

struct Parser<'a> {
    buffer: &'a SourceBuffer,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
    eof_token: Token,
    pub register_table: RegisterTable,
}

impl<'a> Parser<'a> {
    fn new(buffer: &'a SourceBuffer, tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> Self {
        let eof_offset = buffer.len();
        let eof_span = buffer
            .point_span(eof_offset)
            .unwrap_or_else(|_| SourceSpan::point(eof_offset, 1, 1).unwrap());
        let eof_token = Token::eof(eof_offset, eof_span);
        Self {
            buffer,
            tokens,
            cursor: 0,
            diagnostics,
            eof_token,
            register_table: RegisterTable::new(),
        }
    }

    fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.eof_token
        }
    }

    fn advance(&mut self) -> Token {
        if self.cursor < self.tokens.len() {
            let tok = self.tokens[self.cursor].clone();
            self.cursor += 1;
            tok
        } else {
            self.eof_token.clone()
        }
    }

    fn error(&mut self, message: impl Into<String>, span: SourceSpan, code: &'static str) {
        self.diagnostics.push(
            Diagnostic::error(message)
                .with_code(code)
                .with_span(span)
                .with_file_path(self.buffer.name()),
        );
    }

    fn skip_to_semicolon(&mut self) {
        while self.cursor < self.tokens.len() {
            let tok = self.advance();
            if tok.kind == TokenKind::Semicolon || tok.kind == TokenKind::Eof {
                break;
            }
        }
    }

    fn parse_program(&mut self) -> FrontendResult<AstProgram> {
        let full_span = self
            .buffer
            .full_span()
            .unwrap_or_else(|_| SourceSpan::point(0, 1, 1).unwrap());

        let mut version = None;
        let mut includes = Vec::new();
        let mut statements = Vec::new();

        // 1. Optional OPENQASM version
        if self.peek().kind == TokenKind::OpenQasm {
            let start_tok = self.advance();
            let mut spec = String::new();
            let next_tok = self.advance();
            match &next_tok.kind {
                TokenKind::FloatLiteral(f) => spec = format!("{f:.1}"),
                TokenKind::IntLiteral(i) => spec = format!("{i}"),
                TokenKind::Ident(s) => spec = s.clone(),
                TokenKind::StringLiteral(s) => spec = s.clone(),
                _ => {
                    let span = self
                        .buffer
                        .span(start_tok.start(), next_tok.end())
                        .unwrap_or(full_span);
                    self.error(
                        "expected version specifier following 'OPENQASM'",
                        span,
                        "E0001",
                    );
                    self.skip_to_semicolon();
                }
            }

            if !spec.is_empty() {
                let semi = self.peek();
                if semi.kind == TokenKind::Semicolon {
                    let semi_tok = self.advance();
                    let span = self
                        .buffer
                        .span(start_tok.start(), semi_tok.end())
                        .unwrap_or(full_span);
                    version = Some(AstVersion {
                        specifier: spec,
                        span,
                    });
                } else {
                    let span = self
                        .buffer
                        .span(start_tok.start(), next_tok.end())
                        .unwrap_or(full_span);
                    self.error(
                        "expected ';' after OPENQASM version declaration",
                        span,
                        "E0001",
                    );
                    self.skip_to_semicolon();
                }
            }
        }

        // 2. Includes
        while self.peek().kind == TokenKind::Include {
            let start_tok = self.advance();
            let next_tok = self.advance();
            match &next_tok.kind {
                TokenKind::StringLiteral(path) => {
                    let semi = self.peek();
                    if semi.kind == TokenKind::Semicolon {
                        let semi_tok = self.advance();
                        let span = self
                            .buffer
                            .span(start_tok.start(), semi_tok.end())
                            .unwrap_or(full_span);
                        includes.push(AstInclude {
                            path: path.clone(),
                            span,
                        });
                    } else {
                        let span = self
                            .buffer
                            .span(start_tok.start(), next_tok.end())
                            .unwrap_or(full_span);
                        self.error("expected ';' after include statement", span, "E0001");
                        self.skip_to_semicolon();
                    }
                }
                _ => {
                    let span = self
                        .buffer
                        .span(start_tok.start(), next_tok.end())
                        .unwrap_or(full_span);
                    self.error(
                        "expected string literal path following 'include'",
                        span,
                        "E0001",
                    );
                    self.skip_to_semicolon();
                }
            }
        }

        // 3. Statements
        while self.peek().kind != TokenKind::Eof {
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
        }

        let output = if self
            .diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
        {
            None
        } else {
            Some(AstProgram::new(version, includes, statements, full_span))
        };

        FrontendResult::new(output, self.diagnostics.clone())
    }

    fn parse_statement(&mut self) -> Option<AstStatement> {
        let tok = self.peek();
        match &tok.kind {
            TokenKind::Semicolon => {
                // Empty statement
                self.advance();
                None
            }
            TokenKind::Qubit | TokenKind::Bit => self.parse_register_decl(),
            TokenKind::Qreg | TokenKind::Creg => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                let replacement = if kw_tok.kind == TokenKind::Qreg {
                    "qubit[size] name;"
                } else {
                    "bit[size] name;"
                };
                let kw_name = if kw_tok.kind == TokenKind::Qreg {
                    "qreg"
                } else {
                    "creg"
                };
                self.error(
                    format!(
                        "OpenQASM 2 keyword '{kw_name}' is not supported in OpenQASM 3; use '{replacement}' instead"
                    ),
                    span,
                    "E0002",
                );
                self.skip_to_semicolon();
                None
            }
            TokenKind::Measure => self.parse_arrow_measure(),
            TokenKind::Reset => self.parse_reset(),
            TokenKind::Barrier => self.parse_barrier(),
            TokenKind::Gphase => self.parse_gphase(),
            TokenKind::While | TokenKind::For => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "classical control flow loops ('while'/'for') are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_unsupported_block_or_statement();
                None
            }
            TokenKind::Defcal | TokenKind::Defcalgrammar => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "pulse calibration definitions ('defcal') are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_unsupported_block_or_statement();
                None
            }
            TokenKind::Extern => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "extern symbol declarations are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_to_semicolon();
                None
            }
            TokenKind::Delay => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "timing delays ('delay') are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_to_semicolon();
                None
            }
            TokenKind::Gate => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "custom gate definitions ('gate') are not supported in QccIR at this milestone without macro expansion",
                    span,
                    "E0005",
                );
                self.skip_unsupported_block_or_statement();
                None
            }
            TokenKind::Int | TokenKind::Float | TokenKind::Angle | TokenKind::Bool => {
                let kw_tok = self.advance();
                let span = kw_tok.span;
                self.error(
                    "classical scalar memory types are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_to_semicolon();
                None
            }
            TokenKind::Ident(_) => {
                // Could be assignment measurement `c[0] = measure q[0];` or a gate call
                if self.is_assignment_statement() {
                    self.parse_assignment_measure()
                } else {
                    self.parse_gate_call()
                }
            }
            _ => {
                let unk = self.advance();
                let span = unk.span;
                self.error("unexpected token at statement boundary", span, "E0001");
                self.skip_to_semicolon();
                None
            }
        }
    }

    fn skip_unsupported_block_or_statement(&mut self) {
        let mut brace_depth = 0;
        let mut saw_brace = false;

        while self.cursor < self.tokens.len() {
            let tok = self.advance();
            if tok.kind == TokenKind::LBrace {
                brace_depth += 1;
                saw_brace = true;
            } else if tok.kind == TokenKind::RBrace {
                if brace_depth > 0 {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        break;
                    }
                }
            } else if (tok.kind == TokenKind::Semicolon && !saw_brace) || tok.kind == TokenKind::Eof
            {
                break;
            }
        }
    }

    fn parse_register_decl(&mut self) -> Option<AstStatement> {
        let kind_tok = self.advance();
        let kind = match kind_tok.kind {
            TokenKind::Qubit => AstRegisterKind::Qubit,
            TokenKind::Bit => AstRegisterKind::Bit,
            _ => unreachable!(),
        };

        let mut size = None;
        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let size_tok = self.advance();
            match size_tok.kind {
                TokenKind::IntLiteral(sz) => {
                    let sz_val = sz as usize;
                    if sz_val == 0 {
                        let span = self
                            .buffer
                            .span(kind_tok.start(), size_tok.end())
                            .unwrap_or(size_tok.span);
                        self.error(
                            format!(
                                "{kind} register has invalid size 0; register size must be positive"
                            ),
                            span,
                            "E0011",
                        );
                        self.skip_to_semicolon();
                        return None;
                    }
                    size = Some(sz_val);
                }
                _ => {
                    let span = size_tok.span;
                    self.error("expected integer register size in brackets", span, "E0001");
                    self.skip_to_semicolon();
                    return None;
                }
            }
            let rbracket = self.peek();
            if rbracket.kind == TokenKind::RBracket {
                self.advance();
            } else {
                let span = rbracket.span;
                self.error("expected ']' after register size", span, "E0001");
                self.skip_to_semicolon();
                return None;
            }
        }

        let name_tok = self.advance();
        let name = match &name_tok.kind {
            TokenKind::Ident(ident) => ident.clone(),
            TokenKind::Qreg | TokenKind::Creg => {
                let span = name_tok.span;
                self.error(
                    "reserved keyword cannot be used as a register identifier",
                    span,
                    "E0002",
                );
                self.skip_to_semicolon();
                return None;
            }
            _ => {
                let span = name_tok.span;
                self.error("expected register identifier", span, "E0001");
                self.skip_to_semicolon();
                return None;
            }
        };

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            let semi_tok = self.advance();
            let span = self
                .buffer
                .span(kind_tok.start(), semi_tok.end())
                .unwrap_or(kind_tok.span);

            let res = match kind {
                AstRegisterKind::Qubit => self
                    .register_table
                    .declare_quantum(&name, size, span)
                    .map(|_| ()),
                AstRegisterKind::Bit => self
                    .register_table
                    .declare_classical(&name, size, span)
                    .map(|_| ()),
            };
            if let Err(err) = res {
                self.diagnostics.push(err.to_diagnostic(self.buffer.name()));
                return None;
            }

            Some(AstStatement::RegisterDecl(AstRegisterDecl {
                kind,
                name,
                size,
                span,
            }))
        } else {
            let span = self
                .buffer
                .span(kind_tok.start(), name_tok.end())
                .unwrap_or(kind_tok.span);
            self.error("expected ';' after register declaration", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn is_assignment_statement(&self) -> bool {
        let mut idx = self.cursor;
        if idx >= self.tokens.len() {
            return false;
        }
        // Skip identifier
        idx += 1;
        // Skip optional [index]
        if idx < self.tokens.len() && self.tokens[idx].kind == TokenKind::LBracket {
            idx += 1;
            if idx < self.tokens.len() && matches!(self.tokens[idx].kind, TokenKind::IntLiteral(_))
            {
                idx += 1;
                if idx < self.tokens.len() && self.tokens[idx].kind == TokenKind::RBracket {
                    idx += 1;
                }
            }
        }
        idx < self.tokens.len() && self.tokens[idx].kind == TokenKind::Equals
    }

    fn parse_assignment_measure(&mut self) -> Option<AstStatement> {
        let mut parser = MeasurementParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match parser.parse_assignment_measurement(Some(&self.register_table)) {
            Ok(measure) => {
                self.cursor = parser.cursor();
                Some(AstStatement::Measure(measure))
            }
            Err(diag) => {
                self.cursor = parser.cursor();
                self.diagnostics.push(*diag);
                None
            }
        }
    }

    fn parse_arrow_measure(&mut self) -> Option<AstStatement> {
        let mut parser = MeasurementParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match parser.parse_arrow_measurement(Some(&self.register_table)) {
            Ok(measure) => {
                self.cursor = parser.cursor();
                Some(AstStatement::Measure(measure))
            }
            Err(diag) => {
                self.cursor = parser.cursor();
                self.diagnostics.push(*diag);
                None
            }
        }
    }

    fn parse_reset(&mut self) -> Option<AstStatement> {
        let mut parser = ResetParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match parser.parse_reset_statement(Some(&self.register_table)) {
            Ok(reset) => {
                self.cursor = parser.cursor();
                Some(AstStatement::Reset(reset))
            }
            Err(diag) => {
                self.cursor = parser.cursor();
                self.diagnostics.push(*diag);
                None
            }
        }
    }

    fn parse_barrier(&mut self) -> Option<AstStatement> {
        let mut parser = BarrierParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match parser.parse_barrier_statement(Some(&self.register_table)) {
            Ok(barrier) => {
                self.cursor = parser.cursor();
                Some(AstStatement::Barrier(barrier))
            }
            Err(diag) => {
                self.cursor = parser.cursor();
                self.diagnostics.push(*diag);
                None
            }
        }
    }

    fn parse_gphase(&mut self) -> Option<AstStatement> {
        let start_tok = self.advance(); // Consume 'gphase'
        let lparen = self.advance();
        if lparen.kind != TokenKind::LParen {
            let span = lparen.span;
            self.error("expected '(' after 'gphase'", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let angle = self.parse_expression(0)?;

        let rparen = self.advance();
        if rparen.kind != TokenKind::RParen {
            let span = rparen.span;
            self.error("expected ')' after gphase angle expression", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            let semi_tok = self.advance();
            let span = self
                .buffer
                .span(start_tok.start(), semi_tok.end())
                .unwrap_or(start_tok.span);
            Some(AstStatement::GlobalPhase(AstGlobalPhase { angle, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start(), rparen.end())
                .unwrap_or(start_tok.span);
            self.error("expected ';' after gphase statement", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_gate_call(&mut self) -> Option<AstStatement> {
        let mut gate_parser = GateParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match gate_parser.parse_gate_statement(Some(&self.register_table)) {
            Ok(call) => {
                self.cursor = gate_parser.cursor();
                Some(AstStatement::GateCall(call))
            }
            Err(diag) => {
                self.cursor = gate_parser.cursor();
                let code = diag.code.clone();
                let span = diag.span.unwrap_or(self.eof_token.span);
                let msg = diag.message.clone();
                self.diagnostics.push(*diag);

                if code.as_deref() == Some("E0003") {
                    Some(AstStatement::Unsupported(AstUnsupported {
                        kind: "three_qubit_gate".to_string(),
                        raw: msg,
                        span,
                    }))
                } else if code.as_deref() == Some("E0004") {
                    Some(AstStatement::Unsupported(AstUnsupported {
                        kind: "unsupported_gate".to_string(),
                        raw: msg,
                        span,
                    }))
                } else {
                    None
                }
            }
        }
    }

    fn parse_expression(&mut self, min_bp: u8) -> Option<AstExpression> {
        let mut expr_parser = ExpressionParser::with_cursor(self.buffer, &self.tokens, self.cursor);
        match expr_parser.parse_expression(min_bp) {
            Ok(expr) => {
                self.cursor = expr_parser.cursor();
                Some(expr)
            }
            Err(diag) => {
                self.cursor = expr_parser.cursor();
                self.diagnostics.push(*diag);
                None
            }
        }
    }
}
