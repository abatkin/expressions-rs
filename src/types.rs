use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

impl Atom {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Atom::Int(i) => Some(*i != 0),
            Atom::Float(f) => Some(*f != 0.0),
            Atom::Str(s) if s == "true" || s == "false" => Some(s == "true"),
            Atom::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Atom::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Atom::Float(f) => Some(*f),
            _ => None,
        }
    }
    pub fn as_str(&self) -> String {
        match self {
            Atom::Str(s) => s.clone(),
            Atom::Int(i) => i.to_string(),
            Atom::Float(f) => f.to_string(),
            Atom::Bool(b) => b.to_string(),
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Atom::Float(f) => Some(*f),
            Atom::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Basic(Atom),
    Path(Vec<String>),
    Member { object: Box<Expr>, field: String },
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

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to resolve variable: {0:?}")]
    ResolveFailed(Vec<String>),
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
