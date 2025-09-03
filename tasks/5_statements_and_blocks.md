# Step 5 — Statements and blocks (if/else, while)

Goal: Introduce a statement layer with blocks and control flow: if/else, while, break/continue. Blocks are statement-only. Allow newline termination of statements (semicolons optional). Keep expression parser modular and reused.

Key constraints from requirements:
- Blocks are statement-only; no implicit last-expression value.
- Conditions use truthiness (not strictly Bool).
- Newline termination permitted; semicolons optional.
- Friendly errors for misuse.

Deliverables:
1) AST additions
- Stmt enum:
  - Assign { target: LValue, value: Expr } (from step 4)
  - ExprStmt(Expr)
  - If { cond: Expr, then_block: Block, else_block: Option<Block> }
  - While { cond: Expr, body: Block }
  - Break
  - Continue
- Block(Vec<Stmt>) and Program(Vec<Stmt>) as containers.

2) Parser
- Reuse expression parser.
- Whitespace/comments: retain the shared `spacer`; add a statement separator parser that recognizes `;` or newline(s).
- Blocks: `{` stmt* `}`; optional trailing statement separator.
- If/else syntax: `if (expr) block (else block)?`
- While syntax: `while (expr) block`
- Break/Continue: keywords; require statement separator or be followed by `}`.
- File/program parser: zero or more statements.

3) Evaluation/execution
- Introduce control signals: enum Control { None, Break, Continue } and later Return.
- While loop evaluates condition each iteration using truthiness. Handle break/continue by bubbling up a Control signal.
- If evaluates condition via truthiness; execute only one branch.
- Blocks execute statements sequentially; stop early on control signals.
- Keep environment as a stack of scopes; for now, no new scope for blocks unless desired. (Since there is no `let`, you can choose single global scope initially. If preparing for functions, blocks could push a new scope.)

4) Truthiness
- Extend truthiness to composite Value types where needed (e.g., non-empty lists/dicts are truthy). At minimum, support Atom truthiness as earlier and treat lists/dicts as truthy when non-empty.

5) Tests
- If/else with truthy/falsy conditions.
- While loop incrementing a counter; test break and continue.
- Newline termination vs semicolon: both should parse.

6) Modularity
- Keep `expression_parser()` usable on its own.
- Provide `program_parser()` and/or `block_parser()` for statement parsing.
