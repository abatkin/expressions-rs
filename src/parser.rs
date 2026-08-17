use crate::types::error::{Error, Result};
use crate::types::expression::{BinaryOp, Expr, UnaryOp};
use crate::types::primitive::Primitive;
use pest::error::{Error as PestError, ErrorVariant, InputLocation, LineColLocation};
use pest::iterators::Pair;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest::{Parser, Position, Span};

#[derive(pest_derive::Parser)]
#[grammar = "expr.pest"]
struct InnerParser;

/// Where the fragment being parsed sits inside the text the user actually wrote.
///
/// An interpolated expression is parsed from a slice starting mid-string, so pest
/// reports positions relative to that slice. Carrying the whole input alongside
/// the slice's byte offset lets errors be re-anchored onto what the user typed.
#[derive(Clone, Copy)]
pub(crate) struct Origin<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> Origin<'a> {
    /// The fragment is the whole input.
    pub(crate) fn whole(input: &'a str) -> Self {
        Self { input, offset: 0 }
    }

    /// The fragment begins at byte `offset` within `input`.
    pub(crate) fn fragment(input: &'a str, offset: usize) -> Self {
        Self { input, offset }
    }

    /// The fragment itself -- what gets handed to pest.
    fn text(&self) -> &'a str {
        &self.input[self.offset..]
    }

    /// Re-anchor a pest parse failure onto the whole input.
    fn convert(&self, err: PestError<Rule>) -> Error {
        match err.location {
            InputLocation::Pos(p) => self.build(err.variant, p, None),
            InputLocation::Span((s, e)) => self.build(err.variant, s, Some(e)),
        }
    }

    /// A parse failure we detect ourselves rather than one pest reports, covering
    /// the whole of `pair`.
    fn at_pair(&self, pair: &Pair<Rule>, message: String) -> Error {
        let span = pair.as_span();
        self.build(ErrorVariant::CustomError { message }, span.start(), Some(span.end()))
    }

    /// As [`Origin::at_pair`], for a single position within the fragment.
    fn at(&self, start: usize, message: String) -> Error {
        self.build(ErrorVariant::CustomError { message }, start, None)
    }

    /// Positions arrive relative to the fragment; shift them and rebuild the error
    /// through pest against the whole input. Shifting the reported line and column
    /// directly would not work -- the fragment's line 1 is not the input's line 1,
    /// and the source line pest renders in its caret diagram is the fragment's
    /// rather than the user's. Re-running pest's own machinery fixes all three at
    /// once, and keeps the diagram consistent with the numbers beside it.
    fn build(&self, variant: ErrorVariant<Rule>, start: usize, end: Option<usize>) -> Error {
        // A String, not the Rule: the grammar stays an implementation detail, so
        // renaming a rule is not a breaking change for callers.
        let message = variant.message().into_owned();
        let offset = self.offset + start;

        let rebased = match end {
            Some(end) => Span::new(self.input, offset, self.offset + end).map(|span| PestError::new_from_span(variant, span)),
            None => Position::new(self.input, offset).map(|pos| PestError::new_from_pos(variant, pos)),
        };

        match rebased {
            Some(err) => {
                let (line, column) = match err.line_col {
                    LineColLocation::Pos(pos) => pos,
                    LineColLocation::Span(start, _) => start,
                };
                Error::ParseError {
                    line,
                    column,
                    offset,
                    message,
                    rendered: err.to_string(),
                }
            }
            // Unreachable for offsets pest handed us, but guessing a position would
            // be worse than computing the one thing we can still be sure of.
            None => {
                let (line, column) = line_col(self.input, offset);
                Error::ParseError {
                    line,
                    column,
                    offset,
                    message: message.clone(),
                    rendered: message,
                }
            }
        }
    }
}

/// 1-based line and character column of `offset` within `text`.
///
/// Counts `\n` only, which matches both pest and the grammar's `NEWLINE`.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (before.matches('\n').count() + 1, before[line_start..].chars().count() + 1)
}

pub fn parse_expression(input: &str) -> Result<Expr> {
    parse_internal(Origin::whole(input), Rule::program).map(|r| r.0)
}

/// Returns the expression and how far into [`Origin::text`] the parse consumed.
pub(crate) fn parse_internal(origin: Origin, rule: Rule) -> Result<(Expr, usize)> {
    let mut pairs = InnerParser::parse(rule, origin.text()).map_err(|e| origin.convert(e))?;
    let pair = pairs.next().expect("program always produces one pair");

    debug_assert_eq!(pair.as_rule(), rule);
    let end_pos = pair.as_span().end_pos().pos();
    let expr_pair = pair.into_inner().next().expect("program contains expr");
    let expr = parse_expr(expr_pair, origin)?;
    Ok((expr, end_pos))
}

fn pratt() -> PrattParser<Rule> {
    PrattParser::new()
        .op(Op::infix(Rule::op_or, Assoc::Left))
        .op(Op::infix(Rule::op_and, Assoc::Left))
        .op(Op::infix(Rule::op_eq, Assoc::Left))
        .op(Op::infix(Rule::op_cmp, Assoc::Left))
        .op(Op::infix(Rule::op_add, Assoc::Left))
        .op(Op::infix(Rule::op_mul, Assoc::Left))
        .op(Op::infix(Rule::op_pow, Assoc::Right))
}

fn parse_expr(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    match pair.as_rule() {
        Rule::expr => {
            let pairs = pair.into_inner();
            pratt()
                .map_primary(|p: Pair<Rule>| parse_unary(p, origin))
                .map_infix(|lhs: Result<Expr>, op: Pair<Rule>, rhs: Result<Expr>| {
                    let left = lhs?;
                    let right = rhs?;
                    let mut l = left;
                    let mut r = right;
                    let bop = match op.as_rule() {
                        Rule::op_or => BinaryOp::Or,
                        Rule::op_and => BinaryOp::And,
                        Rule::op_eq => {
                            let s = op.as_str();
                            if s.contains("==") { BinaryOp::Eq } else { BinaryOp::Ne }
                        }
                        Rule::op_cmp => {
                            let s = op.as_str();
                            if s.contains("<=") {
                                // a <= b  ==>  b >= a
                                std::mem::swap(&mut l, &mut r);
                                BinaryOp::Ge
                            } else if s.contains(">=") {
                                BinaryOp::Ge
                            } else if s.contains('<') {
                                BinaryOp::Lt
                            } else {
                                BinaryOp::Gt
                            }
                        }
                        Rule::op_add => {
                            if op.as_str().contains('-') {
                                BinaryOp::Sub
                            } else {
                                BinaryOp::Add
                            }
                        }
                        Rule::op_mul => {
                            let s = op.as_str();
                            if s.contains('*') {
                                BinaryOp::Mul
                            } else if s.contains('/') {
                                BinaryOp::Div
                            } else {
                                BinaryOp::Mod
                            }
                        }
                        Rule::op_pow => BinaryOp::Pow,
                        r => {
                            return Err(Error::InternalParserError(format!("unexpected infix op: {:?}", r)));
                        }
                    };
                    Ok(Expr::Binary {
                        left: Box::new(l),
                        op: bop,
                        right: Box::new(r),
                    })
                })
                .parse(pairs)
        }
        _ => Err(Error::InternalParserError(format!("expected expr, got: {:?}", pair))),
    }
}

fn parse_unary(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    match pair.as_rule() {
        Rule::unary => {
            let mut ops: Vec<UnaryOp> = Vec::new();
            let mut inner = pair.into_inner();
            // Collect zero or more unary_op then the postfix expression
            while let Some(next) = inner.peek() {
                if !matches!(next.as_rule(), Rule::unary_op) {
                    break;
                }
                let op_pair = inner.next().unwrap();
                let op_inner = op_pair.into_inner().next().unwrap();
                let op = match op_inner.as_rule() {
                    Rule::not_op => UnaryOp::Not,
                    Rule::neg_op => UnaryOp::Neg,
                    r => {
                        return Err(Error::InternalParserError(format!("unexpected unary op: {:?}", r)));
                    }
                };
                ops.push(op);
            }
            let post = inner.next().expect("unary must end with postfix");
            let mut expr = parse_postfix(post, origin)?;
            for op in ops.into_iter().rev() {
                expr = Expr::Unary { op, expr: Box::new(expr) };
            }
            Ok(expr)
        }
        _ => parse_postfix(pair, origin),
    }
}

fn parse_postfix(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    match pair.as_rule() {
        Rule::postfix => {
            let mut inner = pair.into_inner();
            let first = inner.next().expect("postfix starts with primary");
            let mut expr = parse_primary(first, origin)?;
            for next in inner {
                match next.as_rule() {
                    Rule::call => {
                        let args = parse_call_args(next, origin)?;
                        expr = Expr::Call { callee: Box::new(expr), args };
                    }
                    Rule::index => {
                        let idx_pair = next.into_inner().next().expect("index inner expr");
                        let index_expr = parse_expr(idx_pair, origin)?;
                        expr = Expr::Index {
                            object: Box::new(expr),
                            index: Box::new(index_expr),
                        };
                    }
                    Rule::property => {
                        let name = next.into_inner().next().expect("property ident").as_str().to_string();
                        expr = Expr::Member { object: Box::new(expr), field: name };
                    }
                    r => {
                        return Err(Error::InternalParserError(format!("unexpected postfix op: {:?}", r)));
                    }
                }
            }
            Ok(expr)
        }
        _ => parse_primary(pair, origin),
    }
}

fn parse_call_args(pair: Pair<Rule>, origin: Origin) -> Result<Vec<Expr>> {
    debug_assert_eq!(pair.as_rule(), Rule::call);
    let mut args = Vec::new();
    for p in pair.into_inner() {
        // call contains expr separated by commas -> grammar emits only expr pairs inside
        if matches!(p.as_rule(), Rule::expr) {
            args.push(parse_expr(p, origin)?);
        }
    }
    Ok(args)
}

fn parse_primary(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    match pair.as_rule() {
        Rule::primary => parse_primary(pair.into_inner().next().unwrap(), origin),
        Rule::parens => parse_expr(pair.into_inner().next().unwrap(), origin),
        Rule::ident => Ok(Expr::Var(pair.as_str().to_string())),
        Rule::number => parse_number(pair, origin),
        Rule::boolean => {
            let inner = pair.into_inner().next().unwrap();
            let val = matches!(inner.as_rule(), Rule::true_kw);
            Ok(Expr::Literal(Primitive::Bool(val)))
        }
        Rule::string => {
            let s = unescape_string(&pair, origin)?;
            Ok(Expr::Literal(Primitive::Str(s)))
        }
        Rule::list => parse_list(pair, origin),
        Rule::dict => parse_dict(pair, origin),
        r => Err(Error::InternalParserError(format!("unexpected primary op: {:?}", r))),
    }
}

fn parse_number(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::int => {
            let s = inner.as_str();
            let v: i64 = s.parse().map_err(|_| origin.at_pair(&inner, format!("integer literal out of range for a 64-bit signed integer: {}", s)))?;
            Ok(Expr::Literal(Primitive::Int(v)))
        }
        Rule::float => {
            let s = inner.as_str();
            let v: f64 = s.parse().map_err(|_| origin.at_pair(&inner, format!("invalid float literal: {}", s)))?;
            Ok(Expr::Literal(Primitive::Float(v)))
        }
        r => Err(Error::InternalParserError(format!("unexpected number: {:?}", r))),
    }
}

fn parse_list(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    let mut elems = Vec::new();
    for p in pair.into_inner() {
        if let Rule::expr = p.as_rule() {
            elems.push(parse_expr(p, origin)?);
        }
    }
    Ok(Expr::ListLiteral(elems))
}

fn parse_dict(pair: Pair<Rule>, origin: Origin) -> Result<Expr> {
    let mut items = Vec::new();
    for p in pair.into_inner() {
        if let Rule::pair = p.as_rule() {
            let mut it = p.into_inner();
            let key_pair = it.next().expect("pair key expr");
            let key = parse_expr(key_pair, origin)?;
            let value_pair = it.next().expect("pair value expr");
            let value = parse_expr(value_pair, origin)?;
            items.push((key, value));
        }
    }
    Ok(Expr::DictLiteral(items))
}

fn unescape_string(pair: &Pair<Rule>, origin: Origin) -> Result<String> {
    // The grammar guarantees matching single-byte quotes around the contents.
    let src = pair.as_str();
    let escape_char = src.chars().next().unwrap();
    let inner = &src[1..src.len() - 1];
    // Offsets from `char_indices` are into `inner`, so shift past the open quote to
    // point at the backslash in the fragment.
    let inner_start = pair.as_span().start() + 1;

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, next)) if next == escape_char => out.push(escape_char),
                next => {
                    let message = match next {
                        Some((_, next)) => format!("unknown escape sequence: \\{}", next),
                        None => "string literal ends in a trailing backslash".to_string(),
                    };
                    return Err(origin.at(inner_start + i, message));
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolated_expr() {
        let input = "123}x";
        let (expr, idx) = parse_internal(Origin::whole(input), Rule::delimited_expr).unwrap();
        assert_eq!(expr, Expr::Literal(Primitive::Int(123)));
        assert_eq!(idx, 4);
    }

    /// `(line, column, offset, message)` of the parse error for `input`.
    fn parse_failure(input: &str) -> (usize, usize, usize, String) {
        match parse_expression(input).unwrap_err() {
            Error::ParseError { line, column, offset, message, .. } => (line, column, offset, message),
            other => panic!("expected a parse error, got: {:?}", other),
        }
    }

    #[test]
    fn parse_errors_carry_a_position() {
        let (line, column, offset, message) = parse_failure("1 + + ");
        assert_eq!((line, column, offset), (1, 5, 4));
        assert_eq!(message, "expected unary");
    }

    /// The type renders its own prefix; the payload must not repeat it.
    #[test]
    fn parse_error_message_is_not_doubly_prefixed() {
        let rendered = parse_expression("1 + + ").unwrap_err().to_string();
        assert_eq!(rendered.matches("parse error").count(), 1);
    }

    /// The caret diagram is the useful part of a pest error, so it survives.
    #[test]
    fn parse_errors_keep_the_caret_diagram() {
        let Error::ParseError { rendered, .. } = parse_expression("1 + + ").unwrap_err() else {
            panic!("expected a parse error");
        };
        assert!(rendered.contains("1 | 1 + + "), "{}", rendered);
        assert!(rendered.contains('^'), "{}", rendered);
    }

    #[test]
    fn positions_count_lines_and_are_columns_within_them() {
        let (line, column, offset, _) = parse_failure("1 +\n2 * * 3");
        assert_eq!((line, column, offset), (2, 5, 8));
    }

    /// The grammar's `NEWLINE` is deliberately narrower than pest's builtin: it
    /// omits the lone `\r`, which pest's position reporting does not count as a
    /// line break. Accepting one would put the reported line out of step with the
    /// line the parser is actually on.
    #[test]
    fn a_lone_carriage_return_is_not_whitespace() {
        let (line, column, offset, _) = parse_failure("1 +\r2");
        assert_eq!((line, column, offset), (1, 4, 3));

        // And so does not end a comment, which runs to the end of the input.
        assert!(parse_expression("1 + // c\r2").is_err());
        assert!(parse_expression("1 + // c\r\n2").is_ok());
        assert!(parse_expression("1 + // c\n2").is_ok());
    }

    /// Narrowing `NEWLINE` must not change what a string literal holds. A raw line
    /// break inside one is preserved either way: `string` is not an atomic rule, so
    /// before the change `WHITESPACE` skipped it and now `string_char` matches it,
    /// and `unescape_string` reads the raw span regardless.
    #[test]
    fn raw_line_breaks_in_string_literals_are_preserved() {
        for (input, expected) in [("'a\rb'", "a\rb"), ("'a\nb'", "a\nb")] {
            let expr = parse_expression(input).unwrap_or_else(|e| panic!("{:?}: {}", input, e));
            assert_eq!(expr, Expr::Literal(Primitive::Str(expected.to_string())), "input: {:?}", input);
        }
    }

    /// Both accepted terminators have to count, and both have to leave the reported
    /// offset slicing the caller's own input.
    #[test]
    fn newline_style_does_not_move_reported_positions() {
        for input in ["'a'\n+ 'b' + 99999999999999999999", "'a'\r\n+ 'b' + 99999999999999999999"] {
            let (line, column, offset, _) = parse_failure(input);
            assert_eq!((line, column), (2, 9), "input: {:?}", input);
            assert_eq!(&input[offset..], "99999999999999999999", "input: {:?}", input);
        }
    }

    /// The diagram has to agree with the numbers reported beside it.
    #[test]
    fn rendered_diagram_splits_on_newlines() {
        let Error::ParseError { rendered, .. } = parse_expression("1 +\r\n2 * * 3").unwrap_err() else {
            panic!("expected a parse error");
        };
        assert!(rendered.contains("2 | 2 * * 3"), "{}", rendered);
    }

    /// A literal too large for an `i64` is a user error, not an internal one.
    #[test]
    fn integer_overflow_points_at_the_literal() {
        let (line, column, _, message) = parse_failure("1 + 99999999999999999999");
        assert_eq!((line, column), (1, 5));
        assert!(message.starts_with("integer literal out of range"), "{}", message);
    }

    #[test]
    fn unknown_escape_points_at_the_backslash() {
        let (line, column, offset, message) = parse_failure(r"'a\tb'");
        assert_eq!((line, column, offset), (1, 3, 2));
        assert_eq!(message, r"unknown escape sequence: \t");
    }
}
