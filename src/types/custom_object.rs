use crate::types::error::Result;
use crate::types::value::Value;
use std::any::Any;
use std::fmt::{Debug, Display, Formatter};

pub trait CustomObject: Any {
    fn type_name(&self) -> &'static str {
        "object"
    }
    fn get_member(&self, name: &str) -> Result<Value> {
        Err(crate::types::error::Error::ResolveFailed(name.into()))
    }
    fn get_index(&self, index: i64) -> Result<Value> {
        Err(crate::types::error::Error::NotIndexable(index.to_string()))
    }
    fn get_key_value(&self, key: &str) -> Result<Value> {
        Err(crate::types::error::Error::NotIndexable(key.into()))
    }
    fn as_string(&self) -> Option<String> {
        None
    }
    fn as_float(&self) -> Option<f64> {
        None
    }
    fn as_int(&self) -> Option<i64> {
        None
    }
    fn as_bool(&self) -> Option<bool> {
        None
    }
    fn call(&self, _args: &[Value]) -> Result<Value> {
        Err(crate::types::error::Error::NotCallable)
    }
    fn equals(&self, _other: &Value) -> bool {
        false
    }
    fn display(&self) -> String {
        self.as_string().unwrap_or_else(|| self.type_name().into())
    }
    fn debug(&self) -> String {
        format!("<{}>", self.type_name())
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Display for dyn CustomObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl Debug for dyn CustomObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.debug())
    }
}
