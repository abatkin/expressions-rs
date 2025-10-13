use crate::parser::parse;
use crate::types::error::{Error, Result};
use crate::types::expression::{BinaryOp, Expr, UnaryOp};
use crate::types::list;
use crate::types::value::{Primitive, Value};

pub trait VariableResolver {
    fn resolve(&self, name: &str) -> Option<Value>;
}

pub struct Evaluator<R: VariableResolver> {
    pub resolver: R,
}

impl<R: VariableResolver> Evaluator<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }

    // Evaluate a string with ${...} interpolations
    pub fn evaluate_interpolated(&self, input: &str) -> Result<String> {
        let mut out = String::new();
        let mut rest = input;
        while let Some(idx) = rest.find("${") {
            // copy literal part before the interpolation
            out.push_str(&rest[..idx]);
            let after = &rest[idx + 2..];
            let (expr, consumed) = crate::parser::parse_in_braces(after)?;
            let val = self.evaluate(&expr)?;
            out.push_str(&val.as_str_lossy());
            rest = &after[consumed..];
        }
        // copy the remainder
        out.push_str(rest);
        Ok(out)
    }

    pub fn evaluate_string(&self, input: &str) -> Result<Value> {
        let expr = parse(input)?;
        let result = self.evaluate(&expr).map_err(|e| Error::EvaluationFailed(format!("evaluation error: {}", e)))?;
        Ok(result)
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Literal(p) => Ok(Value::Primitive(p.clone())),
            Expr::Var(name) => self.eval_var(name),
            Expr::ListLiteral(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for e in items {
                    vals.push(self.evaluate(e)?);
                }
                Ok(list::new(vals))
            }
            Expr::DictLiteral(pairs) => {
                let mut map = std::collections::BTreeMap::new();
                for (k_expr, v_expr) in pairs {
                    // evaluate key first, then value, left-to-right
                    let key_v = self.evaluate(k_expr)?;
                    let key_s = if let Value::Primitive(Primitive::Str(s)) = key_v {
                        s
                    } else {
                        return Err(Error::TypeMismatch("dict key must be a string".into()));
                    };
                    let v = self.evaluate(v_expr)?;
                    // duplicates allowed: last wins
                    map.insert(key_s, v);
                }
                Ok(Value::Dict(map))
            }
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Member { object, field } => {
                let obj = self.evaluate(object)?;
                obj.get_member(field)
            }
            Expr::Index { object, index } => {
                let obj_v = self.evaluate(object)?;
                match obj_v {
                    Value::Dict(m) => {
                        let idx_v = self.evaluate(index)?;
                        if let Value::Primitive(Primitive::Str(s)) = idx_v {
                            m.get(&s).cloned().ok_or(Error::NoSuchKey(s))
                        } else {
                            Err(Error::WrongIndexType {
                                target: "dict",
                                message: "expected string key".into(),
                            })
                        }
                    }
                    Value::Object(obj) => {
                        let idx_v = self.evaluate(index)?;
                        if let Value::Primitive(Primitive::Int(i)) = idx_v {
                            obj.get_index(i)
                        } else if let Value::Primitive(Primitive::Str(s)) = idx_v {
                            obj.get_key_value(&s)
                        } else {
                            Err(Error::WrongIndexType {
                                target: "object",
                                message: "expected int or string index".into(),
                            })
                        }
                    }
                    other => {
                        let t = match other {
                            Value::Primitive(Primitive::Int(_)) | Value::Primitive(Primitive::Float(_)) => "number",
                            Value::Primitive(Primitive::Str(_)) => "string",
                            Value::Primitive(Primitive::Bool(_)) => "bool",
                            Value::Func(_) => "func",
                            Value::Dict(_) => "dict",
                            Value::Object(obj) => obj.type_name(),
                        };
                        Err(Error::NotIndexable(t.into()))
                    }
                }
            }
            Expr::Unary { op, expr } => {
                let v = self.evaluate(expr)?;
                match op {
                    UnaryOp::Not => {
                        let b = v.coerce_bool().ok_or(Error::TypeMismatch("'!' expects bool".into()))?;
                        Ok(Value::Primitive(Primitive::Bool(!b)))
                    }
                }
            }
            Expr::Binary { op, left, right } => self.eval_binary(*op, left, right),
        }
    }

    fn eval_var(&self, name: &str) -> Result<Value> {
        match self.resolver.resolve(name) {
            Some(v) => Ok(v),
            None => Err(Error::ResolveFailed(name.to_string())),
        }
    }

    fn eval_call(&self, callee: &Expr, args: &Vec<Expr>) -> Result<Value> {
        let callee_v = self.evaluate(callee)?;
        match callee_v {
            Value::Func(f) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.evaluate(a)?);
                }
                (f)(&vals)
            }
            Value::Object(obj) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.evaluate(a)?);
                }
                obj.call(&vals)
            }
            _ => Err(Error::NotCallable),
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value> {
        use BinaryOp::*;
        match op {
            Or => {
                let l = self.evaluate(left)?;
                let lb = l.coerce_bool().ok_or(Error::TypeMismatch("'||' expects bools".into()))?;
                if lb {
                    return Ok(Value::Primitive(Primitive::Bool(true)));
                }
                let rb = self.evaluate(right)?.coerce_bool().ok_or(Error::TypeMismatch("'&&' expects bools".into()))?;
                Ok(Value::Primitive(Primitive::Bool(lb || rb)))
            }
            And => {
                let l = self.evaluate(left)?;
                let lb = l.coerce_bool().ok_or(Error::TypeMismatch("'&&' expects bools".into()))?;
                if !lb {
                    return Ok(Value::Primitive(Primitive::Bool(false)));
                }
                let rb = self.evaluate(right)?.coerce_bool().ok_or(Error::TypeMismatch("'&&' expects bools".into()))?;
                Ok(Value::Primitive(Primitive::Bool(lb && rb)))
            }
            Eq => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                Ok(Value::Primitive(Primitive::Bool(l == r)))
            }
            Ne => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                Ok(Value::Primitive(Primitive::Bool(l != r)))
            }
            Lt | Le | Gt | Ge => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                // numeric or string comparisons
                if let (Some(a), Some(b)) = (l.to_float_lossy(), r.to_float_lossy()) {
                    let res = match op {
                        Lt => a < b,
                        Le => a <= b,
                        Gt => a > b,
                        Ge => a >= b,
                        _ => unreachable!(),
                    };
                    return Ok(Value::Primitive(Primitive::Bool(res)));
                }
                if let (Value::Primitive(Primitive::Str(a)), Value::Primitive(Primitive::Str(b))) = (&l, &r) {
                    let res = match op {
                        Lt => a < b,
                        Le => a <= b,
                        Gt => a > b,
                        Ge => a >= b,
                        _ => unreachable!(),
                    };
                    return Ok(Value::Primitive(Primitive::Bool(res)));
                }
                Err(Error::TypeMismatch("comparison requires two numbers or two strings".into()))
            }
            Add => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                match (&l, &r) {
                    (Value::Primitive(Primitive::Int(a)), Value::Primitive(Primitive::Int(b))) => Ok(Value::Primitive(Primitive::Int(a + b))),
                    _ => {
                        let (af, bf) = (l.to_float_lossy(), r.to_float_lossy());
                        if let (Some(af), Some(bf)) = (af, bf) {
                            Ok(Value::Primitive(Primitive::Float(af + bf)))
                        } else if let (Value::Primitive(Primitive::Str(as_)), Value::Primitive(Primitive::Str(bs_))) = (&l, &r) {
                            Ok(Value::Primitive(Primitive::Str(format!("{}{}", as_, bs_))))
                        } else {
                            Err(Error::TypeMismatch("'+' expects numbers or strings".into()))
                        }
                    }
                }
            }
            Sub | Mul | Div | Mod | Pow => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                // Preserve integers for Sub, Mul, Mod if both ints
                match (op, &l, &r) {
                    (BinaryOp::Sub, Value::Primitive(Primitive::Int(a)), Value::Primitive(Primitive::Int(b))) => return Ok(Value::Primitive(Primitive::Int(a - b))),
                    (BinaryOp::Mul, Value::Primitive(Primitive::Int(a)), Value::Primitive(Primitive::Int(b))) => return Ok(Value::Primitive(Primitive::Int(a * b))),
                    (BinaryOp::Mod, Value::Primitive(Primitive::Int(_)), Value::Primitive(Primitive::Int(b))) if *b == 0 => return Err(Error::DivideByZero),
                    (BinaryOp::Mod, Value::Primitive(Primitive::Int(a)), Value::Primitive(Primitive::Int(b))) => return Ok(Value::Primitive(Primitive::Int(a % b))),
                    _ => {}
                }
                let (af, bf) = (l.to_float_lossy(), r.to_float_lossy());
                if let (Some(a), Some(b)) = (af, bf) {
                    let res = match op {
                        Sub => a - b,
                        Mul => a * b,
                        Div => {
                            if b == 0.0 {
                                return Err(Error::DivideByZero);
                            }
                            a / b
                        }
                        Mod => {
                            if b == 0.0 {
                                return Err(Error::DivideByZero);
                            }
                            a % b
                        }
                        Pow => a.powf(b),
                        _ => unreachable!(),
                    };
                    Ok(Value::Primitive(Primitive::Float(res)))
                } else {
                    Err(Error::TypeMismatch("arithmetic expects numbers".into()))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use std::rc::Rc;

    use crate::parser::parse;
    use crate::types::value::CustomObject;
    use crate::types::value::Value::Object;

    struct MockResolver;
    impl MockResolver {
        fn new() -> Self {
            Self
        }
    }
    impl VariableResolver for MockResolver {
        fn resolve(&self, key: &str) -> Option<Value> {
            if key == "x" {
                return Some(Value::from(10i64));
            }
            if key == "truth" {
                return Some(Value::from(true));
            }
            if key == "math.add" || key == "add" {
                let f = Rc::new(|args: &[Value]| -> Result<Value> {
                    if args.len() != 2 {
                        return Err(Error::EvaluationFailed("need 2 args".into()));
                    }
                    let a = args[0].to_float_lossy().ok_or(Error::TypeMismatch("number".into()))?;
                    let b = args[1].to_float_lossy().ok_or(Error::TypeMismatch("number".into()))?;
                    Ok(Value::from(a + b))
                });
                return Some(Value::Func(f));
            }
            if key == "global" {
                return Some(Object(Rc::new(MockGlobal {})));
            }
            None
        }
    }

    struct MockGlobal;

    impl CustomObject for MockGlobal {
        fn type_name(&self) -> &'static str {
            "global"
        }

        fn get_member(&self, name: &str) -> Result<Value> {
            match name {
                "a" => Ok(Value::Primitive(Primitive::Str("a".to_string()))),
                "fun" => Ok(Value::Func(Rc::new(|_args: &[Value]| -> Result<Value> { Ok(Value::Primitive(Primitive::Str("yes".to_string()))) }))),
                _ => Err(Error::ResolveFailed(name.to_string())),
            }
        }

        fn get_index(&self, index: i64) -> Result<Value> {
            if index == 0 {
                Ok(Value::Primitive(Primitive::Str("zero".to_string())))
            } else {
                Err(Error::IndexOutOfBounds { index, len: 1 })
            }
        }

        fn get_key_value(&self, key: &str) -> Result<Value> {
            if key == "k" {
                Ok(Value::Primitive(Primitive::Str("v".to_string())))
            } else {
                Err(Error::ResolveFailed(key.to_string()))
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn eval_basic_expressions() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("1").unwrap()).unwrap(), Value::from(1i64));
        assert_eq!(ev.evaluate(&parse("1").unwrap()).unwrap().to_string(), "1");
        assert_eq!(ev.evaluate(&parse("true").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parse("true || false").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parse("true && false").unwrap()).unwrap(), Value::from(false));
    }

    #[test]
    fn eval_literals_and_ops() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("1 + 2 * 3").unwrap()).unwrap(), Value::from(7i64));
        assert_eq!(ev.evaluate(&parse("true && !false").unwrap()).unwrap(), Value::from(true));
        match ev.evaluate(&parse("1/0").unwrap()) {
            Err(Error::DivideByZero) => (),
            other => panic!("expected div by zero, got {:?}", other),
        }
    }

    #[test]
    fn eval_paths_and_calls() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("x").unwrap()).unwrap(), Value::from(10i64));
        assert_eq!(ev.evaluate(&parse("truth || false").unwrap()).unwrap(), Value::from(true));
        let v = ev.evaluate(&parse("add(2, 3)").unwrap()).unwrap();
        match v {
            Value::Primitive(Primitive::Float(f)) => assert!((f - 5.0).abs() < 1e-9),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn eval_from_file_cases() {
        // Load test cases file at compile time
        const CASES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/evaluator_cases.txt"));
        let ev = Evaluator::new(MockResolver::new());
        for (idx, raw_line) in CASES.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, "=>").collect();
            assert_eq!(parts.len(), 2, "Invalid test case format on line {}: '{}'", line_no, raw_line);
            let expr_src = parts[0].trim();
            let expected_str = parts[1].trim();

            let expr = parse(expr_src).expect(&format!("Failed to parse expression on line {}: '{}'", line_no, expr_src));
            let actual_val = ev.evaluate(&expr).expect(&format!("Evaluation failed on line {} for expr '{}': parsed: {:?}", line_no, expr_src, expr));
            let actual_str = actual_val.to_string();

            assert_eq!(actual_str, expected_str, "Mismatch on line {} for expr '{}': got '{}', expected '{}'", line_no, expr_src, actual_str, expected_str);
        }
    }

    #[test]
    fn eval_interpolated_strings() {
        let ev = Evaluator::new(MockResolver::new());

        // sanity: parser parse_in_braces works on simple input
        assert!(crate::parser::parse_in_braces("1 + 2}").is_ok());
        // simple literal with expression
        let src = "Hello ${1 + 2}";
        let idx = src.find("${").unwrap();
        let after = &src[idx + 2..];
        assert_eq!(after, "1 + 2}");
        assert!(crate::parser::parse_in_braces(after).is_ok());
        let s = ev.evaluate_interpolated(src).unwrap();
        assert_eq!(s, "Hello 3");

        // variable path
        let s2 = ev.evaluate_interpolated("x is ${x}").unwrap();
        assert_eq!(s2, "x is 10");

        // multiple interpolations
        let s3 = ev.evaluate_interpolated("${'A'}-${add(2,3)}-${truth}").unwrap();
        assert_eq!(s3, "A-5-true");

        // ensure braces inside strings are handled
        let s4 = ev.evaluate_interpolated("${'curly } brace'} done").unwrap();
        assert_eq!(s4, "curly } brace done");

        // missing closing brace should error
        match ev.evaluate_interpolated("bad ${1+2") {
            Err(Error::ParseFailed(_, _)) => (),
            other => panic!("expected parse error, got {:?}", other),
        }
    }

    #[test]
    fn eval_lists_and_indexing() {
        let ev = Evaluator::new(MockResolver::new());
        // [10, 20, 30][1] => 20
        assert_eq!(ev.evaluate(&parse("[10, 20, 30][1]").unwrap()).unwrap(), Value::from(20i64));
        // [10][1] => IndexOutOfBounds
        match ev.evaluate(&parse("[10][1]").unwrap()) {
            Err(Error::IndexOutOfBounds { index, len }) => {
                assert_eq!(index, 1);
                assert_eq!(len, 1);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
        // [10]["0"] => WrongIndexType
        match ev.evaluate(&parse("[10][\"0\"]").unwrap()) {
            Err(Error::NotIndexable(idx)) => assert_eq!(idx, "0"),
            other => panic!("expected NotIndexable(0), got {:?}", other),
        }
        // negative indices
        assert_eq!(ev.evaluate(&parse("[10, 20, 30][-1]").unwrap()).unwrap(), Value::from(30i64));
        assert_eq!(ev.evaluate(&parse("[10, 20, 30][-3]").unwrap()).unwrap(), Value::from(10i64));
        match ev.evaluate(&parse("[10, 20, 30][-4]").unwrap()) {
            Err(Error::IndexOutOfBounds { index, len }) => {
                assert_eq!(index, -4);
                assert_eq!(len, 3);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
    }

    #[test]
    fn eval_dict_and_member() {
        let ev = Evaluator::new(MockResolver::new());
        // Dict via [key]
        assert_eq!(ev.evaluate(&parse("{\"a\": 1, \"b\": 2}[\"b\"]").unwrap()).unwrap(), Value::from(2i64));
        match ev.evaluate(&parse("{\"a\": 1}[\"z\"]").unwrap()) {
            Err(Error::NoSuchKey(k)) => assert_eq!(k, "z"),
            other => panic!("expected NoSuchKey, got {:?}", other),
        }
        match ev.evaluate(&parse("{\"a\": 1}[0]").unwrap()) {
            Err(Error::WrongIndexType { target, .. }) => assert_eq!(target, "dict"),
            other => panic!("expected WrongIndexType(dict), got {:?}", other),
        }
        // Members: properties and methods
        // string.length property
        assert_eq!(ev.evaluate(&parse("'abc'.length").unwrap()).unwrap(), Value::from(3i64));
        // string methods
        assert_eq!(ev.evaluate(&parse("'ab'.toUpper()").unwrap()).unwrap().to_string(), "AB");
        assert_eq!(ev.evaluate(&parse("' Ab '.trim().length").unwrap()).unwrap(), Value::from(2i64));
        // list.length property
        assert_eq!(ev.evaluate(&parse("[1,2,3].length").unwrap()).unwrap(), Value::from(3i64));
        // dict.length property and keys()/values()
        assert_eq!(ev.evaluate(&parse("{\"a\":1, \"b\":2}.length").unwrap()).unwrap(), Value::from(2i64));
        assert_eq!(ev.evaluate(&parse("{\"a\":1}.keys().length").unwrap()).unwrap(), Value::from(1i64));
        // errors: dict dot key is unknown member now
        match ev.evaluate(&parse("{\"a\": 1}.a").unwrap()) {
            Err(Error::UnknownMember { member, .. }) => assert_eq!(member, "a"),
            other => panic!("expected UnknownMember, got {:?}", other),
        }
        // errors: unknown member on list
        match ev.evaluate(&parse("[1].toUpper").unwrap()) {
            Err(Error::UnknownMember { member, .. }) => assert_eq!(member, "toUpper"),
            other => panic!("expected UnknownMember, got {:?}", other),
        }
        // calling non-call property is NotCallable
        match ev.evaluate(&parse("'abc'.length()").unwrap()) {
            Err(Error::NotCallable) => (),
            other => panic!("expected NotCallable, got {:?}", other),
        }
        // Nested
        assert_eq!(ev.evaluate(&parse("{\"xs\": [10, 20]}[\"xs\"][1]").unwrap()).unwrap(), Value::from(20i64));

        // Computed dict key in literal and runtime enforcement of key type
        assert_eq!(ev.evaluate(&parse("{\"a\" + \"b\": 1}[\"ab\"]").unwrap()).unwrap(), Value::from(1i64));
        match ev.evaluate(&parse("{1: 2}").unwrap()) {
            Err(Error::TypeMismatch(msg)) => assert_eq!(msg, "dict key must be a string"),
            other => panic!("expected TypeMismatch for dict key, got {:?}", other),
        }
    }

    #[test]
    fn eval_truthiness_lists_dicts() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("![]").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parse("!![]").unwrap()).unwrap(), Value::from(false));
        assert_eq!(ev.evaluate(&parse("![1]").unwrap()).unwrap(), Value::from(false));
        assert_eq!(ev.evaluate(&parse("!![1]").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parse("!{}").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parse("!!{\"a\":1}").unwrap()).unwrap(), Value::from(true));
    }
}
