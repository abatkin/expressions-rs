# Changelog

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
