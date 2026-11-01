//! OpenQASM 3 frontend adapter boundary implementation.
//!
//! Provides the parser-independent [`Frontend`] trait and the canonical [`OpenQasmFrontend`]
//! implementation wrapping an internal, lightweight recursive-descent parser for the supported
//! OpenQASM 3 subset.

use crate::diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan};
use crate::frontend::ast::{
    AstBarrier, AstBitRef, AstExprKind, AstExpression, AstGateCall, AstGlobalPhase, AstInclude,
    AstMeasure, AstProgram, AstQubitRef, AstRegisterDecl, AstRegisterKind, AstReset, AstStatement,
    AstUnsupported, AstVersion,
};
use crate::frontend::errors::FrontendResult;
use crate::frontend::source::SourceBuffer;

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
// Internal Private Tokenizer and Parser (Zero Leakage)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum TokenKind<'a> {
    // Keywords
    OpenQasm,
    Include,
    Qubit,
    Bit,
    Measure,
    Reset,
    Barrier,
    Gphase,
    // Reserved / Unsupported Keywords
    Qreg,
    Creg,
    Defcal,
    Defcalgrammar,
    Delay,
    While,
    For,
    Extern,
    Gate,
    Int,
    Float,
    Angle,
    Bool,
    // Literals & Identifiers
    Ident(&'a str),
    IntLiteral(usize),
    FloatLiteral(f64),
    StringLiteral(&'a str),
    // Punctuation & Operators
    Semicolon,
    Comma,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Equals,
    Arrow, // "->"
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

#[derive(Clone, Copy, Debug)]
struct Token<'a> {
    kind: TokenKind<'a>,
    start: usize,
    end: usize,
}

struct Lexer<'a> {
    buffer: &'a SourceBuffer,
    bytes: &'a [u8],
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn tokenize(buffer: &'a SourceBuffer) -> (Vec<Token<'a>>, Vec<Diagnostic>) {
        let mut lexer = Self {
            buffer,
            bytes: buffer.content().as_bytes(),
            cursor: 0,
            diagnostics: Vec::new(),
        };

        let mut tokens = Vec::new();
        loop {
            lexer.skip_whitespace_and_comments();
            if lexer.cursor >= lexer.bytes.len() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    start: lexer.cursor,
                    end: lexer.cursor,
                });
                break;
            }

            if let Some(token) = lexer.next_token() {
                tokens.push(token);
            }
        }

        (tokens, lexer.diagnostics)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.cursor < self.bytes.len() {
            let b = self.bytes[self.cursor];
            if b.is_ascii_whitespace() {
                self.cursor += 1;
            } else if b == b'/' && self.cursor + 1 < self.bytes.len() {
                if self.bytes[self.cursor + 1] == b'/' {
                    // Line comment
                    self.cursor += 2;
                    while self.cursor < self.bytes.len() && self.bytes[self.cursor] != b'\n' {
                        self.cursor += 1;
                    }
                } else if self.bytes[self.cursor + 1] == b'*' {
                    // Block comment
                    let start = self.cursor;
                    self.cursor += 2;
                    let mut closed = false;
                    while self.cursor + 1 < self.bytes.len() {
                        if self.bytes[self.cursor] == b'*' && self.bytes[self.cursor + 1] == b'/' {
                            self.cursor += 2;
                            closed = true;
                            break;
                        }
                        self.cursor += 1;
                    }
                    if !closed {
                        self.cursor = self.bytes.len();
                        if let Ok(span) = self.buffer.span(start, self.cursor) {
                            self.diagnostics.push(
                                Diagnostic::error("unclosed block comment")
                                    .with_code("E0001")
                                    .with_span(span)
                                    .with_file_path(self.buffer.name()),
                            );
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        let start = self.cursor;
        let b = self.bytes[self.cursor];

        // String literal
        if b == b'"' {
            self.cursor += 1;
            let str_start = self.cursor;
            while self.cursor < self.bytes.len() && self.bytes[self.cursor] != b'"' {
                self.cursor += 1;
            }
            if self.cursor >= self.bytes.len() {
                if let Ok(span) = self.buffer.span(start, self.cursor) {
                    self.diagnostics.push(
                        Diagnostic::error("unclosed string literal")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    );
                }
                return None;
            }
            let str_end = self.cursor;
            self.cursor += 1; // Consume closing '"'
            let content = &self.buffer.content()[str_start..str_end];
            return Some(Token {
                kind: TokenKind::StringLiteral(content),
                start,
                end: self.cursor,
            });
        }

        // Single punctuation & operators
        match b {
            b';' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Semicolon,
                    start,
                    end: self.cursor,
                });
            }
            b',' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Comma,
                    start,
                    end: self.cursor,
                });
            }
            b'[' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::LBracket,
                    start,
                    end: self.cursor,
                });
            }
            b']' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::RBracket,
                    start,
                    end: self.cursor,
                });
            }
            b'(' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::LParen,
                    start,
                    end: self.cursor,
                });
            }
            b')' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::RParen,
                    start,
                    end: self.cursor,
                });
            }
            b'{' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::LBrace,
                    start,
                    end: self.cursor,
                });
            }
            b'}' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::RBrace,
                    start,
                    end: self.cursor,
                });
            }
            b'=' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Equals,
                    start,
                    end: self.cursor,
                });
            }
            b'+' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Plus,
                    start,
                    end: self.cursor,
                });
            }
            b'-' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'>' {
                    self.cursor += 2;
                    return Some(Token {
                        kind: TokenKind::Arrow,
                        start,
                        end: self.cursor,
                    });
                }
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Minus,
                    start,
                    end: self.cursor,
                });
            }
            b'*' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Star,
                    start,
                    end: self.cursor,
                });
            }
            b'/' => {
                self.cursor += 1;
                return Some(Token {
                    kind: TokenKind::Slash,
                    start,
                    end: self.cursor,
                });
            }
            _ => {}
        }

        // Numeric literal
        if b.is_ascii_digit() {
            while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                self.cursor += 1;
            }

            let mut is_float = false;
            if self.cursor < self.bytes.len()
                && self.bytes[self.cursor] == b'.'
                && self.cursor + 1 < self.bytes.len()
                && self.bytes[self.cursor + 1].is_ascii_digit()
            {
                is_float = true;
                self.cursor += 1; // Consume '.'
                while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                    self.cursor += 1;
                }
            }

            if self.cursor < self.bytes.len()
                && (self.bytes[self.cursor] == b'e' || self.bytes[self.cursor] == b'E')
            {
                is_float = true;
                self.cursor += 1; // Consume 'e'
                if self.cursor < self.bytes.len()
                    && (self.bytes[self.cursor] == b'+' || self.bytes[self.cursor] == b'-')
                {
                    self.cursor += 1;
                }
                while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                    self.cursor += 1;
                }
            }

            let slice = &self.buffer.content()[start..self.cursor];
            if is_float {
                if let Ok(val) = slice.parse::<f64>() {
                    return Some(Token {
                        kind: TokenKind::FloatLiteral(val),
                        start,
                        end: self.cursor,
                    });
                }
            } else if let Ok(val) = slice.parse::<usize>() {
                return Some(Token {
                    kind: TokenKind::IntLiteral(val),
                    start,
                    end: self.cursor,
                });
            } else if let Ok(val) = slice.parse::<f64>() {
                return Some(Token {
                    kind: TokenKind::FloatLiteral(val),
                    start,
                    end: self.cursor,
                });
            }
        }

        // Identifier or keyword (including Unicode e.g. 'π')
        let ch = self.buffer.content()[self.cursor..].chars().next().unwrap();
        if ch.is_alphabetic() || ch == '_' || ch == 'π' {
            self.cursor += ch.len_utf8();
            while self.cursor < self.buffer.len() {
                let next_ch = self.buffer.content()[self.cursor..].chars().next().unwrap();
                if next_ch.is_alphanumeric() || next_ch == '_' {
                    self.cursor += next_ch.len_utf8();
                } else {
                    break;
                }
            }

            let word = &self.buffer.content()[start..self.cursor];
            let kind = match word {
                "OPENQASM" => TokenKind::OpenQasm,
                "include" => TokenKind::Include,
                "qubit" => TokenKind::Qubit,
                "bit" => TokenKind::Bit,
                "measure" => TokenKind::Measure,
                "reset" => TokenKind::Reset,
                "barrier" => TokenKind::Barrier,
                "gphase" => TokenKind::Gphase,
                "qreg" => TokenKind::Qreg,
                "creg" => TokenKind::Creg,
                "defcal" => TokenKind::Defcal,
                "defcalgrammar" => TokenKind::Defcalgrammar,
                "delay" => TokenKind::Delay,
                "while" => TokenKind::While,
                "for" => TokenKind::For,
                "extern" => TokenKind::Extern,
                "gate" => TokenKind::Gate,
                "int" => TokenKind::Int,
                "float" => TokenKind::Float,
                "angle" => TokenKind::Angle,
                "bool" => TokenKind::Bool,
                _ => TokenKind::Ident(word),
            };

            return Some(Token {
                kind,
                start,
                end: self.cursor,
            });
        }

        // Unknown character
        self.cursor += ch.len_utf8();
        if let Ok(span) = self.buffer.span(start, self.cursor) {
            self.diagnostics.push(
                Diagnostic::error(format!("unexpected character '{ch}'"))
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            );
        }
        None
    }
}

struct Parser<'a> {
    buffer: &'a SourceBuffer,
    tokens: Vec<Token<'a>>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(buffer: &'a SourceBuffer, tokens: Vec<Token<'a>>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            buffer,
            tokens,
            cursor: 0,
            diagnostics,
        }
    }

    fn peek(&self) -> Token<'a> {
        if self.cursor < self.tokens.len() {
            self.tokens[self.cursor]
        } else {
            Token {
                kind: TokenKind::Eof,
                start: self.buffer.len(),
                end: self.buffer.len(),
            }
        }
    }

    fn advance(&mut self) -> Token<'a> {
        let tok = self.peek();
        if self.cursor < self.tokens.len() {
            self.cursor += 1;
        }
        tok
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
            match next_tok.kind {
                TokenKind::FloatLiteral(f) => spec = format!("{f:.1}"),
                TokenKind::IntLiteral(i) => spec = format!("{i}"),
                TokenKind::Ident(s) => spec = s.to_string(),
                TokenKind::StringLiteral(s) => spec = s.to_string(),
                _ => {
                    let span = self
                        .buffer
                        .span(start_tok.start, next_tok.end)
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
                    self.advance();
                    let span = self
                        .buffer
                        .span(start_tok.start, semi.end)
                        .unwrap_or(full_span);
                    version = Some(AstVersion {
                        specifier: spec,
                        span,
                    });
                } else {
                    let span = self
                        .buffer
                        .span(start_tok.start, next_tok.end)
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
            match next_tok.kind {
                TokenKind::StringLiteral(path) => {
                    let semi = self.peek();
                    if semi.kind == TokenKind::Semicolon {
                        self.advance();
                        let span = self
                            .buffer
                            .span(start_tok.start, semi.end)
                            .unwrap_or(full_span);
                        includes.push(AstInclude {
                            path: path.to_string(),
                            span,
                        });
                    } else {
                        let span = self
                            .buffer
                            .span(start_tok.start, next_tok.end)
                            .unwrap_or(full_span);
                        self.error("expected ';' after include statement", span, "E0001");
                        self.skip_to_semicolon();
                    }
                }
                _ => {
                    let span = self
                        .buffer
                        .span(start_tok.start, next_tok.end)
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
        match tok.kind {
            TokenKind::Semicolon => {
                // Empty statement
                self.advance();
                None
            }
            TokenKind::Qubit | TokenKind::Bit => self.parse_register_decl(),
            TokenKind::Qreg | TokenKind::Creg => {
                let kw_tok = self.advance();
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
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
                let span = self
                    .buffer
                    .span(kw_tok.start, kw_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(kw_tok.start).unwrap());
                self.error(
                    "classical scalar memory types are not supported in QccIR at this milestone",
                    span,
                    "E0005",
                );
                self.skip_to_semicolon();
                None
            }
            TokenKind::Ident(name) => {
                // Could be assignment measurement `c[0] = measure q[0];` or a gate call
                if self.is_assignment_measurement() {
                    self.parse_assignment_measure()
                } else {
                    self.parse_gate_call(name)
                }
            }
            _ => {
                let unk = self.advance();
                let span = self
                    .buffer
                    .span(unk.start, unk.end)
                    .unwrap_or_else(|_| self.buffer.point_span(unk.start).unwrap());
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
                    size = Some(sz);
                }
                _ => {
                    let span = self
                        .buffer
                        .span(size_tok.start, size_tok.end)
                        .unwrap_or_else(|_| self.buffer.point_span(size_tok.start).unwrap());
                    self.error("expected integer register size in brackets", span, "E0001");
                    self.skip_to_semicolon();
                    return None;
                }
            }
            let rbracket = self.peek();
            if rbracket.kind == TokenKind::RBracket {
                self.advance();
            } else {
                let span = self
                    .buffer
                    .span(rbracket.start, rbracket.end)
                    .unwrap_or_else(|_| self.buffer.point_span(rbracket.start).unwrap());
                self.error("expected ']' after register size", span, "E0001");
                self.skip_to_semicolon();
                return None;
            }
        }

        let name_tok = self.advance();
        let name = match name_tok.kind {
            TokenKind::Ident(ident) => ident.to_string(),
            TokenKind::Qreg | TokenKind::Creg => {
                let span = self
                    .buffer
                    .span(name_tok.start, name_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
                self.error(
                    "reserved keyword cannot be used as a register identifier",
                    span,
                    "E0002",
                );
                self.skip_to_semicolon();
                return None;
            }
            _ => {
                let span = self
                    .buffer
                    .span(name_tok.start, name_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
                self.error("expected register identifier", span, "E0001");
                self.skip_to_semicolon();
                return None;
            }
        };

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(kind_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(kind_tok.start).unwrap());
            Some(AstStatement::RegisterDecl(AstRegisterDecl {
                kind,
                name,
                size,
                span,
            }))
        } else {
            let span = self
                .buffer
                .span(kind_tok.start, name_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(kind_tok.start).unwrap());
            self.error("expected ';' after register declaration", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn is_assignment_measurement(&self) -> bool {
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
        if idx < self.tokens.len() && self.tokens[idx].kind == TokenKind::Equals {
            idx += 1;
            if idx < self.tokens.len() && self.tokens[idx].kind == TokenKind::Measure {
                return true;
            }
        }
        false
    }

    fn parse_assignment_measure(&mut self) -> Option<AstStatement> {
        let start_tok = self.peek();
        let bit = self.parse_bit_ref()?;

        let eq_tok = self.advance();
        if eq_tok.kind != TokenKind::Equals {
            let span = self
                .buffer
                .span(eq_tok.start, eq_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(eq_tok.start).unwrap());
            self.error("expected '=' in measurement assignment", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let measure_tok = self.advance();
        if measure_tok.kind != TokenKind::Measure {
            let span = self
                .buffer
                .span(measure_tok.start, measure_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(measure_tok.start).unwrap());
            self.error("expected 'measure' keyword following '='", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let qubit = self.parse_qubit_ref()?;

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            Some(AstStatement::Measure(AstMeasure { qubit, bit, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start, qubit.span.end())
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            self.error("expected ';' after measurement assignment", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_arrow_measure(&mut self) -> Option<AstStatement> {
        let start_tok = self.advance(); // Consume 'measure'
        let qubit = self.parse_qubit_ref()?;

        let arrow_tok = self.advance();
        if arrow_tok.kind != TokenKind::Arrow {
            let span = self
                .buffer
                .span(arrow_tok.start, arrow_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(arrow_tok.start).unwrap());
            self.error("expected '->' in measurement arrow syntax", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let bit = self.parse_bit_ref()?;

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            Some(AstStatement::Measure(AstMeasure { qubit, bit, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start, bit.span.end())
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            self.error(
                "expected ';' after measurement arrow statement",
                span,
                "E0001",
            );
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_reset(&mut self) -> Option<AstStatement> {
        let start_tok = self.advance(); // Consume 'reset'
        let qubit = self.parse_qubit_ref()?;

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            Some(AstStatement::Reset(AstReset { qubit, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start, qubit.span.end())
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            self.error("expected ';' after reset statement", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_barrier(&mut self) -> Option<AstStatement> {
        let start_tok = self.advance(); // Consume 'barrier'
        let mut qubits = Vec::new();

        if self.peek().kind != TokenKind::Semicolon {
            loop {
                if let Some(q) = self.parse_qubit_ref() {
                    qubits.push(q);
                } else {
                    self.skip_to_semicolon();
                    return None;
                }

                if self.peek().kind == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            Some(AstStatement::Barrier(AstBarrier { qubits, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start, semi.start)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            self.error("expected ';' after barrier statement", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_gphase(&mut self) -> Option<AstStatement> {
        let start_tok = self.advance(); // Consume 'gphase'
        let lparen = self.advance();
        if lparen.kind != TokenKind::LParen {
            let span = self
                .buffer
                .span(lparen.start, lparen.end)
                .unwrap_or_else(|_| self.buffer.point_span(lparen.start).unwrap());
            self.error("expected '(' after 'gphase'", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let angle = self.parse_expression(0)?;

        let rparen = self.advance();
        if rparen.kind != TokenKind::RParen {
            let span = self
                .buffer
                .span(rparen.start, rparen.end)
                .unwrap_or_else(|_| self.buffer.point_span(rparen.start).unwrap());
            self.error("expected ')' after gphase angle expression", span, "E0001");
            self.skip_to_semicolon();
            return None;
        }

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            Some(AstStatement::GlobalPhase(AstGlobalPhase { angle, span }))
        } else {
            let span = self
                .buffer
                .span(start_tok.start, rparen.end)
                .unwrap_or_else(|_| self.buffer.point_span(start_tok.start).unwrap());
            self.error("expected ';' after gphase statement", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_gate_call(&mut self, gate_name: &'a str) -> Option<AstStatement> {
        let name_tok = self.advance(); // Consume gate name identifier
        let name = gate_name.to_string();

        // 1. Unsupported 3-qubit gates rejection
        if name == "ccx" || name == "cswap" {
            let span = self
                .buffer
                .span(name_tok.start, name_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
            self.error(
                format!("three-qubit gate '{name}' is not supported in QccIR at this milestone"),
                span,
                "E0003",
            );
            self.skip_to_semicolon();
            return Some(AstStatement::Unsupported(AstUnsupported {
                kind: "three_qubit_gate".to_string(),
                raw: name,
                span,
            }));
        }

        // 2. Unsupported derived/custom gates rejection
        if matches!(name.as_str(), "u1" | "u2" | "u3" | "p" | "sx" | "sxdg") {
            let span = self
                .buffer
                .span(name_tok.start, name_tok.end)
                .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
            self.error(
                format!(
                    "gate '{name}' is not supported in QccIR at this milestone without decomposition"
                ),
                span,
                "E0004",
            );
            self.skip_to_semicolon();
            return Some(AstStatement::Unsupported(AstUnsupported {
                kind: "unsupported_gate".to_string(),
                raw: name,
                span,
            }));
        }

        // 3. Optional parameter expressions e.g. rx(pi / 2)
        let mut parameters = Vec::new();
        if self.peek().kind == TokenKind::LParen {
            self.advance(); // Consume '('
            if self.peek().kind != TokenKind::RParen {
                loop {
                    if let Some(param) = self.parse_expression(0) {
                        parameters.push(param);
                    } else {
                        self.skip_to_semicolon();
                        return None;
                    }

                    if self.peek().kind == TokenKind::Comma {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }

            let rparen = self.advance();
            if rparen.kind != TokenKind::RParen {
                let span = self
                    .buffer
                    .span(rparen.start, rparen.end)
                    .unwrap_or_else(|_| self.buffer.point_span(rparen.start).unwrap());
                self.error("expected ')' after gate parameters", span, "E0001");
                self.skip_to_semicolon();
                return None;
            }
        }

        // 4. Ordered qubit operands
        let mut qubits = Vec::new();
        loop {
            if let Some(q) = self.parse_qubit_ref() {
                qubits.push(q);
            } else {
                self.skip_to_semicolon();
                return None;
            }

            if self.peek().kind == TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }

        let semi = self.peek();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(name_tok.start, semi.end)
                .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
            Some(AstStatement::GateCall(AstGateCall {
                name,
                parameters,
                qubits,
                span,
            }))
        } else {
            let span = self
                .buffer
                .span(name_tok.start, semi.start)
                .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
            self.error("expected ';' after gate statement", span, "E0001");
            self.skip_to_semicolon();
            None
        }
    }

    fn parse_qubit_ref(&mut self) -> Option<AstQubitRef> {
        let name_tok = self.advance();
        let name = match name_tok.kind {
            TokenKind::Ident(id) => id.to_string(),
            _ => {
                let span = self
                    .buffer
                    .span(name_tok.start, name_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
                self.error("expected qubit identifier", span, "E0001");
                return None;
            }
        };

        let mut index = None;
        let mut end = name_tok.end;

        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let idx_tok = self.advance();
            match idx_tok.kind {
                TokenKind::IntLiteral(idx) => {
                    index = Some(idx);
                }
                _ => {
                    let span = self
                        .buffer
                        .span(idx_tok.start, idx_tok.end)
                        .unwrap_or_else(|_| self.buffer.point_span(idx_tok.start).unwrap());
                    self.error("expected integer index in qubit reference", span, "E0001");
                    return None;
                }
            }
            let rbracket = self.advance();
            if rbracket.kind == TokenKind::RBracket {
                end = rbracket.end;
            } else {
                let span = self
                    .buffer
                    .span(rbracket.start, rbracket.end)
                    .unwrap_or_else(|_| self.buffer.point_span(rbracket.start).unwrap());
                self.error("expected ']' after qubit index", span, "E0001");
                return None;
            }
        }

        let span = self
            .buffer
            .span(name_tok.start, end)
            .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
        Some(AstQubitRef { name, index, span })
    }

    fn parse_bit_ref(&mut self) -> Option<AstBitRef> {
        let name_tok = self.advance();
        let name = match name_tok.kind {
            TokenKind::Ident(id) => id.to_string(),
            _ => {
                let span = self
                    .buffer
                    .span(name_tok.start, name_tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
                self.error("expected classical bit identifier", span, "E0001");
                return None;
            }
        };

        let mut index = None;
        let mut end = name_tok.end;

        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let idx_tok = self.advance();
            match idx_tok.kind {
                TokenKind::IntLiteral(idx) => {
                    index = Some(idx);
                }
                _ => {
                    let span = self
                        .buffer
                        .span(idx_tok.start, idx_tok.end)
                        .unwrap_or_else(|_| self.buffer.point_span(idx_tok.start).unwrap());
                    self.error("expected integer index in bit reference", span, "E0001");
                    return None;
                }
            }
            let rbracket = self.advance();
            if rbracket.kind == TokenKind::RBracket {
                end = rbracket.end;
            } else {
                let span = self
                    .buffer
                    .span(rbracket.start, rbracket.end)
                    .unwrap_or_else(|_| self.buffer.point_span(rbracket.start).unwrap());
                self.error("expected ']' after bit index", span, "E0001");
                return None;
            }
        }

        let span = self
            .buffer
            .span(name_tok.start, end)
            .unwrap_or_else(|_| self.buffer.point_span(name_tok.start).unwrap());
        Some(AstBitRef { name, index, span })
    }

    fn parse_expression(&mut self, min_bp: u8) -> Option<AstExpression> {
        let mut lhs = self.parse_primary_expression()?;

        loop {
            let tok = self.peek();
            let (lbp, rbp) = match tok.kind {
                TokenKind::Plus | TokenKind::Minus => (1, 2),
                TokenKind::Star | TokenKind::Slash => (3, 4),
                _ => break,
            };

            if lbp < min_bp {
                break;
            }

            self.advance(); // Consume operator
            let rhs = self.parse_expression(rbp)?;
            let span = self
                .buffer
                .span(lhs.span.start(), rhs.span.end())
                .unwrap_or(lhs.span);

            lhs = match tok.kind {
                TokenKind::Plus => AstExpression {
                    kind: AstExprKind::Add(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Minus => AstExpression {
                    kind: AstExprKind::Sub(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Star => AstExpression {
                    kind: AstExprKind::Mul(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Slash => AstExpression {
                    kind: AstExprKind::Div(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                _ => unreachable!(),
            };
        }

        Some(lhs)
    }

    fn parse_primary_expression(&mut self) -> Option<AstExpression> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::FloatLiteral(f) => {
                let span = self
                    .buffer
                    .span(tok.start, tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(tok.start).unwrap());
                Some(AstExpression::constant(f, span))
            }
            TokenKind::IntLiteral(i) => {
                let span = self
                    .buffer
                    .span(tok.start, tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(tok.start).unwrap());
                Some(AstExpression::constant(i as f64, span))
            }
            TokenKind::Ident(name) => {
                let span = self
                    .buffer
                    .span(tok.start, tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(tok.start).unwrap());
                Some(AstExpression::identifier(name, span))
            }
            TokenKind::Minus => {
                let inner = self.parse_expression(5)?;
                let span = self
                    .buffer
                    .span(tok.start, inner.span.end())
                    .unwrap_or(inner.span);
                Some(AstExpression {
                    kind: AstExprKind::Neg(Box::new(inner)),
                    span,
                })
            }
            TokenKind::LParen => {
                let inner = self.parse_expression(0)?;
                let rparen = self.advance();
                if rparen.kind != TokenKind::RParen {
                    let span = self
                        .buffer
                        .span(rparen.start, rparen.end)
                        .unwrap_or_else(|_| self.buffer.point_span(rparen.start).unwrap());
                    self.error("expected ')' closing subexpression", span, "E0001");
                    return None;
                }
                let span = self
                    .buffer
                    .span(tok.start, rparen.end)
                    .unwrap_or(inner.span);
                Some(AstExpression {
                    kind: inner.kind,
                    span,
                })
            }
            _ => {
                let span = self
                    .buffer
                    .span(tok.start, tok.end)
                    .unwrap_or_else(|_| self.buffer.point_span(tok.start).unwrap());
                self.error("expected parameter expression", span, "E0001");
                None
            }
        }
    }
}
