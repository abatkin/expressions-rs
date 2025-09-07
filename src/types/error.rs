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
