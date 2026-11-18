//! Strings with `${...}` expressions in them, parsed once and evaluated later.

use std::ops::Range;

use crate::evaluator::{Evaluator, VariableResolver};
use crate::parser;
use crate::types::coerce::{Coercions, STANDARD};
use crate::types::error::{Error, Result};
use crate::types::expression::Expr;
use crate::types::value::Value;

/// A string with zero or more `${...}` expressions, parsed but not evaluated.
///
/// `$${` produces a literal `${`; any other `$` is literal text.
///
/// ```
/// use simple_expressions::evaluator::VariableResolver;
/// use simple_expressions::template::Template;
/// use simple_expressions::types::value::Value;
///
/// struct Vars;
/// impl VariableResolver for Vars {
///     fn resolve(&self, name: &str) -> Option<Value> {
///         (name == "name").then(|| Value::from("world"))
///     }
/// }
///
/// // Syntax errors are reported here, before anything is resolved.
/// let greeting = Template::parse("Hello ${name}! Cost: $${price}").unwrap();
/// assert!(!greeting.is_literal());
/// assert_eq!(greeting.evaluate(&Vars).unwrap(), "Hello world! Cost: ${price}");
///
/// assert!(Template::parse("Hello ${name").is_err());
/// assert_eq!(Template::parse("$${not an expression}").unwrap().as_literal(), Some("${not an expression}"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    source: String,
    segments: Vec<Segment>,
}

/// One piece of a [`Template`].
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// Text with escapes applied.
    Literal(String),
    /// A `${...}` expression.
    Expression {
        expr: Expr,
        /// Byte range of the whole `${...}` in [`Template::source`].
        span: Range<usize>,
    },
}

impl Template {
    /// Parse every `${...}` in `input`. Nothing is resolved or evaluated.
    ///
    /// Parse errors are [`Error::ParseError`], positioned in `input` as a whole
    /// rather than in the expression the error occurred in. A `${` with no
    /// matching `}` is a parse error.
    pub fn parse(input: &str) -> Result<Template> {
        Self::parse_with(input, &parser::ParseOptions::default())
    }

    /// As [`Template::parse`], with the given limits, which apply to each
    /// `${...}` separately.
    pub fn parse_with(input: &str, options: &parser::ParseOptions) -> Result<Template> {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut pos = 0;

        while let Some(found) = input[pos..].find("${") {
            let dollar = pos + found;
            // Scanning left to right, `$${` is seen before the `${` inside it, so
            // the first `${` is an escape exactly when a `$` comes right before it.
            // This also makes `$$${` a literal `$` followed by the escape.
            if dollar > pos && input.as_bytes()[dollar - 1] == b'$' {
                literal.push_str(&input[pos..dollar - 1]);
                literal.push_str("${");
                pos = dollar + 2;
                continue;
            }

            literal.push_str(&input[pos..dollar]);
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }

            // Parsing against the whole input, rather than the slice after `${`,
            // keeps error positions anchored to what the caller wrote -- otherwise
            // they would be relative to the slice, earlier and possibly on the
            // wrong line.
            let start = dollar + 2;
            let (expr, consumed) = parser::parse_internal(parser::Origin::fragment(input, start), parser::Rule::delimited_expr, options)?;
            let end = start + consumed;
            segments.push(Segment::Expression { expr, span: dollar..end });
            pos = end;
        }

        literal.push_str(&input[pos..]);
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }

        Ok(Template { source: input.to_string(), segments })
    }

    /// The text as written, before escapes are applied.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The parsed pieces, in order. Adjacent literal text is merged.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// True when the source held no `${...}`, so evaluation cannot fail.
    pub fn is_literal(&self) -> bool {
        self.segments.iter().all(|s| matches!(s, Segment::Literal(_)))
    }

    /// The evaluated text, if [`Template::is_literal`].
    pub fn as_literal(&self) -> Option<&str> {
        match self.segments.as_slice() {
            [] => Some(""),
            [Segment::Literal(s)] => Some(s),
            _ => None,
        }
    }

    /// Evaluate every expression, rendered with `Display`, and join the results to
    /// the literal text.
    pub fn evaluate<T: VariableResolver>(&self, resolver: &T) -> Result<String> {
        self.evaluate_with(resolver, &STANDARD)
    }

    /// Like [`Template::evaluate`], with an explicit coercion policy.
    ///
    /// Failures are [`Error::InterpolationFailed`], never a parse error.
    pub fn evaluate_with<T: VariableResolver>(&self, resolver: &T, coercions: &dyn Coercions) -> Result<String> {
        self.evaluate_rendered(resolver, coercions, |value| Ok(value.to_string()))
    }

    /// Like [`Template::evaluate_with`], but each expression's value is turned
    /// into text by `render` instead of `Display`.
    ///
    /// `render` is called once per expression, left to right, after it evaluates
    /// successfully. An `Err` from it is reported like an evaluation failure, as
    /// [`Error::InterpolationFailed`] wrapping the caller's error unchanged.
    ///
    /// ```
    /// use simple_expressions::evaluator::VariableResolver;
    /// use simple_expressions::template::Template;
    /// use simple_expressions::types::coerce::STANDARD;
    /// use simple_expressions::types::error::Error;
    /// use simple_expressions::types::value::Value;
    ///
    /// struct NoVars;
    /// impl VariableResolver for NoVars {
    ///     fn resolve(&self, _: &str) -> Option<Value> {
    ///         None
    ///     }
    /// }
    ///
    /// // accept only scalars
    /// let scalars_only = |v: &Value| match v {
    ///     Value::Primitive(p) => Ok(p.to_string()),
    ///     other => Err(Error::TypeMismatch(format!("cannot interpolate a {}", other.type_name()))),
    /// };
    ///
    /// let ok = Template::parse("n=${1 + 2}").unwrap();
    /// assert_eq!(ok.evaluate_rendered(&NoVars, &STANDARD, scalars_only).unwrap(), "n=3");
    ///
    /// let bad = Template::parse("a${1}b${[1, 2]}").unwrap();
    /// match bad.evaluate_rendered(&NoVars, &STANDARD, scalars_only).unwrap_err() {
    ///     Error::InterpolationFailed { segment, offset, error, .. } => {
    ///         assert_eq!((segment, offset), (3, 6));
    ///         assert!(matches!(*error, Error::TypeMismatch(_)));
    ///     }
    ///     other => panic!("{other:?}"),
    /// }
    /// ```
    pub fn evaluate_rendered<T, R>(&self, resolver: &T, coercions: &dyn Coercions, mut render: R) -> Result<String>
    where
        T: VariableResolver,
        R: FnMut(&Value) -> Result<String>,
    {
        let evaluator = Evaluator::new_with_coercions(resolver, coercions);
        let mut out = String::with_capacity(self.source.len());
        for (index, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Literal(text) => out.push_str(text),
                Segment::Expression { expr, span } => {
                    let text = evaluator.evaluate(expr).and_then(|value| render(&value)).map_err(|error| self.failure(index, span.start, error))?;
                    out.push_str(&text);
                }
            }
        }
        Ok(out)
    }

    /// `error`, located at the expression at `segment`, whose `${` is at byte
    /// `offset` of the source.
    fn failure(&self, segment: usize, offset: usize, error: Error) -> Error {
        let (line, column) = parser::line_col(&self.source, offset);
        Error::InterpolationFailed {
            segment,
            offset,
            line,
            column,
            error: Box::new(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Vars;
    impl VariableResolver for Vars {
        fn resolve(&self, name: &str) -> Option<Value> {
            match name {
                "x" => Some(Value::from(10i64)),
                "name" => Some(Value::from("world")),
                _ => None,
            }
        }
    }

    fn eval(input: &str) -> String {
        Template::parse(input).unwrap().evaluate(&Vars).unwrap()
    }

    fn parse_failure(input: &str) -> (usize, usize, usize) {
        match Template::parse(input).unwrap_err() {
            Error::ParseError { line, column, offset, .. } => (line, column, offset),
            other => panic!("expected a parse error, got: {:?}", other),
        }
    }

    #[test]
    fn segments_and_source() {
        let t = Template::parse("a${x}b${name}").unwrap();
        assert_eq!(t.source(), "a${x}b${name}");
        assert_eq!(
            t.segments(),
            &[
                Segment::Literal("a".into()),
                Segment::Expression { expr: Expr::Var("x".into()), span: 1..5 },
                Segment::Literal("b".into()),
                Segment::Expression { expr: Expr::Var("name".into()), span: 6..13 },
            ]
        );
        assert_eq!(&t.source()[1..5], "${x}");
        assert_eq!(&t.source()[6..13], "${name}");
        assert_eq!(t.evaluate(&Vars).unwrap(), "a10bworld");
    }

    #[test]
    fn is_literal() {
        for input in ["", "abc", "a$b", "$${x}", "$", "$$", "$x", "}"] {
            let t = Template::parse(input).unwrap();
            assert!(t.is_literal(), "{:?}", input);
            assert_eq!(t.as_literal(), Some(t.evaluate(&Vars).unwrap().as_str()), "{:?}", input);
        }
        for input in ["${x}", "a${x}", "$${x}${x}"] {
            let t = Template::parse(input).unwrap();
            assert!(!t.is_literal(), "{:?}", input);
            assert_eq!(t.as_literal(), None, "{:?}", input);
        }
    }

    #[test]
    fn literal_templates_evaluate_without_resolving_anything() {
        struct Panics;
        impl VariableResolver for Panics {
            fn resolve(&self, _: &str) -> Option<Value> {
                panic!("a literal template resolved a variable")
            }
        }
        assert_eq!(Template::parse("a$b $${x}").unwrap().evaluate(&Panics).unwrap(), "a$b ${x}");
    }

    #[test]
    fn dollar_brace_escape() {
        assert_eq!(eval("$${x}"), "${x}");
        assert_eq!(eval("$$${x}"), "$${x}");
        assert_eq!(eval("$$$${x}"), "$$${x}");
        assert_eq!(eval("a$${b}${'c'}"), "a${b}c");
        assert_eq!(eval("$${"), "${");
        assert_eq!(eval("${x}$${x}${x}"), "10${x}10");
        assert_eq!(eval("$${${x}}"), "${10}");
        // the escape does not hide a syntax error that follows it
        assert_eq!(parse_failure("$${ ${1 + + }"), (1, 11, 10));
    }

    #[test]
    fn other_dollars_are_literal() {
        assert_eq!(eval("$"), "$");
        assert_eq!(eval("$$"), "$$");
        assert_eq!(eval("$x"), "$x");
        assert_eq!(eval("a$"), "a$");
        assert_eq!(eval("$ {x}"), "$ {x}");
        assert_eq!(eval("{x}"), "{x}");
        assert_eq!(eval("$x${x}"), "$x10");
    }

    #[test]
    fn multibyte_text_around_expressions() {
        let t = Template::parse("é$${é}${name}ü").unwrap();
        assert_eq!(t.evaluate(&Vars).unwrap(), "é${é}worldü");
    }

    /// A comment inside `${...}` ends at the closing "}" as well as at a line
    /// break. Without that it swallowed the delimiter, and then either failed to
    /// parse or silently ate template text up to some later "}".
    #[test]
    fn comments_end_at_the_closing_brace() {
        assert_eq!(eval("a ${x // note} b"), "a 10 b");
        assert_eq!(eval("a ${x // note} b\n} c"), "a 10 b\n} c");
        assert_eq!(eval("${x // note\n}"), "10");
        assert_eq!(eval("${x + // note\n 1}"), "11");
        // the first "}" ends it, even one meant as part of the comment
        assert_eq!(eval("${x // a } b}"), "10 b}");
        // and a "}" in a string is not a delimiter
        assert_eq!(eval("${'//}' // c}"), "//}");
        // including a comment that opens the interpolation, before any expression
        assert_eq!(eval("a ${// note\nx} b"), "a 10 b");
        assert_eq!(eval("a ${ // note\nx} b"), "a 10 b");
        // ..........................01234567890
        assert_eq!(parse_failure("a ${// c} lost\nx} b"), (1, 9, 8));
        assert_eq!(parse_failure("a ${ // c} lost\nx} b"), (1, 10, 9));
    }

    #[test]
    fn unclosed_interpolation_is_a_parse_error() {
        // ............................01234567
        assert_eq!(parse_failure("abc ${x"), (1, 8, 7));
        assert!(matches!(Template::parse("${").unwrap_err(), Error::ParseError { .. }));
        assert!(matches!(Template::parse("a ${'}'").unwrap_err(), Error::ParseError { .. }));
    }

    #[test]
    fn syntax_error_in_second_expression() {
        // ............................0123456789012345678
        assert_eq!(parse_failure("${x} and ${2 * * }"), (1, 16, 15));
    }

    #[test]
    fn syntax_error_on_a_later_line() {
        // line 3 starts at byte 18; the error is the second `*`, column 13
        assert_eq!(parse_failure("first ${x}\nsecond\nthird ${2 * * }"), (3, 13, 30));
        let Error::ParseError { rendered, .. } = Template::parse("first ${x}\nsecond\nthird ${2 * * }").unwrap_err() else {
            panic!("expected a parse error");
        };
        assert!(rendered.contains("3 | third ${2 * * }"), "{}", rendered);
    }

    #[test]
    fn evaluation_failure_identifies_the_expression() {
        // ...........................012345678901234567890
        let t = Template::parse("${x} and ${missing}!").unwrap();
        match t.evaluate(&Vars).unwrap_err() {
            Error::InterpolationFailed { segment, offset, line, column, error } => {
                assert_eq!(segment, 2);
                assert!(matches!(t.segments()[segment], Segment::Expression { .. }));
                assert_eq!(offset, 9);
                assert_eq!((line, column), (1, 10));
                assert!(matches!(*error, Error::ResolveFailed(ref name) if name == "missing"), "{:?}", error);
            }
            other => panic!("expected an interpolation failure, got: {:?}", other),
        }
    }

    #[test]
    fn evaluation_failure_on_a_later_line() {
        let t = Template::parse("${x}\n  é ${1 / 0}").unwrap();
        let err = t.evaluate(&Vars).unwrap_err();
        let Error::InterpolationFailed { segment, offset, line, column, ref error } = err else {
            panic!("expected an interpolation failure, got: {:?}", err);
        };
        // `é` is two bytes, one column
        assert_eq!((segment, offset, line, column), (2, 10, 2, 5));
        assert!(matches!(**error, Error::DivideByZero));
        assert_eq!(err.to_string(), "evaluation failed in interpolation at line 2, column 5: divide by zero");
    }

    #[test]
    fn templates_can_be_reused_and_cloned() {
        let t = Template::parse("${x + 1}").unwrap();
        let copy = t.clone();
        assert_eq!(t.evaluate(&Vars).unwrap(), "11");
        assert_eq!(t.evaluate(&Vars).unwrap(), "11");
        assert_eq!(copy, t);
        assert_eq!(copy.evaluate(&Vars).unwrap(), "11");
    }

    /// Renders scalars with `Display`, rejects anything else.
    fn scalars_only(value: &Value) -> Result<String> {
        match value {
            Value::Primitive(p) => Ok(p.to_string()),
            other => Err(Error::TypeMismatch(format!("cannot interpolate a {}", other.type_name()))),
        }
    }

    /// `(segment, offset, line, column, error)` of an interpolation failure.
    fn located(err: Error) -> (usize, usize, usize, usize, Error) {
        match err {
            Error::InterpolationFailed { segment, offset, line, column, error } => (segment, offset, line, column, *error),
            other => panic!("expected an interpolation failure, got: {:?}", other),
        }
    }

    #[test]
    fn renderer_output_is_joined_to_the_literal_text() {
        let t = Template::parse("<${x}|${name}>").unwrap();
        assert_eq!(t.evaluate_rendered(&Vars, &STANDARD, |v| Ok(format!("[{}]", v))).unwrap(), "<[10]|[world]>");
    }

    #[test]
    fn rejected_value_identifies_the_expression() {
        // ...........................01234567
        let t = Template::parse("a${x}b${[1, 2]}").unwrap();
        let (segment, offset, line, column, error) = located(t.evaluate_rendered(&Vars, &STANDARD, scalars_only).unwrap_err());
        assert_eq!((segment, offset, line, column), (3, 6, 1, 7));
        assert!(matches!(t.segments()[segment], Segment::Expression { ref span, .. } if &t.source()[span.clone()] == "${[1, 2]}"));
        // the caller's error, unchanged
        assert!(matches!(error, Error::TypeMismatch(ref m) if m == "cannot interpolate a list"), "{:?}", error);
    }

    #[test]
    fn rejected_value_on_a_later_line() {
        let t = Template::parse("one ${x}\ntwo ${{'a': 1}}").unwrap();
        let (segment, offset, line, column, _) = located(t.evaluate_rendered(&Vars, &STANDARD, scalars_only).unwrap_err());
        assert_eq!((segment, offset, line, column), (3, 13, 2, 5));
    }

    #[test]
    fn rejection_is_reported_like_an_evaluation_failure() {
        let t = Template::parse("${x}\n${y}").unwrap();
        let evaluation = t.evaluate_rendered(&Vars, &STANDARD, scalars_only).unwrap_err();
        let rendering = Template::parse("${x}\n${[1]}").unwrap().evaluate_rendered(&Vars, &STANDARD, scalars_only).unwrap_err();
        let (e_seg, e_off, e_line, e_col, _) = located(evaluation);
        let (r_seg, r_off, r_line, r_col, _) = located(rendering);
        assert_eq!((e_seg, e_off, e_line, e_col), (r_seg, r_off, r_line, r_col));
    }

    #[test]
    fn render_sees_each_successful_expression_once_in_order() {
        let seen = RefCell::new(Vec::new());
        let t = Template::parse("lit $${esc} ${x} more ${name}$${} ${x + 1} end").unwrap();
        let out = t
            .evaluate_rendered(&Vars, &STANDARD, |v| {
                seen.borrow_mut().push(v.to_string());
                Ok(v.to_string())
            })
            .unwrap();
        assert_eq!(out, "lit ${esc} 10 more world${} 11 end");
        // literal text and escapes never reach the renderer
        assert_eq!(*seen.borrow(), ["10", "world", "11"]);
    }

    #[test]
    fn literal_templates_never_call_render() {
        for input in ["", "abc", "$${x}", "a$b$$c"] {
            let t = Template::parse(input).unwrap();
            let out = t.evaluate_rendered(&Vars, &STANDARD, |_| panic!("render called for {:?}", input)).unwrap();
            assert_eq!(Some(out.as_str()), t.as_literal());
        }
    }

    #[test]
    fn evaluation_failure_wins_before_render() {
        let seen = RefCell::new(Vec::new());
        let t = Template::parse("${x}${missing}${name}").unwrap();
        let err = t
            .evaluate_rendered(&Vars, &STANDARD, |v| {
                seen.borrow_mut().push(v.to_string());
                Ok(v.to_string())
            })
            .unwrap_err();
        let (segment, _, _, _, error) = located(err);
        assert_eq!(segment, 1);
        assert!(matches!(error, Error::ResolveFailed(_)), "{:?}", error);
        // the failed expression's value never reached render, and evaluation stopped there
        assert_eq!(*seen.borrow(), ["10"]);
    }

    #[test]
    fn render_failure_stops_evaluation() {
        struct Counting(RefCell<Vec<String>>);
        impl VariableResolver for Counting {
            fn resolve(&self, name: &str) -> Option<Value> {
                self.0.borrow_mut().push(name.to_string());
                Some(Value::from(1i64))
            }
        }
        let resolver = Counting(RefCell::new(Vec::new()));
        let t = Template::parse("${a}${b}${c}").unwrap();
        let err = t.evaluate_rendered(&resolver, &STANDARD, |_| Err(Error::EvaluationFailed("no".into()))).unwrap_err();
        let (segment, _, _, _, error) = located(err);
        assert_eq!(segment, 0);
        assert!(matches!(error, Error::EvaluationFailed(ref m) if m == "no"));
        assert_eq!(*resolver.0.borrow(), ["a"]);
    }
}
