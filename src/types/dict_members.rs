use crate::types::error::{Error, Result};
use crate::types::value::{Primitive, Value, method0, method1};
use std::collections::BTreeMap;

pub fn get_dict_member(map: &BTreeMap<String, Value>, name: &str) -> Result<Value> {
    match name {
        "length" => Ok(Value::from(map.len() as i64)),
        "keys" => {
            let keys: Vec<Value> = map.keys().cloned().map(Value::from).collect();
            Ok(method0(move || Ok(Value::List(keys.clone()))))
        }
        "values" => {
            let vals: Vec<Value> = map.values().cloned().collect();
            Ok(method0(move || Ok(Value::List(vals.clone()))))
        }
        "contains" => {
            let base = map.clone();
            Ok(method1(move |arg: &Value| {
                if let Value::Primitive(Primitive::Str(s)) = arg {
                    Ok(Value::from(base.contains_key(s)))
                } else {
                    Err(Error::TypeMismatch("contains expects a string".into()))
                }
            }))
        }
        "get" => {
            let base = map.clone();
            Ok(Value::Func(std::rc::Rc::new(move |args: &[Value]| {
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
