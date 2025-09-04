# Step 6 — Loops: C-style for and foreach (list and dict)

Goal: Add `for` loops in two forms:
- C-style: `for (init?; cond?; post?) block` (desugar to while)
- Foreach over list and dict:
  - `for x in list { ... }`
  - `for (k, v) in dict { ... }` (iterate key-value pairs)

Key constraints from requirements:
- Dict iteration yields key-value pairs; tuple/list destructuring on the left `(k, v)` is required for dicts.
- Truthiness semantics for `cond`.
- Newline termination permitted.

Deliverables:
1) AST
- Extend Stmt with:
  - ForC { init: Option<Box<Stmt>>, cond: Option<Expr>, post: Option<Box<Stmt>>, body: Block }
  - ForInList { var: String, iterable: Expr, body: Block }
  - ForInDict { key: String, val: String, iterable: Expr, body: Block }
- Alternatively, a unified `ForIn { pattern: ForPattern, iterable: Expr, body: Block }` where `ForPattern` is `Var(String)` or `Pair(String,String)`.

2) Parser
- C-style:
  - `for` `(` init_stmt_or_empty `;` cond_expr_or_empty `;` post_stmt_or_empty `)` block
  - `init` and `post` can be Assign or ExprStmt; reuse statement parser pieces.
- Foreach:
  - `for` ident `in` expr block  → list iteration
  - `for` `(` ident `,` ident `)` `in` expr block → dict iteration over pairs
- Respect statement separators/newlines inside init/post parsing if you also allow newlines in `for (...)` parts; simplest is require them inside parentheses with explicit `;`.

3) Evaluation/execution
- ForC: Desugar to:
  - Execute init (if any)
  - While truthy(cond or true if missing) {
      Execute body; if Continue → clear and continue; if Break → exit;
      Execute post (if any)
    }
- ForIn list:
  - Evaluate iterable to Value::List; iterate values by cloning or referencing; assign to `var` in current scope each iteration; execute body with break/continue semantics.
- ForIn dict:
  - Evaluate iterable to Value::Dict; iterate over (key, value) pairs; assign to `key` and `val` variables.
- Friendly errors when iterable is wrong type.

4) Tests
- ForC counting loop; ensure post runs each time.
- Foreach list sums values.
- Foreach dict prints keys/values or accumulates.
- Break and continue in both loop types.

5) Modularity
- Expression parser continues to be reusable; statements compose it.
