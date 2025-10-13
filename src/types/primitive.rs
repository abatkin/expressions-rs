use crate::types::error::{Error, Result};

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
