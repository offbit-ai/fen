//! Error types and diagnostics for the query language
//!
//! This module provides user-friendly error messages with source code
//! highlighting, suggestions, and context.

use std::fmt;
use std::io::Write;

use ariadne::{Color, Config, Label, Report, ReportKind, Source as AriadneSource};

use super::span::Span;

/// Parse error with diagnostic information
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Error kind
    pub kind: ParseErrorKind,
    /// Location in source
    pub span: Span,
    /// The source code being parsed
    pub source: String,
    /// File name for error messages
    pub file_name: String,
    /// Additional context or suggestion
    pub help: Option<String>,
    /// Related errors or notes
    pub notes: Vec<(Span, String)>,
}

impl ParseError {
    /// Create a new parse error
    pub fn new(kind: ParseErrorKind, span: Span, source: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            source: source.into(),
            file_name: "<query>".to_string(),
            help: None,
            notes: Vec::new(),
        }
    }

    /// Set the file name
    pub fn with_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = name.into();
        self
    }

    /// Add a help message
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Add a related note
    pub fn with_note(mut self, span: Span, message: impl Into<String>) -> Self {
        self.notes.push((span, message.into()));
        self
    }

    /// Generate a formatted error report
    pub fn report(&self) -> String {
        let mut buf = Vec::new();
        self.write_report(&mut buf).unwrap_or_default();
        String::from_utf8(buf).unwrap_or_else(|_| self.to_string())
    }

    /// Write the error report to a writer
    pub fn write_report<W: Write>(&self, writer: W) -> std::io::Result<()> {
        let mut builder = Report::build(ReportKind::Error, &self.file_name, self.span.start)
            .with_config(Config::default().with_color(true))
            .with_message(self.kind.message());

        // Main error label
        builder = builder.with_label(
            Label::new((&self.file_name, self.span.start..self.span.end))
                .with_message(self.kind.label())
                .with_color(Color::Red),
        );

        // Add notes
        for (span, message) in &self.notes {
            builder = builder.with_label(
                Label::new((&self.file_name, span.start..span.end))
                    .with_message(message)
                    .with_color(Color::Blue),
            );
        }

        // Add help
        if let Some(help) = &self.help {
            builder = builder.with_help(help);
        }

        let report = builder.finish();
        report.write((&self.file_name, AriadneSource::from(&self.source)), writer)
    }

    /// Get the error message without colors (for logging)
    pub fn plain_message(&self) -> String {
        format!(
            "{}:{}: error: {}\n  --> {}",
            self.file_name,
            self.span.line,
            self.kind.message(),
            self.kind.label()
        )
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.report())
    }
}

impl std::error::Error for ParseError {}

/// Parse error kinds
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    // Lexer errors
    UnexpectedCharacter(char),
    UnterminatedString,
    InvalidNumber(String),
    InvalidEscape(char),

    // Parser errors
    UnexpectedToken {
        expected: Vec<String>,
        found: String,
    },
    UnexpectedEof {
        expected: Vec<String>,
    },
    ExpectedKeyword(String),
    ExpectedIdentifier,
    ExpectedExpression,

    // Query structure errors
    MissingSelectClause,
    MissingFromClause,
    InvalidTableName(String),
    InvalidColumnName(String),
    DuplicateAlias(String),

    // Function errors
    UnknownFunction(String),
    WrongArgumentCount {
        function: String,
        expected: usize,
        found: usize,
    },
    InvalidArgumentType {
        function: String,
        position: usize,
        expected: String,
        found: String,
    },

    // Semantic errors
    UnknownColumn {
        column: String,
        table: String,
        suggestions: Vec<String>,
    },
    UnknownTable(String),
    AmbiguousColumn(String),
    TypeMismatch {
        expected: String,
        found: String,
    },
    UndefinedParameter(String),

    // JSON query errors
    InvalidSyntax(String),
    UnsupportedFeature(String),

    // Generic
    Custom(String),
}

impl ParseErrorKind {
    /// Get the main error message
    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedCharacter(c) => format!("unexpected character '{}'", c),
            Self::UnterminatedString => "unterminated string literal".to_string(),
            Self::InvalidNumber(s) => format!("invalid number '{}'", s),
            Self::InvalidEscape(c) => format!("invalid escape sequence '\\{}'", c),

            Self::UnexpectedToken { expected, found } => {
                let exp = format_expected(expected);
                format!("unexpected token '{}', expected {}", found, exp)
            }
            Self::UnexpectedEof { expected } => {
                let exp = format_expected(expected);
                format!("unexpected end of input, expected {}", exp)
            }
            Self::ExpectedKeyword(kw) => format!("expected keyword '{}'", kw),
            Self::ExpectedIdentifier => "expected identifier".to_string(),
            Self::ExpectedExpression => "expected expression".to_string(),

            Self::MissingSelectClause => "missing SELECT clause".to_string(),
            Self::MissingFromClause => "missing FROM clause".to_string(),
            Self::InvalidTableName(t) => format!("invalid table name '{}'", t),
            Self::InvalidColumnName(c) => format!("invalid column name '{}'", c),
            Self::DuplicateAlias(a) => format!("duplicate alias '{}'", a),

            Self::UnknownFunction(f) => format!("unknown function '{}'", f),
            Self::WrongArgumentCount {
                function,
                expected,
                found,
            } => {
                format!(
                    "function '{}' expects {} argument(s), found {}",
                    function, expected, found
                )
            }
            Self::InvalidArgumentType {
                function,
                position,
                expected,
                found,
            } => {
                format!(
                    "invalid argument type for '{}' at position {}: expected {}, found {}",
                    function, position, expected, found
                )
            }

            Self::UnknownColumn { column, table, .. } => {
                format!("unknown column '{}' in table '{}'", column, table)
            }
            Self::UnknownTable(t) => format!("unknown table '{}'", t),
            Self::AmbiguousColumn(c) => format!("ambiguous column reference '{}'", c),
            Self::TypeMismatch { expected, found } => {
                format!("type mismatch: expected {}, found {}", expected, found)
            }
            Self::UndefinedParameter(p) => format!("undefined parameter '{}'", p),

            Self::InvalidSyntax(msg) => format!("invalid syntax: {}", msg),
            Self::UnsupportedFeature(msg) => format!("unsupported feature: {}", msg),

            Self::Custom(msg) => msg.clone(),
        }
    }

    /// Get the label for the error span
    pub fn label(&self) -> String {
        match self {
            Self::UnexpectedCharacter(_) => "unexpected character here".to_string(),
            Self::UnterminatedString => "string starts here but is never closed".to_string(),
            Self::InvalidNumber(_) => "invalid number literal".to_string(),
            Self::InvalidEscape(_) => "invalid escape sequence".to_string(),

            Self::UnexpectedToken { found, .. } => format!("'{}' is not valid here", found),
            Self::UnexpectedEof { .. } => "input ends here".to_string(),
            Self::ExpectedKeyword(kw) => format!("expected '{}' here", kw),
            Self::ExpectedIdentifier => "expected identifier here".to_string(),
            Self::ExpectedExpression => "expected expression here".to_string(),

            Self::MissingSelectClause => "query must start with SELECT".to_string(),
            Self::MissingFromClause => "query needs a FROM clause".to_string(),
            Self::InvalidTableName(t) => format!("'{}' is not a valid table", t),
            Self::InvalidColumnName(c) => format!("'{}' is not a valid column name", c),
            Self::DuplicateAlias(a) => format!("'{}' is already defined", a),

            Self::UnknownFunction(f) => format!("'{}' is not a known function", f),
            Self::WrongArgumentCount { .. } => "wrong number of arguments".to_string(),
            Self::InvalidArgumentType { .. } => "invalid argument type".to_string(),

            Self::UnknownColumn {
                column,
                suggestions,
                ..
            } => {
                if suggestions.is_empty() {
                    format!("'{}' does not exist", column)
                } else {
                    format!(
                        "'{}' does not exist, did you mean '{}'?",
                        column, suggestions[0]
                    )
                }
            }
            Self::UnknownTable(t) => format!("'{}' is not a valid table", t),
            Self::AmbiguousColumn(c) => format!("'{}' exists in multiple tables", c),
            Self::TypeMismatch { found, .. } => format!("found '{}' here", found),
            Self::UndefinedParameter(p) => format!("'{}' is not defined", p),

            Self::InvalidSyntax(_) => "invalid syntax here".to_string(),
            Self::UnsupportedFeature(_) => "not supported".to_string(),

            Self::Custom(_) => "error here".to_string(),
        }
    }

    /// Get help text if available
    pub fn help(&self) -> Option<String> {
        match self {
            Self::UnknownColumn { suggestions, .. } if !suggestions.is_empty() => {
                Some(format!("available columns: {}", suggestions.join(", ")))
            }
            Self::UnknownFunction(f) => {
                let known = known_functions();
                if let Some(suggestion) = find_similar(f, &known) {
                    Some(format!("did you mean '{}'?", suggestion))
                } else {
                    Some(format!("known functions: {}", known.join(", ")))
                }
            }
            Self::UnknownTable(_) => Some("valid tables are: invoices, contracts".to_string()),
            Self::MissingFromClause => {
                Some("add a FROM clause to specify the table to query".to_string())
            }
            Self::UndefinedParameter(p) => {
                Some(format!("make sure to pass '{}' in the query parameters", p))
            }
            _ => None,
        }
    }
}

/// Query execution error
#[derive(Debug, Clone)]
pub enum QueryError {
    /// Parse error
    Parse(ParseError),
    /// Semantic validation error
    Semantic(ParseError),
    /// Execution error
    Execution(String),
    /// Missing required parameter
    MissingParameter(String),
    /// Invalid parameter type
    InvalidParameterType { name: String, expected: String },
    /// Storage error
    Storage(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{}", e),
            Self::Semantic(e) => write!(f, "{}", e),
            Self::Execution(msg) => write!(f, "execution error: {}", msg),
            Self::MissingParameter(name) => write!(f, "missing required parameter: {}", name),
            Self::InvalidParameterType { name, expected } => {
                write!(f, "parameter '{}' must be {}", name, expected)
            }
            Self::Storage(msg) => write!(f, "storage error: {}", msg),
        }
    }
}

impl std::error::Error for QueryError {}

impl From<ParseError> for QueryError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

// Helper functions

fn format_expected(expected: &[String]) -> String {
    match expected.len() {
        0 => "something else".to_string(),
        1 => expected[0].clone(),
        2 => format!("{} or {}", expected[0], expected[1]),
        _ => {
            let (last, rest) = expected.split_last().unwrap();
            format!("{}, or {}", rest.join(", "), last)
        }
    }
}

fn known_functions() -> Vec<&'static str> {
    vec![
        "VECTOR_DISTANCE",
        "BM25_SCORE",
        "CONTAINS",
        "LOWER",
        "UPPER",
        "COALESCE",
    ]
}

fn find_similar<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let target_upper = target.to_uppercase();
    candidates
        .iter()
        .filter(|c| levenshtein(&target_upper, c) <= 3)
        .min_by_key(|c| levenshtein(&target_upper, c))
        .copied()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_report() {
        let source = "SELECT * FROM unknown_table";
        let span = Span::new(14, 27, 1, 15);
        let err = ParseError::new(
            ParseErrorKind::UnknownTable("unknown_table".to_string()),
            span,
            source,
        )
        .with_help("valid tables are: invoices, contracts");

        let report = err.report();
        assert!(report.contains("unknown_table"));
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("VECTOR_DISTANCE", "VECTOR_DISTANCE"), 0);
        assert_eq!(levenshtein("VECTOR_DISTANC", "VECTOR_DISTANCE"), 1);
        assert_eq!(levenshtein("VEC_DIST", "VECTOR_DISTANCE"), 7);
    }

    #[test]
    fn test_find_similar() {
        let functions = vec!["VECTOR_DISTANCE", "BM25_SCORE", "CONTAINS"];
        assert_eq!(
            find_similar("VECTOR_DISTANC", &functions),
            Some("VECTOR_DISTANCE")
        );
        assert_eq!(find_similar("BM25_SCOR", &functions), Some("BM25_SCORE")); // edit distance 1
        assert_eq!(find_similar("CONTANS", &functions), Some("CONTAINS")); // edit distance 1
        assert_eq!(find_similar("BM25", &functions), None); // edit distance 6 - too far
        assert_eq!(find_similar("SOMETHING_RANDOM", &functions), None);
    }
}
