# Expressions

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
  - Dotted paths are supported to reference nested names: a.b.c. 
  - An identifier may be followed by a function call with parentheses and parameters: ns.func(arg1, arg2).
- Operators
  - Arithmetic: +, -, *, /, %, ^
  - Comparisons: <, <=, >, >=, ==, !=
  - Logical: &&, ||, and unary !
- String interpolation (library API)
  - When using the provided Evaluator, the helper evaluate_interpolated replaces ${ ... } segments with the value of the contained expression. The result is always a string.
