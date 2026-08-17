use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("unable to resolve variable: {0:?}")]
    ResolveFailed(String),
    #[error("variable is not callable")]
    NotCallable,
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    #[error("cannot use {type_name} as {target}")]
    NotCoercible { type_name: String, target: &'static str },
    #[error("divide by zero")]
    DivideByZero,
    #[error("evaluation failed: {0}")]
    EvaluationFailed(String),
    #[error("index out of bounds: {index} (len: {len})")]
    IndexOutOfBounds { index: i64, len: usize },
    #[error("not indexable: {0}")]
    NotIndexable(String),
    #[error("no such key: {0}")]
    NoSuchKey(String),
    #[error("unknown member '{member}' for type {type_name}")]
    UnknownMember { type_name: String, member: String },
    /// Positions are into the whole string handed to the entry point, so they stay
    /// meaningful for an expression interpolated into some larger text.
    #[error("parse error at line {line}, column {column}: {message}")]
    ParseError {
        /// 1-based.
        line: usize,
        /// 1-based, in characters.
        column: usize,
        /// 0-based byte offset, for callers that want to slice the input.
        offset: usize,
        message: String,
        /// The offending line with a caret under `column`.
        rendered: String,
    },
    #[error("internal parse error: {0}")]
    InternalParserError(String),
}

pub type Result<T> = core::result::Result<T, Error>;
