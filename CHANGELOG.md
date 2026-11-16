# Changelog

## 0.5.0 Parsed templates, `$${` escape

### Behavior changes

- **`$${` is now an escape for a literal `${`** in interpolated strings, and starts no expression. `"$${x}"` used to evaluate to `$` followed by the value of `x`; it now evaluates to the literal text `${x}`. Scanning is left to right, so `"$$${x}"` is a literal `$` followed by the escape, giving `$${x}`. A lone `$`, `$$` and `$x` are still literal text. Previously the only way to write a literal `${` was `${'${'}`, which still works.
- An evaluation failure inside `${...}` is now `Error::InterpolationFailed { segment, offset, line, column, error }` instead of `Error::EvaluationFailed("evaluation error: ...")`. It identifies which expression failed (`offset` is the byte offset of its `${` in the template source) and carries the original error unflattened. Its message reads `evaluation failed in interpolation at line L, column C: <error>`. Parse errors are unchanged.

### Added

- `template::Template`, which splits `evaluate_interpolations` into a parse step and an evaluate step. `Template::parse` reports every syntax error up front, with the same `Error::ParseError` positions as before (anchored to the whole input), without resolving or evaluating anything; `evaluate` / `evaluate_with` can then run any number of times and never produce a parse error. `Template` is `Clone`, `Debug` and `PartialEq`.
- `Template::source`, `Template::segments` (`Segment::Literal` / `Segment::Expression { expr, span }`), `Template::is_literal` (no `${...}`, so evaluation cannot fail) and `Template::as_literal` (that text, if so).
- `evaluate_interpolations` and `evaluate_interpolations_with` are now `Template::parse(input)?.evaluate_with(...)`.

## 0.4.1 Bug fixes

- Identifiers that begin with a boolean literal (`trueish`, `false_value`) parse as identifiers again; the literals now require a word boundary.
- Integer arithmetic is checked. `+`, `-`, `*`, unary `-` and `%` return the new `Error::IntegerOverflow` instead of panicking (debug) or silently wrapping (release). `i64::MIN % -1` panicked in every profile, since Rust checks remainder overflow regardless of `overflow-checks`.

## 0.4.0 Structured parse errors, custom coercion

## 0.3.0 Move from chumsky to pest for the parser

## 0.2.1 Add generic "Object" type

## 0.2.0 Major improvements to variable handling and language flexibility

## 0.1.1 Refactor errors to use an actual error type

## 0.1.0 Initial release
