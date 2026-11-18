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
pub(crate) fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (before.matches('\n').count() + 1, before[line_start..].chars().count() + 1)
}

/// How deeply an expression may nest, unless [`ParseOptions`] says otherwise.
pub const DEFAULT_MAX_DEPTH: usize = 64;

/// Limits applied while parsing an expression or a template.
///
/// ```
/// use simple_expressions::parser::{parse_expression_with, ParseOptions};
///
/// let shallow = ParseOptions::default().with_max_depth(3);
/// assert!(parse_expression_with("1 + 2", &shallow).is_ok());
/// assert!(parse_expression_with("1 + 2 + 3 + 4", &shallow).is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseOptions {
    /// The deepest an expression may nest; [`DEFAULT_MAX_DEPTH`] unless changed.
    ///
    /// Depth counts levels from the top of an expression to its deepest part. A
    /// literal or a variable is one level, and each operator, unary `-` or `!`,
    /// `.field`, `[index]`, call, list, dictionary, and pair of parentheses adds
    /// one around what it contains. Chained operators nest too: `a + b + c` is
    /// `(a + b) + c`, three levels deep. Parsing fails with
    /// [`Error::ParseError`] at the first point that goes past the limit.
    ///
    /// The limit exists because parsing, evaluating, and dropping an expression
    /// each recurse once per level, so a deep enough input -- `((((…))))`, or
    /// `1^1^1^…` -- would overflow the stack and abort the process. The default
    /// leaves ample room on a 2 MiB thread, Rust's default for spawned threads,
    /// even in an unoptimized build; raise it only for threads with the stack to
    /// match.
    pub max_depth: usize,
}

impl ParseOptions {
    /// These options with [`ParseOptions::max_depth`] set to `max_depth`.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self { max_depth: DEFAULT_MAX_DEPTH }
    }
}

pub fn parse_expression(input: &str) -> Result<Expr> {
    parse_expression_with(input, &ParseOptions::default())
}

/// As [`parse_expression`], with the given limits.
pub fn parse_expression_with(input: &str, options: &ParseOptions) -> Result<Expr> {
    parse_internal(Origin::whole(input), Rule::program, options).map(|r| r.0)
}

/// Returns the expression and how far into [`Origin::text`] the parse consumed.
pub(crate) fn parse_internal(origin: Origin, rule: Rule, options: &ParseOptions) -> Result<(Expr, usize)> {
    let mut pairs = InnerParser::parse(rule, origin.text()).map_err(|e| origin.convert(e))?;
    let pair = pairs.next().expect("program always produces one pair");

    debug_assert_eq!(pair.as_rule(), rule);
    let end_pos = pair.as_span().end_pos().pos();
    let expr_pair = pair.into_inner().next().expect("program contains expr");
    let builder = Builder { origin, max_depth: options.max_depth };
    let (expr, _) = builder.expr(expr_pair, 0)?;
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

/// An expression and its depth, counted as [`ParseOptions::max_depth`] counts it.
type Built = (Expr, usize);

/// Turns pest's parse tree into an [`Expr`], enforcing the depth limit.
///
/// Each method takes `outer`, how many levels are known to enclose its pair, and
/// returns the depth of what it built. `outer` is a lower bound -- an operand
/// cannot know how many operators will end up above it -- so `outer + depth`
/// never overstates the true depth, which makes checking it at every node safe;
/// at the root `outer` is 0 and the check is exact. Checking on the way down as
/// well is what bounds this code's own recursion: a nested bracket is refused as
/// soon as it is entered, not after recursing to the bottom of it.
struct Builder<'a> {
    origin: Origin<'a>,
    max_depth: usize,
}

impl Builder<'_> {
    /// Fails, at `pair`, if something `depth` levels deep would exceed the limit.
    fn check(&self, depth: usize, pair: &Pair<Rule>) -> Result<()> {
        if depth > self.max_depth {
            let message = format!("expression is nested too deeply (the limit is {} levels)", self.max_depth);
            return Err(self.origin.at(pair.as_span().start(), message));
        }
        Ok(())
    }

    fn expr(&self, pair: Pair<Rule>, outer: usize) -> Result<Built> {
        if pair.as_rule() != Rule::expr {
            return Err(Error::InternalParserError(format!("expected expr, got: {:?}", pair)));
        }
        self.check(outer + 1, &pair)?;
        let pairs = pair.into_inner();

        // pest's Pratt parser recurses once per operator in a run of
        // right-associative `^`, before any of them reaches `map_infix` to be
        // counted. Such a run nests one level per operator anyway, so refusing a
        // long one here is the same check, made before it can overflow. Runs of
        // other operators recurse at most once per precedence level.
        let mut run = 0;
        for p in pairs.clone() {
            match p.as_rule() {
                Rule::op_pow => {
                    run += 1;
                    self.check(outer + run + 1, &p)?;
                }
                Rule::unary => {}
                _ => run = 0,
            }
        }

        pratt()
            .map_primary(|p: Pair<Rule>| self.unary(p, outer))
            .map_infix(|lhs: Result<Built>, op: Pair<Rule>, rhs: Result<Built>| {
                let (left, left_depth) = lhs?;
                let (right, right_depth) = rhs?;
                let depth = 1 + left_depth.max(right_depth);
                self.check(outer + depth, &op)?;
                let expr = Expr::Binary {
                    left: Box::new(left),
                    op: binary_op(&op)?,
                    right: Box::new(right),
                };
                Ok((expr, depth))
            })
            .parse(pairs)
    }

    fn unary(&self, pair: Pair<Rule>, outer: usize) -> Result<Built> {
        if pair.as_rule() != Rule::unary {
            return self.postfix(pair, outer);
        }
        let at = pair.clone();
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
        // The operand is beneath every operator.
        let (mut expr, depth) = self.postfix(post, outer + ops.len())?;
        let depth = depth + ops.len();
        self.check(outer + depth, &at)?;
        for op in ops.into_iter().rev() {
            expr = Expr::Unary { op, expr: Box::new(expr) };
        }
        Ok((expr, depth))
    }

    fn postfix(&self, pair: Pair<Rule>, outer: usize) -> Result<Built> {
        if pair.as_rule() != Rule::postfix {
            return self.primary(pair, outer);
        }
        let mut inner = pair.into_inner();
        let (mut expr, mut depth) = self.primary(inner.next().expect("postfix starts with primary"), outer)?;
        // Each operation wraps everything to its left, one level above the deeper
        // of that and its own arguments.
        for op in inner {
            let at = op.clone();
            match op.as_rule() {
                Rule::call => {
                    let (args, args_depth) = self.exprs(op.into_inner(), outer + 1)?;
                    depth = depth.max(args_depth);
                    expr = Expr::Call { callee: Box::new(expr), args };
                }
                Rule::index => {
                    let (index, index_depth) = self.expr(op.into_inner().next().expect("index inner expr"), outer + 1)?;
                    depth = depth.max(index_depth);
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                Rule::property => {
                    let name = op.into_inner().next().expect("property ident").as_str().to_string();
                    expr = Expr::Member { object: Box::new(expr), field: name };
                }
                r => {
                    return Err(Error::InternalParserError(format!("unexpected postfix op: {:?}", r)));
                }
            }
            depth += 1;
            self.check(outer + depth, &at)?;
        }
        Ok((expr, depth))
    }

    fn primary(&self, pair: Pair<Rule>, outer: usize) -> Result<Built> {
        self.check(outer + 1, &pair)?;
        let origin = self.origin;
        match pair.as_rule() {
            Rule::primary => self.primary(pair.into_inner().next().unwrap(), outer),
            // Parentheses add no node, but they count as a level: pest recurses
            // through them like any other bracket, so a limit that skipped them
            // would leave `((((…))))` to pest's stack guard.
            Rule::parens => {
                let (expr, depth) = self.expr(pair.into_inner().next().unwrap(), outer + 1)?;
                Ok((expr, depth + 1))
            }
            Rule::ident => Ok((Expr::Var(pair.as_str().to_string()), 1)),
            Rule::number => Ok((parse_number(pair, origin)?, 1)),
            Rule::boolean => {
                let inner = pair.into_inner().next().unwrap();
                let val = matches!(inner.as_rule(), Rule::true_kw);
                Ok((Expr::Literal(Primitive::Bool(val)), 1))
            }
            Rule::string => {
                let s = unescape_string(&pair, origin)?;
                Ok((Expr::Literal(Primitive::Str(s)), 1))
            }
            Rule::list => {
                let (elems, depth) = self.exprs(pair.into_inner(), outer + 1)?;
                Ok((Expr::ListLiteral(elems), depth + 1))
            }
            Rule::dict => self.dict(pair, outer),
            r => Err(Error::InternalParserError(format!("unexpected primary op: {:?}", r))),
        }
    }

    fn dict(&self, pair: Pair<Rule>, outer: usize) -> Result<Built> {
        let mut items = Vec::new();
        let mut depth = 0;
        for p in pair.into_inner().filter(|p| p.as_rule() == Rule::pair) {
            let mut it = p.into_inner();
            let (key, key_depth) = self.expr(it.next().expect("pair key"), outer + 1)?;
            let (value, value_depth) = self.expr(it.next().expect("pair value"), outer + 1)?;
            depth = depth.max(key_depth).max(value_depth);
            items.push((key, value));
        }
        Ok((Expr::DictLiteral(items), depth + 1))
    }

    /// Builds every `expr` among `pairs`, skipping punctuation, each `outer` levels
    /// deep. Returns them with the greatest of their depths (0 if there are none).
    fn exprs<'i>(&self, pairs: impl Iterator<Item = Pair<'i, Rule>>, outer: usize) -> Result<(Vec<Expr>, usize)> {
        let mut exprs = Vec::new();
        let mut depth = 0;
        for p in pairs.filter(|p| p.as_rule() == Rule::expr) {
            let (expr, expr_depth) = self.expr(p, outer)?;
            depth = depth.max(expr_depth);
            exprs.push(expr);
        }
        Ok((exprs, depth))
    }
}

fn binary_op(op: &Pair<Rule>) -> Result<BinaryOp> {
    Ok(match op.as_rule() {
        Rule::op_or => BinaryOp::Or,
        Rule::op_and => BinaryOp::And,
        Rule::op_eq => {
            if op.as_str() == "==" {
                BinaryOp::Eq
            } else {
                BinaryOp::Ne
            }
        }
        Rule::op_cmp => match op.as_str() {
            "<=" => BinaryOp::Le,
            ">=" => BinaryOp::Ge,
            "<" => BinaryOp::Lt,
            _ => BinaryOp::Gt,
        },
        Rule::op_add => {
            if op.as_str() == "-" {
                BinaryOp::Sub
            } else {
                BinaryOp::Add
            }
        }
        Rule::op_mul => match op.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            _ => BinaryOp::Mod,
        },
        Rule::op_pow => BinaryOp::Pow,
        r => return Err(Error::InternalParserError(format!("unexpected infix op: {:?}", r))),
    })
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
        let (expr, idx) = parse_internal(Origin::whole(input), Rule::delimited_expr, &ParseOptions::default()).unwrap();
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
    /// break inside one is matched by `string_char` (`string` is atomic, so
    /// `WHITESPACE` never runs inside it), and `unescape_string` reads the raw span.
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

    /// `primary` tries `boolean` before `ident`, so the boolean literals need a
    /// trailing word boundary or every identifier starting with one of them is a
    /// parse error.
    #[test]
    fn identifiers_may_start_with_a_boolean_literal() {
        for name in ["trueish", "true_", "true1", "falsey", "false_value", "false2"] {
            let expr = parse_expression(name).unwrap_or_else(|e| panic!("{:?}: {}", name, e));
            assert_eq!(expr, Expr::Var(name.to_string()), "input: {:?}", name);
        }

        // and the literals themselves still parse as literals
        assert_eq!(parse_expression("true").unwrap(), Expr::Literal(Primitive::Bool(true)));
        assert_eq!(parse_expression("false").unwrap(), Expr::Literal(Primitive::Bool(false)));
        assert_eq!(parse_expression("(true)").unwrap(), Expr::Literal(Primitive::Bool(true)));

        // a boundary that is not alphanumeric still ends the literal
        assert_eq!(
            parse_expression("true.length").unwrap(),
            Expr::Member {
                object: Box::new(Expr::Literal(Primitive::Bool(true))),
                field: "length".to_string(),
            }
        );
    }

    #[test]
    fn unknown_escape_points_at_the_backslash() {
        let (line, column, offset, message) = parse_failure(r"'a\tb'");
        assert_eq!((line, column, offset), (1, 3, 2));
        assert_eq!(message, r"unknown escape sequence: \t");
    }

    #[test]
    fn test_comments_in_string() {
        let expr = parse_expression("'a // not a comment'").unwrap();
        assert_eq!(expr, Expr::Literal(Primitive::Str("a // not a comment".to_string())));
    }

    /// Outside an interpolation there is no "}" to stop at, so a comment still runs
    /// to the end of the line.
    #[test]
    fn top_level_comments_do_not_end_at_a_brace() {
        assert_eq!(parse_expression("1 // a } b").unwrap(), Expr::Literal(Primitive::Int(1)));
        assert_eq!(parse_expression("{'a': 1 // }\n}").unwrap(), parse_expression("{'a': 1}").unwrap());
    }

    /// Depth as `ParseOptions::max_depth` documents it: each input parses at
    /// exactly its own depth and fails one below.
    #[test]
    fn depth_counts_every_level() {
        for (input, depth) in [
            ("1", 1),
            ("x", 1),
            ("-1", 2),
            ("!!x", 3),
            ("(1)", 2),
            ("((1))", 3),
            ("a + b", 2),
            ("a + b + c", 3),
            ("1 + 2 * 3", 3),
            ("2 ^ 3 ^ 4", 3),
            ("x.a.b", 3),
            ("x[0]", 2),
            ("f(a)(b)", 3),
            ("f(a + b)", 3),
            ("-(x[0])", 4),
            ("[]", 1),
            ("[1, [2]]", 3),
            ("{}", 1),
            ("{'a': [1]}", 3),
            ("{[1]: 2}", 3),
        ] {
            let at = |max_depth| parse_expression_with(input, &ParseOptions::default().with_max_depth(max_depth));
            assert!(at(depth).is_ok(), "{:?} should fit in {}: {:?}", input, depth, at(depth));
            assert!(at(depth - 1).is_err(), "{:?} should not fit in {}", input, depth - 1);
        }
    }

    /// The error points into the part of the expression that goes past the limit:
    /// the operator or postfix operation for a chain of them, otherwise the deepest
    /// part.
    #[test]
    fn too_deep_points_into_the_part_past_the_limit() {
        let shallow = ParseOptions::default().with_max_depth(3);
        let failure = |input: &str| match parse_expression_with(input, &shallow).unwrap_err() {
            Error::ParseError { offset, message, .. } => (offset, message),
            other => panic!("expected a parse error, got: {:?}", other),
        };
        // ........................0123456789012
        assert_eq!(failure("1 + 2 + 3 + 4"), (10, "expression is nested too deeply (the limit is 3 levels)".to_string()));
        assert_eq!(failure("[[[1]]]").0, 3);
        assert_eq!(failure("1^1^1^1").0, 5);
        assert_eq!(failure("---1").0, 3);
        // ....................0123456
        assert_eq!(failure("x.a.b.c").0, 5);
        assert_eq!(failure("f(1)(2)(3)").0, 7);
    }

    /// `a <= b` must stay `a <= b`, not become `b >= a`: the tree is public, and
    /// operands are evaluated left to right, so swapping them changes which side
    /// runs (and fails) first.
    #[test]
    fn less_or_equal_keeps_its_operand_order() {
        let var = |name: &str| Box::new(Expr::Var(name.to_string()));
        for (src, op) in [("a <= b", BinaryOp::Le), ("a >= b", BinaryOp::Ge), ("a < b", BinaryOp::Lt), ("a > b", BinaryOp::Gt)] {
            assert_eq!(parse_expression(src).unwrap(), Expr::Binary { left: var("a"), op, right: var("b") }, "input: {:?}", src);
        }
    }
}
