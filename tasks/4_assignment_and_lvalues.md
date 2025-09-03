# Step 4 — Assignment and LValues (statement-only)

Goal: Introduce assignment as statement-only, with support for variable creation on first use (no `let`) and assignment targets for variable, member, and index. Expressions remain modular. No closures or functions here yet.

Key constraints from requirements:
- No `let`: implicit variable creation on first assignment.
- Assignment is statement-only; `x = y` does not produce a value.
- Truthiness allowed globally (relevant later for control flow).
- Friendly errors for invalid lvalues or type mismatches.

Deliverables:
1) AST additions
- LValue:
  - Var(String)
  - Member { object: Box<Expr>, field: String }
  - Index { object: Box<Expr>, index: Box<Expr> }
- Stmt:
  - Assign { target: LValue, value: Expr }
  - ExprStmt(Expr) (to run function calls etc.)
- Keep blocks/statements minimal; a full statement layer will be added in step 5.

2) Parser
- Build an lvalue parser that reuses expression primary/postfix parsing for the left side, then restrict shape:
  - Accept: Var, Member(obj, field), Index(obj, idx)
  - Reject: Call as lvalue (error)
- Assignment statement grammar:
  - lvalue `=` expr statement_terminator
- Statement terminators:
  - Either `;` or newline termination. Implement a `stmt_sep` parser that accepts one or more of: `;` or a newline in the shared spacer.
- Provide a `statement_parser()` that can parse a list of statements as a block later, but in this step you can parse a single assignment or expression statement.
- Maintain an `expression_parser()` export for expression-only contexts.

3) Environment and evaluation
- Introduce Environment as a stack of scopes:
  - Vec<HashMap<String, Value>> with a global scope at index 0.
- Name lookup:
  - For assignment: if variable exists in any scope, update the nearest; else create in current (innermost) scope.
  - For expression Var: read by searching innermost to outermost; friendly error if undefined.
- LValue evaluation for write:
  - Var: assign directly to env.
  - Member: evaluate object to a Dict, clone-or-mutate in place (if you keep Rc/RefCell later; for now, if values are owned, you may need to keep values by reference inside env or model Env as Value graph that supports mutation). For this step, it is acceptable to mutate via interior mutability cell types on List/Dict wrappers.
  - Index: similar; List index must be in range integer; Dict index must be string.
- Friendly errors when assigning into non-dict/non-list via Member/Index or index out of bounds, wrong index type, etc.

4) Tests
- `x = 1` followed by using x
- `user = {"name": "A"}` then `user.name = "B"`
- `arr = [0, 1]; arr[1] = 42;`
- Errors:
  - `3 = x` invalid lvalue
  - `arr[10] = 1` index out of bounds

5) Modularity notes
- Keep expression parser independent. Statement parser should consume expressions and lvalues using shared building blocks.
- Assignment remains statement-only; do not allow `a = b` inside expression contexts.
