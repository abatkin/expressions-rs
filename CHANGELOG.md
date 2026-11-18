# Changelog

## 0.6.0 Nesting limit, parser fixes

### Behavior changes

- Expressions may nest at most 64 levels deep (`parser::DEFAULT_MAX_DEPTH`); deeper input is a parse error. Nesting deep enough used to overflow the stack and abort the process: about 140 chained operators in a debug build on a 2 MiB thread, or 1,000 nested brackets on the main thread. A run of operators at one precedence, however long, counts as one level.
- `Expr::Binary` holds a whole run of operators at one precedence (`first`, then `rest` as operator and operand pairs) instead of a single operation, and `^` has its own `Expr::Power`. `BinaryOp::Pow` is gone. Long runs are parsed, evaluated, cloned, and dropped without recursing per operator.
- Inside `${...}`, a `}` in a comment closes the interpolation, so a comment there can no longer contain one (see Fixed).
- `a <= b` parses as `BinaryOp::Le` instead of being rewritten to `b >= a`, so operands are evaluated left to right again.

### Added

- `parser::ParseOptions`, with `parse_expression_with` and `Template::parse_with`, to change the nesting limit.
- A `stack-guard` feature, which turns on pest's native stack probing on targets where it is not automatic.

### Fixed

- `//` inside a string literal is no longer read as a comment.
- Inside `${...}`, a comment ends at the closing `}` as well as at a line break. It used to swallow the `}`, then fail to parse or silently drop template text up to a later `}`.
- The README's description of string escapes and integer literals matches the parser.

### Dependencies

- Requires pest 2.9.5 or later, for its native stack guard, and enables `pest_derive`'s `grammar-extras` feature.

## 0.5.0 Parsed templates, `$${` escape

### Behavior changes

- `$${` is an escape for a literal `${`. `"$${x}"` used to give `$` followed by the value of `x`; it now gives `${x}`.
- An evaluation failure inside `${...}` is now `Error::InterpolationFailed`, which locates the failing expression and wraps the original error, instead of `Error::EvaluationFailed`.

### Added

- `template::Template`: parse a string once with `Template::parse`, evaluate it later with `evaluate` / `evaluate_with`. `evaluate_interpolations` now uses it.
- `Template::evaluate_rendered`, which renders each value with a caller-supplied closure instead of `Display`. Its errors are reported as `InterpolationFailed` too.

## 0.4.1 Bug fixes

- Identifiers that begin with a boolean literal (`trueish`, `false_value`) parse as identifiers again; the literals now require a word boundary.
- Integer arithmetic is checked. `+`, `-`, `*`, unary `-` and `%` return the new `Error::IntegerOverflow` instead of panicking (debug) or silently wrapping (release). `i64::MIN % -1` panicked in every profile, since Rust checks remainder overflow regardless of `overflow-checks`.

## 0.4.0 Structured parse errors, custom coercion

## 0.3.0 Move from chumsky to pest for the parser

## 0.2.1 Add generic "Object" type

## 0.2.0 Major improvements to variable handling and language flexibility

## 0.1.1 Refactor errors to use an actual error type

## 0.1.0 Initial release
