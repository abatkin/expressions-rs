//! Deep nesting must fail with an error, not overflow the stack. Each case runs on
//! a 2 MiB thread -- Rust's default for spawned threads, and smaller than the main
//! thread's -- so these pass only if the default limit leaves enough headroom
//! there, which in a debug build is the tight case.

use simple_expressions::evaluator::{VariableResolver, evaluate};
use simple_expressions::parser::{DEFAULT_MAX_DEPTH, ParseOptions, parse_expression, parse_expression_with};
use simple_expressions::template::Template;
use simple_expressions::types::error::Error;
use simple_expressions::types::value::Value;

struct Vars;
impl VariableResolver for Vars {
    fn resolve(&self, name: &str) -> Option<Value> {
        (name == "x").then(|| Value::from(1i64))
    }
}

fn on_stack<T: Send + 'static>(bytes: usize, f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(bytes).spawn(f).unwrap().join().unwrap()
}

const SMALL: usize = 2 * 1024 * 1024;

/// Every construct that nests, as an expression exactly `depth` levels deep, and
/// whether it evaluates successfully (rather than failing partway, e.g. on an
/// unknown member) -- either way, it must not overflow.
fn nested(depth: usize) -> Vec<(&'static str, String, bool)> {
    let n = depth - 1;
    vec![
        ("parens", format!("{}1{}", "(".repeat(n), ")".repeat(n)), true),
        ("list", format!("{}1{}", "[".repeat(n), "]".repeat(n)), true),
        ("dict", format!("{}1{}", "{'a': ".repeat(n), "}".repeat(n)), true),
        ("neg", format!("{}1", "-".repeat(n)), true),
        ("not", format!("{}x", "!".repeat(n)), true),
        ("member", format!("x{}", ".a".repeat(n)), false),
        ("index", format!("x{}", "[0]".repeat(n)), false),
        ("call", format!("x{}", "()".repeat(n)), false),
        ("call args", format!("{}x{}", "x(".repeat(n), ")".repeat(n)), false),
    ]
}

#[test]
fn the_default_limit_parses_and_evaluates_on_a_small_stack() {
    for (name, src, succeeds) in nested(DEFAULT_MAX_DEPTH) {
        let result = on_stack(SMALL, move || evaluate(&src, &Vars).map(|v| v.to_string()).map_err(|e| e.to_string()));
        assert!(!matches!(&result, Err(m) if m.contains("nested too deeply")), "{}: {:?}", name, result);
        assert_eq!(result.is_ok(), succeeds, "{}: {:?}", name, result);
    }
}

#[test]
fn one_level_past_the_limit_is_a_parse_error() {
    for (name, src, _) in nested(DEFAULT_MAX_DEPTH + 1) {
        match on_stack(SMALL, move || parse_expression(&src)) {
            Err(Error::ParseError { message, .. }) => assert!(message.contains("nested too deeply"), "{}: {}", name, message),
            other => panic!("{}: expected a parse error, got {:?}", name, other.map(|_| ())),
        }
    }
}

/// Far past the limit, an input is refused before anything recurses through it.
/// Brackets this deep stop in pest, whose own stack guard reports them first.
#[test]
fn far_past_the_limit_is_a_parse_error_not_a_crash() {
    for (name, src, _) in nested(20_000) {
        let guarded = cfg!(any(
            feature = "stack-guard",
            all(
                any(target_arch = "x86_64", target_arch = "aarch64"),
                any(
                    all(target_vendor = "unknown", target_os = "linux", target_env = "gnu"),
                    all(target_vendor = "apple", target_os = "macos"),
                    all(target_vendor = "pc", target_os = "windows", target_env = "msvc"),
                ),
            ),
        ));
        if !guarded && matches!(name, "parens" | "list" | "dict" | "call args") {
            continue;
        }
        let template = format!("a ${{{}}} b", src);
        let result = on_stack(SMALL, move || parse_expression(&src).map(|_| ()));
        assert!(matches!(result, Err(Error::ParseError { .. })), "{}: {:?}", name, result);
        let result = on_stack(SMALL, move || Template::parse(&template).map(|_| ()));
        assert!(matches!(result, Err(Error::ParseError { .. })), "template {}: {:?}", name, result);
    }
}

#[test]
fn the_limit_can_be_raised_and_lowered() {
    let deep = format!("{}1", "-".repeat(199));
    assert!(parse_expression(&deep).is_err());
    let raised = ParseOptions::default().with_max_depth(200);
    let value = on_stack(64 * 1024 * 1024, move || {
        let expr = parse_expression_with(&deep, &raised).unwrap();
        simple_expressions::evaluator::Evaluator::new(&Vars).evaluate(&expr).unwrap().to_string()
    });
    assert_eq!(value, "-1");

    let lowered = ParseOptions::default().with_max_depth(2);
    assert!(Template::parse_with("${1 + 2}", &lowered).is_ok());
    assert!(Template::parse_with("${1 + 2 * 3}", &lowered).is_err());
    // the limit applies to each expression on its own
    assert!(Template::parse_with("${1 + 2} ${1 + 2}", &lowered).is_ok());
}

#[test]
fn long_binary_chains_on_a_small_stack() {
    on_stack(SMALL, || {
        let source = format!("x{}", "+x".repeat(19_999));
        let template = Template::parse(&format!("${{{source}}}")).unwrap();
        assert_eq!(template.clone().evaluate(&Vars).unwrap(), "20000");

        let powers = format!("1{}", "^1".repeat(20_000));
        assert_eq!(evaluate(&powers, &Vars).unwrap().to_string(), "1");

        // alternating precedence is the path that recurses
        let mixed = format!("x{}", "+x*x^x".repeat(10_000));
        assert_eq!(evaluate(&mixed, &Vars).unwrap().to_string(), "10001");
    });
}
