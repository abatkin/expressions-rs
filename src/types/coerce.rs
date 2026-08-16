use crate::types::error::{Error, Result};
use crate::types::primitive::Primitive;
use crate::types::value::Value;

/// A coercion policy: decides how values of one type are read as another.
///
/// The evaluator owns the coercion boundary. Objects report their own opinion
/// through [`Object::as_bool`](crate::types::object::Object::as_bool) and friends;
/// a policy decides whether to consult that opinion, what the rules are for
/// primitives, and what happens when there is no answer.
///
/// A policy that only cares about one conversion should delegate the rest to
/// [`STANDARD`]:
///
/// ```ignore
/// impl Coercions for MyPolicy {
///     fn to_bool(&self, v: &Value) -> Result<bool> { /* custom rules */ }
///     fn to_number(&self, v: &Value) -> Result<f64> { STANDARD.to_number(v) }
/// }
/// ```
///
/// There is deliberately no string conversion here. Turning a value into text is
/// formatting, which cannot fail and belongs to `Display`.
pub trait Coercions {
    fn to_bool(&self, v: &Value) -> Result<bool>;
    fn to_number(&self, v: &Value) -> Result<Number>;
}

/// What a policy found when asked for a number.
///
/// Returning this rather than an `f64` is what lets integer operands produce
/// integer results: the policy reports which kind it found, and the operator
/// applies its own promotion rules. Asking "is it an int?" by probing a separate
/// `to_int` would instead make the *operator* guess, and a truncating policy
/// would silently turn `1.5 * 2` into `2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Int(i) => *i as f64,
            Number::Float(f) => *f,
        }
    }
}

impl From<Number> for Value {
    fn from(n: Number) -> Self {
        match n {
            Number::Int(i) => Value::Primitive(Primitive::Int(i)),
            Number::Float(f) => Value::Primitive(Primitive::Float(f)),
        }
    }
}

fn not_coercible(v: &Value, target: &'static str) -> Error {
    Error::NotCoercible {
        type_name: v.type_name().into(),
        target,
    }
}

/// The default policy, matching the language's built-in behavior.
pub struct StandardCoercions;

/// A shared [`StandardCoercions`], usable wherever a `&'static dyn Coercions` is wanted.
pub static STANDARD: StandardCoercions = StandardCoercions;

impl Coercions for StandardCoercions {
    fn to_bool(&self, v: &Value) -> Result<bool> {
        let b = match v {
            Value::Primitive(p) => p.coerce_bool(),
            Value::Object(obj) => obj.as_bool(),
        };
        b.ok_or_else(|| not_coercible(v, "bool"))
    }

    fn to_number(&self, v: &Value) -> Result<Number> {
        let n = match v {
            Value::Primitive(Primitive::Int(i)) => Some(Number::Int(*i)),
            Value::Primitive(Primitive::Float(f)) => Some(Number::Float(*f)),
            Value::Primitive(_) => None,
            // an object that reports an integer keeps its integer-ness
            Value::Object(obj) => obj.as_int().map(Number::Int).or_else(|| obj.as_float().map(Number::Float)),
        };
        n.ok_or_else(|| not_coercible(v, "number"))
    }
}

/// Ambient state handed to [`Object::call`](crate::types::object::Object::call).
///
/// Functions receive this at call time rather than capturing it at construction
/// time, which is what keeps it off the rest of the `Object` trait. Fields are
/// private so the context can grow (fuel, recursion depth, ...) without another
/// breaking change.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    coercions: &'a dyn Coercions,
}

impl<'a> Context<'a> {
    pub fn new(coercions: &'a dyn Coercions) -> Self {
        Self { coercions }
    }

    /// A context using [`STANDARD`].
    pub fn standard() -> Context<'static> {
        Context { coercions: &STANDARD }
    }

    pub fn coercions(&self) -> &'a dyn Coercions {
        self.coercions
    }

    pub fn to_bool(&self, v: &Value) -> Result<bool> {
        self.coercions.to_bool(v)
    }
    pub fn to_number(&self, v: &Value) -> Result<Number> {
        self.coercions.to_number(v)
    }
}

impl Default for Context<'static> {
    fn default() -> Self {
        Context::standard()
    }
}

/// A strict policy: only an actual bool is a bool, and only an actual number is
/// a number.
pub struct StrictCoercions;

impl Coercions for StrictCoercions {
    fn to_bool(&self, v: &Value) -> Result<bool> {
        match v {
            Value::Primitive(Primitive::Bool(b)) => Ok(*b),
            _ => Err(not_coercible(v, "bool")),
        }
    }

    fn to_number(&self, v: &Value) -> Result<Number> {
        match v {
            Value::Primitive(Primitive::Int(i)) => Ok(Number::Int(*i)),
            Value::Primitive(Primitive::Float(f)) => Ok(Number::Float(*f)),
            _ => Err(not_coercible(v, "number")),
        }
    }
}
