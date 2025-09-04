# Simple Expressions

A small expression language with variables, function calls, simple types and common operators

- Literals
  - Integers: sequence of digits, optionally with a leading '-'.
  - Floats: digits with a decimal point, optionally with a leading '-'.
  - Strings: delimited by single ' or double " quotes. Supported escapes: \n, \r, \t, \\, \", \\'. Newlines are not allowed inside strings unless escaped as a backslash followed by a newline.
  - Booleans: true, false.
- Whitespace and comments
  - Any spaces, tabs, or newlines are ignored.
  - Line comments start with // and continue to the end of the line.
- Grouping
  - Parentheses ( ... ) group sub-expressions.
- Identifiers, variables and functions
  - Identifiers follow the usual rules: start with a letter or underscore, then letters, digits, or underscores.
  - Postfix chaining after any primary expression:
    - Member access: .field (dictionary/object field lookup, no method dispatch)
    - Indexing: [expr]
    - Calls: (arg1, arg2)
    - These can be chained left-to-right: `a.b.c`, `a.b(1, 2).c[0].d(e)`, `foo(1)(2)(3)`, `arr[1+2][0]`.
- Operators
  - Arithmetic: +, -, *, /, %, ^
  - Comparisons: <, <=, >, >=, ==, !=
  - Logical: &&, ||, and unary !
- String interpolation (library API)
  - When using the provided Evaluator, the helper evaluate_interpolated replaces ${ ... } segments with the value of the contained expression. The result is always a string.
