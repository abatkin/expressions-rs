use crate::types::error::{Error, Result};
use crate::types::expression::{BinaryOp, Expr, UnaryOp};
use crate::types::primitive::Primitive;
use pest::Parser;
use pest::iterators::Pair;
use pest::pratt_parser::{Assoc, Op, PrattParser};

#[derive(pest_derive::Parser)]
#[grammar = "expr.pest"]
struct InnerParser;

pub fn parse_expression(input: &str) -> Result<Expr> {
    parse_internal(input, Rule::program).map(|r| r.0)
}

pub(crate) fn parse_internal(input: &str, rule: Rule) -> Result<(Expr, usize)> {
    let mut pairs = InnerParser::parse(rule, input).map_err(|e| Error::ParseError(format!("parse error: {}", e)))?;
    let pair = pairs.next().expect("program always produces one pair");

    debug_assert_eq!(pair.as_rule(), rule);
    let end_pos = pair.as_span().end_pos().pos();
    let expr_pair = pair.into_inner().next().expect("program contains expr");
    let expr = parse_expr(expr_pair)?;
    Ok((expr, end_pos))
}

fn pratt() -> PrattParser<Rule> {
    PrattParser::new()
        .op(Op::infix(Rule::op_or, Assoc::Left))
        .op(Op::infix(Rule::op_and, Assoc::Left))
        .op(Op::infix(Rule::op_eq, Assoc::Left))
        .op(Op::infix(Rule::op_cmp, Assoc::Left))
        .op(Op::infix(Rule::op_add, Assoc::Left))
        .op(Op::infix(Rule::op_mul, Assoc::Left))
        .op(Op::infix(Rule::op_pow, Assoc::Right))
}

fn parse_expr(pair: Pair<Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::expr => {
            let pairs = pair.into_inner();
            pratt()
                .map_primary(|p: Pair<Rule>| parse_unary(p))
                .map_infix(|lhs: Result<Expr>, op: Pair<Rule>, rhs: Result<Expr>| {
                    let left = lhs?;
                    let right = rhs?;
                    let mut l = left;
                    let mut r = right;
                    let bop = match op.as_rule() {
                        Rule::op_or => BinaryOp::Or,
                        Rule::op_and => BinaryOp::And,
                        Rule::op_eq => {
                            let s = op.as_str();
                            if s.contains("==") { BinaryOp::Eq } else { BinaryOp::Ne }
                        }
                        Rule::op_cmp => {
                            let s = op.as_str();
                            if s.contains("<=") {
                                // a <= b  ==>  b >= a
                                std::mem::swap(&mut l, &mut r);
                                BinaryOp::Ge
                            } else if s.contains(">=") {
                                BinaryOp::Ge
                            } else if s.contains('<') {
                                BinaryOp::Lt
                            } else {
                                BinaryOp::Gt
                            }
                        }
                        Rule::op_add => {
                            if op.as_str().contains('-') {
                                BinaryOp::Sub
                            } else {
                                BinaryOp::Add
                            }
                        }
                        Rule::op_mul => {
                            let s = op.as_str();
                            if s.contains('*') {
                                BinaryOp::Mul
                            } else if s.contains('/') {
                                BinaryOp::Div
                            } else {
                                BinaryOp::Mod
                            }
                        }
                        Rule::op_pow => BinaryOp::Pow,
                        r => {
                            return Err(Error::InternalParserError(format!("unexpected infix op: {:?}", r)));
                        }
                    };
                    Ok(Expr::Binary {
                        left: Box::new(l),
                        op: bop,
                        right: Box::new(r),
                    })
                })
                .parse(pairs)
        }
        _ => Err(Error::InternalParserError(format!("expected expr, got: {:?}", pair))),
    }
}

fn parse_unary(pair: Pair<Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::unary => {
            let mut ops: Vec<UnaryOp> = Vec::new();
            let mut inner = pair.into_inner();
            // Collect zero or more unary_op then the postfix expression
            loop {
                let Some(next) = inner.peek() else { break };
                match next.as_rule() {
                    Rule::unary_op => {
                        let op_pair = inner.next().unwrap();
                        let op_inner = op_pair.into_inner().next().unwrap();
                        let op = match op_inner.as_rule() {
                            Rule::not_op => UnaryOp::Not,
                            Rule::neg_op => UnaryOp::Neg,
                            r => {
                                return Err(Error::InternalParserError(format!("unexpected unary op: {:?}", r)));
                            }
                        };
                        ops.push(op);
                    }
                    _ => break,
                }
            }
            let post = inner.next().expect("unary must end with postfix");
            let mut expr = parse_postfix(post)?;
            for op in ops.into_iter().rev() {
                expr = Expr::Unary { op, expr: Box::new(expr) };
            }
            Ok(expr)
        }
        _ => parse_postfix(pair),
    }
}

fn parse_postfix(pair: Pair<Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::postfix => {
            let mut inner = pair.into_inner();
            let first = inner.next().expect("postfix starts with primary");
            let mut expr = parse_primary(first)?;
            for next in inner {
                match next.as_rule() {
                    Rule::call => {
                        let args = parse_call_args(next)?;
                        expr = Expr::Call { callee: Box::new(expr), args };
                    }
                    Rule::index => {
                        let idx_pair = next.into_inner().next().expect("index inner expr");
                        let index_expr = parse_expr(idx_pair)?;
                        expr = Expr::Index {
                            object: Box::new(expr),
                            index: Box::new(index_expr),
                        };
                    }
                    Rule::property => {
                        let name = next.into_inner().next().expect("property ident").as_str().to_string();
                        expr = Expr::Member { object: Box::new(expr), field: name };
                    }
                    r => {
                        return Err(Error::InternalParserError(format!("unexpected postfix op: {:?}", r)));
                    }
                }
            }
            Ok(expr)
        }
        _ => parse_primary(pair),
    }
}

fn parse_call_args(pair: Pair<Rule>) -> Result<Vec<Expr>> {
    debug_assert_eq!(pair.as_rule(), Rule::call);
    let mut args = Vec::new();
    for p in pair.into_inner() {
        // call contains expr separated by commas -> grammar emits only expr pairs inside
        if matches!(p.as_rule(), Rule::expr) {
            args.push(parse_expr(p)?);
        }
    }
    Ok(args)
}

fn parse_primary(pair: Pair<Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::primary => parse_primary(pair.into_inner().next().unwrap()),
        Rule::parens => parse_expr(pair.into_inner().next().unwrap()),
        Rule::ident => Ok(Expr::Var(pair.as_str().to_string())),
        Rule::number => parse_number(pair),
        Rule::boolean => {
            let inner = pair.into_inner().next().unwrap();
            let val = matches!(inner.as_rule(), Rule::true_kw);
            Ok(Expr::Literal(Primitive::Bool(val)))
        }
        Rule::string => {
            let s = unescape_string(pair.as_str())?;
            Ok(Expr::Literal(Primitive::Str(s)))
        }
        Rule::list => parse_list(pair),
        Rule::dict => parse_dict(pair),
        r => Err(Error::InternalParserError(format!("unexpected primary op: {:?}", r))),
    }
}

fn parse_number(pair: Pair<Rule>) -> Result<Expr> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::int => {
            let s = inner.as_str();
            let v: i64 = s.parse().map_err(|_| Error::ParseError(format!("invalid int: {}", s)))?;
            Ok(Expr::Literal(Primitive::Int(v)))
        }
        Rule::float => {
            let s = inner.as_str();
            let v: f64 = s.parse().map_err(|_| Error::ParseError(format!("invalid float: {}", s)))?;
            Ok(Expr::Literal(Primitive::Float(v)))
        }
        r => Err(Error::InternalParserError(format!("unexpected number: {:?}", r))),
    }
}

fn parse_list(pair: Pair<Rule>) -> Result<Expr> {
    let mut elems = Vec::new();
    for p in pair.into_inner() {
        if let Rule::expr = p.as_rule() {
            elems.push(parse_expr(p)?);
        }
    }
    Ok(Expr::ListLiteral(elems))
}

fn parse_dict(pair: Pair<Rule>) -> Result<Expr> {
    let mut items = Vec::new();
    for p in pair.into_inner() {
        if let Rule::pair = p.as_rule() {
            let mut it = p.into_inner();
            let key_pair = it.next().expect("pair key expr");
            let key = parse_expr(key_pair)?;
            let value_pair = it.next().expect("pair value expr");
            let value = parse_expr(value_pair)?;
            items.push((key, value));
        }
    }
    Ok(Expr::DictLiteral(items))
}

fn unescape_string(src: &str) -> Result<String> {
    // strip surrounding quotes if present (supports both ' and ")
    // let raw = if src.starts_with('"') && src.ends_with('"') && src.len() >= 2 {
    //     &src[1..src.len() - 1]
    // } else if src.starts_with('\'') && src.ends_with('\'') && src.len() >= 2 {
    //     &src[1..src.len() - 1]
    // } else {
    //     src
    // };
    let escape_char = src.chars().next().unwrap();
    let mut out = String::with_capacity(src.len() - 2);
    let mut chars = src[1..src.len() - 1].chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(next) if next == escape_char => out.push(escape_char),
                _ => return Err(Error::ParseError(format!("invalid escape character {}", c))),
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolated_expr() {
        let input = "123}x";
        let (expr, idx) = parse_internal(input, Rule::delimited_expr).unwrap();
        assert_eq!(expr, Expr::Literal(Primitive::Int(123)));
        assert_eq!(idx, 4);
    }
}
