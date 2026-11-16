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
    /// Integer arithmetic that does not fit in an `i64`. Reported rather than
    /// wrapped, so a result is never silently wrong.
    #[error("integer overflow in '{op}'")]
    IntegerOverflow {
        /// The operator that overflowed, as written in the source.
        op: &'static str,
    },
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
    /// A `${...}` in a [`Template`](crate::template::Template) failed to evaluate
    /// or render; `error` is the underlying failure.
    #[error("evaluation failed in interpolation at line {line}, column {column}: {error}")]
    InterpolationFailed {
        /// Index into [`Template::segments`](crate::template::Template::segments)
        /// of the failing expression.
        segment: usize,
        /// 0-based byte offset of the failing `${` in the template source.
        offset: usize,
        /// 1-based line of `offset`.
        line: usize,
        /// 1-based column of `offset`, in characters.
        column: usize,
        #[source]
        error: Box<Error>,
    },
    #[error("internal parse error: {0}")]
    InternalParserError(String),
}

pub type Result<T> = core::result::Result<T, Error>;
