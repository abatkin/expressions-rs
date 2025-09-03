# Step 7 — Functions (definitions and calls)

Goal: Add user-defined functions with definitions and calls; no closures initially. Functions are first-class values and can be assigned to variables; nested functions are only via assignment of function values to variables. Implement `return` to exit early from functions.

Key constraints from requirements:
- Forbid closures initially (no capturing of outer locals). Globals/builtins are accessible.
- Functions assignable to variables; nested functions via assignment only.
- Assignment stays statement-only.

Deliverables:
1) AST
- Extend Stmt with:
  - FnDef { name: String, params: Vec<String>, body: Block }
  - Return(Option<Expr>)
- Extend Value with Value::Func(Function).
- Function representation:
  - enum Function { User { params: Vec<String>, body: Block }, Native(fn(&[Value]) -> Result<Value>) }

2) Parser
- Function definition: `fn name(params...) block`
  - Params: `ident (',' ident)*` with optional trailing comma.
- Calls are already part of postfix chain; they now accept any callee expression that evaluates to a function value.
- Return statement: `return` expr? statement_terminator

3) Evaluation
- Name binding: defining a function binds a Value::Func in the current scope.
- Calling a function:
  - Evaluate callee to Value::Func.
  - Evaluate args left-to-right.
  - Create a new call scope (push a new scope frame):
    - Bind parameters by value to arguments (arity must match; friendly error otherwise).
  - Execute function body block until completion or Return signal.
  - On `Return(Some expr))` produce the value of expr; `Return(None)` yields a Unit/None value (choose a Value::Unit or special Atom/Value variant).
  - Ensure no closures: disallow references to outer local variables from inside function body. The simplest is to only allow resolving names from the call scope and global scope; do not capture current locals when creating the function value.

4) Numeric behavior clean-up
- Int/Int division → Float; ensure evaluator enforces this globally now.

5) Tests
- Simple function returning a value.
- Recursive function like factorial.
- Wrong arity error.
- Function as a value assigned to a var and then called.
- Return without expression.

6) Modularity
- Expression parser already supports calls; statements now include FnDef and Return. Expression-only contexts remain unaffected.
