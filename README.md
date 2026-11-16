# Simple Expressions

A small expression language with variables, function calls, simple types and common operators.

- Literals
  - Integers: sequence of digits, optionally with a leading '-'.
  - Floats: digits with a decimal point, optionally with a leading '-'.
  - Strings: delimited by single ' or double " quotes. Supported escapes: \n, \r, \t, \\, \", \\'. Newlines are not allowed inside strings unless escaped as a backslash followed by a newline (\\ + newline).
  - Booleans: true, false.
- Collections
  - Lists: [expr, expr, ...]
    - Example: [1, 2, 3], ["a", 1+2]
  - Dictionaries (maps): { key_expr: value_expr, ... }
    - Keys can be any expression, but at runtime must evaluate to strings; duplicate keys are allowed, last one wins.
    - Example: {"a": 1, "b": 2}, {"a"+"b": 3}
- Whitespace and comments
  - Any spaces, tabs, or newlines are ignored.
  - Line comments start with // and continue to the end of the line.
- Grouping
  - Parentheses ( ... ) group sub-expressions.
- Identifiers, variables and functions
  - Identifiers follow the usual rules: start with a letter or underscore, then letters, digits, or underscores. A keyword is only a keyword as a whole word, so `trueish` and `false_value` are ordinary variable names.
  - Postfix chaining after any primary expression:
    - Member access: .field
    - Indexing: [expr]
    - Calls: (arg1, arg2)
    - These can be chained left-to-right: `a.b.c`, `a.b(1, 2).c[0].d(e)`, `foo(1)(2)(3)`, `arr[1+2][0]`.
- Indexing rules
  - Lists: index with an integer. Negative indices count from the end (e.g., [-1] is last). Out-of-bounds causes an error.
  - Dicts: index with a string key. Missing keys cause an error. Use builtin get(...) to provide a default (see below).
- Operators
  - Arithmetic: +, -, *, /, %, ^ (exponentiation; right-associative)
  - Comparisons: <, <=, >, >=, ==, !=
  - Logical: &&, ||, and unary !
  - Notes:
    - '+' supports number addition and string concatenation.
    - Comparisons work on numbers (with int/float coercion) or on strings. Other mixes are errors.
    - Arithmetic on two integers stays an integer, except for '/' and '^', which always produce a float (5 / 2 is 2.5).
    - Integers are 64-bit and signed; a result that does not fit is an error rather than a wrapped or truncated value. Floats follow IEEE 754 and saturate to infinity instead.
    - '==' and '!=' do not coerce: an integer never equals a float, and a string never equals a number.
- Truthiness (used by !, &&, ||)
  - These are the rules of the default coercion policy; see "Custom coercions" below to change them.
  - Numbers: 0/0.0 is false; any other number is true.
  - Booleans: as-is.
  - Strings: only the literal strings "true" and "false" coerce to booleans; other strings are not allowed in logical ops.
  - Lists/Dicts: empty is false; non-empty is true.
  - Functions: not coercible to bool.
- Custom coercions (library API)
  - Truthiness and "what counts as a number" are decided by a `Coercions` policy rather than being fixed in the language.
  - `StandardCoercions` (the default) implements the rules above; `StrictCoercions` accepts only real bools and real numbers.
  - Pass your own with `evaluate_with(...)`, `evaluate_interpolations_with(...)`, or `Evaluator::new_with_coercions(...)`.
  - The policy reaches into function bodies too: a function receives a `Context` and should convert arguments with `cx.to_bool(v)` / `cx.to_number(v)` rather than matching on `Value` directly, so it agrees with the operators.
  - `to_number` reports whether it found an integer or a float, so integer operands can produce integer results.
  - Turning values into text is deliberately not part of the policy; that is formatting, and it goes through `Display`.
- String interpolation (library API)
  - `evaluate_interpolations(...)` replaces each `${ ... }` with the value of the contained expression, rendered with `Display`. The result is always a string.
  - Example: evaluating "Hello ${1 + 2}" yields "Hello 3". Braces inside quoted strings are handled; a missing closing '}' is a parse error.
  - Escape: `$${` produces a literal `${` and starts no expression, so "$${x}" yields "${x}". Text is scanned left to right, so "$$${x}" is a literal `$` followed by the escape and yields "$${x}". Any other `$` (a lone `$`, `$$`, `$x`) is literal text.
    - Behavior change in 0.5.0: before, "$${x}" yielded `$` followed by the value of `x`.
  - Parse once, evaluate later: `Template::parse(text)` (in `simple_expressions::template`) parses every `${ ... }` without resolving or evaluating anything, and reports syntax errors with positions in the whole text. The `Template` can be stored (it is `Clone` and `Debug`) and evaluated any number of times with `template.evaluate(&resolver)` or `template.evaluate_with(&resolver, &coercions)`; evaluating never re-parses and never produces a parse error. `evaluate_interpolations` is shorthand for `Template::parse(text)?.evaluate(&resolver)`.
    - `template.is_literal()` is true when the text has no `${ ... }`; evaluating it then cannot fail, and `template.as_literal()` returns the text (with escapes applied) without needing a resolver.
    - `template.source()` is the text as written; `template.segments()` lists the parsed `Segment::Literal(text)` and `Segment::Expression { expr, span }` pieces in order, where `span` is the byte range of the `${ ... }` in the source.
    - Custom rendering: `template.evaluate_rendered(&resolver, &coercions, render)` hands each expression's value to `render: FnMut(&Value) -> Result<String>` instead of using `Display`, and joins its output to the literal text. Use it to restrict which types a template may produce, or to format them differently. `render` is called once per expression, left to right, only after that expression evaluates successfully; it never sees literal text or `$${` escapes. `evaluate_with` is `evaluate_rendered` with `|v| Ok(v.to_string())`.
  - When an expression fails to evaluate, or `render` returns an `Err` for its value, the error is `Error::InterpolationFailed { segment, offset, line, column, error }`: `segment` indexes `template.segments()`, `offset`/`line`/`column` locate the failing `${` in the source, and `error` is the underlying failure, or the renderer's own error unchanged. Evaluation stops at the first failure of either kind.

```rust
use simple_expressions::template::Template;
use simple_expressions::types::{coerce::STANDARD, error::Error, value::Value};

let greeting = Template::parse("Hello ${name}! Cost: $${price}")?; // syntax errors surface here
// ...later, possibly many times:
let text = greeting.evaluate(&resolver)?; // "Hello world! Cost: ${price}"

// only strings, numbers and booleans may be interpolated
let scalars_only = |v: &Value| match v {
    Value::Primitive(p) => Ok(p.to_string()),
    other => Err(Error::TypeMismatch(format!("cannot interpolate a {}", other.type_name()))),
};
let text = greeting.evaluate_rendered(&resolver, &STANDARD, scalars_only)?;
// a list in "a${x}b${[1, 2]}" fails with InterpolationFailed { segment: 3, offset: 6, line: 1, column: 7, error: TypeMismatch(..) }
```

Built-in members and functions

- Strings
  - .length (property): number of characters
  - .toUpper(): uppercase copy
  - .toLower(): lowercase copy
  - .trim(): copy with leading/trailing whitespace removed
  - .contains(str): whether the substring occurs
  - .substring(start[, end]): slice by character index; negative indices count from the end; end is exclusive
- Lists
  - .length (property): number of elements
  - .contains(value): true if any element equals the value
  - .get(index, default): element at index (negative allowed); returns default if out-of-bounds
  - .join(sep): join elements by sep into a string (elements are stringified)
- Dicts
  - .length (property): number of entries
  - .keys(): list of keys (strings)
  - .values(): list of values
  - .contains(keyStr): whether a key exists
  - .get(keyStr, default): value for key or default if missing

Notes on member access and calls
- Member access works on strings, lists, and dicts to reach the properties/methods listed above. It does not retrieve arbitrary dict entries; use indexing: dict["field"] to read a value by key.
- Calls work on any expression that evaluates to a function value. For example, a function stored in a dict can be invoked as `obj["func"](1,2)` after indexing.

Examples
- `[true][0] => true`
- `{"ab": 1, "cd": 2}["a" + "b"] => 1`
- `"abcd".length => 4`
- `"abcd".toUpper() => ABCD`
- `"abcd".substring(1, 2) => b`
- `["a", "b", "c"].join(",") => a,b,c`
- `{"a": 1, "b": 2}.get("c", "blah") => blah`

