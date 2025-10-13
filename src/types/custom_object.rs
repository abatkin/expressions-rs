use crate::types::error::Result;
use crate::types::value::Value;

pub trait CustomObject {
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
    fn to_string(&self) -> Option<String> {
        None
    }
    fn to_float(&self) -> Option<f64> {
        None
    }
    fn to_int(&self) -> Option<i64> {
        None
    }
    fn to_bool(&self) -> Option<bool> {
        None
    }
    fn call(&self, _args: &[Value]) -> Result<Value> {
        Err(crate::types::error::Error::NotCallable)
    }
    fn equals(&self, _other: &Value) -> bool {
        false
    }
}
