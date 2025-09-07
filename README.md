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
  - Identifiers follow the usual rules: start with a letter or underscore, then letters, digits, or underscores.
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
- Truthiness (used by !, &&, ||)
  - Numbers: 0/0.0 is false; any other number is true.
  - Booleans: as-is.
  - Strings: only the literal strings "true" and "false" coerce to booleans; other strings are not allowed in logical ops.
  - Lists/Dicts: empty is false; non-empty is true.
  - Functions: not coercible to bool.
- String interpolation (library API)
  - When using the provided Evaluator, evaluate_interpolated replaces ${ ... } segments with the value of the contained expression. The result is always a string.
  - Example: evaluating "Hello ${1 + 2}" yields "Hello 3". Braces inside quoted strings are handled; a missing closing '}' is an error.

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

