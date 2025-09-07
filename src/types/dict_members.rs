use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members, method0, method1};
use crate::types::value::{Primitive, Value};
use std::collections::BTreeMap;

impl Members for BTreeMap<String, Value> {
    fn get_member(&self, name: &str) -> Result<Member> {
        match name {
            "length" => Ok(Member::Property(Value::from(self.len() as i64))),
            "keys" => {
                let keys: Vec<Value> = self.keys().cloned().map(Value::from).collect();
                Ok(method0(move || Ok(Value::List(keys.clone()))))
            }
            "values" => {
                let vals: Vec<Value> = self.values().cloned().collect();
                Ok(method0(move || Ok(Value::List(vals.clone()))))
            }
            "contains" => {
                let base = self.clone();
                Ok(method1(move |arg: &Value| {
                    if let Value::Primitive(Primitive::Str(s)) = arg {
                        Ok(Value::from(base.contains_key(s)))
                    } else {
                        Err(Error::TypeMismatch("contains expects a string".into()))
                    }
                }))
            }
            "get" => {
                let base = self.clone();
                Ok(Member::Method(std::rc::Rc::new(move |args: &[Value]| {
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
}
