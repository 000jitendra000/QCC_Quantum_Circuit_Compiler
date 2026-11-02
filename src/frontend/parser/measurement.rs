//! Parsing routines for OpenQASM 3 measurement statements.
//!
//! Handles single-bit and whole-register assignment measurement (`c[0] = measure q[0];`, `c = measure q;`),
//! legacy arrow measurement (`measure q[0] -> c[0];`, `measure q -> c;`), quantum source and classical
//! destination type validation, and error recovery.

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::AstMeasure;
use crate::frontend::parser::operands::OperandParser;
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Parser for OpenQASM 3 quantum measurement statements.
pub struct MeasurementParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> MeasurementParser<'a> {
    /// Constructs a new [`MeasurementParser`] over the given token slice and source buffer.
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

    /// Constructs a [`MeasurementParser`] starting at a specific token cursor position.
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

    /// Parses an assignment measurement statement: `target = measure source;`.
    ///
    /// Supports:
    /// - Single-bit measurement: `c[i] = measure q[j];`
    /// - Whole-register measurement: `c = measure q;`
    /// - Scalar measurement: `c = measure q;` (where `bit c; qubit q;`)
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if syntax is invalid, if `measure` is missing following `=`,
    /// or if register types, bounds, or sizes fail validation.
    pub fn parse_assignment_measurement(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstMeasure, Box<Diagnostic>> {
        let start_pos = self.peek().start();

        // 1. Parse classical destination bit reference (permitting unindexed array)
        let mut op_parser = OperandParser::with_cursor(self.buffer, self.tokens, self.cursor);
        let bit = match op_parser.parse_bit_operand(None, true) {
            Ok(b) => {
                self.cursor = op_parser.cursor();
                b
            }
            Err(d) => {
                self.cursor = op_parser.cursor();
                self.skip_to_semicolon();
                return Err(d);
            }
        };

        // 2. Expect '='
        let eq_tok = self.advance().clone();
        if eq_tok.kind != TokenKind::Equals {
            let span = eq_tok.span;
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected '=' in measurement assignment")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        }

        // 3. Expect 'measure'
        let measure_tok = self.advance().clone();
        if measure_tok.kind != TokenKind::Measure {
            let span = measure_tok.span;
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error(
                    "expected 'measure' keyword following '=' in measurement assignment",
                )
                .with_code("E0001")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            ));
        }

        // 4. Parse quantum source qubit reference (permitting unindexed array)
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

        // 5. Expect terminating ';'
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
                Diagnostic::error("expected ';' after measurement assignment")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        };

        let span = self.buffer.span(start_pos, end_pos).unwrap_or(qubit.span);

        // 6. Validate quantum/classical typing, bounds, and register size matching
        if let Some(tbl) = table {
            if let Err(err) = tbl.validate_measurement(&qubit, &bit, span) {
                return Err(Box::new(err.to_diagnostic(self.buffer.name())));
            }
        }

        Ok(AstMeasure::new(qubit, bit, span))
    }

    /// Parses an arrow measurement statement: `measure source -> target;`.
    ///
    /// Supports:
    /// - Single-bit measurement: `measure q[i] -> c[j];`
    /// - Whole-register measurement: `measure q -> c;`
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if syntax is invalid, if `->` is missing, or if register
    /// types, bounds, or sizes fail validation.
    pub fn parse_arrow_measurement(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstMeasure, Box<Diagnostic>> {
        let start_tok = self.advance().clone(); // Consume 'measure'
        let start_pos = start_tok.start();

        // 1. Parse quantum source qubit reference
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

        // 2. Expect '->'
        let arrow_tok = self.advance().clone();
        if arrow_tok.kind != TokenKind::Arrow {
            let span = arrow_tok.span;
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected '->' in measurement arrow syntax")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        }

        // 3. Parse classical destination bit reference
        let mut op_parser = OperandParser::with_cursor(self.buffer, self.tokens, self.cursor);
        let bit = match op_parser.parse_bit_operand(None, true) {
            Ok(b) => {
                self.cursor = op_parser.cursor();
                b
            }
            Err(d) => {
                self.cursor = op_parser.cursor();
                self.skip_to_semicolon();
                return Err(d);
            }
        };

        // 4. Expect terminating ';'
        let semi = self.peek().clone();
        let end_pos = if semi.kind == TokenKind::Semicolon {
            self.advance();
            semi.end()
        } else {
            let span = self
                .buffer
                .span(start_pos, bit.span.end())
                .unwrap_or(bit.span);
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error("expected ';' after measurement arrow statement")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ));
        };

        let span = self.buffer.span(start_pos, end_pos).unwrap_or(bit.span);

        // 5. Validate quantum/classical typing, bounds, and register size matching
        if let Some(tbl) = table {
            if let Err(err) = tbl.validate_measurement(&qubit, &bit, span) {
                return Err(Box::new(err.to_diagnostic(self.buffer.name())));
            }
        }

        Ok(AstMeasure::new(qubit, bit, span))
    }
}
