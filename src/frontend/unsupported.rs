//! Unsupported OpenQASM 3 syntax detection and diagnostic mapping.
//!
//! Enforces language conformance boundaries and fail-closed diagnostics for OpenQASM 3
//! features that are deferred to later compiler milestones (conditionals, loops, subroutines,
//! user gate definitions, pulse calibrations, unsupported classical scalar types, modifiers, and timing).

use crate::diagnostics::qasm::{
    QASM_RESERVED_KEYWORD, QASM_UNSUPPORTED_3Q_GATE, QASM_UNSUPPORTED_FEATURE,
    QASM_UNSUPPORTED_GATE,
};
use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::token::TokenKind;

/// Categories of unsupported OpenQASM constructs detected by the compiler frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsupportedConstruct {
    /// Classical conditional branching (`if`, `else`).
    Conditional,
    /// Classical control-flow loops (`while`, `for`, `break`, `continue`).
    Loop,
    /// Classical or quantum subroutines (`def`, `return`, `extern`, `end`).
    Subroutine,
    /// User-defined gate definitions (`gate`).
    GateDefinition,
    /// Pulse-level calibrations and directives (`defcal`, `defcalgrammar`, `cal`).
    Calibration,
    /// Classical scalar and aggregate types (`int`, `uint`, `float`, `angle`, `bool`, `complex`).
    ClassicalType,
    /// Modifiers and compiler directives (`const`, `readonly`, `pragma`).
    ModifierOrDirective,
    /// Timing, delay, and duration constructs (`delay`, `duration`, `stretch`).
    Timing,
    /// Legacy OpenQASM 2 keywords (`qreg`, `creg`).
    LegacyKeyword,
    /// Three-qubit gates requiring synthesis or decomposition (`ccx`, `cswap`).
    ThreeQubitGate,
    /// Standard OpenQASM gates requiring basis decomposition (`u1`, `u2`, `u3`, `p`, etc.).
    UnsupportedGate,
}

impl UnsupportedConstruct {
    /// Returns the standardized compiler diagnostic error code for this unsupported construct.
    #[inline]
    pub fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::LegacyKeyword => QASM_RESERVED_KEYWORD, // E0002
            Self::ThreeQubitGate => QASM_UNSUPPORTED_3Q_GATE, // E0003
            Self::UnsupportedGate => QASM_UNSUPPORTED_GATE, // E0004
            _ => QASM_UNSUPPORTED_FEATURE,                // E0005
        }
    }

    /// Returns a stable identifier string used in [`crate::frontend::ast::AstUnsupported::kind`].
    #[inline]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Conditional => "conditional",
            Self::Loop => "loop",
            Self::Subroutine => "subroutine",
            Self::GateDefinition => "gate_definition",
            Self::Calibration => "calibration",
            Self::ClassicalType => "classical_type",
            Self::ModifierOrDirective => "modifier_or_directive",
            Self::Timing => "timing",
            Self::LegacyKeyword => "legacy_keyword",
            Self::ThreeQubitGate => "three_qubit_gate",
            Self::UnsupportedGate => "unsupported_gate",
        }
    }

    /// Returns a descriptive error message for this construct and triggering keyword/name.
    pub fn diagnostic_message(&self, keyword_or_name: &str) -> String {
        match self {
            Self::LegacyKeyword => {
                let replacement = match keyword_or_name {
                    "qreg" => "use 'qubit[size] name;' instead",
                    "creg" => "use 'bit[size] name;' instead",
                    _ => "use OpenQASM 3 register declarations instead",
                };
                format!(
                    "OpenQASM 2 keyword '{keyword_or_name}' is not supported in OpenQASM 3; {replacement}"
                )
            }
            Self::ThreeQubitGate => format!(
                "three-qubit gate '{keyword_or_name}' is not supported in QccIR at this milestone; decomposition into 1- and 2-qubit operations is required"
            ),
            Self::UnsupportedGate => format!(
                "gate '{keyword_or_name}' is not supported in QccIR at this milestone without decomposition"
            ),
            Self::Conditional => format!(
                "classical conditional branching ('{keyword_or_name}') is not supported in QccIR at this milestone"
            ),
            Self::Loop => match keyword_or_name {
                "while" | "for" => {
                    "classical control flow loops ('while'/'for') are not supported in QccIR at this milestone".to_string()
                }
                _ => format!(
                    "classical control flow loop statement ('{keyword_or_name}') is not supported in QccIR at this milestone"
                ),
            },
            Self::Subroutine => match keyword_or_name {
                "extern" => "extern symbol declarations are not supported in QccIR at this milestone".to_string(),
                _ => format!(
                    "subroutines ('{keyword_or_name}') are not supported in QccIR at this milestone"
                ),
            },
            Self::GateDefinition => format!(
                "custom gate definitions ('{keyword_or_name}') are not supported in QccIR at this milestone without macro expansion"
            ),
            Self::Calibration => match keyword_or_name {
                "defcal" | "defcalgrammar" => {
                    "pulse calibration definitions ('defcal') are not supported in QccIR at this milestone".to_string()
                }
                _ => format!(
                    "pulse calibration ('{keyword_or_name}') is not supported in QccIR at this milestone"
                ),
            },
            Self::ClassicalType => format!(
                "classical scalar memory types ('{keyword_or_name}') are not supported in QccIR at this milestone"
            ),
            Self::ModifierOrDirective => format!(
                "modifier or compiler directive ('{keyword_or_name}') is not supported in QccIR at this milestone"
            ),
            Self::Timing => format!(
                "timing construct ('{keyword_or_name}') is not supported in QccIR at this milestone"
            ),
        }
    }

    /// Returns an optional advisory note explaining migration path or roadmap milestone.
    pub fn note(&self, _keyword_or_name: &str) -> Option<String> {
        match self {
            Self::LegacyKeyword => Some("OpenQASM 3 uses 'qubit[n] q;' for quantum registers and 'bit[n] c;' for classical registers.".to_string()),
            Self::ThreeQubitGate => Some("3-qubit gate decomposition is planned for Milestone M4 synthesis passes.".to_string()),
            Self::UnsupportedGate => Some("supported MVP gates: x, y, z, h, s, sdg, t, tdg, rx, ry, rz, cx, cz, swap".to_string()),
            Self::Conditional => Some("conditional execution is scheduled for Phase 32+ (dynamic circuits).".to_string()),
            Self::Loop => Some("classical loops are scheduled for Milestone M6 control-flow extensions.".to_string()),
            Self::Subroutine => Some("subroutine inlining and linkage are scheduled for Milestone M6.".to_string()),
            Self::GateDefinition => Some("custom gate expansion is scheduled for Phase 26 (gate definition macro expansion).".to_string()),
            Self::Calibration => Some("pulse calibrations (OpenPulse) are deferred to backend-specific microcode phases.".to_string()),
            Self::ClassicalType => Some("classical computation on rich scalar types is scheduled for Milestone M6.".to_string()),
            Self::ModifierOrDirective => Some("directives and modifiers will be evaluated in future optimization passes.".to_string()),
            Self::Timing => Some("circuit timing and scheduling delays are scheduled for Milestone M4.".to_string()),
        }
    }

    /// Constructs a fully populated [`Diagnostic`] for this construct.
    pub fn to_diagnostic(
        &self,
        keyword_or_name: &str,
        span: SourceSpan,
        file_path: &str,
    ) -> Diagnostic {
        let mut diag = Diagnostic::error(self.diagnostic_message(keyword_or_name))
            .with_code(self.diagnostic_code())
            .with_span(span)
            .with_file_path(file_path);

        if let Some(note) = self.note(keyword_or_name) {
            diag = diag.with_note(note);
        }

        diag
    }
}

/// Classifies a token kind into an [`UnsupportedConstruct`] and its keyword name, if unsupported.
pub fn classify_unsupported_token(
    kind: &TokenKind,
) -> Option<(UnsupportedConstruct, &'static str)> {
    match kind {
        TokenKind::If => Some((UnsupportedConstruct::Conditional, "if")),
        TokenKind::Else => Some((UnsupportedConstruct::Conditional, "else")),
        TokenKind::While => Some((UnsupportedConstruct::Loop, "while")),
        TokenKind::For => Some((UnsupportedConstruct::Loop, "for")),
        TokenKind::Break => Some((UnsupportedConstruct::Loop, "break")),
        TokenKind::Continue => Some((UnsupportedConstruct::Loop, "continue")),
        TokenKind::Def => Some((UnsupportedConstruct::Subroutine, "def")),
        TokenKind::Return => Some((UnsupportedConstruct::Subroutine, "return")),
        TokenKind::Extern => Some((UnsupportedConstruct::Subroutine, "extern")),
        TokenKind::End => Some((UnsupportedConstruct::Subroutine, "end")),
        TokenKind::Gate => Some((UnsupportedConstruct::GateDefinition, "gate")),
        TokenKind::Defcal => Some((UnsupportedConstruct::Calibration, "defcal")),
        TokenKind::Defcalgrammar => Some((UnsupportedConstruct::Calibration, "defcalgrammar")),
        TokenKind::Cal => Some((UnsupportedConstruct::Calibration, "cal")),
        TokenKind::Int => Some((UnsupportedConstruct::ClassicalType, "int")),
        TokenKind::Uint => Some((UnsupportedConstruct::ClassicalType, "uint")),
        TokenKind::Float => Some((UnsupportedConstruct::ClassicalType, "float")),
        TokenKind::Angle => Some((UnsupportedConstruct::ClassicalType, "angle")),
        TokenKind::Bool => Some((UnsupportedConstruct::ClassicalType, "bool")),
        TokenKind::Complex => Some((UnsupportedConstruct::ClassicalType, "complex")),
        TokenKind::Const => Some((UnsupportedConstruct::ModifierOrDirective, "const")),
        TokenKind::Readonly => Some((UnsupportedConstruct::ModifierOrDirective, "readonly")),
        TokenKind::Pragma => Some((UnsupportedConstruct::ModifierOrDirective, "pragma")),
        TokenKind::Delay => Some((UnsupportedConstruct::Timing, "delay")),
        TokenKind::Duration => Some((UnsupportedConstruct::Timing, "duration")),
        TokenKind::Stretch => Some((UnsupportedConstruct::Timing, "stretch")),
        TokenKind::Qreg => Some((UnsupportedConstruct::LegacyKeyword, "qreg")),
        TokenKind::Creg => Some((UnsupportedConstruct::LegacyKeyword, "creg")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::SourceSpan;

    #[test]
    fn test_unsupported_construct_codes() {
        assert_eq!(
            UnsupportedConstruct::LegacyKeyword.diagnostic_code(),
            "E0002"
        );
        assert_eq!(
            UnsupportedConstruct::ThreeQubitGate.diagnostic_code(),
            "E0003"
        );
        assert_eq!(
            UnsupportedConstruct::UnsupportedGate.diagnostic_code(),
            "E0004"
        );
        assert_eq!(UnsupportedConstruct::Conditional.diagnostic_code(), "E0005");
        assert_eq!(UnsupportedConstruct::Loop.diagnostic_code(), "E0005");
        assert_eq!(UnsupportedConstruct::Subroutine.diagnostic_code(), "E0005");
        assert_eq!(
            UnsupportedConstruct::GateDefinition.diagnostic_code(),
            "E0005"
        );
        assert_eq!(UnsupportedConstruct::Calibration.diagnostic_code(), "E0005");
        assert_eq!(
            UnsupportedConstruct::ClassicalType.diagnostic_code(),
            "E0005"
        );
        assert_eq!(
            UnsupportedConstruct::ModifierOrDirective.diagnostic_code(),
            "E0005"
        );
        assert_eq!(UnsupportedConstruct::Timing.diagnostic_code(), "E0005");
    }

    #[test]
    fn test_unsupported_token_classification() {
        assert_eq!(
            classify_unsupported_token(&TokenKind::If),
            Some((UnsupportedConstruct::Conditional, "if"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Else),
            Some((UnsupportedConstruct::Conditional, "else"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::While),
            Some((UnsupportedConstruct::Loop, "while"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::For),
            Some((UnsupportedConstruct::Loop, "for"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Gate),
            Some((UnsupportedConstruct::GateDefinition, "gate"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Defcal),
            Some((UnsupportedConstruct::Calibration, "defcal"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Cal),
            Some((UnsupportedConstruct::Calibration, "cal"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Def),
            Some((UnsupportedConstruct::Subroutine, "def"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Qreg),
            Some((UnsupportedConstruct::LegacyKeyword, "qreg"))
        );
        assert_eq!(
            classify_unsupported_token(&TokenKind::Creg),
            Some((UnsupportedConstruct::LegacyKeyword, "creg"))
        );
        assert_eq!(classify_unsupported_token(&TokenKind::Qubit), None);
        assert_eq!(classify_unsupported_token(&TokenKind::Bit), None);
    }

    #[test]
    fn test_to_diagnostic() {
        let span = SourceSpan::single_line(0, 5, 1, 1, 6).unwrap();
        let diag = UnsupportedConstruct::Conditional.to_diagnostic("if", span, "test.qasm");
        assert_eq!(diag.code.as_deref(), Some("E0005"));
        assert!(diag.message.contains("classical conditional branching"));
        assert_eq!(diag.file_path.as_deref(), Some("test.qasm"));
        assert!(!diag.notes.is_empty());
    }
}
