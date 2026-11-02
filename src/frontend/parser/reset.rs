//! Parsing routines for OpenQASM 3 state reset statements.
//!
//! Handles single-qubit and whole-register active resets (`reset q[0];`, `reset q;`),
//! validating that operands reference declared quantum registers (never classical bits).

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::AstReset;
use crate::frontend::parser::operands::OperandParser;
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Parser for OpenQASM 3 reset statements.
pub struct ResetParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> ResetParser<'a> {
    /// Constructs a new [`ResetParser`] over the given token slice and source buffer.
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

    /// Constructs a [`ResetParser`] starting at a specific token cursor position.
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

    /// Skips tokens until a statement-terminating semicolon or EOF is reached.
    pub fn skip_to_semicolon(&mut self) {
        while self.cursor < self.tokens.len() {
            let tok = self.advance();
            if tok.kind == TokenKind::Semicolon || tok.kind == TokenKind::Eof {
                break;
            }
        }
    }

    /// Parses a state reset statement: `reset q;` or `reset q[k];`.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if syntax is malformed, if the qubit operand is invalid,
    /// or if the semicolon is missing.
    pub fn parse_reset_statement(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstReset, Box<Diagnostic>> {
        let start_tok = self.advance().clone(); // Consume 'reset'
        let start_pos = start_tok.start();

        // 1. Parse quantum operand (permitting unindexed array for whole-register reset)
        let mut op_parser = OperandParser::with_cursor(self.buffer, self.tokens, self.cursor);
        let qubit = match op_parser.parse_qubit_operand(None, true) {
            Ok(q) => {
                self.cursor = op_parser.cursor();
                q
            }
            Err(d) => {
                self.cursor = op_parser.cursor();
                self.skip_to_semicolon();
                return Err(d);
            }
        };

        // 2. Expect terminating ';'
        let semi = self.peek().clone();
        let end_pos = if semi.kind == TokenKind::Semicolon {
            self.advance();
            semi.end()
        } else {
            let span = self
                .buffer
                .span(start_pos, qubit.span.end())
                .unwrap_or(qubit.span);
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected ';' after reset statement")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        };

        let span = self.buffer.span(start_pos, end_pos).unwrap_or(qubit.span);

        // 3. Validate against register table (must be declared quantum register, in bounds)
        if let Some(tbl) = table {
            if let Err(err) = tbl.validate_qubit_ref(&qubit, true) {
                return Err(Box::new(err.to_diagnostic(self.buffer.name())));
            }
        }

        Ok(AstReset::new(qubit, span))
    }
}
