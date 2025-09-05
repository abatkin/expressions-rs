### Step 2 — Lists and dicts (literals and read-only access)

#### Goals
- Add list and dict literals to the expression language.
- Support read-only indexing and member access.
- Keep the expression parser modular and reusable (no API regressions for the public parser function).

#### Constraints
- Dict keys must be strings.
- Truthiness is uniform; for lists/dicts it is based on emptiness.
- Dot access is dictionary traversal: obj.field ≡ obj["field"] for dicts. Dot on non-dicts is a friendly error.
- Missing key or out-of-bounds index returns a friendly error; never panic.

---

### Grammar and AST

#### AST additions
- Expr::ListLiteral(Vec<Expr>)
- Expr::DictLiteral(Vec<(String, Expr)>)
  - Keys are string literals only (i.e. no need to accept variables or function calls even though they may return a string)

No other AST changes are required.

#### Parser updates
- Extend primary to include new literal forms so they chain naturally with postfix:
  - List literal
    - Syntax: [expr, expr, ...] (optional trailing comma).
    - Elements are full expressions.
  - Dict literal
    - Syntax: {"key": expr, "k2": expr, ...} (optional trailing comma).
    - Keys must be string literals (single/double quotes; use existing escape handling).
- Postfix operators remain unchanged and apply to these primaries:
  - Call: (args)
  - Index: [expr]
  - Member: .ident

Must parse successfully:
- [1, 2, 3]
- {"a": 1, "b": 2}
- {"xs": [10, 20]}["xs"][1]
- {"a": 1}.a
- {"a": [1,2,3]}.a[0]

Must fail with ParseFailed:
- {a: 1}
- {1: 2}
- {"a" 1} (missing colon)

---

### Evaluation semantics

#### Literal evaluation
- Expr::ListLiteral(items): evaluate items left-to-right → Ok(Value::List(Vec<Value>)).
- Expr::DictLiteral(pairs): evaluate values left-to-right (keys already strings) → Ok(Value::Dict(BTreeMap<String, Value>)).
  - BTreeMap implies key-sorted order

#### Indexing (Expr::Index { object, index })
Evaluate object first, then index.

For lists:
- Index must be an integer (Value::Primitive(Primitive::Int(i))).
- Negative indices are supported: i < 0 counts from end.
  - effective = len + i (i is negative).
  - If effective < 0 or effective >= len: Err(IndexOutOfBounds { index: i, len }).
- Non-integer index: Err(WrongIndexType { target: "list", message: "expected int index" }).

For dicts:
- Index must be a string (Value::Primitive(Primitive::Str(s))).
- Missing key: Err(NoSuchKey(s)).
- Non-string index: Err(WrongIndexType { target: "dict", message: "expected string key" }).

For other types:
- Err(NotIndexable(type_name)), where type_name is a friendly type string (e.g., "number", "bool", "func").

#### Member access (Expr::Member { object, field })
- Evaluate object.
- If object is a dict:
  - If key exists: return copy of value.
  - Else: Err(NoSuchKey(field)).
- Otherwise: Err(NotADict).

#### Truthiness and unary !
- Value::coerce_bool already returns Some(false) for empty list/dict, Some(true) for non-empty.
- Unary ! flips the boolean; non-coercible values produce TypeMismatch("'!' expects bool").
- Expectations to test:
  - ![] ⇒ true, !![] ⇒ false
  - ![1] ⇒ false, !![1] ⇒ true
  - !{} ⇒ true, !!{"a":1} ⇒ false

---

### Errors

Extend Error enum with these friendly variants and suggested Display messages:
- IndexOutOfBounds { index: i64, len: usize }
  - Message: index out of bounds: {index} (len: {len})
- WrongIndexType { target: &'static str, message: String }
  - Examples: { target: "list", message: "expected int index" }, { target: "dict", message: "expected string key" }
- NotADict
  - Message: not a dict
- NotIndexable(String)
  - Message: not indexable: {type_name}
- NoSuchKey(String)
  - Message: no such key: {key}

Keep existing variants (ResolveFailed, NotCallable, TypeMismatch, DivideByZero, EvaluationFailed, ParseFailed, Unsupported).

All errors must be returned via Result::Err; no panics.

---

### Display and Debug

- Display should show list/dict contents
  - Update Value::as_str_lossy() for lists/dicts to render contents, so Display (which relies on it) shows:
    - List: [elem1, elem2, ...]
    - Dict: {key1: value1, key2: value2} (BTreeMap order)
  - Note: Primitive::Str prints without quotes; thus ["a"] appears as [a]. Prefer structural assertions in tests when ambiguity matters.
- Debug for Value already shows contents; keep as-is.

---

### Tests

Add parser unit tests:
- List literals:
  - [1, 2, 3]
  - [1,2,] (trailing comma)
  - [1, [2], 3]
- Dict literals:
  - {"a": 1, "b": 2}
  - {"a": 1,} (trailing comma)
  - Reject non-string keys: {a: 1}, {1: 2}
- Postfix combinations:
  - {"xs": [10, 20]}["xs"][1]
  - {"a": 1}.a
  - Verify AST shape for chaining (Index(Index(DictLiteral, "xs"), 1); Member(DictLiteral, "a")).

Add evaluator unit tests:
- Lists:
  - [10, 20, 30][1] ⇒ 20
  - [10][1] ⇒ Err(IndexOutOfBounds { index: 1, len: 1 })
  - [10]["0"] ⇒ Err(WrongIndexType { target: "list", ... })
  - Negative indices:
    - [10, 20, 30][-1] ⇒ 30
    - [10, 20, 30][-3] ⇒ 10
    - [10, 20, 30][-4] ⇒ Err(IndexOutOfBounds { index: -4, len: 3 })
- Dict via [key]:
  - {"a": 1, "b": 2}["b"] ⇒ 2
  - {"a": 1}["z"] ⇒ Err(NoSuchKey("z"))
  - {"a": 1}[0] ⇒ Err(WrongIndexType { target: "dict", ... })
- Member sugar:
  - {"a": 1}.a ⇒ 1
  - {"a": 1}.z ⇒ Err(NoSuchKey("z"))
  - [1].len ⇒ Err(NotADict)
- Nested access:
  - {"xs": [10, 20]}["xs"][1] ⇒ 20
- Truthiness:
  - ![] ⇒ true; !![] ⇒ false
  - ![1] ⇒ false; !![1] ⇒ true
  - !{} ⇒ true; !!{"a":1} ⇒ false

Testing notes:
- Keep existing golden "to_string" tests; for lists/dicts, prefer structural assertions unless checking formatting.

---

### Modularity
- Maintain the public parser() function. Integrate new literal grammars inside expr_and_spacer() primary.
- Do not change external APIs beyond adding AST and Error variants.

---

### Implementation checklist
- [ ] Add Error variants: IndexOutOfBounds { index, len }, WrongIndexType { target, message }, NotADict, NotIndexable(String), NoSuchKey(String)
- [ ] Add Expr::ListLiteral and Expr::DictLiteral
- [ ] Parser: extend primary to parse list/dict literals with string-literal keys only
- [ ] Evaluator: implement new literals; implement Index (with negative indices) and Member per rules above
- [ ] Display: render list/dict contents in Value::as_str_lossy()
- [ ] Tests: add parser and evaluator tests as outlined
