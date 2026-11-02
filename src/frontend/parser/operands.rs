//! Parsing routines for OpenQASM 3 qubit and classical bit operands.
//!
//! Handles single-qubit references (`q[0]`, `q_scalar`), classical bit references (`c[0]`, `c_scalar`),
//! comma-separated operand lists, and bounds/type validation against the static [`RegisterTable`].

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{AstBitRef, AstQubitRef};
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Parser specialized for quantum and classical register operands.
pub struct OperandParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> OperandParser<'a> {
    /// Constructs a new [`OperandParser`] over the given token slice and source buffer.
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

    /// Constructs an [`OperandParser`] starting at a specific token cursor position.
    pub fn with_cursor(buffer: &'a SourceBuffer, tokens: &'a [Token], cursor: usize) -> Self {
        let mut parser = Self::new(buffer, tokens);
        parser.set_cursor(cursor);
        parser
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

    /// Returns `true` if the cursor has reached EOF or the end of the token slice.
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

    /// Parses a single qubit reference (scalar `q` or indexed `q[k]`), resolving against
    /// the optional [`RegisterTable`] to validate declaration and bounds.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the syntax is malformed or if the register reference is invalid.
    pub fn parse_qubit_ref(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstQubitRef, Box<Diagnostic>> {
        let name_tok = self.advance().clone();
        let name = match &name_tok.kind {
            TokenKind::Ident(id) => id.clone(),
            _ => {
                let span = name_tok.span;
                return Err(Box::new(
                    Diagnostic::error("expected qubit identifier")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let mut index = None;
        let mut end = name_tok.end();

        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let idx_tok = self.advance().clone();
            match idx_tok.kind {
                TokenKind::IntLiteral(idx) => {
                    index = Some(idx as usize);
                }
                _ => {
                    let span = idx_tok.span;
                    return Err(Box::new(
                        Diagnostic::error("expected integer index in qubit reference")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    ));
                }
            }

            let rbracket = self.peek().clone();
            if rbracket.kind == TokenKind::RBracket {
                self.advance();
                end = rbracket.end();
            } else {
                let span = rbracket.span;
                return Err(Box::new(
                    Diagnostic::error("expected ']' after qubit index")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        }

        let span = self
            .buffer
            .span(name_tok.start(), end)
            .unwrap_or(name_tok.span);
        let q_ref = AstQubitRef { name, index, span };

        if let Some(tbl) = table {
            if let Err(err) = tbl.resolve_qubit(&q_ref) {
                return Err(Box::new(err.to_diagnostic(self.buffer.name())));
            }
        }

        Ok(q_ref)
    }

    /// Parses a single classical bit reference (scalar `c` or indexed `c[k]`), resolving against
    /// the optional [`RegisterTable`].
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the syntax is malformed or if the register reference is invalid.
    pub fn parse_bit_ref(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstBitRef, Box<Diagnostic>> {
        let name_tok = self.advance().clone();
        let name = match &name_tok.kind {
            TokenKind::Ident(id) => id.clone(),
            _ => {
                let span = name_tok.span;
                return Err(Box::new(
                    Diagnostic::error("expected classical bit identifier")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        let mut index = None;
        let mut end = name_tok.end();

        if self.peek().kind == TokenKind::LBracket {
            self.advance(); // Consume '['
            let idx_tok = self.advance().clone();
            match idx_tok.kind {
                TokenKind::IntLiteral(idx) => {
                    index = Some(idx as usize);
                }
                _ => {
                    let span = idx_tok.span;
                    return Err(Box::new(
                        Diagnostic::error("expected integer index in bit reference")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    ));
                }
            }

            let rbracket = self.peek().clone();
            if rbracket.kind == TokenKind::RBracket {
                self.advance();
                end = rbracket.end();
            } else {
                let span = rbracket.span;
                return Err(Box::new(
                    Diagnostic::error("expected ']' after bit index")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        }

        let span = self
            .buffer
            .span(name_tok.start(), end)
            .unwrap_or(name_tok.span);
        let b_ref = AstBitRef { name, index, span };

        if let Some(tbl) = table {
            if let Err(err) = tbl.resolve_bit(&b_ref) {
                return Err(Box::new(err.to_diagnostic(self.buffer.name())));
            }
        }

        Ok(b_ref)
    }

    /// Parses a comma-separated list of qubit operands (e.g. `q[0], q[1]`).
    ///
    /// Stops when a terminating token (e.g., semicolon, newline, EOF) or a non-qubit token is reached.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if an operand fails to parse, if a trailing comma is present without
    /// an operand, or if a comma is missing between consecutive operands.
    pub fn parse_qubit_operands(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<Vec<AstQubitRef>, Box<Diagnostic>> {
        let mut operands = Vec::new();

        // If the next token is a semicolon or EOF, the operand list is empty
        if self.peek().kind == TokenKind::Semicolon || self.is_at_end() {
            return Ok(operands);
        }

        loop {
            let q_ref = self.parse_qubit_ref(table)?;
            operands.push(q_ref);

            if self.peek().kind == TokenKind::Comma {
                let comma_tok = self.advance().clone();
                // Check for trailing comma before semicolon
                if self.peek().kind == TokenKind::Semicolon {
                    let span = comma_tok.span;
                    return Err(Box::new(
                        Diagnostic::error("trailing comma in gate operand list")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    ));
                }
            } else {
                break;
            }
        }

        Ok(operands)
    }
}
