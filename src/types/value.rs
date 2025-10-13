use crate::types::error::{Error, Result};
pub(crate) use crate::types::object::CustomObject;
use crate::types::string_members::get_string_member;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

impl Primitive {
    pub fn as_bool(&self) -> Option<bool> {
        self.coerce_bool()
    }
    pub fn as_int(&self) -> Option<i64> {
        if let Primitive::Int(i) = self { Some(*i) } else { None }
    }
    pub fn as_float(&self) -> Option<f64> {
        if let Primitive::Float(f) = self { Some(*f) } else { None }
    }
    pub fn as_str(&self) -> String {
        self.as_str_lossy()
    }

    // Newer, explicit coercions
    pub fn coerce_bool(&self) -> Option<bool> {
        match self {
            Primitive::Int(i) => Some(*i != 0),
            Primitive::Float(f) => Some(*f != 0.0),
            Primitive::Str(s) if s == "true" || s == "false" => Some(s == "true"),
            Primitive::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Primitive::Float(f) => Some(*f),
            Primitive::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
    pub fn as_str_lossy(&self) -> String {
        match self {
            Primitive::Str(s) => s.clone(),
            Primitive::Int(i) => i.to_string(),
            Primitive::Float(f) => f.to_string(),
            Primitive::Bool(b) => b.to_string(),
        }
    }
}

impl From<i64> for Primitive {
    fn from(v: i64) -> Self {
        Primitive::Int(v)
    }
}
impl From<f64> for Primitive {
    fn from(v: f64) -> Self {
        Primitive::Float(v)
    }
}
impl From<bool> for Primitive {
    fn from(v: bool) -> Self {
        Primitive::Bool(v)
    }
}
impl From<String> for Primitive {
    fn from(v: String) -> Self {
        Primitive::Str(v)
    }
}
impl From<&str> for Primitive {
    fn from(v: &str) -> Self {
        Primitive::Str(v.to_string())
    }
}

impl TryFrom<Primitive> for i64 {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Int(i) = p { Ok(i) } else { Err(Error::TypeMismatch("expected int".into())) }
    }
}
impl TryFrom<Primitive> for f64 {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Float(f) = p { Ok(f) } else { Err(Error::TypeMismatch("expected float".into())) }
    }
}
impl TryFrom<Primitive> for bool {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Bool(b) = p { Ok(b) } else { Err(Error::TypeMismatch("expected bool".into())) }
    }
}
impl TryFrom<Primitive> for String {
    type Error = Error;
    fn try_from(p: Primitive) -> Result<Self> {
        if let Primitive::Str(s) = p { Ok(s) } else { Err(Error::TypeMismatch("expected string".into())) }
    }
}

pub type Callable = Rc<dyn Fn(&[Value]) -> Result<Value>>;
#[derive(Clone)]
pub enum Value {
    Primitive(Primitive),
    Func(Callable),
    Object(Rc<dyn CustomObject>),
}

impl Value {
    pub fn coerce_bool(&self) -> Option<bool> {
        match self {
            Value::Primitive(p) => p.coerce_bool(),
            Value::Func(_) => None,
            Value::Object(obj) => obj.as_bool(),
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Value::Primitive(p) => p.to_float_lossy(),
            Value::Object(obj) => obj.as_float(),
            _ => None,
        }
    }
    pub fn as_str_lossy(&self) -> String {
        match self {
            Value::Primitive(p) => p.as_str_lossy(),
            Value::Func(_) => "<func>".into(),
            Value::Object(obj) => obj.as_string().unwrap_or_else(|| format!("{}", obj)),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Primitive(Primitive::Str(_)) => "string",
            Value::Primitive(Primitive::Int(_)) | Value::Primitive(Primitive::Float(_)) => "number",
            Value::Primitive(Primitive::Bool(_)) => "bool",
            Value::Func(_) => "func",
            Value::Object(obj) => obj.type_name(),
        }
    }

    pub fn get_member(&self, name: &str) -> Result<Value> {
        match self {
            Value::Primitive(Primitive::Str(s)) => get_string_member(s, name),
            Value::Object(obj) => obj.get_member(name),
            _ => Err(Error::UnknownMember {
                type_name: self.type_name().into(),
                member: name.to_string(),
            }),
        }
    }
}

impl Display for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str_lossy())
    }
}
impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str_lossy())
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Primitive(p) => write!(f, "{}", p),
            Value::Func(_) => write!(f, "<func>"),
            Value::Object(obj) => write!(f, "{}", obj),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Primitive(p1), Value::Primitive(p2)) => p1 == p2,
            (Value::Func(_), Value::Func(_)) => false,
            (Value::Object(obj1), other) => obj1.equals(other),
            (other, Value::Object(obj2)) => obj2.equals(other),
            _ => false,
        }
    }
}

impl From<Primitive> for Value {
    fn from(p: Primitive) -> Self {
        Value::Primitive(p)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Primitive(v.into())
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Primitive(v.into())
    }
}

impl TryFrom<Value> for i64 {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected int".into())) }
    }
}
impl TryFrom<Value> for f64 {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected float".into())) }
    }
}
impl TryFrom<Value> for bool {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected bool".into())) }
    }
}
impl TryFrom<Value> for String {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self> {
        if let Value::Primitive(p) = v { p.try_into() } else { Err(Error::TypeMismatch("expected string".into())) }
    }
}

pub fn method0<F>(f: F) -> Value
where
    F: Fn() -> Result<Value> + 'static,
{
    Value::Func(Rc::new(move |args: &[Value]| {
        if !args.is_empty() {
            return Err(Error::EvaluationFailed("expected 0 args".into()));
        }
        f()
    }))
}

pub fn method1<F>(f: F) -> Value
where
    F: Fn(&Value) -> Result<Value> + 'static,
{
    Value::Func(Rc::new(move |args: &[Value]| {
        if args.len() != 1 {
            return Err(Error::EvaluationFailed("expected 1 arg".into()));
        }
        f(&args[0])
    }))
}
