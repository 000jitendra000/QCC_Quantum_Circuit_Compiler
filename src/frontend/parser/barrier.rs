//! Parsing routines for OpenQASM 3 barrier synchronization statements.
//!
//! Handles global barriers (`barrier;`) and selective barriers over indexed qubits or quantum
//! registers (`barrier q[0], q[1];`, `barrier q;`), validating operands against declared quantum registers.

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::AstBarrier;
use crate::frontend::parser::operands::OperandParser;
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Parser for OpenQASM 3 barrier synchronization statements.
pub struct BarrierParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> BarrierParser<'a> {
    /// Constructs a new [`BarrierParser`] over the given token slice and source buffer.
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

    /// Constructs a [`BarrierParser`] starting at a specific token cursor position.
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

    /// Parses a barrier statement: `barrier;` or `barrier q[0], q[1];` or `barrier q;`.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if syntax is malformed, if operands are not declared quantum
    /// registers, or if the terminating semicolon is missing.
    pub fn parse_barrier_statement(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstBarrier, Box<Diagnostic>> {
        let start_tok = self.advance().clone(); // Consume 'barrier'
        let start_pos = start_tok.start();

        // Check for global barrier (`barrier;`)
        if self.peek().kind == TokenKind::Semicolon {
            let semi = self.advance().clone();
            let span = self
                .buffer
                .span(start_pos, semi.end())
                .unwrap_or(start_tok.span);
            return Ok(AstBarrier::new(Vec::new(), span));
        }

        // Parse comma-separated list of qubit operands (permitting unindexed array registers)
        let mut op_parser = OperandParser::with_cursor(self.buffer, self.tokens, self.cursor);
        let qubits = match op_parser.parse_qubit_operands_allow_unindexed(None, true) {
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

        // Expect terminating ';'
        let semi = self.peek().clone();
        let end_pos = if semi.kind == TokenKind::Semicolon {
            self.advance();
            semi.end()
        } else {
            let last_end = qubits
                .last()
                .map(|q| q.span.end())
                .unwrap_or(start_tok.end());
            let span = self
                .buffer
                .span(start_pos, last_end)
                .unwrap_or(start_tok.span);
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected ';' after barrier statement")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        };

        let span = self
            .buffer
            .span(start_pos, end_pos)
            .unwrap_or(start_tok.span);

        // Validate each operand against register table
        if let Some(tbl) = table {
            for q in &qubits {
                if let Err(err) = tbl.validate_qubit_ref(q, true) {
                    return Err(Box::new(err.to_diagnostic(self.buffer.name())));
                }
            }
        }

        Ok(AstBarrier::new(qubits, span))
    }
}
