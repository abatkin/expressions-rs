# Step 2 — Lists and dicts (literals and read-only access)

Goal: Introduce list and dict literals in the expression language and enable read-only indexing/member access on them. This prepares for later mutation and assignments. Keep expression parser modular.

Key constraints from requirements:
- Dict keys can only be strings (either identifiers mapped to strings later via semantics, or string literals; in this step prefer string literals to be explicit).
- Truthiness across all types will be used later; start shaping Value accordingly.
- Dot access is dictionary traversal; there is no method dispatch. `obj.field` means get the value for key "field" on a dict-like value. For non-dict values, this is an error (friendly error value).
- Missing key or out-of-bounds index should return a friendly error value, not panic.

Deliverables in this step:
1) Runtime types
- Introduce a top-level Value enum beyond Atom to represent composite runtime values:
  - Value::Atom(Atom)
  - Value::List(Vec<Value>)
  - Value::Dict(std::collections::BTreeMap<String, Value>) (order not required)
  - Value::Func(...) to be added later (stub now Optional)
- Alternatively, if you want to keep Atom-only evaluator now, you can implement parse-only parts and stub evaluator to return Error for any composite evaluation. However, implementing Value now will reduce churn later.

2) Parser additions
- List literal: `[expr, expr, ...]` with optional trailing comma.
- Dict literal: `{ "key": expr, "k2": expr, ... }` with optional trailing comma.
  - Only allow string literal keys in this step.
- Integrate list/dict literals as `primary` options so they naturally work with postfix chain: e.g., `{"a": [1,2]}["a"][0]`.

3) Expr variants
- Add Expr::ListLit(Vec<Expr>) and Expr::DictLit(Vec<(String, Expr)>)
- Keep Member/Index/Call from step 1.

4) Evaluator semantics (read-only)
- Evaluate:
  - ListLit → Value::List of evaluated elements
  - DictLit → Value::Dict of evaluated values
  - Index:
    - If object is List and index is an Atom int within bounds, yield element; else friendly error Value (or Error type if you keep Result<Value>)
    - If object is Dict and index is a string (Atom::Str), return value if present; else friendly error
  - Member:
    - If object is Dict, treat as string-key access; same as Index with a string key
    - Otherwise, friendly error
- For this step, Binary/Unary arithmetic may still operate only on Atom operands. If either side is non-Atom Value, return a friendly error.

5) API shape
- Consider splitting evaluation into two layers:
  - evaluate_expr_to_value(&Expr) -> Result<Value>
  - if original API returns Atom, keep evaluate_string using a projection for pure-Atom results and return errors for composite cases until later steps extend capabilities.

6) Tests
- Parsing:
  - `[1, 2, 3]`
  - `{"a": 1, "b": 2}`
  - Nesting and postfix: `{"xs": [10, 20]}["xs"][1]`
  - Member sugar: `{"a": 1}.a`
- Evaluation:
  - List indexing in-range/out-of-range → value / friendly error
  - Dict lookup missing key → friendly error
  - Dict member access equals string-key lookup

7) Friendly error policy
- Define a specific Error variant(s), e.g.:
  - Error::IndexOutOfBounds(idx, len)
  - Error::WrongIndexType(ty)
  - Error::NotADict
  - Error::NoSuchKey(key)
- Ensure all such errors are reported without panics.

8) Modularity note
- Keep expression parser exportable as a function (e.g., expression_parser()) to be reused by statement parser later.
