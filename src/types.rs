use crate::value::Primitive;
use std::fmt::Debug;
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
    #[error("index out of bounds: {index} (len: {len})")]
    IndexOutOfBounds { index: i64, len: usize },
    #[error("{target}: {message}")]
    WrongIndexType { target: &'static str, message: String },
    #[error("not a dict")]
    NotADict,
    #[error("not indexable: {0}")]
    NotIndexable(String),
    #[error("no such key: {0}")]
    NoSuchKey(String),
    #[error("unknown member '{member}' for type {type_name}")]
    UnknownMember { type_name: String, member: String },
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Primitive),
    Var(String),
    ListLiteral(Vec<Expr>),
    DictLiteral(Vec<(Expr, Expr)>),
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
