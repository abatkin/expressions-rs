use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members, method1};
use crate::types::value::{Primitive, Value};

impl Members for Vec<Value> {
    fn get_member(&self, name: &str) -> Result<Member> {
        match name {
            "length" => Ok(Member::Property(Value::from(self.len() as i64))),
            "contains" => {
                let base = self.clone();
                Ok(method1(move |arg: &Value| Ok(Value::from(base.iter().any(|v| v == arg)))))
            }
            "get" => {
                let base = self.clone();
                Ok(Member::Method(std::rc::Rc::new(move |args: &[Value]| {
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
                let base = self.clone();
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
}
