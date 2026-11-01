use std::fmt;

/// Represents a location or contiguous region within a source text.
///
/// # Validity Rules
/// A [`SourceSpan`] is valid if and only if all of the following hold:
/// 1. Byte offsets: `start <= end`.
/// 2. 1-indexed lines: `start_line >= 1` and `end_line >= 1`.
/// 3. 1-indexed columns: `start_col >= 1` and `end_col >= 1`.
/// 4. Line ordering: `start_line <= end_line`.
/// 5. Column ordering on single-line spans: if `start_line == end_line`, then `start_col <= end_col`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceSpan {
    start: usize,
    end: usize,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
}

/// Errors returned when attempting to construct an invalid [`SourceSpan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanError {
    /// The starting byte offset exceeds the ending byte offset (`start > end`).
    InvalidByteRange { start: usize, end: usize },
    /// Line or column index is 0 (must be 1-indexed).
    ZeroLineOrColumn { line: usize, col: usize },
    /// Starting line is greater than ending line (`start_line > end_line`).
    InvertedLines { start_line: usize, end_line: usize },
    /// On the same line, starting column exceeds ending column (`start_col > end_col`).
    InvertedColumns {
        line: usize,
        start_col: usize,
        end_col: usize,
    },
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByteRange { start, end } => {
                write!(
                    f,
                    "invalid byte range: start offset {start} > end offset {end}"
                )
            }
            Self::ZeroLineOrColumn { line, col } => {
                write!(
                    f,
                    "zero line or column: line {line}, column {col} (must be >= 1)"
                )
            }
            Self::InvertedLines {
                start_line,
                end_line,
            } => {
                write!(
                    f,
                    "inverted lines: start line {start_line} > end line {end_line}"
                )
            }
            Self::InvertedColumns {
                line,
                start_col,
                end_col,
            } => {
                write!(
                    f,
                    "inverted columns on line {line}: start column {start_col} > end column {end_col}"
                )
            }
        }
    }
}

impl std::error::Error for SpanError {}

impl SourceSpan {
    /// Checks whether the given coordinates satisfy all [`SourceSpan`] validity rules.
    pub fn is_valid(
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> bool {
        if start > end {
            return false;
        }
        if start_line == 0 || start_col == 0 || end_line == 0 || end_col == 0 {
            return false;
        }
        if start_line > end_line {
            return false;
        }
        if start_line == end_line && start_col > end_col {
            return false;
        }
        true
    }

    /// Creates a new [`SourceSpan`], validating all invariants.
    pub fn new(
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Result<Self, SpanError> {
        if start > end {
            return Err(SpanError::InvalidByteRange { start, end });
        }
        if start_line == 0 {
            return Err(SpanError::ZeroLineOrColumn {
                line: start_line,
                col: start_col,
            });
        }
        if start_col == 0 {
            return Err(SpanError::ZeroLineOrColumn {
                line: start_line,
                col: start_col,
            });
        }
        if end_line == 0 {
            return Err(SpanError::ZeroLineOrColumn {
                line: end_line,
                col: end_col,
            });
        }
        if end_col == 0 {
            return Err(SpanError::ZeroLineOrColumn {
                line: end_line,
                col: end_col,
            });
        }
        if start_line > end_line {
            return Err(SpanError::InvertedLines {
                start_line,
                end_line,
            });
        }
        if start_line == end_line && start_col > end_col {
            return Err(SpanError::InvertedColumns {
                line: start_line,
                start_col,
                end_col,
            });
        }

        Ok(Self {
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        })
    }

    /// Creates a single-point [`SourceSpan`] (such as a single cursor location).
    pub fn point(offset: usize, line: usize, col: usize) -> Result<Self, SpanError> {
        Self::new(offset, offset, line, col, line, col)
    }

    /// Creates a single-line [`SourceSpan`].
    pub fn single_line(
        start: usize,
        end: usize,
        line: usize,
        start_col: usize,
        end_col: usize,
    ) -> Result<Self, SpanError> {
        Self::new(start, end, line, start_col, line, end_col)
    }

    /// Returns the 0-indexed start byte offset.
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the 0-indexed end byte offset (exclusive).
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Returns the byte length of the span.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the span has zero byte length.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the 1-indexed starting line.
    #[inline]
    pub fn start_line(&self) -> usize {
        self.start_line
    }

    /// Returns the 1-indexed starting column.
    #[inline]
    pub fn start_col(&self) -> usize {
        self.start_col
    }

    /// Returns the 1-indexed ending line.
    #[inline]
    pub fn end_line(&self) -> usize {
        self.end_line
    }

    /// Returns the 1-indexed ending column.
    #[inline]
    pub fn end_col(&self) -> usize {
        self.end_col
    }

    /// Returns `true` if the span spans across multiple lines.
    #[inline]
    pub fn is_multiline(&self) -> bool {
        self.start_line != self.end_line
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start_line == self.end_line {
            if self.start_col == self.end_col {
                write!(f, "{}:{}", self.start_line, self.start_col)
            } else {
                write!(f, "{}:{}-{}", self.start_line, self.start_col, self.end_col)
            }
        } else {
            write!(
                f,
                "{}:{}-{}:{}",
                self.start_line, self.start_col, self.end_line, self.end_col
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_point_span() {
        let span = SourceSpan::point(10, 1, 5).unwrap();
        assert_eq!(span.start(), 10);
        assert_eq!(span.end(), 10);
        assert!(span.is_empty());
        assert_eq!(span.to_string(), "1:5");
    }

    #[test]
    fn test_valid_single_line_span() {
        let span = SourceSpan::single_line(0, 10, 2, 1, 11).unwrap();
        assert_eq!(span.len(), 10);
        assert!(!span.is_multiline());
        assert_eq!(span.to_string(), "2:1-11");
    }

    #[test]
    fn test_valid_multiline_span() {
        let span = SourceSpan::new(5, 50, 1, 6, 3, 12).unwrap();
        assert!(span.is_multiline());
        assert_eq!(span.to_string(), "1:6-3:12");
    }

    #[test]
    fn test_invalid_spans() {
        assert!(SourceSpan::new(10, 5, 1, 1, 1, 5).is_err());
        assert!(SourceSpan::new(0, 5, 0, 1, 1, 5).is_err());
        assert!(SourceSpan::new(0, 5, 1, 0, 1, 5).is_err());
        assert!(SourceSpan::new(0, 5, 2, 1, 1, 5).is_err());
        assert!(SourceSpan::new(0, 5, 1, 10, 1, 5).is_err());
    }
}
