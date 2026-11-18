use crate::types::primitive::Primitive;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Primitive),
    Var(String),
    ListLiteral(Vec<Expr>),
    DictLiteral(Vec<(Expr, Expr)>),
    Member {
        object: Box<Expr>,
        field: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    /// Operators of one precedence, applied left to right: `a - b + c` is
    /// `first: a, rest: [(Sub, b), (Add, c)]`.
    Binary {
        first: Box<Expr>,
        rest: Vec<(BinaryOp, Expr)>,
    },
    /// `^`, applied right to left: `a ^ b ^ c` is `bases: [a, b], exponent: c`.
    Power {
        bases: Vec<Expr>,
        exponent: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
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
}
