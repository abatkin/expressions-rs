use crate::parser::{Atom, BinaryOp, Expr, UnaryOp};

#[derive(Debug)]
pub enum EvalError {
    ResolveFailed(Vec<String>),
    NotCallable(Vec<String>),
    NotAValue(Vec<String>),
    TypeError(&'static str),
    DivideByZero,
    FunctionError(String),
    Unsupported(&'static str),
}

pub trait Variable {
    fn as_atom(&self) -> Result<Atom, EvalError>;
    fn call(&self, _args: Vec<Atom>) -> Result<Atom, EvalError> {
        Err(EvalError::NotCallable(Vec::new()))
    }
}

pub trait VariableResolver {
    fn resolve(&self, path: &[String]) -> Option<Box<dyn Variable>>;
}

pub struct Evaluator<R: VariableResolver> {
    pub resolver: R,
}

impl<R: VariableResolver> Evaluator<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }

    // Evaluate a string with ${...} interpolations. Each ${...} is parsed with the
    // chumsky parser that expects a closing '}', so no manual brace tracking is needed.
    pub fn evaluate_interpolated(&self, input: &str) -> Result<String, EvalError> {
        let mut out = String::new();
        let mut rest = input;
        while let Some(idx) = rest.find("${") {
            // copy literal part before the interpolation
            out.push_str(&rest[..idx]);
            let after = &rest[idx + 2..];
            match crate::parser::parse_in_braces(after) {
                Ok((expr, consumed)) => {
                    let val = self.evaluate(&expr)?;
                    out.push_str(&val.as_str());
                    rest = &after[consumed..];
                }
                Err(e) => {
                    return Err(EvalError::FunctionError(format!("interpolation parse error: {}", e)));
                }
            }
        }
        // copy the remainder
        out.push_str(rest);
        Ok(out)
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<Atom, EvalError> {
        match expr {
            Expr::Basic(a) => match a {
                Atom::Int(i) => Ok(Atom::Int(*i)),
                Atom::Float(f) => Ok(Atom::Float(*f)),
                Atom::Str(s) => Ok(Atom::Str(s.clone())),
                Atom::Bool(b) => Ok(Atom::Bool(*b)),
            },
            Expr::Path(p) => self.eval_path(p),
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Member { object, field } => self.eval_member(object, field),
            Expr::Unary { op, expr } => {
                let v = self.evaluate(expr)?;
                match op {
                    UnaryOp::Not => {
                        let b = v.as_bool().ok_or(EvalError::TypeError("'!' expects bool"))?;
                        Ok(Atom::Bool(!b))
                    }
                }
            }
            Expr::Binary { op, left, right } => self.eval_binary(*op, left, right),
        }
    }

    fn eval_path(&self, p: &Vec<String>) -> Result<Atom, EvalError> {
        match self.resolver.resolve(p) {
            Some(v) => v.as_atom(),
            None => Err(EvalError::ResolveFailed(p.clone())),
        }
    }

    fn eval_call(&self, callee: &Expr, args: &Vec<Expr>) -> Result<Atom, EvalError> {
        // Only support calling a Path directly for now
        let path = match callee {
            Expr::Path(p) => p.clone(),
            Expr::Member { object, field } => {
                // If member chain is purely a Path-based object, flatten and call
                if let Some(mut base) = self.flatten_member_path(object) {
                    base.push(field.clone());
                    base
                } else {
                    return Err(EvalError::Unsupported("calling non-path/member callee"));
                }
            }
            _ => return Err(EvalError::Unsupported("calling non-path callee")),
        };
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.evaluate(a)?);
        }
        match self.resolver.resolve(&path) {
            Some(v) => v.call(vals),
            None => Err(EvalError::ResolveFailed(path)),
        }
    }

    fn eval_member(&self, object: &Expr, field: &str) -> Result<Atom, EvalError> {
        // If the object is a path (or member chain), flatten and resolve as a longer path
        if let Some(mut base) = self.flatten_member_path(object) {
            let mut p = Vec::new();
            p.append(&mut base);
            p.push(field.to_string());
            return self.eval_path(&p);
        }
        // Otherwise, member access on non-path result is unsupported in this minimal evaluator
        Err(EvalError::Unsupported("member access on non-path is unsupported"))
    }

    fn flatten_member_path(&self, expr: &Expr) -> Option<Vec<String>> {
        match expr {
            Expr::Path(p) => Some(p.clone()),
            Expr::Member { object, field } => {
                let mut left = self.flatten_member_path(object)?;
                left.push(field.clone());
                Some(left)
            }
            _ => None,
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Atom, EvalError> {
        use BinaryOp::*;
        match op {
            Or => {
                let l = self.evaluate(left)?;
                let lb = l.as_bool().ok_or(EvalError::TypeError("'||' expects bools"))?;
                if lb {
                    return Ok(Atom::Bool(true));
                }
                let rb = self.evaluate(right)?.as_bool().ok_or(EvalError::TypeError("'||' expects bools"))?;
                Ok(Atom::Bool(lb || rb))
            }
            And => {
                let l = self.evaluate(left)?;
                let lb = l.as_bool().ok_or(EvalError::TypeError("'&&' expects bools"))?;
                if !lb {
                    return Ok(Atom::Bool(false));
                }
                let rb = self.evaluate(right)?.as_bool().ok_or(EvalError::TypeError("'&&' expects bools"))?;
                Ok(Atom::Bool(lb && rb))
            }
            Eq | Ne => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                let eq = self.atom_eq(&l, &r)?;
                Ok(Atom::Bool(if let Eq = op { eq } else { !eq }))
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
                    return Ok(Atom::Bool(res));
                }
                if let (Atom::Str(a), Atom::Str(b)) = (&l, &r) {
                    let res = match op {
                        Lt => a < b,
                        Le => a <= b,
                        Gt => a > b,
                        Ge => a >= b,
                        _ => unreachable!(),
                    };
                    return Ok(Atom::Bool(res));
                }
                Err(EvalError::TypeError("comparison requires two numbers or two strings"))
            }
            Add => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                match (l, r) {
                    (Atom::Int(a), Atom::Int(b)) => Ok(Atom::Int(a + b)),
                    (a, b) => {
                        let (af, bf) = (a.to_float_lossy(), b.to_float_lossy());
                        if let (Some(af), Some(bf)) = (af, bf) {
                            Ok(Atom::Float(af + bf))
                        } else if let (Atom::Str(as_), Atom::Str(bs_)) = (a, b) {
                            Ok(Atom::Str(as_ + &bs_))
                        } else {
                            Err(EvalError::TypeError("'+' expects numbers or strings"))
                        }
                    }
                }
            }
            Sub | Mul | Div | Mod | Pow => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                // Preserve integers for Sub, Mul, Mod if both ints
                match (op, &l, &r) {
                    (BinaryOp::Sub, Atom::Int(a), Atom::Int(b)) => return Ok(Atom::Int(a - b)),
                    (BinaryOp::Mul, Atom::Int(a), Atom::Int(b)) => return Ok(Atom::Int(a * b)),
                    (BinaryOp::Mod, Atom::Int(_), Atom::Int(b)) if *b == 0 => return Err(EvalError::DivideByZero),
                    (BinaryOp::Mod, Atom::Int(a), Atom::Int(b)) => return Ok(Atom::Int(a % b)),
                    _ => {}
                }
                let (af, bf) = (l.to_float_lossy(), r.to_float_lossy());
                if let (Some(a), Some(b)) = (af, bf) {
                    let res = match op {
                        Sub => a - b,
                        Mul => a * b,
                        Div => {
                            if b == 0.0 {
                                return Err(EvalError::DivideByZero);
                            }
                            a / b
                        }
                        Mod => {
                            if b == 0.0 {
                                return Err(EvalError::DivideByZero);
                            }
                            a % b
                        }
                        Pow => a.powf(b),
                        _ => unreachable!(),
                    };
                    Ok(Atom::Float(res))
                } else {
                    Err(EvalError::TypeError("arithmetic expects numbers"))
                }
            }
        }
    }

    fn atom_eq(&self, a: &Atom, b: &Atom) -> Result<bool, EvalError> {
        Ok(match (a, b) {
            (Atom::Int(x), Atom::Int(y)) => x == y,
            (Atom::Float(x), Atom::Float(y)) => x == y,
            (Atom::Int(x), Atom::Float(y)) => (*x as f64) == *y,
            (Atom::Float(x), Atom::Int(y)) => *x == (*y as f64),
            (Atom::Bool(x), Atom::Bool(y)) => x == y,
            (Atom::Str(x), Atom::Str(y)) => x == y,
            _ => return Err(EvalError::TypeError("'==' expects comparable types")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstVar(Atom);
    impl Variable for ConstVar {
        fn as_atom(&self) -> Result<Atom, EvalError> {
            Ok(self.0.clone())
        }
    }

    struct FuncVar(fn(Vec<Atom>) -> Result<Atom, EvalError>);
    impl Variable for FuncVar {
        fn as_atom(&self) -> Result<Atom, EvalError> {
            Err(EvalError::NotAValue(vec![]))
        }
        fn call(&self, args: Vec<Atom>) -> Result<Atom, EvalError> {
            (self.0)(args)
        }
    }

    use crate::parser::parse;
    use std::collections::HashMap;

    struct MockResolver {
        // store fully-qualified path joined by '.'
        map: HashMap<String, Box<dyn Variable>>,
    }
    impl MockResolver {
        fn new() -> Self {
            Self { map: HashMap::new() }
        }
        fn put_path(mut self, path: &str, var: Box<dyn Variable>) -> Self {
            self.map.insert(path.to_string(), var);
            self
        }
    }
    impl VariableResolver for MockResolver {
        fn resolve(&self, path: &[String]) -> Option<Box<dyn Variable>> {
            let key = path.join(".");
            // For tests, return some built-ins regardless of map contents
            if key == "x" {
                return Some(Box::new(ConstVar(Atom::Int(10))) as Box<dyn Variable>);
            }
            if key == "truth" {
                return Some(Box::new(ConstVar(Atom::Bool(true))) as Box<dyn Variable>);
            }
            if key == "math.add" {
                return Some(Box::new(FuncVar(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::FunctionError("need 2 args".into()));
                    }
                    let a = args[0].to_float_lossy().ok_or(EvalError::TypeError("number"))?;
                    let b = args[1].to_float_lossy().ok_or(EvalError::TypeError("number"))?;
                    Ok(Atom::Float(a + b))
                })) as Box<dyn Variable>);
            }
            // Fall back to any explicitly provided mapping (value semantics aren't preserved across calls; used rarely here)
            if self.map.contains_key(&key) {
                return Some(Box::new(ConstVar(Atom::Int(0))) as Box<dyn Variable>);
            }
            None
        }
    }

    #[test]
    fn eval_basic_expressions() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("1").unwrap()).unwrap(), Atom::Int(1));
        assert_eq!(ev.evaluate(&parse("1").unwrap()).unwrap().as_str(), "1");
        assert_eq!(ev.evaluate(&parse("true").unwrap()).unwrap(), Atom::Bool(true));
        assert_eq!(ev.evaluate(&parse("true || false").unwrap()).unwrap(), Atom::Bool(true));
        assert_eq!(ev.evaluate(&parse("true && false").unwrap()).unwrap(), Atom::Bool(false));
    }

    #[test]
    fn eval_literals_and_ops() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("1 + 2 * 3").unwrap()).unwrap(), Atom::Int(7));
        assert_eq!(ev.evaluate(&parse("true && !false").unwrap()).unwrap(), Atom::Bool(true));
        match ev.evaluate(&parse("1/0").unwrap()) {
            Err(EvalError::DivideByZero) => (),
            other => panic!("expected div by zero, got {:?}", other),
        }
    }

    #[test]
    fn eval_paths_and_calls() {
        let ev = Evaluator::new(MockResolver::new());
        assert_eq!(ev.evaluate(&parse("x").unwrap()).unwrap(), Atom::Int(10));
        assert_eq!(ev.evaluate(&parse("truth || false").unwrap()).unwrap(), Atom::Bool(true));
        let v = ev.evaluate(&parse("math.add(2, 3)").unwrap()).unwrap();
        match v {
            Atom::Float(f) => assert!((f - 5.0).abs() < 1e-9),
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
            assert!(parts.len() == 2, "Invalid test case format on line {}: '{}'", line_no, raw_line);
            let expr_src = parts[0].trim();
            let expected_str = parts[1].trim();

            let expr = parse(expr_src).expect(&format!("Failed to parse expression on line {}: '{}'", line_no, expr_src));
            let actual_atom = ev.evaluate(&expr).expect(&format!("Evaluation failed on line {} for expr '{}': parsed: {:?}", line_no, expr_src, expr));
            let actual_str = actual_atom.as_str();

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
        // let s3 = ev.evaluate_interpolated("${'A'}-${math.add(2,3)}-${truth}").unwrap();
        // assert_eq!(s3, "A-5-true");

        // ensure braces inside strings are handled
        let s4 = ev.evaluate_interpolated("${'curly } brace'} done").unwrap();
        assert_eq!(s4, "curly } brace done");

        // missing closing brace should error
        match ev.evaluate_interpolated("bad ${1+2") {
            Err(EvalError::FunctionError(_)) => (),
            other => panic!("expected parse error, got {:?}", other),
        }
    }
}
