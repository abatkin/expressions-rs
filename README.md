# Expressions

This is a simple expression language.

- Strings delimited by double or single quotes, quotes, newlines and backslashes are escaped with a backslash.
- Numbers without a decimal point are integers, with a decimal point they are floats.
- Boolean values are true and false.
- Comments start with a // and continue to the end of the line.
- Parentheses are used to group expressions.
- Operators are: +, -, *, /, %, ^, <, >, <=, >=, ==, !=, &&, ||, !
- Variables can be referenced by name (letter followed by letters, numbers, or underscores)
- Functions can be called by name (letter followed by letters, numbers, or underscores) followed by parentheses, with any arguments contained within
- Identifiers (variables and functions) may be part of a complex object with components separated by periods (i.e. a.b.c)