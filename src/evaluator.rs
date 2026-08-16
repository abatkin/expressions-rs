use crate::parser;
use crate::types::coerce::{Coercions, Context, Number, STANDARD};
use crate::types::error::{Error, Result};
use crate::types::expression::{BinaryOp, Expr, UnaryOp};
use crate::types::primitive::Primitive;
use crate::types::value::Value;
use crate::types::{dict, list};

pub fn evaluate<T: VariableResolver>(input: &str, resolver: &T) -> Result<Value> {
    evaluate_with(input, resolver, &STANDARD)
}

/// Like [`evaluate`], with an explicit coercion policy.
pub fn evaluate_with<T: VariableResolver>(input: &str, resolver: &T, coercions: &dyn Coercions) -> Result<Value> {
    let expr = parser::parse_expression(input)?;
    let evaluator = Evaluator::new_with_coercions(resolver, coercions);
    let result = evaluator.evaluate(&expr).map_err(|e| Error::EvaluationFailed(format!("evaluation error: {}", e)))?;
    Ok(result)
}

pub fn evaluate_interpolations<T: VariableResolver>(input: &str, resolver: &T) -> Result<String> {
    evaluate_interpolations_with(input, resolver, &STANDARD)
}

/// Like [`evaluate_interpolations`], with an explicit coercion policy.
pub fn evaluate_interpolations_with<T: VariableResolver>(input: &str, resolver: &T, coercions: &dyn Coercions) -> Result<String> {
    let mut out = String::new();
    let mut rest = input;
    while let Some(idx) = rest.find("${") {
        // copy literal part before the interpolation
        out.push_str(&rest[..idx]);
        let after = &rest[idx + 2..];
        let (expr, consumed) = parser::parse_internal(after, parser::Rule::delimited_expr)?;
        let evaluator = Evaluator::new_with_coercions(resolver, coercions);
        let result = evaluator.evaluate(&expr).map_err(|e| Error::EvaluationFailed(format!("evaluation error: {}", e)))?;
        let result_str = result.to_string();
        out.push_str(result_str.as_str());
        rest = &after[consumed..];
    }
    // copy the remainder
    out.push_str(rest);
    Ok(out)
}

pub trait VariableResolver {
    fn resolve(&self, name: &str) -> Option<Value>;
}

pub struct Evaluator<'a, R: VariableResolver> {
    resolver: &'a R,
    context: Context<'a>,
}

impl<'a, R: VariableResolver> Evaluator<'a, R> {
    pub fn new(resolver: &'a R) -> Self {
        Self {
            resolver,
            context: Context::new(&STANDARD),
        }
    }

    pub fn new_with_coercions(resolver: &'a R, coercions: &'a dyn Coercions) -> Self {
        Self {
            resolver,
            context: Context::new(coercions),
        }
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
                Ok(dict::new(map))
            }
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Member { object, field } => {
                let obj = self.evaluate(object)?;
                obj.get_member(field)
            }
            Expr::Index { object, index } => {
                let obj_v = self.evaluate(object)?;
                match obj_v {
                    Value::Object(obj) => {
                        let idx_v = self.evaluate(index)?;
                        if let Value::Primitive(Primitive::Int(i)) = idx_v {
                            obj.get_index(i)
                        } else if let Value::Primitive(Primitive::Str(s)) = idx_v {
                            obj.get_key_value(&s)
                        } else {
                            Err(Error::NotIndexable(idx_v.as_str_lossy()))
                        }
                    }
                    other => {
                        let t = match other {
                            Value::Primitive(Primitive::Int(_)) | Value::Primitive(Primitive::Float(_)) => "number",
                            Value::Primitive(Primitive::Str(_)) => "string",
                            Value::Primitive(Primitive::Bool(_)) => "bool",
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
                        let b = self.context.to_bool(&v)?;
                        Ok(Value::Primitive(Primitive::Bool(!b)))
                    }
                    UnaryOp::Neg => {
                        let v = self.evaluate(expr)?;
                        match v {
                            Value::Primitive(Primitive::Int(i)) => Ok(Value::Primitive(Primitive::Int(-i))),
                            Value::Primitive(Primitive::Float(f)) => Ok(Value::Primitive(Primitive::Float(-f))),
                            _ => Err(Error::TypeMismatch("'-' expects number".into())),
                        }
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
            Value::Object(obj) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.evaluate(a)?);
                }
                obj.call(&vals, &self.context)
            }
            _ => Err(Error::NotCallable),
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value> {
        use BinaryOp::*;
        match op {
            Or => {
                let l = self.evaluate(left)?;
                let lb = self.context.to_bool(&l)?;
                if lb {
                    return Ok(Value::Primitive(Primitive::Bool(true)));
                }
                let r = self.evaluate(right)?;
                let rb = self.context.to_bool(&r)?;
                Ok(Value::Primitive(Primitive::Bool(lb || rb)))
            }
            And => {
                let l = self.evaluate(left)?;
                let lb = self.context.to_bool(&l)?;
                if !lb {
                    return Ok(Value::Primitive(Primitive::Bool(false)));
                }
                let r = self.evaluate(right)?;
                let rb = self.context.to_bool(&r)?;
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
                // two integers compare exactly: i64 -> f64 loses precision above 2^53,
                // which otherwise makes '<' disagree with '=='
                if let (Value::Primitive(Primitive::Int(a)), Value::Primitive(Primitive::Int(b))) = (&l, &r) {
                    let res = match op {
                        Lt => a < b,
                        Le => a <= b,
                        Gt => a > b,
                        Ge => a >= b,
                        _ => unreachable!(),
                    };
                    return Ok(Value::Primitive(Primitive::Bool(res)));
                }
                // NOTE: deliberately not routed through the coercion policy. "Is it a
                // number?" is the operator's dispatch test here, not a conversion --
                // failing it selects string comparison instead. See Add, below.
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
                        // NOTE: not routed through the coercion policy either. A policy
                        // that read strings as numbers would silently turn '"2" + "3"'
                        // from "23" into 5 -- an operator-dispatch change, not a coercion.
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
                // these operators only ever mean arithmetic, so the policy decides
                // outright what counts as a number -- and reports whether it found
                // an integer, which is what lets the result stay one
                let (ln, rn) = (self.context.to_number(&l)?, self.context.to_number(&r)?);
                if let (Number::Int(a), Number::Int(b)) = (ln, rn) {
                    match op {
                        Sub => return Ok(Value::from(a - b)),
                        Mul => return Ok(Value::from(a * b)),
                        Mod if b == 0 => return Err(Error::DivideByZero),
                        Mod => return Ok(Value::from(a % b)),
                        // Div and Pow are always float: 5 / 2 is 2.5, not 2
                        _ => {}
                    }
                }
                let (a, b) = (ln.as_f64(), rn.as_f64());
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
                Ok(Value::from(res))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use std::rc::Rc;

    use crate::types::function;
    use crate::types::value::Object;

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
                let f = function::new(Rc::new(|args: &[Value], cx: &Context| -> Result<Value> {
                    if args.len() != 2 {
                        return Err(Error::EvaluationFailed("need 2 args".into()));
                    }
                    // goes through the active policy, not a hardcoded conversion
                    Ok(Value::from(cx.to_number(&args[0])?.as_f64() + cx.to_number(&args[1])?.as_f64()))
                }));
                return Some(f);
            }
            if key == "global" {
                return Some(Value::Object(Rc::new(MockGlobal {})));
            }
            None
        }
    }

    struct MockGlobal;

    impl Object for MockGlobal {
        fn type_name(&self) -> &'static str {
            "global"
        }

        fn get_member(&self, name: &str) -> Result<Value> {
            match name {
                "a" => Ok(Value::Primitive(Primitive::Str("a".to_string()))),
                "fun" => Ok(function::new(Rc::new(|_args: &[Value], _cx: &Context| -> Result<Value> { Ok(Value::Primitive(Primitive::Str("yes".to_string()))) }))),
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
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        assert_eq!(ev.evaluate(&parser::parse_expression("1").unwrap()).unwrap(), Value::from(1i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("1").unwrap()).unwrap().to_string(), "1");
        assert_eq!(ev.evaluate(&parser::parse_expression("true").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("true || false").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("true && false").unwrap()).unwrap(), Value::from(false));
    }

    #[test]
    fn eval_literals_and_ops() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        assert_eq!(ev.evaluate(&parser::parse_expression("1 + 2 * 3").unwrap()).unwrap(), Value::from(7i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("true && !false").unwrap()).unwrap(), Value::from(true));
        match ev.evaluate(&parser::parse_expression("1/0").unwrap()) {
            Err(Error::DivideByZero) => (),
            other => panic!("expected div by zero, got {:?}", other),
        }
    }

    #[test]
    fn eval_paths_and_calls() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        assert_eq!(ev.evaluate(&parser::parse_expression("x").unwrap()).unwrap(), Value::from(10i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("truth || false").unwrap()).unwrap(), Value::from(true));
        let v = ev.evaluate(&parser::parse_expression("add(2, 3)").unwrap()).unwrap();
        match v {
            Value::Primitive(Primitive::Float(f)) => assert!((f - 5.0).abs() < 1e-9),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn eval_from_file_cases() {
        // Load test cases file at compile time
        const CASES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/evaluator_cases.txt"));
        let resolver = MockResolver::new();
        eval_from_file(CASES, |expr_src| evaluate(expr_src, &resolver).map(|v| v.to_string()));
    }

    fn eval_from_file<F>(cases: &str, evaluator: F)
    where
        F: Fn(&str) -> Result<String>,
    {
        for (idx, raw_line) in cases.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, "=>").collect();
            assert_eq!(parts.len(), 2, "Invalid test case format on line {}: '{}'", line_no, raw_line);
            let expr_src = parts[0].trim();
            let expected_str = parts[1].trim();

            let actual_val = evaluator(expr_src);
            assert!(actual_val.is_ok(), "Evaluation failed on line {} for expr '{}': {}", line_no, expr_src, actual_val.unwrap_err());
            let actual_str = actual_val.unwrap();

            assert_eq!(actual_str, expected_str, "Mismatch on line {} for expr '{}': got '{}', expected '{}'", line_no, expr_src, actual_str, expected_str);
        }
    }

    #[test]
    fn eval_lists_and_indexing() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        // [10, 20, 30][1] => 20
        assert_eq!(ev.evaluate(&parser::parse_expression("[10, 20, 30][1]").unwrap()).unwrap(), Value::from(20i64));
        // [10][1] => IndexOutOfBounds
        match ev.evaluate(&parser::parse_expression("[10][1]").unwrap()) {
            Err(Error::IndexOutOfBounds { index, len }) => {
                assert_eq!(index, 1);
                assert_eq!(len, 1);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
        // [10]["0"] => WrongIndexType
        match ev.evaluate(&parser::parse_expression("[10][\"0\"]").unwrap()) {
            Err(Error::NotIndexable(idx)) => assert_eq!(idx, "0"),
            other => panic!("expected NotIndexable(0), got {:?}", other),
        }
        // negative indices
        assert_eq!(ev.evaluate(&parser::parse_expression("[10, 20, 30][-1]").unwrap()).unwrap(), Value::from(30i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("[10, 20, 30][-3]").unwrap()).unwrap(), Value::from(10i64));
        match ev.evaluate(&parser::parse_expression("[10, 20, 30][-4]").unwrap()) {
            Err(Error::IndexOutOfBounds { index, len }) => {
                assert_eq!(index, -4);
                assert_eq!(len, 3);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
    }

    #[test]
    fn eval_dict_and_member() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        // Dict via [key]
        assert_eq!(ev.evaluate(&parser::parse_expression("{\"a\": 1, \"b\": 2}[\"b\"]").unwrap()).unwrap(), Value::from(2i64));
        match ev.evaluate(&parser::parse_expression("{\"a\": 1}[\"z\"]").unwrap()) {
            Err(Error::NoSuchKey(k)) => assert_eq!(k, "z"),
            other => panic!("expected NoSuchKey, got {:?}", other),
        }
        match ev.evaluate(&parser::parse_expression("{\"a\": 1}[0]").unwrap()) {
            Err(Error::NotIndexable(idx)) => assert_eq!(idx, "0"),
            other => panic!("expected NotIndexable(0), got {:?}", other),
        }
        // Members: properties and methods
        // string.length property
        assert_eq!(ev.evaluate(&parser::parse_expression("'abc'.length").unwrap()).unwrap(), Value::from(3i64));
        // string methods
        assert_eq!(ev.evaluate(&parser::parse_expression("'ab'.toUpper()").unwrap()).unwrap().to_string(), "AB");
        assert_eq!(ev.evaluate(&parser::parse_expression("' Ab '.trim().length").unwrap()).unwrap(), Value::from(2i64));
        // list.length property
        assert_eq!(ev.evaluate(&parser::parse_expression("[1,2,3].length").unwrap()).unwrap(), Value::from(3i64));
        // dict.length property and keys()/values()
        assert_eq!(ev.evaluate(&parser::parse_expression("{\"a\":1, \"b\":2}.length").unwrap()).unwrap(), Value::from(2i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("{\"a\":1}.keys().length").unwrap()).unwrap(), Value::from(1i64));
        // errors: dict dot key is unknown member now
        match ev.evaluate(&parser::parse_expression("{\"a\": 1}.a").unwrap()) {
            Err(Error::UnknownMember { member, .. }) => assert_eq!(member, "a"),
            other => panic!("expected UnknownMember, got {:?}", other),
        }
        // errors: unknown member on list
        match ev.evaluate(&parser::parse_expression("[1].toUpper").unwrap()) {
            Err(Error::UnknownMember { member, .. }) => assert_eq!(member, "toUpper"),
            other => panic!("expected UnknownMember, got {:?}", other),
        }
        // calling non-call property is NotCallable
        match ev.evaluate(&parser::parse_expression("'abc'.length()").unwrap()) {
            Err(Error::NotCallable) => (),
            other => panic!("expected NotCallable, got {:?}", other),
        }
        // Nested
        assert_eq!(ev.evaluate(&parser::parse_expression("{\"xs\": [10, 20]}[\"xs\"][1]").unwrap()).unwrap(), Value::from(20i64));

        // Computed dict key in literal and runtime enforcement of key type
        assert_eq!(ev.evaluate(&parser::parse_expression("{\"a\" + \"b\": 1}[\"ab\"]").unwrap()).unwrap(), Value::from(1i64));
        match ev.evaluate(&parser::parse_expression("{1: 2}").unwrap()) {
            Err(Error::TypeMismatch(msg)) => assert_eq!(msg, "dict key must be a string"),
            other => panic!("expected TypeMismatch for dict key, got {:?}", other),
        }
    }

    #[test]
    fn eval_truthiness_lists_dicts() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        assert_eq!(ev.evaluate(&parser::parse_expression("![]").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("!![]").unwrap()).unwrap(), Value::from(false));
        assert_eq!(ev.evaluate(&parser::parse_expression("![1]").unwrap()).unwrap(), Value::from(false));
        assert_eq!(ev.evaluate(&parser::parse_expression("!![1]").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("!{}").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("!!{\"a\":1}").unwrap()).unwrap(), Value::from(true));
    }

    /// Integer operands keep integer results. The .txt corpus cannot catch this,
    /// because Int(5) and Float(5.0) both display as "5".
    #[test]
    fn numeric_tower_preserves_ints() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        let eval = |src: &str| ev.evaluate(&parser::parse_expression(src).unwrap()).unwrap();

        assert_eq!(eval("5 - 2"), Value::from(3i64));
        assert_eq!(eval("2 * 3"), Value::from(6i64));
        assert_eq!(eval("7 % 3"), Value::from(1i64));
        assert_eq!(eval("1 + 2"), Value::from(3i64));

        // Div and Pow are always float, even for integer operands
        assert_eq!(eval("5 / 2"), Value::from(2.5f64));
        assert_eq!(eval("4 / 2"), Value::from(2.0f64));
        assert_eq!(eval("2 ^ 3"), Value::from(8.0f64));

        // one float operand promotes the result
        assert_eq!(eval("2 * 3.0"), Value::from(6.0f64));
        assert_eq!(eval("5.0 - 2"), Value::from(3.0f64));

        // divide-by-zero still fires on the integer path
        match ev.evaluate(&parser::parse_expression("1 % 0").unwrap()) {
            Err(Error::DivideByZero) => (),
            other => panic!("expected DivideByZero, got {:?}", other),
        }
        match ev.evaluate(&parser::parse_expression("1 / 0").unwrap()) {
            Err(Error::DivideByZero) => (),
            other => panic!("expected DivideByZero, got {:?}", other),
        }
    }

    /// Above 2^53 an i64 does not survive a round trip through f64, so comparing
    /// two integers must not go through one.
    #[test]
    fn large_integers_compare_exactly() {
        let resolver = MockResolver::new();
        let ev = Evaluator::new(&resolver);
        let eval = |src: &str| ev.evaluate(&parser::parse_expression(src).unwrap()).unwrap();

        assert_eq!(eval("9007199254740992 < 9007199254740993"), Value::from(true));
        assert_eq!(eval("9007199254740993 > 9007199254740992"), Value::from(true));
        assert_eq!(eval("9007199254740992 >= 9007199254740993"), Value::from(false));
        // consistent with equality, which was always exact
        assert_eq!(eval("9007199254740992 == 9007199254740993"), Value::from(false));
        // mixed int/float comparison still goes through f64
        assert_eq!(eval("1 < 1.5"), Value::from(true));
        assert_eq!(eval("2.5 > 2"), Value::from(true));
    }

    /// Only bools are bools; everything else is a type error.
    #[test]
    fn strict_coercions_reject_truthiness() {
        use crate::types::coerce::StrictCoercions;
        let resolver = MockResolver::new();
        let strict = StrictCoercions;
        let ev = Evaluator::new_with_coercions(&resolver, &strict);

        // standard policy accepts these
        let std_ev = Evaluator::new(&resolver);
        assert_eq!(std_ev.evaluate(&parser::parse_expression("1 && true").unwrap()).unwrap(), Value::from(true));
        assert_eq!(std_ev.evaluate(&parser::parse_expression("![]").unwrap()).unwrap(), Value::from(true));

        // strict policy does not
        match ev.evaluate(&parser::parse_expression("1 && true").unwrap()) {
            Err(Error::NotCoercible { type_name, target }) => {
                assert_eq!(type_name, "number");
                assert_eq!(target, "bool");
            }
            other => panic!("expected NotCoercible, got {:?}", other),
        }
        match ev.evaluate(&parser::parse_expression("![]").unwrap()) {
            Err(Error::NotCoercible { type_name, .. }) => assert_eq!(type_name, "list"),
            other => panic!("expected NotCoercible, got {:?}", other),
        }
        // actual bools still work
        assert_eq!(ev.evaluate(&parser::parse_expression("true && !false").unwrap()).unwrap(), Value::from(true));
    }

    /// A custom policy overrides only what it cares about and delegates the rest,
    /// and the override is visible both to operators and inside function bodies.
    #[test]
    fn custom_coercions_reach_function_bodies() {
        use crate::types::coerce::{Coercions, STANDARD};

        struct NumericStrings;
        impl Coercions for NumericStrings {
            fn to_bool(&self, v: &Value) -> Result<bool> {
                // empty string is false, any other string is true
                if let Value::Primitive(Primitive::Str(s)) = v {
                    return Ok(!s.is_empty());
                }
                STANDARD.to_bool(v)
            }
            fn to_number(&self, v: &Value) -> Result<Number> {
                if let Value::Primitive(Primitive::Str(s)) = v {
                    // an integral string stays an integer
                    if let Ok(i) = s.parse::<i64>() {
                        return Ok(Number::Int(i));
                    }
                    if let Ok(f) = s.parse::<f64>() {
                        return Ok(Number::Float(f));
                    }
                }
                STANDARD.to_number(v)
            }
        }

        let resolver = MockResolver::new();
        let policy = NumericStrings;
        let ev = Evaluator::new_with_coercions(&resolver, &policy);

        // truthiness override, used by the '!' operator
        assert_eq!(ev.evaluate(&parser::parse_expression("!''").unwrap()).unwrap(), Value::from(true));
        assert_eq!(ev.evaluate(&parser::parse_expression("!'xyz'").unwrap()).unwrap(), Value::from(false));
        // the standard policy has no opinion on arbitrary strings
        match Evaluator::new(&resolver).evaluate(&parser::parse_expression("!'xyz'").unwrap()) {
            Err(Error::NotCoercible { type_name, target }) => {
                assert_eq!(type_name, "string");
                assert_eq!(target, "bool");
            }
            other => panic!("expected NotCoercible, got {:?}", other),
        }

        // the number override reaches inside add(), which calls cx.to_number()
        match ev.evaluate(&parser::parse_expression("add('2', 3)").unwrap()).unwrap() {
            Value::Primitive(Primitive::Float(f)) => assert!((f - 5.0).abs() < 1e-9),
            other => panic!("expected float, got {:?}", other),
        }
        // ... and the default policy still rejects it
        match Evaluator::new(&resolver).evaluate(&parser::parse_expression("add('2', 3)").unwrap()) {
            Err(Error::NotCoercible { target, .. }) => assert_eq!(target, "number"),
            other => panic!("expected NotCoercible, got {:?}", other),
        }

        // a policy can yield exact integers, and the operator keeps them
        assert_eq!(ev.evaluate(&parser::parse_expression("'2' * 3").unwrap()).unwrap(), Value::from(6i64));
        assert_eq!(ev.evaluate(&parser::parse_expression("'2.5' * 2").unwrap()).unwrap(), Value::from(5.0f64));

        // delegated conversions are unchanged
        assert_eq!(ev.evaluate(&parser::parse_expression("![]").unwrap()).unwrap(), Value::from(true));
    }

    #[test]
    fn eval_interpolation() {
        let resolver = MockResolver::new();
        assert_eq!(evaluate_interpolations("${'abc'}", &resolver).unwrap(), "abc");
        assert_eq!(evaluate_interpolations("${   'abc' }", &resolver).unwrap(), "abc");
        assert_eq!(evaluate_interpolations("${   'abc' } ", &resolver).unwrap(), "abc ");
        assert_eq!(evaluate_interpolations("x${'abc'}y", &resolver).unwrap(), "xabcy");
        assert_eq!(evaluate_interpolations("x${'abc\"\\''}\"y", &resolver).unwrap(), "xabc\"'\"y");
        assert_eq!(evaluate_interpolations("x${[1,2,3][1]}y", &resolver).unwrap(), "x2y");
        assert_eq!(evaluate_interpolations("x${{'foo': 'bar', 'baz': 'bam'}['foo']}y", &resolver).unwrap(), "xbary");
        assert_eq!(evaluate_interpolations("x${{\"foo\": \"bar\", \"baz\": \"bam\"}[\"foo\"]}y", &resolver).unwrap(), "xbary");
    }
}
