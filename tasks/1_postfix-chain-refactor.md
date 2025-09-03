# Step 1 — Postfix chain refactor (expression core)

Goal: Replace the current special-cased dotted path + single-call handling with a unified, extensible postfix-chain for expressions: primary followed by any number of member access `.field`, index `[expr]`, and call `(args)` operations. This lays the foundation for lists/dicts, assignment targets, and functions while keeping the expression parser modular.

Key constraints from requirements:
- Expression parser must remain modular and usable stand-alone (there will be contexts with expressions only, no statements).
- Future features will add lists/dicts, ternary, assignment (statement-only), and statements/blocks. Do not introduce statements in this step.
- Dot methods are dictionary traversal, not method dispatch.

Deliverables in this step:
1) AST updates (minimally invasive, forward-compatible)
- Expr variants:
  - Keep: Basic(Atom), Unary, Binary.
  - Replace Path(Vec<String>) with Var(String) for a base identifier and add Member/Index/Call in a uniform way.
  - New/updated:
    - Var(String)
    - Member { object: Box<Expr>, field: String }
    - Index { object: Box<Expr>, index: Box<Expr> }
    - Call { callee: Box<Expr>, args: Vec<Expr> }
- Optional helper: an accessor to recognize a simple variable name quickly.

2) Parser changes (chumsky)
- Primary:
  - literals (existing)
  - identifiers → Expr::Var
  - parenthesized expression
- Postfix loop (left-assoc): repeatedly apply one of
  - Call: `(` args? `)`
  - Index: `[` expr `]`
  - Member: `.` ident
- Precedence: postfix > unary > pow (right-assoc) > mul/div/mod > add/sub > comparisons > equality > && > ||.
- Remove current special-cased `path`/`path_or_call`/`path_call_and_members` in favor of the generic postfix loop.
- Shared spacer retained; comments/whitespace as before.

3) Evaluator updates (temporary behavior)
- Name lookup currently uses VariableResolver::resolve on full paths. Update to:
  - Eval Var(name) → treat as a path with one segment for resolve([]) compatibility; or create a helper that resolves a flat name.
  - Eval Member/Index now should not try to flatten entire path into Vec<String>. Instead, do real traversal:
    - For this step, you may keep old functionality by supporting Member(object, field) only when `object` flattens to a Vec<String>. Keep a TODO to implement full value traversal in step 2.
  - Eval Call should accept any Expr callee, not only Path; similarly, keep a temporary path flattening as an interim solution.
- Keep existing Atom-only arithmetic semantics unchanged in this step.

4) Public API
- Maintain parse(input) -> Result<Expr> with the new AST shape.
- Keep a separate entry point to obtain the expression parser (e.g., parser::parser()) suitable for embedding.

5) Tests (unit or doc tests)
- Parsing:
  - `a.b.c` → Member(Member(Var("a"), "b"), "c")
  - `a.b(1, 2).c[0].d(e)` → correct postfix chain shape
  - `(a + b).c(d)` is allowed and parsed as postfix applied to parenthesized expr
  - `foo(1)(2)(3)` chains calls
  - `arr[1+2][0]`
- Precedence:
  - `a.b + c.d` parses as Binary(Add, postfix(a.b), postfix(c.d))
  - `a.b(c) && d.e` groups correctly
- Evaluator temp behavior:
  - Existing path/call tests still pass; adapt resolver usage minimally. Where necessary, resolve Var or flatten member chain into a Vec<String> as before.

6) Migration notes
- Mark flatten_member_path as deprecated/TODO for removal in step 2 when full value traversal (dict/list) is implemented.
- Update any examples in README to reflect the new more general postfix support if present.

Implementation outline (parser):
- ident = text::ident().map(String::from)
- primary = choice(literal, ident→Var, parens(expr)) .padded_by(spacer)
- postfix = primary.then(
    choice(
      args_list delimited by ( ) → |args| Postfix::Call(args),
      expr delimited by [ ] → Postfix::Index(expr),
      dot ident → Postfix::Member(name)
    ).repeated()
  ).map(|(base, posts)| apply sequentially)
- Replace references to Path/PathOrCall in higher-precedence levels with postfix.

Notes on modularity:
- Keep expr parser constructor function exported (e.g., parser::expression_parser()) to reuse in statements later.
- Do not introduce statements, semicolons, or assignments here to avoid churn.
