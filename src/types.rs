use std::collections::BTreeMap;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to resolve variable: {0:?}")]
    ResolveFailed(String),
    #[error("variable is not callable")]
    NotCallable,
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    #[error("divide by zero")]
    DivideByZero,
    #[error("evaluation failed: {0}")]
    EvaluationFailed(String),
    #[error("parse failed: {0} inside interpolation (near: '{1}')")]
    ParseFailed(String, String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

impl Primitive {
    // Back-compat helpers matching the prior Atom API
    pub fn as_bool(&self) -> Option<bool> {
        self.coerce_bool()
    }
    pub fn as_int(&self) -> Option<i64> {
        if let Primitive::Int(i) = self { Some(*i) } else { None }
    }
    pub fn as_float(&self) -> Option<f64> {
        if let Primitive::Float(f) = self { Some(*f) } else { None }
    }
    pub fn as_str(&self) -> String {
        self.as_str_lossy()
    }

    // Newer, explicit coercions
    pub fn coerce_bool(&self) -> Option<bool> {
        match self {
            Primitive::Int(i) => Some(*i != 0),
            Primitive::Float(f) => Some(*f != 0.0),
            Primitive::Str(s) if s == "true" || s == "false" => Some(s == "true"),
            Primitive::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Primitive::Float(f) => Some(*f),
            Primitive::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
    pub fn as_str_lossy(&self) -> String {
        match self {
            Primitive::Str(s) => s.clone(),
            Primitive::Int(i) => i.to_string(),
            Primitive::Float(f) => f.to_string(),
            Primitive::Bool(b) => b.to_string(),
        }
    }
}

impl From<i64> for Primitive {
    fn from(v: i64) -> Self {
        Primitive::Int(v)
    }
}
impl From<f64> for Primitive {
    fn from(v: f64) -> Self {
        Primitive::Float(v)
    }
}
impl From<bool> for Primitive {
    fn from(v: bool) -> Self {
        Primitive::Bool(v)
    }
}
impl From<String> for Primitive {
    fn from(v: String) -> Self {
        Primitive::Str(v)
    }
}
impl From<&str> for Primitive {
    fn from(v: &str) -> Self {
        Primitive::Str(v.to_string())
    }
}

impl TryFrom<Primitive> for i64 {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Int(i) = p { Ok(i) } else { Err(Error::TypeMismatch("expected int".into())) }
    }
}
impl TryFrom<Primitive> for f64 {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Float(f) = p { Ok(f) } else { Err(Error::TypeMismatch("expected float".into())) }
    }
}
impl TryFrom<Primitive> for bool {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Bool(b) = p { Ok(b) } else { Err(Error::TypeMismatch("expected bool".into())) }
    }
}
impl TryFrom<Primitive> for String {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Str(s) = p { Ok(s) } else { Err(Error::TypeMismatch("expected string".into())) }
    }
}

type Callable = Rc<dyn Fn(&[Value]) -> Result<Value>>;
#[derive(Clone)]
pub enum Value {
    Primitive(Primitive),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
    Func(Callable),
}

impl Value {
    pub fn coerce_bool(&self) -> Option<bool> {
        match self {
            Value::Primitive(p) => p.coerce_bool(),
            Value::List(vs) => Some(!vs.is_empty()),
            Value::Dict(m) => Some(!m.is_empty()),
            Value::Func(_) => None,
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Value::Primitive(p) => p.to_float_lossy(),
            _ => None,
        }
    }
    pub fn as_str_lossy(&self) -> String {
        match self {
            Value::Primitive(p) => p.as_str_lossy(),
            Value::List(_) => "[list]".into(),
            Value::Dict(_) => "{dict}".into(),
            Value::Func(_) => "<func>".into(),
        }
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str_lossy())
    }
}
impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str_lossy())
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Primitive(p) => write!(f, "{}", p),
            Value::List(vs) => write!(f, "[{}]", vs.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")),
            Value::Dict(m) => write!(f, "{{{}}}", m.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<_>>().join(", ")),
            Value::Func(_) => write!(f, "<func>"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Primitive(p1), Value::Primitive(p2)) => p1 == p2,
            (Value::List(l1), Value::List(l2)) => l1 == l2,
            (Value::Dict(d1), Value::Dict(d2)) => d1 == d2,
            (Value::Func(_), Value::Func(_)) => false,
            _ => false,
        }
    }
}

impl From<Primitive> for Value {
    fn from(p: Primitive) -> Self {
        Value::Primitive(p)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Primitive(v.into())
    }
}

impl TryFrom<Value> for i64 {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected int".into())) }
    }
}
impl TryFrom<Value> for f64 {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected float".into())) }
    }
}
impl TryFrom<Value> for bool {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected bool".into())) }
    }
}
impl TryFrom<Value> for String {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected string".into())) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Primitive),
    Var(String),
    Member { object: Box<Expr>, field: String },
    Index { object: Box<Expr>, index: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}
