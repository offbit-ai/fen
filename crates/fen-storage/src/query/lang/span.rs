//! Source span tracking for error reporting
//!
//! This module provides span types that track positions in source code
//! for precise error reporting.

use std::ops::Range;

/// A span in source code with line and column information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset start (inclusive)
    pub start: usize,
    /// Byte offset end (exclusive)
    pub end: usize,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
}

impl Span {
    /// Create a new span
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self { start, end, line, column }
    }

    /// Create a span from a range with line/column info
    pub fn from_range(range: Range<usize>, line: usize, column: usize) -> Self {
        Self {
            start: range.start,
            end: range.end,
            line,
            column,
        }
    }

    /// Create a zero-width span at a position
    pub fn point(offset: usize, line: usize, column: usize) -> Self {
        Self {
            start: offset,
            end: offset,
            line,
            column,
        }
    }

    /// Get the byte range
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Get the length in bytes
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if span is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Merge two spans into one that covers both
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: if self.line <= other.line { self.column } else { other.column },
        }
    }

    /// Create a span that starts at this span and ends at another
    pub fn to(&self, other: &Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            line: self.line,
            column: self.column,
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self { start: 0, end: 0, line: 1, column: 1 }
    }
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.start..span.end
    }
}

/// A value with an associated source span
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

#[allow(dead_code)]
impl<T> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq> Eq for Spanned<T> {}

/// Source code with line index for fast line lookups
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Source {
    /// The source code
    pub code: String,
    /// File name (for error messages)
    pub name: String,
    /// Byte offsets of line starts
    line_starts: Vec<usize>,
}

#[allow(dead_code)]
impl Source {
    /// Create a new source from code
    pub fn new(name: impl Into<String>, code: impl Into<String>) -> Self {
        let code = code.into();
        let mut line_starts = vec![0];

        for (i, c) in code.char_indices() {
            if c == '\n' {
                line_starts.push(i + 1);
            }
        }

        Self {
            code,
            name: name.into(),
            line_starts,
        }
    }

    /// Get the line number (1-indexed) for a byte offset
    pub fn line_at(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line + 1,
            Err(line) => line,
        }
    }

    /// Get the column number (1-indexed) for a byte offset
    pub fn column_at(&self, offset: usize) -> usize {
        let line = self.line_at(offset);
        let line_start = self.line_starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
        offset - line_start + 1
    }

    /// Get line and column for a byte offset
    pub fn position_at(&self, offset: usize) -> (usize, usize) {
        (self.line_at(offset), self.column_at(offset))
    }

    /// Create a span from a byte range
    pub fn span(&self, range: Range<usize>) -> Span {
        let (line, column) = self.position_at(range.start);
        Span::from_range(range, line, column)
    }

    /// Get the text of a line (1-indexed)
    pub fn line_text(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }

        let start = self.line_starts[line - 1];
        let end = self.line_starts
            .get(line)
            .copied()
            .unwrap_or(self.code.len());

        Some(self.code[start..end].trim_end_matches('\n'))
    }

    /// Get total number of lines
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Get the source code
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Get the file name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_line_tracking() {
        let src = Source::new("test.fen", "SELECT id\nFROM invoices\nWHERE x = 1");

        assert_eq!(src.line_count(), 3);
        assert_eq!(src.line_at(0), 1);
        assert_eq!(src.line_at(5), 1);
        assert_eq!(src.line_at(10), 2);
        assert_eq!(src.line_at(24), 3);
    }

    #[test]
    fn test_source_column_tracking() {
        let src = Source::new("test.fen", "SELECT id\nFROM invoices");

        assert_eq!(src.column_at(0), 1);  // S in SELECT
        assert_eq!(src.column_at(7), 8);  // i in id
        assert_eq!(src.column_at(10), 1); // F in FROM
        assert_eq!(src.column_at(15), 6); // i in invoices
    }

    #[test]
    fn test_span_merge() {
        let a = Span::new(0, 5, 1, 1);
        let b = Span::new(10, 15, 2, 1);
        let merged = a.merge(&b);

        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 15);
    }
}
