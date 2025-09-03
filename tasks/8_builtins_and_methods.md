# Step 8 — Built-ins and dot access policy

Goal: Provide a minimal set of built-in functions available in the global scope and clarify that dot `.` access is purely dict key traversal (not method dispatch). If you want to expose list/dict utilities, either export them as globals (e.g., `push(list, v)`) or place functions in dicts to make `obj.util(args)` work via traversal.

Key constraints from requirements:
- Everything accessible via `.` should be dict traversal. There is no hidden method dispatch.
- Dict ordering does not matter.

Deliverables:
1) Built-ins
- Provide a small stdlib in the root/global scope as a dict of functions:
  - print(...)
  - len(x)
  - type(x)
  - keys(dict), values(dict), items(dict) returning list of pairs
- Represent as Value::Func(Native(...)) entries in the global environment.

2) Dot access policy
- Document that `obj.field` retrieves the value under key `"field"`. If that value is a function, `obj.field(args)` calls it because calls work on any callee expression.
- If you want method-like sugar, you can organize values like: `list_methods = { "push": fn(l, x) { ... } }` and then `list_methods.push(my_list, 1)`; or attach such dict under `list` key in the environment. The runtime does not add implicit `this`.

3) Parser
- No changes; dot/member access is already supported from earlier steps.

4) Evaluator
- Implement native built-ins as host callbacks producing friendly errors on wrong types.

5) Tests
- Calling built-ins; dot traversal to reach functions stored inside dicts.

6) Documentation
- Update README with a section on built-ins and dot traversal semantics; emphasize no magic methods.
