# Simple Expressions

A small expression language with variables, function calls, simple types and common operators.

- Literals
  - Integers: a sequence of digits, which must fit in a 64-bit signed integer. A literal never includes a sign: `-5` is unary minus applied to `5`. One consequence is that the most negative integer, -9223372036854775808, cannot be written as a literal, because 9223372036854775808 is out of range.
  - Floats: digits with a decimal point and digits on both sides (`1.5`), a leading point (`.5`), or an exponent (`1e10`, `1.5e-3`, `2E+4`). `1.` is not a float. As with integers, a leading '-' is unary minus.
  - Strings: delimited by single ' or double " quotes. The only escapes are `\n` (newline), `\\` (backslash), and the string's own quote character (`\'` in a single-quoted string, `\"` in a double-quoted one). Any other escape, including `\r`, `\t`, and the other kind of quote, is a parse error. Raw line breaks are allowed inside strings and are kept as written.
  - Booleans: true, false.
- Collections
  - Lists: [expr, expr, ...]
    - Example: [1, 2, 3], ["a", 1+2]
  - Dictionaries (maps): { key_expr: value_expr, ... }
    - Keys can be any expression, but at runtime must evaluate to strings; duplicate keys are allowed, last one wins.
    - Example: {"a": 1, "b": 2}, {"a"+"b": 3}
- Whitespace and comments
  - Any spaces, tabs, or newlines are ignored.
  - Line comments start with // and continue to the end of the line. Inside `${ ... }` a comment also ends at the first `}`, which closes the interpolation, so a comment there cannot contain a `}`. In

    ```
    ${ x + // add {y} later
       1 }
    ```

    the `}` after `{y` closes the interpolation, leaving `x +` with nothing after it: a parse error.
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
  - `evaluate_interpolations(...)` replaces each `${ ... }` with the value of the contained expression, rendered with `Display`. Example: "Hello ${1 + 2}" yields "Hello 3". A missing closing '}' is a parse error.
  - `$${` produces a literal `${`, so "$${x}" yields "${x}" (before 0.5.0 it yielded `$` followed by the value of `x`). Any other `$` is literal text.
  - `Template::parse(...)` parses once and reports syntax errors up front; the template can then be evaluated many times. `evaluate_rendered(...)` takes a closure that turns each value into text, e.g. to reject lists.
  - Evaluation and rendering failures are `Error::InterpolationFailed`, which says which `${ ... }` failed.
- Nesting limit (library API)
  - An expression may nest at most 64 levels deep. A literal or variable is one level, and each operator, unary `-` or `!`, `.field`, `[index]`, call, list, dictionary, and pair of parentheses adds one. Chained operators count too: `a + b + c` is `(a + b) + c`, three levels.
  - Deeper input is a parse error rather than a stack overflow that aborts the process. pest matches brackets before the limit is checked, though, so for brackets nested far past it the error comes from pest's own stack guard ("stack limit reached"). That guard is automatic only on the x86_64 and aarch64 `-unknown-linux-gnu`, `-apple-darwin` and `-pc-windows-msvc` targets. Elsewhere, including musl Linux and windows-gnu, enable this crate's `stack-guard` feature or bound the input's length.
  - Change the limit with `ParseOptions::default().with_max_depth(n)`, passed to `parse_expression_with(...)` (then evaluate with `Evaluator`) or `Template::parse_with(...)`; in a template it applies to each `${ ... }` separately. Raise it only on threads with the stack to match: the default fits a 2 MiB thread even in a debug build.

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

