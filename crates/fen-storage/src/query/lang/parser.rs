//! Parser for the Fen query language
//!
//! Converts tokens into an abstract syntax tree (AST).

// ParseError is intentionally large to provide rich error context including source code.
// This is acceptable since errors are not on the hot path.
#![allow(clippy::result_large_err)]

use crate::query::lang::ast::{
    AggregateExpr, BinaryOperator, ColumnRef, Expr, FenQuery, FilterExpr, FunctionCall,
    FunctionName, Literal, NullsOrder, OrderByClause, OrderByItem, PipelineOp, QueryTarget,
    SelectItem, SortDirection, UnaryOperator, ZipClause, ZipMode,
};
use crate::query::lang::error::{ParseError, ParseErrorKind};
use crate::query::lang::lexer::{Lexer, Token, TokenKind};
use crate::query::lang::span::Span;

/// Query parser
pub struct QueryParser<'a> {
    source: &'a str,
    tokens: Vec<Token<'a>>,
    pos: usize,
    file_name: String,
}

impl<'a> QueryParser<'a> {
    /// Create a new parser
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            tokens: Vec::new(),
            pos: 0,
            file_name: "<query>".to_string(),
        }
    }

    /// Set the file name for error messages
    pub fn with_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = name.into();
        self
    }

    /// Parse the query
    pub fn parse(&mut self) -> Result<FenQuery, ParseError> {
        // Tokenize
        let mut lexer = Lexer::new(self.source);
        self.tokens = lexer.tokenize()?.to_vec();
        self.pos = 0;

        // Parse SELECT
        self.expect_keyword(TokenKind::Select)?;
        let select = self.parse_select_list()?;

        // Parse FROM
        self.expect_keyword(TokenKind::From)?;
        let (from, alias) = self.parse_from_clause()?;

        // Parse optional ZIP clause
        let zip = if self.check(&TokenKind::Zip)
            || self.check(&TokenKind::Inner)
            || self.check(&TokenKind::Left)
            || self.check(&TokenKind::Cross)
        {
            Some(self.parse_zip_clause()?)
        } else {
            None
        };

        // Parse optional WHERE
        let filter = if self.check(&TokenKind::Where) {
            self.advance();
            Some(FilterExpr::new(self.parse_expression()?))
        } else {
            None
        };

        // Parse optional ORDER BY
        let order_by = if self.check(&TokenKind::OrderBy) {
            self.advance();
            Some(self.parse_order_by()?)
        } else {
            None
        };

        // Parse optional LIMIT
        let limit = if self.check(&TokenKind::Limit) {
            self.advance();
            Some(self.parse_integer()?)
        } else {
            None
        };

        // Parse optional OFFSET
        let offset = if self.check(&TokenKind::Offset) {
            self.advance();
            Some(self.parse_integer()?)
        } else {
            None
        };

        // Parse optional pipeline operations (|> VALIDATE, |> ANALYZE, etc.)
        let pipeline = if self.check(&TokenKind::Pipe) {
            Some(self.parse_pipeline()?)
        } else {
            None
        };

        // Ensure we've consumed all input
        if !self.is_at_end() {
            let token = self.current();
            return Err(self.error(
                ParseErrorKind::UnexpectedToken {
                    expected: vec!["end of query".to_string()],
                    found: token.kind.as_str().to_string(),
                },
                token.span,
            ));
        }

        Ok(FenQuery {
            from,
            from_alias: alias,
            select,
            filter,
            order_by,
            limit,
            offset,
            zip,
            pipeline,
        })
    }

    // ========== Parsing helpers ==========

    fn parse_select_list(&mut self) -> Result<Vec<SelectItem>, ParseError> {
        let mut items = Vec::new();

        loop {
            items.push(self.parse_select_item()?);

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance(); // consume comma
        }

        Ok(items)
    }

    fn parse_select_item(&mut self) -> Result<SelectItem, ParseError> {
        let expr = self.parse_expression()?;

        // Parse optional alias
        let alias = if self.check(&TokenKind::As) {
            self.advance();
            Some(self.parse_identifier()?)
        } else if self.check_ident() && !self.check(&TokenKind::From) {
            // Implicit alias (without AS)
            Some(self.parse_identifier()?)
        } else {
            None
        };

        Ok(SelectItem { expr, alias })
    }

    fn parse_from_clause(&mut self) -> Result<(QueryTarget, Option<String>), ParseError> {
        let table_name = self.parse_identifier()?;

        let target = match table_name.to_lowercase().as_str() {
            "invoices" => QueryTarget::Invoices,
            "contracts" => QueryTarget::Contracts,
            _ => {
                let span = self.previous().span;
                return Err(self
                    .error(ParseErrorKind::UnknownTable(table_name), span)
                    .with_help("valid tables are: invoices, contracts"));
            }
        };

        // Parse optional alias
        let alias = if self.check_ident()
            && !self.check(&TokenKind::Where)
            && !self.check(&TokenKind::OrderBy)
            && !self.check(&TokenKind::Limit)
            && !self.check(&TokenKind::Offset)
            && !self.check(&TokenKind::Zip)
            && !self.check(&TokenKind::Inner)
            && !self.check(&TokenKind::Left)
            && !self.check(&TokenKind::Cross)
        {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        Ok((target, alias))
    }

    fn parse_zip_clause(&mut self) -> Result<ZipClause, ParseError> {
        // Parse optional mode prefix: INNER ZIP, LEFT ZIP, CROSS ZIP
        let mode = if self.check(&TokenKind::Inner) {
            self.advance();
            self.expect_keyword(TokenKind::Zip)?;
            ZipMode::Inner
        } else if self.check(&TokenKind::Left) {
            self.advance();
            self.expect_keyword(TokenKind::Zip)?;
            ZipMode::Left
        } else if self.check(&TokenKind::Cross) {
            self.advance();
            self.expect_keyword(TokenKind::Zip)?;
            ZipMode::Cross
        } else {
            // Just ZIP (defaults to Inner)
            self.expect_keyword(TokenKind::Zip)?;
            ZipMode::Inner
        };

        // Parse table name
        let table_name = self.parse_identifier()?;
        let table = match table_name.to_lowercase().as_str() {
            "invoices" => QueryTarget::Invoices,
            "contracts" => QueryTarget::Contracts,
            _ => {
                let span = self.previous().span;
                return Err(self
                    .error(ParseErrorKind::UnknownTable(table_name), span)
                    .with_help("valid tables are: invoices, contracts"));
            }
        };

        // Parse optional alias for ZIP table
        let alias = if self.check_ident()
            && !self.check(&TokenKind::On)
            && !self.check(&TokenKind::Where)
            && !self.check(&TokenKind::OrderBy)
            && !self.check(&TokenKind::Limit)
            && !self.check(&TokenKind::Offset)
        {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        // Parse ON condition
        self.expect_keyword(TokenKind::On)?;
        let on = self.parse_expression()?;

        Ok(ZipClause {
            table,
            alias,
            on,
            mode,
        })
    }

    /// Parse pipeline operations: |> VALIDATE, |> ANALYZE, etc.
    fn parse_pipeline(&mut self) -> Result<Vec<PipelineOp>, ParseError> {
        let mut ops = Vec::new();

        while self.check(&TokenKind::Pipe) {
            self.advance(); // consume |>
            ops.push(self.parse_pipeline_op()?);
        }

        Ok(ops)
    }

    /// Parse a single pipeline operation
    fn parse_pipeline_op(&mut self) -> Result<PipelineOp, ParseError> {
        let token = self.current();

        match &token.kind {
            TokenKind::Validate => {
                self.advance();
                self.parse_validate_op()
            }
            TokenKind::Analyze => {
                self.advance();
                self.parse_analyze_op()
            }
            TokenKind::CrossValidate => {
                self.advance();
                self.parse_cross_validate_op()
            }
            TokenKind::Aggregate => {
                self.advance();
                self.parse_aggregate_op()
            }
            TokenKind::Graph => {
                self.advance();
                self.parse_graph_op()
            }
            _ => {
                let span = token.span;
                Err(self.error(
                    ParseErrorKind::UnexpectedToken {
                        expected: vec![
                            "VALIDATE".to_string(),
                            "ANALYZE".to_string(),
                            "CROSS_VALIDATE".to_string(),
                            "AGGREGATE".to_string(),
                            "GRAPH".to_string(),
                        ],
                        found: token.kind.as_str().to_string(),
                    },
                    span,
                ))
            }
        }
    }

    /// Parse VALIDATE [WITH [rules...]] [FAIL_FAST]
    fn parse_validate_op(&mut self) -> Result<PipelineOp, ParseError> {
        let mut rules = None;
        let mut fail_fast = false;

        // Parse optional WITH [rules...]
        if self.check(&TokenKind::With) {
            self.advance();
            rules = Some(self.parse_string_list()?);
        }

        // Parse optional FAIL_FAST (check for identifier "FAIL_FAST")
        if self.check_ident() {
            if let TokenKind::Ident(s) = &self.current().kind {
                if s.eq_ignore_ascii_case("FAIL_FAST") {
                    self.advance();
                    fail_fast = true;
                }
            }
        }

        Ok(PipelineOp::Validate { rules, fail_fast })
    }

    /// Parse ANALYZE [BASELINE ...] or ANALYZE [WITH [analyzers...]] [INCLUDE_SCORES]
    fn parse_analyze_op(&mut self) -> Result<PipelineOp, ParseError> {
        // Check for ANALYZE BASELINE syntax
        if self.check(&TokenKind::Baseline) {
            return self.parse_analyze_baseline_op();
        }

        let mut analyzers = None;
        let mut include_scores = false;

        // Parse optional WITH [analyzers...]
        if self.check(&TokenKind::With) {
            self.advance();
            analyzers = Some(self.parse_string_list()?);
        }

        // Parse optional INCLUDE_SCORES
        if self.check_ident() {
            if let TokenKind::Ident(s) = &self.current().kind {
                if s.eq_ignore_ascii_case("INCLUDE_SCORES") {
                    self.advance();
                    include_scores = true;
                }
            }
        }

        Ok(PipelineOp::Analyze {
            analyzers,
            include_scores,
        })
    }

    /// Parse ANALYZE BASELINE [group_by] [WINDOW n DAYS] [THRESHOLD f] [METRICS [...]]
    /// Example: |> ANALYZE BASELINE vendor_name WINDOW 90 DAYS THRESHOLD 2.0 METRICS ['total_amount']
    fn parse_analyze_baseline_op(&mut self) -> Result<PipelineOp, ParseError> {
        self.expect_keyword(TokenKind::Baseline)?;

        // Parse optional group_by field (defaults to "vendor_name")
        let group_by = if self.check_ident()
            && !self.check(&TokenKind::Window)
            && !self.check(&TokenKind::Threshold)
            && !self.check(&TokenKind::Metrics)
        {
            self.parse_identifier()?
        } else {
            "vendor_name".to_string()
        };

        // Parse optional WINDOW n DAYS (defaults to 90)
        let window_days = if self.check(&TokenKind::Window) {
            self.advance();
            let days = self.parse_integer()? as u32;
            // Expect DAYS keyword
            self.expect_keyword(TokenKind::Days)?;
            days
        } else {
            90
        };

        // Parse optional THRESHOLD f (defaults to 2.0)
        let threshold = if self.check(&TokenKind::Threshold) {
            self.advance();
            self.parse_number()?
        } else {
            2.0
        };

        // Parse optional METRICS [...]
        let metrics = if self.check(&TokenKind::Metrics) {
            self.advance();
            self.parse_string_list()?
        } else {
            vec!["total_amount".to_string()]
        };

        Ok(PipelineOp::AnalyzeBaseline {
            group_by,
            window_days,
            threshold,
            metrics,
        })
    }

    /// Parse CROSS_VALIDATE ON (inv.field, con.field), ... [TOLERANCE value]
    fn parse_cross_validate_op(&mut self) -> Result<PipelineOp, ParseError> {
        self.expect_keyword(TokenKind::On)?;

        let mut fields = Vec::new();

        // Parse field pairs
        loop {
            self.expect(&TokenKind::LParen)?;
            let field1 = self.parse_qualified_name()?;
            self.expect(&TokenKind::Comma)?;
            let field2 = self.parse_qualified_name()?;
            self.expect(&TokenKind::RParen)?;

            fields.push((field1, field2));

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance(); // consume comma
        }

        // Parse optional TOLERANCE value
        let mut tolerance = 0.0;
        if self.check_ident() {
            if let TokenKind::Ident(s) = &self.current().kind {
                if s.eq_ignore_ascii_case("TOLERANCE") {
                    self.advance();
                    tolerance = self.parse_float_or_int()?;
                }
            }
        }

        Ok(PipelineOp::CrossValidate { fields, tolerance })
    }

    /// Parse AGGREGATE BY expr, ... INTO agg_func(expr) AS alias, ...
    fn parse_aggregate_op(&mut self) -> Result<PipelineOp, ParseError> {
        // Parse BY clause for group_by
        let group_by = if self.check_ident() {
            if let TokenKind::Ident(s) = &self.current().kind {
                if s.eq_ignore_ascii_case("BY") {
                    self.advance();
                    self.parse_expression_list()?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Parse INTO clause for aggregations
        let aggregations = if self.check_ident() {
            if let TokenKind::Ident(s) = &self.current().kind {
                if s.eq_ignore_ascii_case("INTO") {
                    self.advance();
                    self.parse_aggregation_list()?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Ok(PipelineOp::Aggregate {
            group_by,
            aggregations,
        })
    }

    /// Parse GRAPH TRAVERSE field [DEPTH n] or GRAPH ENRICH
    fn parse_graph_op(&mut self) -> Result<PipelineOp, ParseError> {
        let token = self.current();

        match &token.kind {
            TokenKind::Traverse => {
                self.advance();

                // Parse field name (e.g., vendor_name)
                let field = self.parse_identifier()?;

                // Parse optional DEPTH n (defaults to 1)
                let depth = if self.check_ident() {
                    if let TokenKind::Ident(s) = &self.current().kind {
                        if s.eq_ignore_ascii_case("DEPTH") {
                            self.advance();
                            self.parse_integer()? as u32
                        } else {
                            1
                        }
                    } else {
                        1
                    }
                } else {
                    1
                };

                Ok(PipelineOp::GraphTraverse { field, depth })
            }
            TokenKind::Enrich => {
                self.advance();
                Ok(PipelineOp::GraphEnrich)
            }
            _ => {
                let span = token.span;
                Err(self.error(
                    ParseErrorKind::UnexpectedToken {
                        expected: vec!["TRAVERSE".to_string(), "ENRICH".to_string()],
                        found: token.kind.as_str().to_string(),
                    },
                    span,
                ))
            }
        }
    }

    /// Parse a bracketed list of strings: ['a', 'b', 'c']
    fn parse_string_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut strings = Vec::new();

        // Expect opening bracket [
        let token = self.current();
        if let TokenKind::Ident(s) = &token.kind {
            // Check for [ as an identifier edge case
            if s == &"[" {
                self.advance();
            }
        }

        // Try parsing as square bracket notation
        // For simplicity, support both ['a', 'b'] and ('a', 'b')
        if self.check(&TokenKind::LParen) {
            self.advance();
            while let TokenKind::String(s) = &self.current().kind {
                strings.push(s.clone());
                self.advance();

                if !self.check(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(&TokenKind::RParen)?;
        }

        Ok(strings)
    }

    /// Parse a qualified name like "inv.total_amount"
    fn parse_qualified_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.parse_identifier()?;

        if self.check(&TokenKind::Dot) {
            self.advance();
            let col = self.parse_identifier()?;
            name = format!("{}.{}", name, col);
        }

        Ok(name)
    }

    /// Parse a float or integer as f64
    fn parse_float_or_int(&mut self) -> Result<f64, ParseError> {
        let token = self.current();
        match &token.kind {
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(f)
            }
            TokenKind::Integer(i) => {
                let f = *i as f64;
                self.advance();
                Ok(f)
            }
            _ => {
                let span = token.span;
                Err(self.error(
                    ParseErrorKind::UnexpectedToken {
                        expected: vec!["number".to_string()],
                        found: token.kind.as_str().to_string(),
                    },
                    span,
                ))
            }
        }
    }

    /// Parse a comma-separated list of expressions
    fn parse_expression_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = Vec::new();

        loop {
            exprs.push(self.parse_expression()?);

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();

            // Stop if we hit a keyword
            if self.check_ident() {
                if let TokenKind::Ident(s) = &self.current().kind {
                    if s.eq_ignore_ascii_case("INTO") {
                        break;
                    }
                }
            }
        }

        Ok(exprs)
    }

    /// Parse aggregation expressions: COUNT(*) AS total, SUM(amount) AS sum_amount
    fn parse_aggregation_list(&mut self) -> Result<Vec<AggregateExpr>, ParseError> {
        let mut aggs = Vec::new();

        loop {
            // Parse function call
            let name = self.parse_identifier()?;
            self.expect(&TokenKind::LParen)?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::RParen)?;

            // Parse AS alias
            self.expect_keyword(TokenKind::As)?;
            let alias = self.parse_identifier()?;

            let function = name.parse::<FunctionName>().unwrap();

            aggs.push(AggregateExpr {
                function,
                expr,
                alias,
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        Ok(aggs)
    }

    fn parse_order_by(&mut self) -> Result<OrderByClause, ParseError> {
        let mut items = Vec::new();

        loop {
            let expr = self.parse_expression()?;

            let direction = if self.check(&TokenKind::Desc) {
                self.advance();
                SortDirection::Desc
            } else if self.check(&TokenKind::Asc) {
                self.advance();
                SortDirection::Asc
            } else {
                SortDirection::Asc
            };

            let nulls = if self.check(&TokenKind::Nulls) {
                self.advance();
                if self.check(&TokenKind::First) {
                    self.advance();
                    Some(NullsOrder::First)
                } else if self.check(&TokenKind::Last) {
                    self.advance();
                    Some(NullsOrder::Last)
                } else {
                    let token = self.current();
                    return Err(self.error(
                        ParseErrorKind::UnexpectedToken {
                            expected: vec!["FIRST".to_string(), "LAST".to_string()],
                            found: token.kind.as_str().to_string(),
                        },
                        token.span,
                    ));
                }
            } else {
                None
            };

            items.push(OrderByItem {
                expr,
                direction,
                nulls,
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        Ok(OrderByClause { items })
    }

    // ========== Expression parsing (precedence climbing) ==========

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expression()?;

        while self.check(&TokenKind::Or) {
            self.advance();
            let right = self.parse_and_expression()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not_expression()?;

        while self.check(&TokenKind::And) {
            self.advance();
            let right = self.parse_not_expression()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_not_expression(&mut self) -> Result<Expr, ParseError> {
        if self.check(&TokenKind::Not) {
            self.advance();
            let expr = self.parse_not_expression()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Not,
                expr: Box::new(expr),
            });
        }

        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive()?;

        if let Some(op) = self.match_comparison_op() {
            let right = self.parse_additive()?;
            return Ok(Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            });
        }

        // Handle LIKE / ILIKE
        if self.check(&TokenKind::Like) {
            self.advance();
            let right = self.parse_additive()?;
            return Ok(Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Like,
                right: Box::new(right),
            });
        }

        if self.check(&TokenKind::ILike) {
            self.advance();
            let right = self.parse_additive()?;
            return Ok(Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::ILike,
                right: Box::new(right),
            });
        }

        // Handle IS NULL / IS NOT NULL
        if self.check(&TokenKind::Is) {
            self.advance();
            let negated = if self.check(&TokenKind::Not) {
                self.advance();
                true
            } else {
                false
            };
            self.expect_keyword(TokenKind::Null)?;

            let null_check = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Literal::Null)),
            };

            return if negated {
                Ok(Expr::UnaryOp {
                    op: UnaryOperator::Not,
                    expr: Box::new(null_check),
                })
            } else {
                Ok(null_check)
            };
        }

        Ok(left)
    }

    fn match_comparison_op(&mut self) -> Option<BinaryOperator> {
        let op = match self.current().kind {
            TokenKind::Eq => BinaryOperator::Eq,
            TokenKind::NotEq => BinaryOperator::NotEq,
            TokenKind::Lt => BinaryOperator::Lt,
            TokenKind::LtEq => BinaryOperator::LtEq,
            TokenKind::Gt => BinaryOperator::Gt,
            TokenKind::GtEq => BinaryOperator::GtEq,
            _ => return None,
        };
        self.advance();
        Some(op)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.current().kind {
                TokenKind::Plus => BinaryOperator::Add,
                TokenKind::Minus => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.current().kind {
                TokenKind::Star => BinaryOperator::Multiply,
                TokenKind::Slash => BinaryOperator::Divide,
                TokenKind::Percent => BinaryOperator::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.check(&TokenKind::Minus) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Minus,
                expr: Box::new(expr),
            });
        }

        if self.check(&TokenKind::Plus) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOperator::Plus,
                expr: Box::new(expr),
            });
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.current();

        match &token.kind {
            // Parenthesized expression
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RParen)?;
                Ok(expr)
            }

            // Literals
            TokenKind::Integer(i) => {
                let i = *i;
                self.advance();
                Ok(Expr::Literal(Literal::Integer(i)))
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(Expr::Literal(Literal::Float(f)))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal(Literal::String(s)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal(Literal::Boolean(true)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal(Literal::Boolean(false)))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Literal(Literal::Null))
            }

            // Parameter
            TokenKind::Parameter(name) => {
                let name = name.to_string();
                self.advance();
                Ok(Expr::Parameter(name))
            }

            // Wildcard
            TokenKind::Star => {
                self.advance();
                Ok(Expr::Wildcard)
            }

            // Identifier (column, function, or qualified wildcard)
            TokenKind::Ident(_) => {
                let name = self.parse_identifier()?;

                // Check for function call
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let args = self.parse_function_args()?;
                    self.expect(&TokenKind::RParen)?;

                    let func_name = name.parse::<FunctionName>().unwrap();
                    return Ok(Expr::Function(FunctionCall {
                        name: func_name,
                        args,
                    }));
                }

                // Check for qualified reference (table.column or table.*)
                if self.check(&TokenKind::Dot) {
                    self.advance();

                    if self.check(&TokenKind::Star) {
                        self.advance();
                        return Ok(Expr::QualifiedWildcard(name));
                    }

                    let column = self.parse_identifier()?;
                    return Ok(Expr::Column(ColumnRef::qualified(name, column)));
                }

                // Simple column reference
                Ok(Expr::Column(ColumnRef::new(name)))
            }

            _ => {
                let span = token.span;
                Err(self.error(ParseErrorKind::ExpectedExpression, span))
            }
        }
    }

    fn parse_function_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        if self.check(&TokenKind::RParen) {
            return Ok(Vec::new());
        }

        let mut args = Vec::new();

        loop {
            args.push(self.parse_expression()?);

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        Ok(args)
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        let token = self.current();
        match &token.kind {
            TokenKind::Ident(s) => {
                let s = s.to_string();
                self.advance();
                Ok(s)
            }
            _ => {
                let span = token.span;
                Err(self.error(ParseErrorKind::ExpectedIdentifier, span))
            }
        }
    }

    fn parse_integer(&mut self) -> Result<u64, ParseError> {
        let token = self.current();
        match &token.kind {
            TokenKind::Integer(i) if *i >= 0 => {
                let i = *i as u64;
                self.advance();
                Ok(i)
            }
            _ => {
                let span = token.span;
                Err(self.error(
                    ParseErrorKind::UnexpectedToken {
                        expected: vec!["positive integer".to_string()],
                        found: token.kind.as_str().to_string(),
                    },
                    span,
                ))
            }
        }
    }

    fn parse_number(&mut self) -> Result<f64, ParseError> {
        let token = self.current();
        match &token.kind {
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(f)
            }
            TokenKind::Integer(i) => {
                let f = *i as f64;
                self.advance();
                Ok(f)
            }
            _ => {
                let span = token.span;
                Err(self.error(
                    ParseErrorKind::UnexpectedToken {
                        expected: vec!["number".to_string()],
                        found: token.kind.as_str().to_string(),
                    },
                    span,
                ))
            }
        }
    }

    // ========== Token navigation ==========

    fn current(&self) -> &Token<'a> {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            // Return a synthetic EOF token
            static EOF_TOKEN: Token<'static> = Token {
                kind: TokenKind::Eof,
                span: Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            };
            &EOF_TOKEN
        })
    }

    fn previous(&self) -> &Token<'a> {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .unwrap_or_else(|| {
                static EOF_TOKEN: Token<'static> = Token {
                    kind: TokenKind::Eof,
                    span: Span {
                        start: 0,
                        end: 0,
                        line: 1,
                        column: 1,
                    },
                };
                &EOF_TOKEN
            })
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn check_ident(&self) -> bool {
        if self.is_at_end() {
            return false;
        }
        matches!(self.current().kind, TokenKind::Ident(_))
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            let token = self.current();
            Err(self.error(
                ParseErrorKind::UnexpectedToken {
                    expected: vec![kind.as_str().to_string()],
                    found: token.kind.as_str().to_string(),
                },
                token.span,
            ))
        }
    }

    fn expect_keyword(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if self.check(&kind) {
            self.advance();
            Ok(())
        } else {
            let token = self.current();
            Err(self.error(
                ParseErrorKind::ExpectedKeyword(kind.as_str().to_string()),
                token.span,
            ))
        }
    }

    // ========== Error construction ==========

    fn error(&self, kind: ParseErrorKind, span: Span) -> ParseError {
        let mut err =
            ParseError::new(kind.clone(), span, self.source).with_file_name(&self.file_name);

        // Add automatic help based on error kind
        if let Some(help) = kind.help() {
            err = err.with_help(help);
        }

        err
    }
}

/// Parse a query string
pub fn parse_query(source: &str) -> Result<FenQuery, ParseError> {
    QueryParser::new(source).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let query = parse_query("SELECT id FROM invoices").unwrap();

        assert_eq!(query.from, QueryTarget::Invoices);
        assert_eq!(query.select.len(), 1);
        assert!(query.filter.is_none());
    }

    #[test]
    fn test_parse_select_with_alias() {
        let query = parse_query("SELECT id, invoice_number AS num FROM invoices inv").unwrap();

        assert_eq!(query.from, QueryTarget::Invoices);
        assert_eq!(query.from_alias, Some("inv".to_string()));
        assert_eq!(query.select.len(), 2);
        assert_eq!(query.select[1].alias, Some("num".to_string()));
    }

    #[test]
    fn test_parse_wildcard() {
        let query = parse_query("SELECT * FROM invoices").unwrap();

        assert_eq!(query.select.len(), 1);
        assert!(matches!(query.select[0].expr, Expr::Wildcard));
    }

    #[test]
    fn test_parse_qualified_column() {
        let query = parse_query("SELECT inv.id FROM invoices inv").unwrap();

        assert_eq!(query.select.len(), 1);
        if let Expr::Column(col) = &query.select[0].expr {
            assert_eq!(col.table, Some("inv".to_string()));
            assert_eq!(col.column, "id");
        } else {
            panic!("Expected column expression");
        }
    }

    #[test]
    fn test_parse_function_call() {
        let query =
            parse_query("SELECT VECTOR_DISTANCE(embedding, :vec) AS score FROM invoices").unwrap();

        assert_eq!(query.select.len(), 1);
        if let Expr::Function(func) = &query.select[0].expr {
            assert_eq!(func.name, FunctionName::VectorDistance);
            assert_eq!(func.args.len(), 2);
        } else {
            panic!("Expected function expression");
        }
    }

    #[test]
    fn test_parse_where_clause() {
        let query = parse_query("SELECT id FROM invoices WHERE total_amount > 100").unwrap();

        assert!(query.filter.is_some());
    }

    #[test]
    fn test_parse_complex_where() {
        let query = parse_query(
            "SELECT id FROM invoices \
             WHERE vendor_name = 'Acme' AND total_amount > 100 OR status = 'pending'",
        )
        .unwrap();

        assert!(query.filter.is_some());
    }

    #[test]
    fn test_parse_order_by() {
        let query =
            parse_query("SELECT id FROM invoices ORDER BY invoice_date DESC, id ASC").unwrap();

        let order = query.order_by.unwrap();
        assert_eq!(order.items.len(), 2);
        assert_eq!(order.items[0].direction, SortDirection::Desc);
        assert_eq!(order.items[1].direction, SortDirection::Asc);
    }

    #[test]
    fn test_parse_limit_offset() {
        let query = parse_query("SELECT id FROM invoices LIMIT 10 OFFSET 20").unwrap();

        assert_eq!(query.limit, Some(10));
        assert_eq!(query.offset, Some(20));
    }

    #[test]
    fn test_parse_arithmetic_in_order_by() {
        let query = parse_query(
            "SELECT id, VECTOR_DISTANCE(embedding, :vec) AS v_score, \
                    BM25_SCORE(text, :terms) AS t_score \
             FROM invoices \
             ORDER BY 0.7 * v_score + 0.3 * t_score ASC",
        )
        .unwrap();

        let order = query.order_by.unwrap();
        assert_eq!(order.items.len(), 1);
        // The expression should be a binary op (addition of two multiplications)
        assert!(matches!(order.items[0].expr, Expr::BinaryOp { .. }));
    }

    #[test]
    fn test_parse_complete_hybrid_query() {
        let query_str = r#"
            SELECT inv.id, inv.invoice_number,
                VECTOR_DISTANCE(inv.embedding, :query_vector) AS semantic_score,
                BM25_SCORE(inv.extracted_text, :search_terms) AS text_score
            FROM invoices inv
            WHERE inv.vendor_name = 'Acme Corp'
                AND VECTOR_DISTANCE(inv.embedding, :query_vector) < 0.3
                AND CONTAINS(inv.extracted_text, :search_terms)
            ORDER BY 0.7 * semantic_score + 0.3 * text_score ASC
            LIMIT 10
        "#;

        let query = parse_query(query_str).unwrap();

        assert_eq!(query.from, QueryTarget::Invoices);
        assert_eq!(query.from_alias, Some("inv".to_string()));
        assert_eq!(query.select.len(), 4);
        assert!(query.filter.is_some());
        assert!(query.order_by.is_some());
        assert_eq!(query.limit, Some(10));

        // Verify uses vector search
        assert!(query.uses_vector_search());
        assert!(query.uses_text_search());
        assert!(query.uses_contains());

        // Verify parameters
        let params = query.parameter_names();
        assert!(params.contains(&"query_vector".to_string()));
        assert!(params.contains(&"search_terms".to_string()));
    }

    #[test]
    fn test_error_unknown_table() {
        let result = parse_query("SELECT id FROM unknown_table");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, ParseErrorKind::UnknownTable(_)));
    }

    #[test]
    fn test_error_missing_from() {
        let result = parse_query("SELECT id WHERE x = 1");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, ParseErrorKind::ExpectedKeyword(_)));
    }

    #[test]
    fn test_error_report_formatting() {
        let result = parse_query("SELECT id FROM unknown_table");
        let err = result.unwrap_err();

        let report = err.report();
        // The report should contain the error location
        assert!(report.contains("unknown_table"));
    }

    // ========== ZIP clause tests ==========

    #[test]
    fn test_parse_zip_basic() {
        let query = parse_query(
            "SELECT inv.*, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        assert_eq!(query.from, QueryTarget::Invoices);
        assert_eq!(query.from_alias, Some("inv".to_string()));

        let zip = query.zip.unwrap();
        assert_eq!(zip.table, QueryTarget::Contracts);
        assert_eq!(zip.alias, Some("con".to_string()));
        assert_eq!(zip.mode, ZipMode::Inner);
    }

    #[test]
    fn test_parse_zip_with_where() {
        let query = parse_query(
            "SELECT inv.id, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000",
        )
        .unwrap();

        assert!(query.zip.is_some());
        assert!(query.filter.is_some());

        let zip = query.zip.unwrap();
        assert_eq!(zip.table, QueryTarget::Contracts);
    }

    #[test]
    fn test_parse_inner_zip() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             INNER ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        let zip = query.zip.unwrap();
        assert_eq!(zip.mode, ZipMode::Inner);
    }

    #[test]
    fn test_parse_left_zip() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             LEFT ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        let zip = query.zip.unwrap();
        assert_eq!(zip.mode, ZipMode::Left);
    }

    #[test]
    fn test_parse_cross_zip() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             CROSS ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        let zip = query.zip.unwrap();
        assert_eq!(zip.mode, ZipMode::Cross);
    }

    #[test]
    fn test_parse_zip_without_alias() {
        let query = parse_query(
            "SELECT * FROM invoices \
             ZIP contracts ON vendor_name = party_name",
        )
        .unwrap();

        assert_eq!(query.from_alias, None);

        let zip = query.zip.unwrap();
        assert_eq!(zip.table, QueryTarget::Contracts);
        assert_eq!(zip.alias, None);
    }

    #[test]
    fn test_parse_zip_complex_on_condition() {
        let query = parse_query(
            "SELECT inv.id, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name AND inv.currency = con.currency \
             WHERE inv.total_amount > 1000 \
             ORDER BY inv.invoice_date DESC \
             LIMIT 50",
        )
        .unwrap();

        let zip = query.zip.unwrap();
        assert_eq!(zip.table, QueryTarget::Contracts);
        assert!(query.filter.is_some());
        assert!(query.order_by.is_some());
        assert_eq!(query.limit, Some(50));
    }

    #[test]
    fn test_is_zip_query() {
        let regular = parse_query("SELECT id FROM invoices").unwrap();
        assert!(!regular.is_zip_query());

        let zip_query =
            parse_query("SELECT * FROM invoices ZIP contracts ON vendor_name = party_name")
                .unwrap();
        assert!(zip_query.is_zip_query());
    }

    #[test]
    fn test_error_zip_unknown_table() {
        let result =
            parse_query("SELECT * FROM invoices ZIP unknown_table ON vendor_name = party_name");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, ParseErrorKind::UnknownTable(_)));
    }

    #[test]
    fn test_error_zip_missing_on() {
        let result =
            parse_query("SELECT * FROM invoices ZIP contracts WHERE vendor_name = party_name");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err.kind, ParseErrorKind::ExpectedKeyword(_)));
    }

    // ========== Pipeline (|>) tests ==========

    #[test]
    fn test_parse_pipe_validate() {
        let query = parse_query("SELECT * FROM invoices |> VALIDATE").unwrap();

        assert!(query.has_pipeline());
        let pipeline = query.pipeline.unwrap();
        assert_eq!(pipeline.len(), 1);
        assert!(matches!(pipeline[0], PipelineOp::Validate { .. }));
    }

    #[test]
    fn test_parse_pipe_validate_with_rules() {
        let query =
            parse_query("SELECT * FROM invoices |> VALIDATE WITH ('math_check', 'date_check')")
                .unwrap();

        assert!(query.has_pipeline());
        let pipeline = query.pipeline.unwrap();
        assert_eq!(pipeline.len(), 1);

        if let PipelineOp::Validate { rules, fail_fast } = &pipeline[0] {
            assert!(rules.is_some());
            assert_eq!(rules.as_ref().unwrap().len(), 2);
            assert!(!*fail_fast);
        } else {
            panic!("Expected Validate operation");
        }
    }

    #[test]
    fn test_parse_pipe_validate_fail_fast() {
        let query = parse_query("SELECT * FROM invoices |> VALIDATE FAIL_FAST").unwrap();

        let pipeline = query.pipeline.unwrap();
        if let PipelineOp::Validate { rules, fail_fast } = &pipeline[0] {
            assert!(rules.is_none());
            assert!(*fail_fast);
        } else {
            panic!("Expected Validate operation");
        }
    }

    #[test]
    fn test_parse_pipe_analyze() {
        let query = parse_query("SELECT * FROM invoices |> ANALYZE").unwrap();

        let pipeline = query.pipeline.unwrap();
        assert!(matches!(pipeline[0], PipelineOp::Analyze { .. }));
    }

    #[test]
    fn test_parse_pipe_analyze_with_options() {
        let query = parse_query(
            "SELECT * FROM invoices |> ANALYZE WITH ('anomaly_detector') INCLUDE_SCORES",
        )
        .unwrap();

        let pipeline = query.pipeline.unwrap();
        if let PipelineOp::Analyze {
            analyzers,
            include_scores,
        } = &pipeline[0]
        {
            assert!(analyzers.is_some());
            assert!(*include_scores);
        } else {
            panic!("Expected Analyze operation");
        }
    }

    #[test]
    fn test_parse_pipe_chain() {
        let query = parse_query(
            "SELECT * FROM invoices WHERE total_amount > 1000 \
             |> VALIDATE WITH ('math_check') \
             |> ANALYZE",
        )
        .unwrap();

        let pipeline = query.pipeline.unwrap();
        assert_eq!(pipeline.len(), 2);
        assert!(matches!(pipeline[0], PipelineOp::Validate { .. }));
        assert!(matches!(pipeline[1], PipelineOp::Analyze { .. }));
    }

    #[test]
    fn test_parse_pipe_cross_validate() {
        let query = parse_query(
            "SELECT inv.*, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             |> CROSS_VALIDATE ON (inv.total_amount, con.total_value)",
        )
        .unwrap();

        let pipeline = query.pipeline.unwrap();
        assert_eq!(pipeline.len(), 1);

        if let PipelineOp::CrossValidate { fields, tolerance } = &pipeline[0] {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "inv.total_amount");
            assert_eq!(fields[0].1, "con.total_value");
            assert!(*tolerance == 0.0);
        } else {
            panic!("Expected CrossValidate operation");
        }
    }

    #[test]
    fn test_parse_pipe_cross_validate_tolerance() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             |> CROSS_VALIDATE ON (inv.total_amount, con.total_value) TOLERANCE 0.05",
        )
        .unwrap();

        let pipeline = query.pipeline.unwrap();
        if let PipelineOp::CrossValidate { fields, tolerance } = &pipeline[0] {
            assert_eq!(fields.len(), 1);
            assert!((*tolerance - 0.05).abs() < 0.001);
        } else {
            panic!("Expected CrossValidate operation");
        }
    }

    #[test]
    fn test_parse_pipe_aggregate() {
        let query = parse_query(
            "SELECT * FROM invoices \
             |> AGGREGATE BY vendor_name INTO SUM(total_amount) AS total",
        )
        .unwrap();

        let pipeline = query.pipeline.unwrap();
        if let PipelineOp::Aggregate {
            group_by,
            aggregations,
        } = &pipeline[0]
        {
            assert_eq!(group_by.len(), 1);
            assert_eq!(aggregations.len(), 1);
            assert_eq!(aggregations[0].alias, "total");
        } else {
            panic!("Expected Aggregate operation");
        }
    }

    #[test]
    fn test_parse_pipe_complete_flow() {
        let query = parse_query(
            "SELECT inv.invoice_number, inv.total_amount, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000 \
             ORDER BY inv.total_amount DESC \
             LIMIT 100 \
             |> VALIDATE WITH ('math_check', 'date_check') \
             |> CROSS_VALIDATE ON (inv.total_amount, con.total_value) TOLERANCE 0.01 \
             |> ANALYZE",
        )
        .unwrap();

        assert!(query.is_zip_query());
        assert!(query.filter.is_some());
        assert!(query.order_by.is_some());
        assert_eq!(query.limit, Some(100));

        let pipeline = query.pipeline.unwrap();
        assert_eq!(pipeline.len(), 3);
        assert!(matches!(pipeline[0], PipelineOp::Validate { .. }));
        assert!(matches!(pipeline[1], PipelineOp::CrossValidate { .. }));
        assert!(matches!(pipeline[2], PipelineOp::Analyze { .. }));
    }

    #[test]
    fn test_no_pipeline() {
        let query = parse_query("SELECT * FROM invoices").unwrap();
        assert!(!query.has_pipeline());
        assert!(query.pipeline.is_none());
    }
}
