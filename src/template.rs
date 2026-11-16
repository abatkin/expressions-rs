//! Strings with `${...}` expressions in them, parsed once and evaluated later.
//!
//! [`evaluate_interpolations`](crate::evaluator::evaluate_interpolations) parses and
//! evaluates in one pass. [`Template`] separates the two, so a syntax error can be
//! reported when the text is loaded, and the same template can be evaluated many
//! times without re-parsing.

use std::ops::Range;

use crate::evaluator::{Evaluator, VariableResolver};
use crate::parser;
use crate::types::coerce::{Coercions, STANDARD};
use crate::types::error::{Error, Result};
use crate::types::expression::Expr;

/// A string with zero or more `${...}` expressions, parsed but not evaluated.
///
/// `$${` is an escape: it produces the two characters `${` and starts no
/// expression. Any other `$` is literal text.
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
    /// Text copied to the output as-is, with escapes already applied: the source
    /// `$${` appears here as `${`.
    Literal(String),
    /// A `${...}` expression.
    Expression {
        expr: Expr,
        /// Byte range in [`Template::source`] of the whole interpolation, from the
        /// `$` of `${` through the closing `}`.
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
            let (expr, consumed) = parser::parse_internal(parser::Origin::fragment(input, start), parser::Rule::delimited_expr)?;
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

    /// The parsed pieces, in order. Adjacent literal text is always merged into
    /// one [`Segment::Literal`], so two literals are never next to each other.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// True when the source held no `${...}`. Evaluating such a template returns
    /// [`Template::as_literal`] and cannot fail.
    pub fn is_literal(&self) -> bool {
        self.segments.iter().all(|s| matches!(s, Segment::Literal(_)))
    }

    /// The text evaluation would produce, if it does not depend on any
    /// expression: `Some` exactly when [`Template::is_literal`] is true.
    pub fn as_literal(&self) -> Option<&str> {
        match self.segments.as_slice() {
            [] => Some(""),
            [Segment::Literal(s)] => Some(s),
            _ => None,
        }
    }

    /// Evaluate every expression and join the results to the literal text, using
    /// the standard coercion policy.
    ///
    /// See [`Template::evaluate_with`].
    pub fn evaluate<T: VariableResolver>(&self, resolver: &T) -> Result<String> {
        self.evaluate_with(resolver, &STANDARD)
    }

    /// Like [`Template::evaluate`], with an explicit coercion policy.
    ///
    /// Each result is rendered with `Display`. Expressions are evaluated left to
    /// right, stopping at the first failure, which is reported as
    /// [`Error::InterpolationFailed`] identifying the expression. Never returns a
    /// parse error.
    pub fn evaluate_with<T: VariableResolver>(&self, resolver: &T, coercions: &dyn Coercions) -> Result<String> {
        let evaluator = Evaluator::new_with_coercions(resolver, coercions);
        let mut out = String::with_capacity(self.source.len());
        for (index, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Literal(text) => out.push_str(text),
                Segment::Expression { expr, span } => {
                    let value = evaluator.evaluate(expr).map_err(|error| {
                        let (line, column) = parser::line_col(&self.source, span.start);
                        Error::InterpolationFailed {
                            segment: index,
                            offset: span.start,
                            line,
                            column,
                            error: Box::new(error),
                        }
                    })?;
                    out.push_str(&value.to_string());
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::value::Value;

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
}
