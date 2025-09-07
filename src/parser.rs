use crate::types::error::{Error, Result};
use crate::types::expression::{BinaryOp, Expr, UnaryOp};
use crate::types::value::Primitive;
use chumsky::prelude::*;

// Postfix operators: call, index, member
#[derive(Debug, Clone)]
enum Postfix {
    Call(Vec<Expr>),
    Index(Expr),
    Member(String),
}

fn expr_and_spacer<'src>() -> (impl Parser<'src, &'src str, ()> + Clone, impl Parser<'src, &'src str, Expr> + Clone) {
    // Whitespace and comments
    let line_comment = just("//").ignore_then(any().filter(|c: &char| *c != '\n').repeated()).ignored();
    let ws = one_of(" \t\r\n").repeated().at_least(1).ignored();
    let spacer = choice((ws, line_comment)).repeated().ignored();

    // Identifiers
    let ident = text::ident().map(|s: &str| s.to_string());

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
        .map(Primitive::Str);

    let string_dq = just('"')
        .ignore_then(choice((escape, any().filter(|c: &char| *c != '\\' && *c != '"' && *c != '\n'))).repeated().collect::<String>())
        .then_ignore(just('"'))
        .map(Primitive::Str);

    // Numbers: optional leading '-', digits, optional fractional part
    let digits = text::digits(10);
    let number = just('-')
        .or_not()
        .then(digits)
        .then(just('.').then(digits).or_not())
        .to_slice()
        .map(|s: &str| if s.contains('.') { Primitive::Float(s.parse::<f64>().unwrap()) } else { Primitive::Int(s.parse::<i64>().unwrap()) });

    let boolean = choice((text::keyword("true").to(Primitive::Bool(true)), text::keyword("false").to(Primitive::Bool(false))));

    // Parentheses grouping will be handled via recursive expression parser
    let expr = recursive(|expr| {
        // Arguments list for calls
        let args = expr
            .clone()
            .separated_by(just(',').padded_by(spacer))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('(').padded_by(spacer), just(')').padded_by(spacer));

        // List literal: [expr, expr, ...]
        let list_lit = expr
            .clone()
            .separated_by(just(',').padded_by(spacer))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('[').padded_by(spacer), just(']').padded_by(spacer))
            .map(Expr::ListLiteral);

        // Dict literal: {key_expr: value_expr, ...} where key_expr can be any expression; runtime enforces string keys
        let dict_pair = expr.clone().then_ignore(just(':').padded_by(spacer)).then(expr.clone());
        let dict_lit = dict_pair
            .separated_by(just(',').padded_by(spacer))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('{').padded_by(spacer), just('}').padded_by(spacer))
            .map(Expr::DictLiteral);

        // Primary: literals, identifiers as Var, parenthesized, list/dict literals
        let primary = choice((
            dict_lit,
            list_lit,
            choice((string_sq, string_dq, number, boolean.clone())).map(Expr::Literal),
            ident.map(Expr::Var),
            expr.clone().delimited_by(just('(').padded_by(spacer), just(')').padded_by(spacer)),
        ))
        .padded_by(spacer);

        let index = expr.clone().delimited_by(just('[').padded_by(spacer), just(']').padded_by(spacer)).map(Postfix::Index);

        let member = just('.').ignore_then(text::ident().map(|s: &str| s.to_string())).map(Postfix::Member);

        let call = args.clone().map(Postfix::Call);

        let postfix_chain = choice((call, index, member)).repeated().collect::<Vec<_>>();

        let postfix = primary.then(postfix_chain).map(|(base, posts)| {
            let mut acc = base;
            for p in posts {
                acc = match p {
                    Postfix::Call(a) => Expr::Call { callee: Box::new(acc), args: a },
                    Postfix::Index(i) => Expr::Index { object: Box::new(acc), index: Box::new(i) },
                    Postfix::Member(f) => Expr::Member { object: Box::new(acc), field: f },
                };
            }
            acc
        });

        // Unary '!'
        let unary = just('!').repeated().foldr(postfix, |_bang, rhs| Expr::Unary { op: UnaryOp::Not, expr: Box::new(rhs) });

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

// Main entry point for parsing an expression, returns the AST (Expr) or Error
pub fn parse(input: &str) -> Result<Expr> {
    match parser().parse(input).into_result() {
        Ok(ast) => Ok(ast),
        Err(errs) => {
            let joined = errs.into_iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
            let snippet: String = input.chars().take(80).collect();
            let msg = if joined.trim().is_empty() { "parse error".to_string() } else { joined };
            Err(Error::ParseFailed(msg, snippet))
        }
    }
}

// Parse an expression that must be terminated by a closing '}' and return
// the parsed Expr along with the number of bytes consumed (including the '}').
pub(crate) fn parse_in_braces(input: &str) -> Result<(Expr, usize)> {
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
            Err(Error::ParseFailed(msg, snippet))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literals() {
        assert_eq!(parse("123").unwrap(), Expr::Literal(Primitive::Int(123)));
        assert_eq!(parse("-42").unwrap(), Expr::Literal(Primitive::Int(-42)));
        assert_eq!(parse("3.14").unwrap(), Expr::Literal(Primitive::Float(3.14)));
        assert_eq!(parse("true").unwrap(), Expr::Literal(Primitive::Bool(true)));
        assert_eq!(parse("false").unwrap(), Expr::Literal(Primitive::Bool(false)));
        assert_eq!(parse("'hi'").unwrap(), Expr::Literal(Primitive::Str("hi".into())));
        assert_eq!(parse("'hi'\n// comment\n\"ok\"").unwrap(), Expr::Literal(Primitive::Str("ok".into())));
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
    fn list_literals() {
        use Expr::*;
        assert_eq!(parse("[1, 2, 3]").unwrap(), ListLiteral(vec![Literal(Primitive::Int(1)), Literal(Primitive::Int(2)), Literal(Primitive::Int(3))]));
        assert_eq!(parse("[1,2,]").unwrap(), ListLiteral(vec![Literal(Primitive::Int(1)), Literal(Primitive::Int(2))]));
        assert_eq!(
            parse("[1, [2], 3]").unwrap(),
            ListLiteral(vec![Literal(Primitive::Int(1)), ListLiteral(vec![Literal(Primitive::Int(2))]), Literal(Primitive::Int(3)),])
        );
    }

    #[test]
    fn dict_literals() {
        use Expr::*;
        let d = parse("{\"a\": 1, \"b\": 2}").unwrap();
        assert_eq!(
            d,
            DictLiteral(vec![(Literal(Primitive::Str("a".into())), Literal(Primitive::Int(1))), (Literal(Primitive::Str("b".into())), Literal(Primitive::Int(2))),])
        );
        let d2 = parse("{\"a\": 1,}").unwrap();
        assert_eq!(d2, DictLiteral(vec![(Literal(Primitive::Str("a".into())), Literal(Primitive::Int(1)))]));

        // allow non-string keys at parse-time; runtime will enforce type
        assert_eq!(parse("{a: 1}").unwrap(), DictLiteral(vec![(Var("a".into()), Literal(Primitive::Int(1)))]));
        assert_eq!(parse("{1: 2}").unwrap(), DictLiteral(vec![(Literal(Primitive::Int(1)), Literal(Primitive::Int(2)))]));

        // still reject malformed missing colon
        match parse("{\"a\" 1}") {
            Err(Error::ParseFailed(_, _)) => (),
            other => panic!("expected parse failed, got {:?}", other),
        }
    }

    #[test]
    fn postfix_combinations() {
        use Expr::*;
        let ast = parse("{\"xs\": [10, 20]}[\"xs\"][1]").unwrap();
        // Expect Index(Index(DictLiteral, "xs"), 1)
        match ast {
            Index { object, index } => {
                assert_eq!(*index, Literal(Primitive::Int(1)));
                match *object {
                    Index { object: inner_obj, index: inner_idx } => {
                        match *inner_idx {
                            Literal(Primitive::Str(ref s)) => assert_eq!(s, "xs"),
                            _ => panic!("inner index should be string literal"),
                        }
                        match *inner_obj {
                            DictLiteral(_) => (),
                            _ => panic!("inner object should be dict literal"),
                        }
                    }
                    _ => panic!("outer object should be index"),
                }
            }
            _ => panic!("bad ast shape: {:?}", ast),
        }

        let ast2 = parse("{\"a\": 1}.a").unwrap();
        match ast2 {
            Member { ref object, ref field } => {
                assert_eq!(field, "a");
                match **object {
                    DictLiteral(_) => (),
                    _ => panic!("member base should be dict literal"),
                }
            }
            _ => panic!("bad ast shape"),
        }

        let ast3 = parse("{\"a\": [1,2,3]}.a[0]").unwrap();
        match ast3 {
            Index { object, index } => {
                assert_eq!(*index, Literal(Primitive::Int(0)));
                match *object {
                    Member { object: base, field } => {
                        assert_eq!(field, "a");
                        match *base {
                            DictLiteral(_) => (),
                            _ => panic!("member base should be dict literal"),
                        }
                    }
                    _ => panic!("expected member then index"),
                }
            }
            _ => panic!("bad ast shape"),
        }
    }

    #[test]
    fn postfix_member_chain_var() {
        let ast = parse("a.b.c").unwrap();
        let expected = Expr::Member {
            object: Box::new(Expr::Member {
                object: Box::new(Expr::Var("a".into())),
                field: "b".into(),
            }),
            field: "c".into(),
        };
        assert_eq!(ast, expected);
    }

    #[test]
    fn postfix_mixed_chain_shapes() {
        let ast = parse("a.b(1, 2).c[0].d(e)").unwrap();
        // a.b(1,2).c[0].d(e)
        // Verify outermost is Call(...)
        match ast {
            Expr::Call { ref callee, ref args } => {
                assert_eq!(args.len(), 1);
                // callee should be Member(..., field: "d")
                match **callee {
                    Expr::Member { ref object, ref field } => {
                        assert_eq!(field, "d");
                        // object should be Index(Member(Call(Member(Var("a"),"b"), [1,2]), field:"c"), index:0)
                        match **object {
                            Expr::Index { ref object, ref index } => {
                                // index == 0
                                assert_eq!(**index, Expr::Literal(Primitive::Int(0)));
                                // object is Member(..., "c")
                                match **object {
                                    Expr::Member { ref object, ref field } => {
                                        assert_eq!(field, "c");
                                        // object is Call(Member(Var("a"),"b"), [1,2])
                                        match **object {
                                            Expr::Call { ref callee, ref args } => {
                                                assert_eq!(args.len(), 2);
                                                assert_eq!(args[0], Expr::Literal(Primitive::Int(1)));
                                                // second arg is 2
                                                assert_eq!(args[1], Expr::Literal(Primitive::Int(2)));
                                                // callee: Member(Var("a"),"b")
                                                let expected_callee = Expr::Member {
                                                    object: Box::new(Expr::Var("a".into())),
                                                    field: "b".into(),
                                                };
                                                assert_eq!(**callee, expected_callee);
                                            }
                                            _ => panic!("expected call before .c"),
                                        }
                                    }
                                    _ => panic!("expected member .c"),
                                }
                            }
                            _ => panic!("expected index [0]"),
                        }
                    }
                    _ => panic!("expected outer member .d"),
                }
            }
            _ => panic!("expected outer call (e)"),
        }
    }

    #[test]
    fn postfix_parenthesized_base() {
        // (a + b).c(d)
        let ast = parse("(a + b).c(d)").unwrap();
        match ast {
            Expr::Call { callee, args } => {
                assert_eq!(args.len(), 1);
                match *callee {
                    Expr::Member { object, field } => {
                        assert_eq!(field, "c");
                        match *object {
                            Expr::Binary { op: BinaryOp::Add, .. } => (),
                            _ => panic!("member should be applied to parenthesized binary expr"),
                        }
                    }
                    _ => panic!("expected member callee"),
                }
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn postfix_call_chains() {
        let ast = parse("foo(1)(2)(3)").unwrap();
        // foo(1)(2)(3) => Call(Call(Call(Var("foo"),1),2),3)
        fn is_int(e: &Expr, v: i64) -> bool {
            *e == Expr::Literal(Primitive::Int(v))
        }
        match ast {
            Expr::Call { callee: c3, args: a3 } => {
                assert!(a3.len() == 1 && is_int(&a3[0], 3));
                match *c3 {
                    Expr::Call { callee: c2, args: a2 } => {
                        assert!(a2.len() == 1 && is_int(&a2[0], 2));
                        match *c2 {
                            Expr::Call { callee: c1, args: a1 } => {
                                assert!(a1.len() == 1 && is_int(&a1[0], 1));
                                assert_eq!(*c1, Expr::Var("foo".into()));
                            }
                            _ => panic!("expected second call"),
                        }
                    }
                    _ => panic!("expected first call"),
                }
            }
            _ => panic!("expected outer call"),
        }
    }

    #[test]
    fn postfix_index_chains() {
        let ast = parse("arr[1+2][0]").unwrap();
        match ast {
            Expr::Index { object, index } => {
                assert_eq!(*index, Expr::Literal(Primitive::Int(0)));
                match *object {
                    Expr::Index { object, index } => {
                        match *index {
                            Expr::Binary { op: BinaryOp::Add, .. } => (),
                            _ => panic!("expected 1+2 as index expr"),
                        }
                        assert_eq!(*object, Expr::Var("arr".into()));
                    }
                    _ => panic!("expected inner index"),
                }
            }
            _ => panic!("expected outer index"),
        }
    }

    #[test]
    fn precedence_with_postfix_vs_add() {
        let ast = parse("a.b + c.d").unwrap();
        match ast {
            Expr::Binary { op: BinaryOp::Add, left, right } => {
                // both sides should be postfix chains
                match *left {
                    Expr::Member { object, field } => {
                        assert_eq!(field, "b");
                        assert_eq!(*object, Expr::Var("a".into()));
                    }
                    _ => panic!("left not a member"),
                }
                match *right {
                    Expr::Member { object, field } => {
                        assert_eq!(field, "d");
                        assert_eq!(*object, Expr::Var("c".into()));
                    }
                    _ => panic!("right not a member"),
                }
            }
            _ => panic!("top not add"),
        }
    }

    #[test]
    fn precedence_with_postfix_and_logical() {
        let ast = parse("a.b(c) && d.e").unwrap();
        match ast {
            Expr::Binary { op: BinaryOp::And, left, right } => {
                match *left {
                    Expr::Call { callee, args } => {
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], Expr::Var("c".into()));
                        match *callee {
                            Expr::Member { object, field } => {
                                assert_eq!(field, "b");
                                assert_eq!(*object, Expr::Var("a".into()));
                            }
                            _ => panic!("left not call(member)"),
                        }
                    }
                    _ => panic!("left not call"),
                }
                match *right {
                    Expr::Member { object, field } => {
                        assert_eq!(field, "e");
                        assert_eq!(*object, Expr::Var("d".into()));
                    }
                    _ => panic!("right not member"),
                }
            }
            _ => panic!("top not &&"),
        }
    }

    #[test]
    fn parse_in_braces_allows_suffix() {
        let res = parse_in_braces("'A'}-");
        assert!(res.is_ok(), "parse_in_braces failed: {:?}", res);
        let (expr, consumed) = res.unwrap();
        assert_eq!(consumed, 4, "consumed should include string and '}}'");
        assert_eq!(expr, Expr::Literal(Primitive::Str("A".into())));
    }
}
