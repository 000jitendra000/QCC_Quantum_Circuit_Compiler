//! Parsing routines for OpenQASM 3 standard quantum gate statements.
//!
//! Enforces case-sensitive gate signatures, parameter-first syntax (`rx(theta) q[1]`),
//! parameter and qubit operand arity validation, operand order preservation, and source span tracking.

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{AstGateCall, AstQubitRef};
use crate::frontend::parser::expressions::ExpressionParser;
use crate::frontend::parser::operands::OperandParser;
use crate::frontend::parser::registers::RegisterTable;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Canonical metadata describing the parameter and qubit arity of a supported gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateSignature {
    /// Canonical lowercase gate name in OpenQASM 3.
    pub name: &'static str,
    /// Number of qubit operands required by the gate.
    pub num_qubits: usize,
    /// Number of parameter expressions required by the gate.
    pub num_params: usize,
    /// Whether this gate requires a parameter list in parentheses.
    pub is_parameterized: bool,
}

impl GateSignature {
    /// Constructs a non-parameterized gate signature.
    pub const fn fixed(name: &'static str, num_qubits: usize) -> Self {
        Self {
            name,
            num_qubits,
            num_params: 0,
            is_parameterized: false,
        }
    }

    /// Constructs a parameterized gate signature.
    pub const fn parameterized(name: &'static str, num_qubits: usize, num_params: usize) -> Self {
        Self {
            name,
            num_qubits,
            num_params,
            is_parameterized: true,
        }
    }
}

/// Static registry of supported MVP quantum gates.
pub const SUPPORTED_GATES: &[GateSignature] = &[
    // Single-qubit fixed gates
    GateSignature::fixed("x", 1),
    GateSignature::fixed("y", 1),
    GateSignature::fixed("z", 1),
    GateSignature::fixed("h", 1),
    GateSignature::fixed("s", 1),
    GateSignature::fixed("sdg", 1),
    GateSignature::fixed("t", 1),
    GateSignature::fixed("tdg", 1),
    // Single-qubit parameterized rotations
    GateSignature::parameterized("rx", 1, 1),
    GateSignature::parameterized("ry", 1, 1),
    GateSignature::parameterized("rz", 1, 1),
    // Two-qubit entangling gates
    GateSignature::fixed("cx", 2),
    GateSignature::fixed("cz", 2),
    GateSignature::fixed("swap", 2),
];

/// Looks up the gate signature for the given gate name.
///
/// OpenQASM gate names are strictly case-sensitive. Returns `None` if the name does not
/// match any supported gate in [`SUPPORTED_GATES`].
pub fn lookup_gate_signature(name: &str) -> Option<GateSignature> {
    SUPPORTED_GATES.iter().copied().find(|sig| sig.name == name)
}

/// Checks if a lowercase version of `name` matches a supported gate, returning the canonical
/// name if a case mismatch is detected.
pub fn find_similar_gate(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    SUPPORTED_GATES
        .iter()
        .find(|sig| sig.name == lower)
        .map(|sig| sig.name)
}

/// Parser specialized for standard OpenQASM quantum gate statements.
pub struct GateParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
}

impl<'a> GateParser<'a> {
    /// Constructs a new [`GateParser`] over the given token slice and source buffer.
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

    /// Constructs a [`GateParser`] starting at a specific token cursor position.
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

    /// Parses a complete gate statement (e.g. `h q[0];`, `cx q[0], q[1];`, `rx(pi/2) q[1];`).
    ///
    /// Validates gate name, parameter arity, qubit operand arity, and bounds against [`RegisterTable`].
    /// Preserves exact written operand ordering and expression trees without lossy evaluation.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if the gate name is unknown/unsupported, parameters or operands are malformed,
    /// arity is violated, or register references are invalid.
    pub fn parse_gate_statement(
        &mut self,
        table: Option<&RegisterTable>,
    ) -> Result<AstGateCall, Box<Diagnostic>> {
        let name_tok = self.advance().clone();
        let name = match &name_tok.kind {
            TokenKind::Ident(ident) => ident.clone(),
            _ => {
                let span = name_tok.span;
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected gate identifier")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }
        };

        // 1. Verify gate identity and check supported signatures
        let sig = match lookup_gate_signature(&name) {
            Some(sig) => sig,
            None => {
                let span = name_tok.span;
                self.skip_to_semicolon();

                // Known 3-qubit gates rejection
                if name == "ccx" || name == "cswap" {
                    return Err(Box::new(
                        Diagnostic::error(format!(
                            "three-qubit gate '{name}' is not supported in QccIR at this milestone"
                        ))
                        .with_code("E0003")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                    ));
                }

                // Known derived/custom gates rejection
                if matches!(name.as_str(), "u1" | "u2" | "u3" | "p" | "sx" | "sxdg") {
                    return Err(Box::new(
                        Diagnostic::error(format!(
                            "gate '{name}' is not supported in QccIR at this milestone without decomposition"
                        ))
                        .with_code("E0004")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                    ));
                }

                // General unknown or incorrectly cased gate
                let mut diag = Diagnostic::error(format!("unknown or unsupported gate '{name}'"))
                    .with_code("E0020")
                    .with_span(span)
                    .with_file_path(self.buffer.name());

                if let Some(suggested) = find_similar_gate(&name) {
                    diag = diag.with_note(format!(
                        "gate names in OpenQASM are case-sensitive; did you mean '{suggested}'?"
                    ));
                }

                return Err(Box::new(diag));
            }
        };

        // 2. Parse parameter list (if parameterized or if parens provided)
        let mut parameters = Vec::new();
        if self.peek().kind == TokenKind::LParen {
            let lparen_tok = self.advance().clone();

            if self.peek().kind != TokenKind::RParen {
                loop {
                    let mut expr_parser =
                        ExpressionParser::with_cursor(self.buffer, self.tokens, self.cursor);
                    let expr = match expr_parser.parse_expression(0) {
                        Ok(e) => {
                            self.cursor = expr_parser.cursor();
                            e
                        }
                        Err(d) => {
                            self.cursor = expr_parser.cursor();
                            self.skip_to_semicolon();
                            return Err(d);
                        }
                    };
                    parameters.push(expr);

                    if self.peek().kind == TokenKind::Comma {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }

            let rparen_tok = self.peek().clone();
            if rparen_tok.kind == TokenKind::RParen {
                self.advance();
            } else {
                let span = self
                    .buffer
                    .span(lparen_tok.start(), rparen_tok.start())
                    .unwrap_or(lparen_tok.span);
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error("expected ')' after gate parameters")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }

            let param_span = self
                .buffer
                .span(lparen_tok.start(), rparen_tok.end())
                .unwrap_or(lparen_tok.span);

            // Arity check on parameters
            if !sig.is_parameterized {
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error(format!(
                        "gate '{}' takes 0 parameters, but {} parameter(s) provided",
                        sig.name,
                        parameters.len()
                    ))
                    .with_code("E0021")
                    .with_span(param_span)
                    .with_file_path(self.buffer.name()),
                ));
            }

            if parameters.len() != sig.num_params {
                self.skip_to_semicolon();
                return Err(Box::new(
                    Diagnostic::error(format!(
                        "gate '{}' takes {} parameter(s), but {} provided",
                        sig.name,
                        sig.num_params,
                        parameters.len()
                    ))
                    .with_code("E0021")
                    .with_span(param_span)
                    .with_file_path(self.buffer.name()),
                ));
            }
        } else if sig.is_parameterized {
            // Missing parameter list for a parameterized gate
            let span = name_tok.span;
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error(format!(
                    "gate '{}' requires {} parameter(s), but none was provided",
                    sig.name, sig.num_params
                ))
                .with_code("E0021")
                .with_span(span)
                .with_file_path(self.buffer.name())
                .with_note(format!("syntax is '{}(<expr>) <operands>;'", sig.name)),
            ));
        }

        // 3. Parse ordered qubit operands
        let mut op_parser = OperandParser::with_cursor(self.buffer, self.tokens, self.cursor);
        let qubits: Vec<AstQubitRef> = match op_parser.parse_qubit_operands(table) {
            Ok(qs) => {
                self.cursor = op_parser.cursor();
                qs
            }
            Err(diag) => {
                self.cursor = op_parser.cursor();
                self.skip_to_semicolon();
                return Err(diag);
            }
        };

        // 4. Validate qubit operand arity
        if qubits.len() != sig.num_qubits {
            let start = name_tok.start();
            let end = qubits
                .last()
                .map(|q| q.span.end())
                .unwrap_or(name_tok.end());
            let span = self.buffer.span(start, end).unwrap_or(name_tok.span);
            self.skip_to_semicolon();
            return Err(Box::new(
                Diagnostic::error(format!(
                    "gate '{}' takes {} qubit operand(s), but {} provided",
                    sig.name,
                    sig.num_qubits,
                    qubits.len()
                ))
                .with_code("E0022")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            ));
        }

        // 5. Consume terminating semicolon
        let semi_tok = self.peek().clone();
        if semi_tok.kind == TokenKind::Semicolon {
            self.advance();
            let span = self
                .buffer
                .span(name_tok.start(), semi_tok.end())
                .unwrap_or(name_tok.span);

            Ok(AstGateCall {
                name,
                parameters,
                qubits,
                span,
            })
        } else {
            let start = name_tok.start();
            let end = qubits
                .last()
                .map(|q| q.span.end())
                .unwrap_or(name_tok.end());
            let span = self.buffer.span(start, end).unwrap_or(name_tok.span);
            self.skip_to_semicolon();
            Err(Box::new(
                Diagnostic::error("expected ';' after gate statement")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            ))
        }
    }
}
