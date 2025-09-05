use crate::parser::parse;
use crate::types::{BinaryOp, Error, Expr, Primitive, Result, UnaryOp, Value};

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
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Member { .. } => Err(Error::Unsupported("member access is not supported in evaluator".into())),
            Expr::Index { .. } => Err(Error::Unsupported("indexing is not yet supported in evaluator".into())),
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
            Eq | Ne => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                let eq = match (&l, &r) {
                    (Value::Primitive(lp), Value::Primitive(rp)) => self.prim_eq(lp, rp)?,
                    _ => return Err(Error::TypeMismatch("'==' expects comparable primitives".into())),
                };
                Ok(Value::Primitive(Primitive::Bool(if let Eq = op { eq } else { !eq })))
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

    fn prim_eq(&self, a: &Primitive, b: &Primitive) -> Result<bool> {
        Ok(match (a, b) {
            (Primitive::Int(x), Primitive::Int(y)) => x == y,
            (Primitive::Float(x), Primitive::Float(y)) => x == y,
            (Primitive::Int(x), Primitive::Float(y)) => (*x as f64) == *y,
            (Primitive::Float(x), Primitive::Int(y)) => *x == (*y as f64),
            (Primitive::Bool(x), Primitive::Bool(y)) => x == y,
            (Primitive::Str(x), Primitive::Str(y)) => x == y,
            _ => return Err(Error::TypeMismatch("'==' expects comparable types".into())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    use crate::parser::parse;

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
            None
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
}
