use crate::types::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

impl Primitive {
    /// Rendering for humans. This is formatting, not coercion: it never fails,
    /// and it is deliberately not something a [`Coercions`](crate::types::coerce::Coercions)
    /// policy can influence.
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
