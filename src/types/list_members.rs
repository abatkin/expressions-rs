use crate::types::error::{Error, Result};
use crate::types::value::{Primitive, Value, method1};

pub fn get_list_member(list: &[Value], name: &str) -> Result<Value> {
    match name {
        "length" => Ok(Value::from(list.len() as i64)),
        "contains" => {
            let base = list.to_owned();
            Ok(method1(move |arg: &Value| Ok(Value::from(base.iter().any(|v| v == arg)))))
        }
        "get" => {
            let base = list.to_owned();
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
            let base = list.to_owned();
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
