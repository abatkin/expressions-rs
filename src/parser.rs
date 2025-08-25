use chumsky::prelude::*;

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

fn expr_and_spacer<'src>() -> (impl Parser<'src, &'src str, ()> + Clone, impl Parser<'src, &'src str, Expr> + Clone) {
    // Whitespace and comments
    let line_comment = just("//").ignore_then(any().filter(|c: &char| *c != '\n').repeated()).ignored();
    let ws = one_of(" \t\r\n").repeated().at_least(1).ignored();
    let spacer = choice((ws, line_comment)).repeated().ignored();

    // Identifiers
    let ident = text::ident().map(|s: &str| s.to_string());

    let path = ident.separated_by(just('.')).at_least(1).collect::<Vec<_>>();

    // Strings: support single or double quotes with escapes \n, \\, \", \'
    let escape = just('\\').ignore_then(choice((
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        just('\\').to('\\'),
        just('"').to('"'),
        just('\'').to('\''),
        // Allow escaping newline directly
        just('\n').to('\n'),
    )));

    let string_sq = just('\'')
        .ignore_then(choice((escape, any().filter(|c: &char| *c != '\\' && *c != '\'' && *c != '\n'))).repeated().collect::<String>())
        .then_ignore(just('\''))
        .map(Atom::Str);

    let string_dq = just('"')
        .ignore_then(choice((escape, any().filter(|c: &char| *c != '\\' && *c != '"' && *c != '\n'))).repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(Atom::Str);

    // Numbers: optional leading '-', digits, optional fractional part
    let digits = text::digits(10);
    let number = just('-')
        .or_not()
        .then(digits)
        .then(just('.').then(digits).or_not())
        .to_slice()
        .map(|s: &str| if s.contains('.') { Atom::Float(s.parse::<f64>().unwrap()) } else { Atom::Int(s.parse::<i64>().unwrap()) });

    let boolean = choice((text::keyword("true").to(Atom::Bool(true)), text::keyword("false").to(Atom::Bool(false))));

    // Parentheses grouping will be handled via recursive expression parser
    let expr = recursive(|expr| {
        // Postfix: function call (single) after a dotted path
        let args = expr
            .clone()
            .separated_by(just(',').padded_by(spacer))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('(').padded_by(spacer), just(')').padded_by(spacer));

        let path_or_call = path.then(args.or_not()).map(|(p, maybe_args)| match maybe_args {
            Some(a) => Expr::Call { callee: Box::new(Expr::Path(p)), args: a },
            None => Expr::Path(p),
        });

        let member_chain = just('.').ignore_then(text::ident().map(|s: &str| s.to_string())).repeated().collect::<Vec<_>>();

        let path_call_and_members = path_or_call.then(member_chain).map(|(base, tail)| {
            let mut acc = base;
            for name in tail {
                acc = Expr::Member { object: Box::new(acc), field: name };
            }
            acc
        });

        let primary = choice((
            choice((string_sq, string_dq, number, boolean.clone())).map(Expr::Basic),
            path_call_and_members,
            expr.delimited_by(just('(').padded_by(spacer), just(')').padded_by(spacer)),
        ))
        .padded_by(spacer);

        // Unary '!'
        let unary = just('!').repeated().foldr(primary, |_bang, rhs| Expr::Unary { op: UnaryOp::Not, expr: Box::new(rhs) });

        // Exponentiation '^' (right-assoc) using recursion
        let pow = recursive(|pow| {
            unary.clone().then(just('^').padded_by(spacer).ignore_then(pow).or_not()).map(|(lhs, rhs)| match rhs {
                Some(r) => Expr::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(lhs),
                    right: Box::new(r),
                },
                None => lhs,
            })
        });

        let mul_div_mod = pow.clone().foldl(
            choice((just('*').to(BinaryOp::Mul), just('/').to(BinaryOp::Div), just('%').to(BinaryOp::Mod))).padded_by(spacer).then(pow).repeated(),
            |lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            },
        );

        let add_sub = mul_div_mod
            .clone()
            .foldl(choice((just('+').to(BinaryOp::Add), just('-').to(BinaryOp::Sub))).padded_by(spacer).then(mul_div_mod).repeated(), |lhs, (op, rhs)| {
                Expr::Binary {
                    op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                }
            });

        let cmp = add_sub.clone().foldl(
            choice((just("<=").to(BinaryOp::Le), just(">=").to(BinaryOp::Ge), just('<').to(BinaryOp::Lt), just('>').to(BinaryOp::Gt)))
                .padded_by(spacer)
                .then(add_sub)
                .repeated(),
            |lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            },
        );

        let eq = cmp
            .clone()
            .foldl(choice((just("==").to(BinaryOp::Eq), just("!=").to(BinaryOp::Ne))).padded_by(spacer).then(cmp).repeated(), |lhs, (op, rhs)| Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });

        let and = eq.clone().foldl(just("&&").to(BinaryOp::And).padded_by(spacer).then(eq).repeated(), |lhs, (op, rhs)| Expr::Binary {
            op,
            left: Box::new(lhs),
            right: Box::new(rhs),
        });

        let or = and.clone().foldl(just("||").to(BinaryOp::Or).padded_by(spacer).then(and).repeated(), |lhs, (op, rhs)| Expr::Binary {
            op,
            left: Box::new(lhs),
            right: Box::new(rhs),
        });

        or.padded_by(spacer)
    });

    (spacer, expr)
}

pub fn parser<'src>() -> impl Parser<'src, &'src str, Expr> {
    let (spacer, expr) = expr_and_spacer();

    // Allow multiple expressions separated by whitespace/comments and take the last one
    let program = spacer
        .clone()
        .ignore_then(expr.clone())
        .then_ignore(spacer.clone())
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|mut v: Vec<Expr>| v.pop().unwrap());

    program.then_ignore(end())
}

pub fn parse(input: &str) -> Result<Expr, String> {
    match parser().parse(input).into_result() {
        Ok(ast) => Ok(ast),
        Err(errs) => {
            let joined = errs.into_iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
            let snippet: String = input.chars().take(80).collect();
            let msg = if joined.trim().is_empty() { "parse error".to_string() } else { joined };
            Err(format!("{} (near: '{}')", msg, snippet))
        }
    }
}

// Parse an expression that must be terminated by a closing '}' and return
// the parsed Expr along with the number of bytes consumed (including the '}').
pub fn parse_in_braces(input: &str) -> Result<(Expr, usize), String> {
    // Use the existing expression parser and require a trailing '}' using parser combinators.
    // This leverages the parser's own handling of strings, escapes, and nesting instead of manual scanning.
    let (_spacer, expr) = expr_and_spacer();
    let p = expr
        .clone()
        .then_ignore(just('}'))
        .map_with(|e, extra| {
            let span = extra.span();
            (e, span.end)
        })
        .then_ignore(any().repeated()); // allow trailing content after '}' so callers can continue scanning

    match p.parse(input).into_result() {
        Ok((ast, consumed)) => Ok((ast, consumed)),
        Err(errs) => {
            let joined = errs.into_iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
            let snippet: String = input.chars().take(80).collect();
            let msg = if joined.trim().is_empty() { "parse error".to_string() } else { joined };
            Err(format!("{} inside interpolation (near: '{}')", msg, snippet))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literals() {
        assert_eq!(parse("123").unwrap(), Expr::Basic(Atom::Int(123)));
        assert_eq!(parse("-42").unwrap(), Expr::Basic(Atom::Int(-42)));
        assert_eq!(parse("3.14").unwrap(), Expr::Basic(Atom::Float(3.14)));
        assert_eq!(parse("true").unwrap(), Expr::Basic(Atom::Bool(true)));
        assert_eq!(parse("false").unwrap(), Expr::Basic(Atom::Bool(false)));
        assert_eq!(parse("'hi'").unwrap(), Expr::Basic(Atom::Str("hi".into())));
        assert_eq!(parse("'hi'\n// comment\n\"ok\"").unwrap(), Expr::Basic(Atom::Str("ok".into())));
    }

    #[test]
    fn parse_binary() {
        let ast = parse("1 + 2 * 3").unwrap();
        // ensure it builds
        if let Expr::Binary { op: BinaryOp::Add, .. } = ast {
        } else {
            panic!("bad ast");
        }
    }

    #[test]
    fn parse_boolean() {
        let ast = parse("1 + 2 * 3").unwrap();
        // ensure it builds
        if let Expr::Binary { op: BinaryOp::Add, .. } = ast {
        } else {
            panic!("bad ast");
        }
    }
    #[test]
    fn parse_calls_and_paths() {
        let ast = parse("foo.bar(baz, 1+2).qux").unwrap();
        // Just check it parses
        let _ = ast;
    }

    #[test]
    fn parse_in_braces_allows_suffix() {
        let res = parse_in_braces("'A'}-");
        assert!(res.is_ok(), "parse_in_braces failed: {:?}", res);
        let (expr, consumed) = res.unwrap();
        assert_eq!(consumed, 4, "consumed should include string and '}}'");
        assert_eq!(expr, Expr::Basic(Atom::Str("A".into())));
    }
}
