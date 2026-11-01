//! Source buffer abstractions providing line-start indexing and exact [`SourceSpan`] calculation.

use std::fs;
use std::path::Path;

use crate::diagnostics::{SourceSpan, SpanError};

/// An immutable source code buffer with indexed line starts for efficient
/// and exact 1-indexed [`SourceSpan`] calculation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBuffer {
    /// Logical name or file path associated with the source code.
    name: String,
    /// The raw UTF-8 source text.
    content: String,
    /// 0-indexed byte offsets where each line begins.
    line_starts: Vec<usize>,
}

impl SourceBuffer {
    /// Creates a new [`SourceBuffer`] from a source name and text content.
    ///
    /// Computes and caches line-start byte offsets for rapid line and column lookups.
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let name = name.into();
        let content = content.into();
        let mut line_starts = vec![0];

        for (idx, b) in content.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(idx + 1);
            }
        }

        Self {
            name,
            content,
            line_starts,
        }
    }

    /// Loads a source file from the filesystem and constructs a [`SourceBuffer`].
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let p = path.as_ref();
        let content = fs::read_to_string(p)?;
        let name = p.display().to_string();
        Ok(Self::new(name, content))
    }

    /// Returns the logical name or path of the source buffer.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the raw source text.
    #[inline]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the total byte length of the source text.
    #[inline]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Returns `true` if the source text is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the total number of lines in the source buffer.
    ///
    /// An empty buffer has 1 empty line.
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Computes the 1-indexed `(line, column)` coordinates for a given byte offset.
    ///
    /// Columns are 1-indexed character counts from the beginning of the line.
    ///
    /// # Errors
    /// Returns [`SpanError::InvalidByteRange`] if `offset > self.len()` or if `offset`
    /// does not fall on a valid UTF-8 character boundary.
    pub fn line_col_at(&self, offset: usize) -> Result<(usize, usize), SpanError> {
        if offset > self.content.len() || !self.content.is_char_boundary(offset) {
            return Err(SpanError::InvalidByteRange {
                start: offset,
                end: self.content.len(),
            });
        }

        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next_line) => next_line.saturating_sub(1),
        };

        let line_start = self.line_starts[line_idx];
        let line_slice = &self.content[line_start..offset];
        let col = 1 + line_slice.chars().count();
        let line = line_idx + 1;

        Ok((line, col))
    }

    /// Constructs a validated [`SourceSpan`] for a byte range `[start, end)`.
    ///
    /// # Errors
    /// Returns [`SpanError`] if `start > end`, if offsets exceed buffer length,
    /// or if span invariant checks fail.
    pub fn span(&self, start: usize, end: usize) -> Result<SourceSpan, SpanError> {
        if start > end {
            return Err(SpanError::InvalidByteRange { start, end });
        }
        let (start_line, start_col) = self.line_col_at(start)?;
        let (end_line, end_col) = self.line_col_at(end)?;
        SourceSpan::new(start, end, start_line, start_col, end_line, end_col)
    }

    /// Constructs a single-point [`SourceSpan`] at `offset`.
    pub fn point_span(&self, offset: usize) -> Result<SourceSpan, SpanError> {
        self.span(offset, offset)
    }

    /// Constructs a [`SourceSpan`] covering the entire buffer text.
    pub fn full_span(&self) -> Result<SourceSpan, SpanError> {
        self.span(0, self.content.len())
    }

    /// Returns the raw text of the specified 1-indexed line, stripping trailing `\r` and `\n`.
    pub fn line_text(&self, line_1indexed: usize) -> Option<&str> {
        if line_1indexed == 0 || line_1indexed > self.line_starts.len() {
            return None;
        }

        let line_idx = line_1indexed - 1;
        let start = self.line_starts[line_idx];
        let end = if line_idx + 1 < self.line_starts.len() {
            self.line_starts[line_idx + 1]
        } else {
            self.content.len()
        };

        let mut slice = &self.content[start..end];
        if slice.ends_with('\n') {
            slice = &slice[..slice.len() - 1];
        }
        if slice.ends_with('\r') {
            slice = &slice[..slice.len() - 1];
        }
        Some(slice)
    }

    /// Extracts the substring referenced by a [`SourceSpan`].
    pub fn slice(&self, span: SourceSpan) -> Option<&str> {
        if span.start() <= span.end() && span.end() <= self.content.len() {
            self.content.get(span.start()..span.end())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buf = SourceBuffer::new("empty.qasm", "");
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.line_count(), 1);
        let (line, col) = buf.line_col_at(0).unwrap();
        assert_eq!(line, 1);
        assert_eq!(col, 1);
        let span = buf.full_span().unwrap();
        assert_eq!(span.start(), 0);
        assert_eq!(span.end(), 0);
    }

    #[test]
    fn test_single_line_buffer() {
        let buf = SourceBuffer::new("single.qasm", "qubit[2] q;");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_col_at(0).unwrap(), (1, 1));
        assert_eq!(buf.line_col_at(5).unwrap(), (1, 6)); // '['
        assert_eq!(buf.line_col_at(11).unwrap(), (1, 12)); // after ';'

        let span = buf.span(0, 5).unwrap(); // "qubit"
        assert_eq!(buf.slice(span), Some("qubit"));
        assert_eq!(span.start_line(), 1);
        assert_eq!(span.start_col(), 1);
        assert_eq!(span.end_line(), 1);
        assert_eq!(span.end_col(), 6);
    }

    #[test]
    fn test_multiline_buffer() {
        let src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\n";
        let buf = SourceBuffer::new("test.qasm", src);
        assert_eq!(buf.line_count(), 4); // 3 lines + 1 after trailing newline

        assert_eq!(buf.line_text(1), Some("OPENQASM 3.0;"));
        assert_eq!(buf.line_text(2), Some("qubit[2] q;"));
        assert_eq!(buf.line_text(3), Some("h q[0];"));
        assert_eq!(buf.line_text(4), Some(""));
        assert_eq!(buf.line_text(5), None);

        // "qubit[2] q;" starts at offset 14
        let (line, col) = buf.line_col_at(14).unwrap();
        assert_eq!(line, 2);
        assert_eq!(col, 1);

        let span = buf.span(14, 25).unwrap();
        assert_eq!(buf.slice(span), Some("qubit[2] q;"));
        assert_eq!(span.start_line(), 2);
        assert_eq!(span.start_col(), 1);
        assert_eq!(span.end_line(), 2);
        assert_eq!(span.end_col(), 12);
    }

    #[test]
    fn test_unicode_column_counting() {
        let src = "gphase(π / 2);";
        let buf = SourceBuffer::new("unicode.qasm", src);
        // 'π' is 2 bytes at offset 7..9
        let pi_span = buf.span(7, 9).unwrap();
        assert_eq!(buf.slice(pi_span), Some("π"));
        assert_eq!(pi_span.start_col(), 8); // after "gphase(" (7 ascii chars -> 1-based col 8)
        assert_eq!(pi_span.end_col(), 9); // length of 'π' is 1 char -> col 9
    }

    #[test]
    fn test_out_of_bounds() {
        let buf = SourceBuffer::new("test.qasm", "foo");
        assert!(buf.line_col_at(10).is_err());
        assert!(buf.span(0, 10).is_err());
        assert!(buf.span(5, 2).is_err());
    }
}
