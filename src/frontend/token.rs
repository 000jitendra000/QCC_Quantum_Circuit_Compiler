//! Token representation and lexical categories for OpenQASM source code.

use std::fmt;
use std::ops::Range;

use crate::diagnostics::SourceSpan;

/// Syntactic category of a scanned OpenQASM token.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // --- Supported Directives & Core Keywords ---
    /// `OPENQASM` version header keyword.
    OpenQasm,
    /// `include` directive keyword.
    Include,
    /// `qubit` register declaration keyword.
    Qubit,
    /// `bit` classical register declaration keyword.
    Bit,
    /// `measure` projective measurement keyword.
    Measure,
    /// `reset` quantum state reset keyword.
    Reset,
    /// `barrier` synchronization keyword.
    Barrier,
    /// `gphase` explicit global phase keyword.
    Gphase,

    // --- Reserved OpenQASM 2 / Unsupported Language Keywords ---
    /// `qreg` legacy OpenQASM 2 keyword.
    Qreg,
    /// `creg` legacy OpenQASM 2 keyword.
    Creg,
    /// `defcal` pulse calibration definition keyword.
    Defcal,
    /// `defcalgrammar` pulse grammar directive keyword.
    Defcalgrammar,
    /// `cal` inline calibration block keyword.
    Cal,
    /// `delay` timing delay keyword.
    Delay,
    /// `while` classical loop keyword.
    While,
    /// `for` classical loop keyword.
    For,
    /// `in` loop membership keyword.
    In,
    /// `if` classical branch keyword.
    If,
    /// `else` classical branch keyword.
    Else,
    /// `def` subroutine definition keyword.
    Def,
    /// `return` statement keyword.
    Return,
    /// `break` loop termination keyword.
    Break,
    /// `continue` loop continuation keyword.
    Continue,
    /// `end` block ending keyword.
    End,
    /// `extern` external symbol declaration keyword.
    Extern,
    /// `gate` user-defined gate definition keyword.
    Gate,
    /// `int` signed integer type keyword.
    Int,
    /// `uint` unsigned integer type keyword.
    Uint,
    /// `float` floating-point type keyword.
    Float,
    /// `angle` angle type keyword.
    Angle,
    /// `bool` boolean type keyword.
    Bool,
    /// `complex` complex scalar type keyword.
    Complex,
    /// `duration` physical duration keyword.
    Duration,
    /// `stretch` timing stretch keyword.
    Stretch,
    /// `const` constant declaration keyword.
    Const,
    /// `readonly` modifier keyword.
    Readonly,
    /// `pragma` compiler directive keyword.
    Pragma,

    // --- Literals & Identifiers ---
    /// User identifier or named gate/constant (e.g. `q`, `c`, `theta`, `pi`, `π`).
    Ident(String),
    /// Non-negative 64-bit integer literal.
    IntLiteral(u64),
    /// 64-bit floating point literal (decimal or scientific notation).
    FloatLiteral(f64),
    /// Quoted string literal without enclosing double quotes.
    StringLiteral(String),

    // --- Punctuation and Delimiters ---
    /// Semicolon `;`
    Semicolon,
    /// Comma `,`
    Comma,
    /// Colon `:`
    Colon,
    /// Left bracket `[`
    LBracket,
    /// Right bracket `]`
    RBracket,
    /// Left parenthesis `(`
    LParen,
    /// Right parenthesis `)`
    RParen,
    /// Left brace `{`
    LBrace,
    /// Right brace `}`
    RBrace,

    // --- Operators ---
    /// Assignment operator `=`
    Equals,
    /// Measurement arrow `->`
    Arrow,
    /// Addition operator `+`
    Plus,
    /// Subtraction or negation operator `-`
    Minus,
    /// Multiplication operator `*`
    Star,
    /// Division operator `/`
    Slash,
    /// Modulo operator `%`
    Percent,
    /// Exponentiation operator `^`
    Caret,

    // --- Comparison & Compound Operators ---
    /// Equality comparison `==`
    DoubleEquals,
    /// Inequality comparison `!=`
    NotEquals,
    /// Less than `<`
    Less,
    /// Less than or equal `<=`
    LessEquals,
    /// Greater than `>`
    Greater,
    /// Greater than or equal `>=`
    GreaterEquals,
    /// Compound add-assign `+=`
    PlusEquals,
    /// Compound sub-assign `-=`
    MinusEquals,
    /// Compound mul-assign `*=`
    StarEquals,
    /// Compound div-assign `/=`
    SlashEquals,

    // --- End of Input ---
    /// End-of-file sentinel.
    Eof,
}

impl TokenKind {
    /// Returns a human-readable display string for this token kind.
    pub fn description(&self) -> &'static str {
        match self {
            Self::OpenQasm => "keyword 'OPENQASM'",
            Self::Include => "keyword 'include'",
            Self::Qubit => "keyword 'qubit'",
            Self::Bit => "keyword 'bit'",
            Self::Measure => "keyword 'measure'",
            Self::Reset => "keyword 'reset'",
            Self::Barrier => "keyword 'barrier'",
            Self::Gphase => "keyword 'gphase'",
            Self::Qreg => "keyword 'qreg'",
            Self::Creg => "keyword 'creg'",
            Self::Defcal => "keyword 'defcal'",
            Self::Defcalgrammar => "keyword 'defcalgrammar'",
            Self::Cal => "keyword 'cal'",
            Self::Delay => "keyword 'delay'",
            Self::While => "keyword 'while'",
            Self::For => "keyword 'for'",
            Self::In => "keyword 'in'",
            Self::If => "keyword 'if'",
            Self::Else => "keyword 'else'",
            Self::Def => "keyword 'def'",
            Self::Return => "keyword 'return'",
            Self::Break => "keyword 'break'",
            Self::Continue => "keyword 'continue'",
            Self::End => "keyword 'end'",
            Self::Extern => "keyword 'extern'",
            Self::Gate => "keyword 'gate'",
            Self::Int => "keyword 'int'",
            Self::Uint => "keyword 'uint'",
            Self::Float => "keyword 'float'",
            Self::Angle => "keyword 'angle'",
            Self::Bool => "keyword 'bool'",
            Self::Complex => "keyword 'complex'",
            Self::Duration => "keyword 'duration'",
            Self::Stretch => "keyword 'stretch'",
            Self::Const => "keyword 'const'",
            Self::Readonly => "keyword 'readonly'",
            Self::Pragma => "keyword 'pragma'",
            Self::Ident(_) => "identifier",
            Self::IntLiteral(_) => "integer literal",
            Self::FloatLiteral(_) => "floating-point literal",
            Self::StringLiteral(_) => "string literal",
            Self::Semicolon => "';'",
            Self::Comma => "','",
            Self::Colon => "':'",
            Self::LBracket => "'['",
            Self::RBracket => "']'",
            Self::LParen => "'('",
            Self::RParen => "')'",
            Self::LBrace => "'{'",
            Self::RBrace => "'}'",
            Self::Equals => "'='",
            Self::Arrow => "'->'",
            Self::Plus => "'+'",
            Self::Minus => "'-'",
            Self::Star => "'*'",
            Self::Slash => "'/'",
            Self::Percent => "'%'",
            Self::Caret => "'^'",
            Self::DoubleEquals => "'=='",
            Self::NotEquals => "'!='",
            Self::Less => "'<'",
            Self::LessEquals => "'<='",
            Self::Greater => "'>'",
            Self::GreaterEquals => "'>='",
            Self::PlusEquals => "'+='",
            Self::MinusEquals => "'-='",
            Self::StarEquals => "'*='",
            Self::SlashEquals => "'/='",
            Self::Eof => "end-of-file",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// A scanned token from the OpenQASM source text with exact text and [`SourceSpan`].
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// Semantic token category and payload.
    pub kind: TokenKind,
    /// Verbatim text slice of the token in the source buffer.
    pub text: String,
    /// 0-indexed byte offset where the token begins in the source text.
    pub byte_start: usize,
    /// 0-indexed byte offset where the token ends (exclusive).
    pub byte_end: usize,
    /// Validated 1-indexed line and column source span.
    pub span: SourceSpan,
}

impl Token {
    /// Constructs a new [`Token`].
    pub fn new(
        kind: TokenKind,
        text: impl Into<String>,
        byte_start: usize,
        byte_end: usize,
        span: SourceSpan,
    ) -> Self {
        Self {
            kind,
            text: text.into(),
            byte_start,
            byte_end,
            span,
        }
    }

    /// Constructs an end-of-file sentinel token at the specified byte offset and point span.
    pub fn eof(offset: usize, span: SourceSpan) -> Self {
        Self {
            kind: TokenKind::Eof,
            text: String::new(),
            byte_start: offset,
            byte_end: offset,
            span,
        }
    }

    /// Returns `true` if this token is the end-of-file sentinel.
    #[inline]
    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::Eof)
    }

    /// Returns the token kind reference.
    #[inline]
    pub fn kind(&self) -> &TokenKind {
        &self.kind
    }

    /// Returns the raw verbatim text of the token.
    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the validated [`SourceSpan`] for this token.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the 0-indexed byte range `[byte_start, byte_end)`.
    #[inline]
    pub fn byte_range(&self) -> Range<usize> {
        self.byte_start..self.byte_end
    }

    /// Returns the 0-indexed start byte offset.
    #[inline]
    pub fn start(&self) -> usize {
        self.byte_start
    }

    /// Returns the 0-indexed end byte offset (exclusive).
    #[inline]
    pub fn end(&self) -> usize {
        self.byte_end
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.kind, self.span)
    }
}
