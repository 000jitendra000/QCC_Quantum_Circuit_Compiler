//! Parsing routines for OpenQASM 3 version directives, includes, and register declarations.

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{AstInclude, AstRegisterDecl, AstRegisterKind, AstVersion};
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Parser specialized for OpenQASM program headers, includes, and register declarations.
pub struct DeclarationParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> DeclarationParser<'a> {
    /// Constructs a new [`DeclarationParser`] over the given token slice and source buffer.
    pub fn new(buffer: &'a SourceBuffer, tokens: &'a [Token]) -> Self {
        let eof_offset = buffer.len();
        let eof_span = buffer
            .point_span(eof_offset)
            .unwrap_or_else(|_| SourceSpan::point(eof_offset, 1, 1).unwrap());
        let eof_token = Token::eof(eof_offset, eof_span);
        Self {
            buffer,
            tokens,
            cursor: 0,
            eof_token,
        }
    }

    /// Returns the current token cursor position.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Sets the token cursor position.
    #[inline]
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.tokens.len());
    }

    /// Returns `true` if the cursor has reached EOF.
    #[inline]
    pub fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len() || self.peek().kind == TokenKind::Eof
    }

    /// Peeks at the next token without advancing.
    #[inline]
    pub fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.eof_token
        }
    }

    /// Advances the cursor and returns a reference to the consumed token.
    #[inline]
    pub fn advance(&mut self) -> &Token {
        if self.cursor < self.tokens.len() {
            let tok = &self.tokens[self.cursor];
            self.cursor += 1;
            tok
        } else {
            &self.eof_token
        }
    }

    /// Skips tokens until a statement-terminating semicolon or EOF is reached.
    pub fn skip_to_semicolon(&mut self) {
        while self.cursor < self.tokens.len() {
            let tok = self.advance();
            if tok.kind == TokenKind::Semicolon || tok.kind == TokenKind::Eof {
                break;
            }
        }
    }

    /// Parses an optional OpenQASM version header (`OPENQASM 3.0;`).
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the `OPENQASM` keyword is present but is not followed by
    /// a valid version specifier and terminating semicolon.
    pub fn parse_version(&mut self) -> Result<Option<AstVersion>, Box<Diagnostic>> {
        if self.peek().kind != TokenKind::OpenQasm {
            return Ok(None);
        }

        let start_tok = self.advance().clone();
        let next_tok = self.advance().clone();
        let spec = match &next_tok.kind {
            TokenKind::FloatLiteral(f) => format!("{f:.1}"),
            TokenKind::IntLiteral(i) => format!("{i}"),
            TokenKind::Ident(s) | TokenKind::StringLiteral(s) => s.clone(),
            _ => {
                let span = self
                    .buffer
                    .span(start_tok.start(), next_tok.end())
                    .unwrap_or(start_tok.span);
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected version specifier following 'OPENQASM'")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let semi = self.peek().clone();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start(), semi.end())
                .unwrap_or(start_tok.span);
            Ok(Some(AstVersion::new(spec, span)))
        } else {
            let span = self
                .buffer
                .span(start_tok.start(), next_tok.end())
                .unwrap_or(start_tok.span);
            self.skip_to_semicolon();
            Err(Box::new(
                Diagnostic::error("expected ';' after OPENQASM version declaration")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ))
        }
    }

    /// Parses a single include directive (`include "stdgates.inc";`).
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the `include` directive is malformed or lacks a valid path.
    pub fn parse_include(&mut self) -> Result<AstInclude, Box<Diagnostic>> {
        let start_tok = self.advance().clone();
        let next_tok = self.advance().clone();

        let path = match &next_tok.kind {
            TokenKind::StringLiteral(p) => p.clone(),
            _ => {
                let span = self
                    .buffer
                    .span(start_tok.start(), next_tok.end())
                    .unwrap_or(start_tok.span);
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected string literal path following 'include'")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let semi = self.peek().clone();
        if semi.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(start_tok.start(), semi.end())
                .unwrap_or(start_tok.span);
            Ok(AstInclude::new(path, span))
        } else {
            let span = self
                .buffer
                .span(start_tok.start(), next_tok.end())
                .unwrap_or(start_tok.span);
            self.skip_to_semicolon();
            Err(Box::new(
                Diagnostic::error("expected ';' after include statement")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ))
        }
    }

    /// Parses all consecutive include statements from the current token stream.
    pub fn parse_includes(&mut self) -> (Vec<AstInclude>, Vec<Diagnostic>) {
        let mut includes = Vec::new();
        let mut diags = Vec::new();

        while self.peek().kind == TokenKind::Include {
            match self.parse_include() {
                Ok(inc) => includes.push(inc),
                Err(d) => diags.push(*d),
            }
        }

        (includes, diags)
    }

    /// Parses a single register declaration statement (`qubit[n] q;`, `qubit q;`, `bit[n] c;`, `bit c;`)
    /// and registers it into the provided [`RegisterTable`].
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the declaration is syntactically invalid (missing brackets, non-integer size,
    /// missing identifier, missing semicolon), has an invalid size (size 0), or duplicates an existing identifier.
    pub fn parse_register_decl(
        &mut self,
        table: &mut RegisterTable,
    ) -> Result<AstRegisterDecl, Box<Diagnostic>> {
        let kind_tok = self.advance().clone();
        let kind = match kind_tok.kind {
            TokenKind::Qubit => AstRegisterKind::Qubit,
            TokenKind::Bit => AstRegisterKind::Bit,
            _ => {
                let span = kind_tok.span;
                return Err(Box::new(
                    Diagnostic::error("expected 'qubit' or 'bit' register declaration keyword")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let mut size = None;
        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let size_tok = self.advance().clone();
            match size_tok.kind {
                TokenKind::IntLiteral(sz) => {
                    let sz_val = sz as usize;
                    if sz_val == 0 {
                        // Reject zero-sized register declaration immediately
                        let span = self
                            .buffer
                            .span(kind_tok.start(), size_tok.end())
                            .unwrap_or(size_tok.span);
                        self.skip_to_semicolon();
                        return Err(Box::new(
                            Diagnostic::error(format!(
                                "{kind} register has invalid size 0; register size must be positive"
                            ))
                            .with_code("E0011")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                        ));
                    }
                    size = Some(sz_val);
                }
                _ => {
                    let span = size_tok.span;
                    self.skip_to_semicolon();
                    return Err(Box::new(
                        Diagnostic::error("expected integer register size in brackets")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    ));
                }
            }

            let rbracket = self.peek().clone();
            if rbracket.kind == TokenKind::RBracket {
                self.advance();
            } else {
                let span = rbracket.span;
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected ']' after register size")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        }

        let name_tok = self.advance().clone();
        let name = match &name_tok.kind {
            TokenKind::Ident(ident) => ident.clone(),
            TokenKind::Qreg | TokenKind::Creg => {
                let span = name_tok.span;
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("reserved keyword cannot be used as a register identifier")
                        .with_code("E0002")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
            _ => {
                let span = name_tok.span;
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected register identifier")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let semi = self.peek().clone();
        if semi.kind != TokenKind::Semicolon {
            let span = self
                .buffer
                .span(kind_tok.start(), name_tok.end())
                .unwrap_or(kind_tok.span);
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected ';' after register declaration")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        }
        self.advance(); // Consume ';'

        let full_span = self
            .buffer
            .span(kind_tok.start(), semi.end())
            .unwrap_or(kind_tok.span);

        // Register into symbol table and validate duplicates / collisions
        match kind {
            AstRegisterKind::Qubit => {
                table
                    .declare_quantum(&name, size, full_span)
                    .map_err(|e| Box::new(e.to_diagnostic(self.buffer.name())))?;
            }
            AstRegisterKind::Bit => {
                table
                    .declare_classical(&name, size, full_span)
                    .map_err(|e| Box::new(e.to_diagnostic(self.buffer.name())))?;
            }
        }

        Ok(AstRegisterDecl::new(kind, name, size, full_span))
    }
}
