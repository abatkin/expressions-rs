use crate::types::error::{Error, Result};
pub(crate) use crate::types::object::Object;
use crate::types::primitive::Primitive;
use crate::types::string_members::get_string_member;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

#[derive(Clone)]
pub enum Value {
    Primitive(Primitive),
    Object(Rc<dyn Object>),
}

impl Value {
    pub fn coerce_bool(&self) -> Option<bool> {
        match self {
            Value::Primitive(p) => p.coerce_bool(),
            Value::Object(obj) => obj.as_bool(),
        }
    }
    pub fn to_float_lossy(&self) -> Option<f64> {
        match self {
            Value::Primitive(p) => p.to_float_lossy(),
            Value::Object(obj) => obj.as_float(),
        }
    }
    pub fn as_str_lossy(&self) -> String {
        match self {
            Value::Primitive(p) => p.as_str_lossy(),
            Value::Object(obj) => obj.as_string().unwrap_or_else(|| format!("{}", obj)),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Primitive(Primitive::Str(_)) => "string",
            Value::Primitive(Primitive::Int(_)) | Value::Primitive(Primitive::Float(_)) => "number",
            Value::Primitive(Primitive::Bool(_)) => "bool",
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
            Value::Object(obj) => write!(f, "{}", obj),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Primitive(p1), Value::Primitive(p2)) => p1 == p2,
            (Value::Object(obj1), other) => obj1.equals(other),
            (other, Value::Object(obj2)) => obj2.equals(other),
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
