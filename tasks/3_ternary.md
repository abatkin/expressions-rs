# Step 3 — Ternary operator

Goal: Add ternary conditional operator `cond ? then_expr : else_expr` to the expression grammar and evaluator with correct precedence and short-circuiting semantics. Expressions remain modular.

Key constraints from requirements:
- Truthiness is allowed for any type (0/0.0 falsy, "false" falsy, empty containers may be decided later; at minimum keep Atom rules and later extend to Value).
- Blocks are statement-only, so ternary is purely an expression.

Deliverables:
1) AST
- Add `Expr::Ternary { cond: Box<Expr>, then_br: Box<Expr>, else_br: Box<Expr> }`.

2) Parser
- Precedence: Place ternary between assignment (when added later) and logical-or.
  - With current layers, parse ternary right after the `or` level.
- Associativity: Right-associative: `a ? b : c ? d : e` → `a ? b : (c ? d : e)`.
- Grammar sketch with chumsky:
  - let logic = existing `or` parser
  - ternary = recursive(|self| logic.clone().then(just('?').ignore_then(self.clone()).then_ignore(just(':')).then(self.clone()).or_not()))
    .map(|(c, rest)| match rest { Some((t, e)) => Expr::Ternary{cond: Box::new(c), then_br: Box::new(t), else_br: Box::new(e)}, None => c })

3) Evaluator
- Evaluate `cond` to a Value/Atom, compute truthiness, then evaluate only one branch (short-circuit):
  - If truthy → evaluate `then_br`
  - Else → evaluate `else_br`
- Result type is the same evaluation type used for expressions in the current step.

4) Tests
- Parsing: `x ? y : z`, nested ternaries.
- Short-circuit: ensure only chosen branch is evaluated (e.g., using a function call with side-effect in tests or a mock).
- Truthiness across types: `0 ? 1 : 2` → 2; `1 ? 3 : 4` → 3; `"false" ? 9 : 8` → 8.

5) Modularity
- Ternary must plug into the expression parser builder function without introducing statements.
