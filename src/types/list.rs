use crate::types::error::{Error, Result};
use crate::types::object::Object;
use crate::types::value::{Primitive, Value, method1};
use std::any::Any;
use std::rc::Rc;

pub fn new(items: Vec<Value>) -> Value {
    Value::Object(Rc::new(ListObject::new(items)))
}

pub struct ListObject {
    list: Vec<Value>,
}

impl ListObject {
    pub fn new(list: Vec<Value>) -> ListObject {
        ListObject { list }
    }
}

impl Object for ListObject {
    fn type_name(&self) -> &'static str {
        "list"
    }

    fn get_member(&self, name: &str) -> Result<Value> {
        match name {
            "length" => Ok(Value::from(self.list.len() as i64)),
            "contains" => {
                let base = self.list.clone();
                Ok(method1(move |arg: &Value| Ok(Value::from(base.iter().any(|v| v == arg)))))
            }
            "get" => {
                let base = self.list.clone();
                Ok(Value::Func(std::rc::Rc::new(move |args: &[Value]| {
                    if args.len() != 2 {
                        return Err(Error::EvaluationFailed("expected 2 args".into()));
                    }
                    let idx = match &args[0] {
                        Value::Primitive(Primitive::Int(i)) => *i,
                        _ => return Err(Error::TypeMismatch("get expects int index".into())),
                    };
                    let len = base.len() as i64;
                    let eff = if idx < 0 { len + idx } else { idx };
                    if eff < 0 || eff >= len {
                        return Ok(args[1].clone());
                    }
                    Ok(base[eff as usize].clone())
                })))
            }
            "join" => {
                let base = self.list.clone();
                Ok(method1(move |arg: &Value| {
                    let joiner = if let Value::Primitive(Primitive::Str(s)) = arg {
                        s.clone()
                    } else {
                        return Err(Error::TypeMismatch("join expects a string joiner".into()));
                    };
                    let parts: Vec<String> = base.iter().map(|v| v.as_str_lossy()).collect();
                    Ok(Value::from(parts.join(&joiner)))
                }))
            }
            _ => Err(Error::UnknownMember {
                type_name: "list".into(),
                member: name.to_string(),
            }),
        }
    }

    fn get_index(&self, index: i64) -> Result<Value> {
        let len = self.list.len() as i64;
        let eff = if index < 0 { len + index } else { index };
        if eff < 0 || eff >= len {
            return Err(Error::IndexOutOfBounds { index, len: self.list.len() });
        }
        Ok(self.list[eff as usize].clone())
    }

    fn as_string(&self) -> Option<String> {
        Some(format!("[{}]", self.list.iter().map(|v| v.as_str_lossy()).collect::<Vec<_>>().join(", ")))
    }

    fn as_bool(&self) -> Option<bool> {
        Some(!self.list.is_empty())
    }

    fn equals(&self, other: &Value) -> bool {
        if let Value::Object(other_obj) = other
            && let Some(other_list) = other_obj.as_any().downcast_ref::<ListObject>()
        {
            self.list == *other_list.list
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
