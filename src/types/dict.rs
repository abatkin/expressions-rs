use crate::types::error::{Error, Result};
use crate::types::object::Object;
use crate::types::primitive::Primitive;
use crate::types::value::Value;
use crate::types::{function, list};

use crate::types::function::method0;
use std::any::Any;
use std::collections::BTreeMap;
use std::rc::Rc;

pub fn new(map: BTreeMap<String, Value>) -> Value {
    Value::Object(Rc::new(DictObject::new(map)))
}
pub struct DictObject {
    map: BTreeMap<String, Value>,
}

impl DictObject {
    pub fn new(map: BTreeMap<String, Value>) -> DictObject {
        DictObject { map }
    }
}

impl Object for DictObject {
    fn type_name(&self) -> &'static str {
        "dict"
    }
    fn get_member(&self, name: &str) -> Result<Value> {
        match name {
            "length" => Ok(Value::from(self.map.len() as i64)),
            "keys" => {
                let keys: Vec<Value> = self.map.keys().cloned().map(Value::from).collect();
                Ok(function::method0(move || Ok(list::new(keys.clone()))))
            }
            "values" => {
                let vals: Vec<Value> = self.map.values().cloned().collect();
                Ok(method0(move || Ok(list::new(vals.clone()))))
            }
            "contains" => {
                let base = self.map.clone();
                Ok(function::method1(move |arg: &Value| {
                    if let Value::Primitive(Primitive::Str(s)) = arg {
                        Ok(Value::from(base.contains_key(s)))
                    } else {
                        Err(Error::TypeMismatch("contains expects a string".into()))
                    }
                }))
            }
            "get" => {
                let base = self.map.clone();
                Ok(function::new(std::rc::Rc::new(move |args: &[Value]| {
                    if args.len() != 2 {
                        return Err(Error::EvaluationFailed("expected 2 args".into()));
                    }
                    let key = match &args[0] {
                        Value::Primitive(Primitive::Str(s)) => s.clone(),
                        _ => return Err(Error::TypeMismatch("get expects string key".into())),
                    };
                    if let Some(v) = base.get(&key) { Ok(v.clone()) } else { Ok(args[1].clone()) }
                })))
            }
            _ => Err(Error::UnknownMember {
                type_name: "dict".into(),
                member: name.to_string(),
            }),
        }
    }

    fn get_key_value(&self, key: &str) -> Result<Value> {
        self.map.get(key).cloned().ok_or(Error::NoSuchKey(key.to_string()))
    }

    fn as_string(&self) -> Option<String> {
        Some(format!("{{{}}}", self.map.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<_>>().join(", ")))
    }

    fn as_bool(&self) -> Option<bool> {
        Some(!self.map.is_empty())
    }

    fn equals(&self, other: &Value) -> bool {
        if let Value::Object(other_obj) = other
            && let Some(other_dict) = other_obj.as_any().downcast_ref::<DictObject>()
        {
            self.map == other_dict.map
        } else {
            false
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
