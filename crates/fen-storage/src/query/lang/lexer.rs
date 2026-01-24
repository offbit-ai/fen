//! Lexer for the Fen query language
//!
//! Tokenizes SQL-like query strings with support for our custom functions.

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, multispace0, multispace1, one_of},
    combinator::{opt, recognize, value},
    multi::many0,
    sequence::{pair, preceded, tuple},
    IResult,
};
use nom_locate::LocatedSpan;

use super::error::{ParseError, ParseErrorKind};
use super::span::Span;

/// Input type with position tracking
pub type Input<'a> = LocatedSpan<&'a str>;

/// Token with span information
#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

impl<'a> Token<'a> {
    pub fn new(kind: TokenKind<'a>, input: Input<'a>, len: usize) -> Self {
        Self {
            kind,
            span: Span::new(
                input.location_offset(),
                input.location_offset() + len,
                input.location_line() as usize,
                input.get_column(),
            ),
        }
    }
}

/// Token kinds
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind<'a> {
    // Keywords
    Select,
    From,
    Where,
    And,
    Or,
    Not,
    As,
    OrderBy,
    Asc,
    Desc,
    Limit,
    Offset,
    Nulls,
    First,
    Last,
    Like,
    ILike,
    In,
    Is,
    Null,
    True,
    False,
    Between,

    // Identifiers and literals
    Ident(&'a str),
    String(String),
    Integer(i64),
    Float(f64),
    Parameter(&'a str),

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Punctuation
    LParen,
    RParen,
    Comma,
    Dot,
    Colon,
    Semicolon,

    // Special
    Eof,
}

impl<'a> TokenKind<'a> {
    #[allow(dead_code)]
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Select
                | TokenKind::From
                | TokenKind::Where
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::Not
                | TokenKind::As
                | TokenKind::OrderBy
                | TokenKind::Asc
                | TokenKind::Desc
                | TokenKind::Limit
                | TokenKind::Offset
                | TokenKind::Like
                | TokenKind::ILike
                | TokenKind::In
                | TokenKind::Is
                | TokenKind::Null
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Between
        )
    }

    pub fn as_str(&self) -> &str {
        match self {
            TokenKind::Select => "SELECT",
            TokenKind::From => "FROM",
            TokenKind::Where => "WHERE",
            TokenKind::And => "AND",
            TokenKind::Or => "OR",
            TokenKind::Not => "NOT",
            TokenKind::As => "AS",
            TokenKind::OrderBy => "ORDER BY",
            TokenKind::Asc => "ASC",
            TokenKind::Desc => "DESC",
            TokenKind::Limit => "LIMIT",
            TokenKind::Offset => "OFFSET",
            TokenKind::Nulls => "NULLS",
            TokenKind::First => "FIRST",
            TokenKind::Last => "LAST",
            TokenKind::Like => "LIKE",
            TokenKind::ILike => "ILIKE",
            TokenKind::In => "IN",
            TokenKind::Is => "IS",
            TokenKind::Null => "NULL",
            TokenKind::True => "TRUE",
            TokenKind::False => "FALSE",
            TokenKind::Between => "BETWEEN",
            TokenKind::Ident(s) => s,
            TokenKind::String(_) => "<string>",
            TokenKind::Integer(_) => "<integer>",
            TokenKind::Float(_) => "<float>",
            TokenKind::Parameter(_) => "<parameter>",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Eq => "=",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::Comma => ",",
            TokenKind::Dot => ".",
            TokenKind::Colon => ":",
            TokenKind::Semicolon => ";",
            TokenKind::Eof => "<EOF>",
        }
    }
}

/// Lexer state
pub struct Lexer<'a> {
    input: &'a str,
    tokens: Vec<Token<'a>>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            tokens: Vec::new(),
        }
    }

    /// Tokenize the input
    pub fn tokenize(&mut self) -> Result<&[Token<'a>], ParseError> {
        let input = Input::new(self.input);
        match tokenize_all(input) {
            Ok((_, tokens)) => {
                self.tokens = tokens;
                Ok(&self.tokens)
            }
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => {
                let offset = e.input.location_offset();
                let line = e.input.location_line() as usize;
                let col = e.input.get_column();

                let kind = if e.input.is_empty() {
                    ParseErrorKind::UnexpectedEof {
                        expected: vec!["token".to_string()],
                    }
                } else {
                    let c = e.input.chars().next().unwrap_or('?');
                    ParseErrorKind::UnexpectedCharacter(c)
                };

                Err(ParseError::new(
                    kind,
                    Span::new(offset, offset + 1, line, col),
                    self.input,
                ))
            }
            Err(nom::Err::Incomplete(_)) => {
                Err(ParseError::new(
                    ParseErrorKind::UnexpectedEof {
                        expected: vec!["more input".to_string()],
                    },
                    Span::new(self.input.len(), self.input.len(), 1, 1),
                    self.input,
                ))
            }
        }
    }

    /// Get the tokens
    #[allow(dead_code)]
    pub fn tokens(&self) -> &[Token<'a>] {
        &self.tokens
    }
}

// Parser combinators for tokenization

fn tokenize_all(input: Input) -> IResult<Input, Vec<Token>> {
    let (input, tokens) = many0(preceded(multispace0, token))(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, tokens))
}

fn token(input: Input) -> IResult<Input, Token> {
    alt((
        keyword_or_ident,
        number,
        string_literal,
        parameter,
        operator,
        punctuation,
    ))(input)
}

fn keyword_or_ident(input: Input) -> IResult<Input, Token> {
    let start = input.clone();

    // First character must be alphabetic or underscore (not digit!)
    let (input, ident) = recognize(pair(
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_'),
    ))(input)?;

    let ident_str = *ident.fragment();
    let len = ident_str.len();

    // Check for "ORDER BY" as two words
    let (input, kind) = if ident_str.eq_ignore_ascii_case("ORDER") {
        let (input, _) = multispace1(input)?;
        let (input, _by) = tag_no_case("BY")(input)?;
        (input, TokenKind::OrderBy)
    } else {
        let kind = match ident_str.to_uppercase().as_str() {
            "SELECT" => TokenKind::Select,
            "FROM" => TokenKind::From,
            "WHERE" => TokenKind::Where,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "NOT" => TokenKind::Not,
            "AS" => TokenKind::As,
            "ASC" => TokenKind::Asc,
            "DESC" => TokenKind::Desc,
            "LIMIT" => TokenKind::Limit,
            "OFFSET" => TokenKind::Offset,
            "NULLS" => TokenKind::Nulls,
            "FIRST" => TokenKind::First,
            "LAST" => TokenKind::Last,
            "LIKE" => TokenKind::Like,
            "ILIKE" => TokenKind::ILike,
            "IN" => TokenKind::In,
            "IS" => TokenKind::Is,
            "NULL" => TokenKind::Null,
            "TRUE" => TokenKind::True,
            "FALSE" => TokenKind::False,
            "BETWEEN" => TokenKind::Between,
            _ => TokenKind::Ident(ident_str),
        };
        (input, kind)
    };

    let token = Token::new(kind, start, len);
    Ok((input, token))
}

fn number(input: Input) -> IResult<Input, Token> {
    let start = input.clone();

    // Try to parse a float first
    let float_result: IResult<Input, Input> = recognize(tuple((
        opt(char('-')),
        take_while1(|c: char| c.is_ascii_digit()),
        char('.'),
        take_while1(|c: char| c.is_ascii_digit()),
        opt(tuple((
            one_of("eE"),
            opt(one_of("+-")),
            take_while1(|c: char| c.is_ascii_digit()),
        ))),
    )))(input.clone());

    if let Ok((input, num_str)) = float_result {
        let s = *num_str.fragment();
        if let Ok(f) = s.parse::<f64>() {
            return Ok((input, Token::new(TokenKind::Float(f), start, s.len())));
        }
    }

    // Try integer
    let (input, num_str) = recognize(pair(
        opt(char('-')),
        take_while1(|c: char| c.is_ascii_digit()),
    ))(input)?;

    let s = *num_str.fragment();
    let i = s.parse::<i64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input.clone(), nom::error::ErrorKind::Digit))
    })?;

    Ok((input, Token::new(TokenKind::Integer(i), start, s.len())))
}

fn string_literal(input: Input) -> IResult<Input, Token> {
    let start = input.clone();

    // Single-quoted string
    let (input, _) = char('\'')(input)?;
    let (input, content) = take_until("'")(input)?;
    let (input, _) = char('\'')(input)?;

    let s = content.fragment().to_string();
    let len = s.len() + 2; // Include quotes

    Ok((input, Token::new(TokenKind::String(s), start, len)))
}

fn parameter(input: Input) -> IResult<Input, Token> {
    let start = input.clone();
    let (input, _) = char(':')(input)?;
    let (input, name) = take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)?;

    let name_str = *name.fragment();
    let len = 1 + name_str.len();

    Ok((input, Token::new(TokenKind::Parameter(name_str), start, len)))
}

fn operator(input: Input) -> IResult<Input, Token> {
    let start = input.clone();

    let (input, kind) = alt((
        value(TokenKind::LtEq, tag("<=")),
        value(TokenKind::GtEq, tag(">=")),
        value(TokenKind::NotEq, tag("!=")),
        value(TokenKind::NotEq, tag("<>")),
        value(TokenKind::Eq, char('=')),
        value(TokenKind::Lt, char('<')),
        value(TokenKind::Gt, char('>')),
        value(TokenKind::Plus, char('+')),
        value(TokenKind::Minus, char('-')),
        value(TokenKind::Star, char('*')),
        value(TokenKind::Slash, char('/')),
        value(TokenKind::Percent, char('%')),
    ))(input)?;

    let len = match kind {
        TokenKind::LtEq | TokenKind::GtEq | TokenKind::NotEq => 2,
        _ => 1,
    };

    Ok((input, Token::new(kind, start, len)))
}

fn punctuation(input: Input) -> IResult<Input, Token> {
    let start = input.clone();

    let (input, kind) = alt((
        value(TokenKind::LParen, char('(')),
        value(TokenKind::RParen, char(')')),
        value(TokenKind::Comma, char(',')),
        value(TokenKind::Dot, char('.')),
        value(TokenKind::Colon, char(':')),
        value(TokenKind::Semicolon, char(';')),
    ))(input)?;

    Ok((input, Token::new(kind, start, 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_select() {
        let mut lexer = Lexer::new("SELECT id FROM invoices");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].kind, TokenKind::Select));
        assert!(matches!(tokens[1].kind, TokenKind::Ident("id")));
        assert!(matches!(tokens[2].kind, TokenKind::From));
        assert!(matches!(tokens[3].kind, TokenKind::Ident("invoices")));
    }

    #[test]
    fn test_tokenize_numbers() {
        let mut lexer = Lexer::new("42 3.14 -5 1.0e10");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0].kind, TokenKind::Integer(42)));
        assert!(matches!(tokens[1].kind, TokenKind::Float(f) if (f - 3.14).abs() < 0.001));
        assert!(matches!(tokens[2].kind, TokenKind::Integer(-5)));
    }

    #[test]
    fn test_tokenize_string() {
        let mut lexer = Lexer::new("'hello world'");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::String(s) if s == "hello world"));
    }

    #[test]
    fn test_tokenize_parameter() {
        let mut lexer = Lexer::new(":query_vector :limit");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].kind, TokenKind::Parameter("query_vector")));
        assert!(matches!(tokens[1].kind, TokenKind::Parameter("limit")));
    }

    #[test]
    fn test_tokenize_operators() {
        let mut lexer = Lexer::new("= != < <= > >= + - * /");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 10);
        assert!(matches!(tokens[0].kind, TokenKind::Eq));
        assert!(matches!(tokens[1].kind, TokenKind::NotEq));
        assert!(matches!(tokens[2].kind, TokenKind::Lt));
        assert!(matches!(tokens[3].kind, TokenKind::LtEq));
    }

    #[test]
    fn test_tokenize_order_by() {
        let mut lexer = Lexer::new("ORDER BY id ASC");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0].kind, TokenKind::OrderBy));
        assert!(matches!(tokens[1].kind, TokenKind::Ident("id")));
        assert!(matches!(tokens[2].kind, TokenKind::Asc));
    }

    #[test]
    fn test_tokenize_complex_query() {
        let mut lexer = Lexer::new(
            "SELECT inv.id, VECTOR_DISTANCE(inv.embedding, :vec) AS score \
             FROM invoices inv \
             WHERE score < 0.3 \
             ORDER BY score ASC \
             LIMIT 10"
        );
        let tokens = lexer.tokenize().unwrap();

        assert!(tokens.len() > 20);
        assert!(matches!(tokens[0].kind, TokenKind::Select));
    }

    #[test]
    fn test_span_tracking() {
        let mut lexer = Lexer::new("SELECT id");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 6);
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[0].span.column, 1);

        assert_eq!(tokens[1].span.start, 7);
        assert_eq!(tokens[1].span.end, 9);
        assert_eq!(tokens[1].span.line, 1);
        assert_eq!(tokens[1].span.column, 8);
    }
}
