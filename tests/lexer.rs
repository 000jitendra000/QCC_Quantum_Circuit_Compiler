//! Integration test runner for the OpenQASM lexer.

#[path = "lexer/golden.rs"]
mod golden;

#[path = "lexer/tokens.rs"]
mod tokens;

#[path = "lexer/numeric.rs"]
mod numeric;

#[path = "lexer/comments.rs"]
mod comments;

#[path = "lexer/strings.rs"]
mod strings;

#[path = "lexer/unicode.rs"]
mod unicode;

#[path = "lexer/limits.rs"]
mod limits;

#[path = "lexer/fuzz_smoke.rs"]
mod fuzz_smoke;
