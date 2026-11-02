//! OpenQASM source lexer providing reliable tokenization, exact source spans,
//! comprehensive edge-case handling, and configurable resource limits.

use crate::diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan};
use crate::frontend::errors::FrontendError;
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Configurable limits to prevent unbounded resource consumption during lexical analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LexerLimits {
    /// Maximum allowed source code length in bytes. Default: 16 MiB (16,777,216 bytes).
    pub max_source_bytes: usize,
    /// Maximum number of tokens permitted. Default: 500,000.
    pub max_tokens: usize,
    /// Maximum permitted byte length of a single token. Default: 65,536 bytes.
    pub max_token_bytes: usize,
}

impl Default for LexerLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 16 * 1024 * 1024,
            max_tokens: 500_000,
            max_token_bytes: 65_536,
        }
    }
}

/// The result of lexical analysis bundling emitted tokens and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct LexResult {
    /// The sequence of scanned tokens, terminated by [`TokenKind::Eof`].
    pub tokens: Vec<Token>,
    /// All diagnostics (errors, warnings) collected during scanning.
    pub diagnostics: Vec<Diagnostic>,
}

impl LexResult {
    /// Returns `true` if scanning succeeded without any [`DiagnosticLevel::Error`] diagnostics.
    pub fn is_ok(&self) -> bool {
        !self.has_errors()
    }

    /// Returns `true` if any diagnostic has [`DiagnosticLevel::Error`] severity.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    /// Returns an iterator over all error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
    }

    /// Returns a slice of the emitted tokens.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns a slice of the collected diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Unwraps the token sequence or returns [`FrontendError::ParseFailed`] if errors occurred.
    pub fn into_tokens(self) -> Result<Vec<Token>, FrontendError> {
        let err_count = self
            .diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count();
        if err_count > 0 {
            Err(FrontendError::ParseFailed { count: err_count })
        } else {
            Ok(self.tokens)
        }
    }
}

/// Standalone OpenQASM source lexer.
pub struct Lexer<'a> {
    buffer: &'a SourceBuffer,
    bytes: &'a [u8],
    cursor: usize,
    limits: LexerLimits,
    diagnostics: Vec<Diagnostic>,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    /// Tokenizes the source buffer using default [`LexerLimits`].
    ///
    /// Returns a tuple of `(tokens, diagnostics)`.
    pub fn tokenize(buffer: &'a SourceBuffer) -> (Vec<Token>, Vec<Diagnostic>) {
        let res = Self::tokenize_with_limits(buffer, LexerLimits::default());
        (res.tokens, res.diagnostics)
    }

    /// Tokenizes the source buffer with explicit [`LexerLimits`].
    pub fn tokenize_with_limits(buffer: &'a SourceBuffer, limits: LexerLimits) -> LexResult {
        let mut lexer = Self {
            buffer,
            bytes: buffer.content().as_bytes(),
            cursor: 0,
            limits,
            diagnostics: Vec::new(),
            tokens: Vec::new(),
        };

        lexer.run();
        LexResult {
            tokens: lexer.tokens,
            diagnostics: lexer.diagnostics,
        }
    }

    fn run(&mut self) {
        // 1. Source size limit check
        if self.buffer.len() > self.limits.max_source_bytes {
            let full_span = self
                .buffer
                .full_span()
                .unwrap_or_else(|_| SourceSpan::point(0, 1, 1).unwrap());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "source buffer size ({} bytes) exceeds limit of {} bytes",
                    self.buffer.len(),
                    self.limits.max_source_bytes
                ))
                .with_code("E0101")
                .with_span(full_span)
                .with_file_path(self.buffer.name()),
            );
            self.tokens.push(Token::eof(0, full_span));
            return;
        }

        while self.cursor < self.bytes.len() {
            // Check token count limit
            if self.tokens.len() >= self.limits.max_tokens {
                let point = self
                    .buffer
                    .point_span(self.cursor)
                    .unwrap_or_else(|_| SourceSpan::point(0, 1, 1).unwrap());
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "token count limit exceeded: reached maximum limit of {} tokens",
                        self.limits.max_tokens
                    ))
                    .with_code("E0102")
                    .with_span(point)
                    .with_file_path(self.buffer.name()),
                );
                break;
            }

            self.skip_whitespace_and_comments();
            if self.cursor >= self.bytes.len() {
                break;
            }

            if let Some(token) = self.scan_next_token() {
                self.tokens.push(token);
            }
        }

        // Emit final EOF token
        let eof_offset = self.buffer.len();
        let eof_span = self
            .buffer
            .point_span(eof_offset)
            .unwrap_or_else(|_| SourceSpan::point(eof_offset, 1, 1).unwrap());
        self.tokens.push(Token::eof(eof_offset, eof_span));
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
                                Diagnostic::error("unterminated block comment")
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

    fn scan_next_token(&mut self) -> Option<Token> {
        let start = self.cursor;
        let b = self.bytes[self.cursor];

        // 1. Quoted string literal: "..."
        if b == b'"' {
            return self.scan_string_literal();
        }

        // 2. Numeric literal (integer, decimal, leading dot, scientific)
        if b.is_ascii_digit()
            || (b == b'.'
                && self.cursor + 1 < self.bytes.len()
                && self.bytes[self.cursor + 1].is_ascii_digit())
        {
            return self.scan_numeric_literal();
        }

        // 3. Punctuation & Operators
        if let Some(tok) = self.scan_operator_or_punctuation() {
            return Some(tok);
        }

        // 4. Identifier or Keyword
        let ch = self.buffer.content()[self.cursor..].chars().next().unwrap();
        if ch.is_alphabetic() || ch == '_' || ch == 'π' {
            return self.scan_identifier_or_keyword();
        }

        // 5. Unrecognized Character
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

    fn scan_string_literal(&mut self) -> Option<Token> {
        let start = self.cursor;
        self.cursor += 1; // Consume opening '"'
        let content_start = self.cursor;

        while self.cursor < self.bytes.len() && self.bytes[self.cursor] != b'"' {
            if self.bytes[self.cursor] == b'\n' {
                // Line breaks inside string literals are reported as unterminated
                let span = self
                    .buffer
                    .span(start, self.cursor)
                    .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
                self.diagnostics.push(
                    Diagnostic::error("unterminated string literal before newline")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                );
                return None;
            }
            self.cursor += 1;
        }

        if self.cursor >= self.bytes.len() {
            let span = self
                .buffer
                .span(start, self.cursor)
                .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
            self.diagnostics.push(
                Diagnostic::error("unterminated string literal at end of input")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            );
            return None;
        }

        let content_end = self.cursor;
        self.cursor += 1; // Consume closing '"'
        let end = self.cursor;

        let token_len = end - start;
        if token_len > self.limits.max_token_bytes {
            let span = self
                .buffer
                .span(start, end)
                .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "string literal length ({} bytes) exceeds limit of {} bytes",
                    token_len, self.limits.max_token_bytes
                ))
                .with_code("E0103")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            );
            return None;
        }

        let content = self.buffer.content()[content_start..content_end].to_string();
        let raw_text = self.buffer.content()[start..end].to_string();
        let span = self
            .buffer
            .span(start, end)
            .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());

        Some(Token::new(
            TokenKind::StringLiteral(content),
            raw_text,
            start,
            end,
            span,
        ))
    }

    fn scan_numeric_literal(&mut self) -> Option<Token> {
        let start = self.cursor;
        let mut has_dot = false;
        let mut has_exp = false;
        let mut malformed = false;

        // Leading dot: .5
        if self.bytes[self.cursor] == b'.' {
            has_dot = true;
            self.cursor += 1;
            while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                self.cursor += 1;
            }
        } else {
            // Leading digits
            while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                self.cursor += 1;
            }

            // Decimal point: 123.456 or 123.
            if self.cursor < self.bytes.len() && self.bytes[self.cursor] == b'.' {
                has_dot = true;
                self.cursor += 1;
                while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                    self.cursor += 1;
                }
            }
        }

        // Check for multiple decimal points: e.g. 1.2.3
        if self.cursor < self.bytes.len() && self.bytes[self.cursor] == b'.' {
            malformed = true;
            while self.cursor < self.bytes.len()
                && (self.bytes[self.cursor].is_ascii_digit() || self.bytes[self.cursor] == b'.')
            {
                self.cursor += 1;
            }
            let span = self
                .buffer
                .span(start, self.cursor)
                .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
            self.diagnostics.push(
                Diagnostic::error("malformed floating-point literal: multiple decimal points")
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
            );
        }

        // Scientific notation exponent: e+10, E-5, e2
        if !malformed
            && self.cursor < self.bytes.len()
            && (self.bytes[self.cursor] == b'e' || self.bytes[self.cursor] == b'E')
        {
            has_exp = true;
            self.cursor += 1; // Consume 'e' or 'E'
            if self.cursor < self.bytes.len()
                && (self.bytes[self.cursor] == b'+' || self.bytes[self.cursor] == b'-')
            {
                self.cursor += 1; // Consume sign
            }

            let exp_digits_start = self.cursor;
            while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_digit() {
                self.cursor += 1;
            }

            if self.cursor == exp_digits_start {
                malformed = true;
                let span = self
                    .buffer
                    .span(start, self.cursor)
                    .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
                self.diagnostics.push(
                    Diagnostic::error(
                        "malformed scientific notation: exponent requires at least one digit",
                    )
                    .with_code("E0001")
                    .with_span(span)
                    .with_file_path(self.buffer.name()),
                );
            }
        }

        let end = self.cursor;
        let token_len = end - start;
        if token_len > self.limits.max_token_bytes {
            let span = self
                .buffer
                .span(start, end)
                .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "numeric literal length ({} bytes) exceeds limit of {} bytes",
                    token_len, self.limits.max_token_bytes
                ))
                .with_code("E0103")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            );
            return None;
        }

        let raw_text = self.buffer.content()[start..end].to_string();
        let span = self
            .buffer
            .span(start, end)
            .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());

        if malformed {
            // Emitted diagnostic above; return fallback float literal to allow continued scanning
            return Some(Token::new(
                TokenKind::FloatLiteral(0.0),
                raw_text,
                start,
                end,
                span,
            ));
        }

        if has_dot || has_exp {
            match raw_text.parse::<f64>() {
                Ok(val) => {
                    if val.is_infinite() {
                        self.diagnostics.push(
                            Diagnostic::error("floating-point literal magnitude exceeds range")
                                .with_code("E0001")
                                .with_span(span)
                                .with_file_path(self.buffer.name()),
                        );
                    }
                    Some(Token::new(
                        TokenKind::FloatLiteral(val),
                        raw_text,
                        start,
                        end,
                        span,
                    ))
                }
                Err(_) => {
                    self.diagnostics.push(
                        Diagnostic::error("malformed floating-point literal")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    );
                    Some(Token::new(
                        TokenKind::FloatLiteral(0.0),
                        raw_text,
                        start,
                        end,
                        span,
                    ))
                }
            }
        } else {
            match raw_text.parse::<u64>() {
                Ok(val) => Some(Token::new(
                    TokenKind::IntLiteral(val),
                    raw_text,
                    start,
                    end,
                    span,
                )),
                Err(_) => {
                    self.diagnostics.push(
                        Diagnostic::error("integer literal exceeds 64-bit unsigned limit")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    );
                    Some(Token::new(
                        TokenKind::IntLiteral(0),
                        raw_text,
                        start,
                        end,
                        span,
                    ))
                }
            }
        }
    }

    fn scan_operator_or_punctuation(&mut self) -> Option<Token> {
        let start = self.cursor;
        let b = self.bytes[self.cursor];

        let (kind, len) = match b {
            b';' => (TokenKind::Semicolon, 1),
            b',' => (TokenKind::Comma, 1),
            b':' => (TokenKind::Colon, 1),
            b'[' => (TokenKind::LBracket, 1),
            b']' => (TokenKind::RBracket, 1),
            b'(' => (TokenKind::LParen, 1),
            b')' => (TokenKind::RParen, 1),
            b'{' => (TokenKind::LBrace, 1),
            b'}' => (TokenKind::RBrace, 1),
            b'%' => (TokenKind::Percent, 1),
            b'^' => (TokenKind::Caret, 1),
            b'=' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::DoubleEquals, 2)
                } else {
                    (TokenKind::Equals, 1)
                }
            }
            b'!' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::NotEquals, 2)
                } else {
                    return None;
                }
            }
            b'<' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::LessEquals, 2)
                } else {
                    (TokenKind::Less, 1)
                }
            }
            b'>' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::GreaterEquals, 2)
                } else {
                    (TokenKind::Greater, 1)
                }
            }
            b'+' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::PlusEquals, 2)
                } else {
                    (TokenKind::Plus, 1)
                }
            }
            b'-' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'>' {
                    (TokenKind::Arrow, 2)
                } else if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'='
                {
                    (TokenKind::MinusEquals, 2)
                } else {
                    (TokenKind::Minus, 1)
                }
            }
            b'*' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::StarEquals, 2)
                } else {
                    (TokenKind::Star, 1)
                }
            }
            b'/' => {
                if self.cursor + 1 < self.bytes.len() && self.bytes[self.cursor + 1] == b'=' {
                    (TokenKind::SlashEquals, 2)
                } else {
                    (TokenKind::Slash, 1)
                }
            }
            _ => return None,
        };

        self.cursor += len;
        let end = self.cursor;
        let raw_text = self.buffer.content()[start..end].to_string();
        let span = self
            .buffer
            .span(start, end)
            .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());

        Some(Token::new(kind, raw_text, start, end, span))
    }

    fn scan_identifier_or_keyword(&mut self) -> Option<Token> {
        let start = self.cursor;
        let ch = self.buffer.content()[self.cursor..].chars().next().unwrap();
        self.cursor += ch.len_utf8();

        while self.cursor < self.buffer.len() {
            let next_ch = self.buffer.content()[self.cursor..].chars().next().unwrap();
            if next_ch.is_alphanumeric() || next_ch == '_' {
                self.cursor += next_ch.len_utf8();
            } else {
                break;
            }
        }

        let end = self.cursor;
        let token_len = end - start;
        if token_len > self.limits.max_token_bytes {
            let span = self
                .buffer
                .span(start, end)
                .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "identifier length ({} bytes) exceeds limit of {} bytes",
                    token_len, self.limits.max_token_bytes
                ))
                .with_code("E0103")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            );
            return None;
        }

        let word = &self.buffer.content()[start..end];
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
            "cal" => TokenKind::Cal,
            "delay" => TokenKind::Delay,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "def" => TokenKind::Def,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "end" => TokenKind::End,
            "extern" => TokenKind::Extern,
            "gate" => TokenKind::Gate,
            "int" => TokenKind::Int,
            "uint" => TokenKind::Uint,
            "float" => TokenKind::Float,
            "angle" => TokenKind::Angle,
            "bool" => TokenKind::Bool,
            "complex" => TokenKind::Complex,
            "duration" => TokenKind::Duration,
            "stretch" => TokenKind::Stretch,
            "const" => TokenKind::Const,
            "readonly" => TokenKind::Readonly,
            "pragma" => TokenKind::Pragma,
            _ => TokenKind::Ident(word.to_string()),
        };

        let span = self
            .buffer
            .span(start, end)
            .unwrap_or_else(|_| self.buffer.point_span(start).unwrap());

        Some(Token::new(kind, word.to_string(), start, end, span))
    }
}
