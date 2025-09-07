use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members, method0, method1};
use crate::types::value::{Primitive, Value};

impl Members for String {
    fn get_member(&self, name: &str) -> Result<Member> {
        match name {
            "length" => Ok(Member::Property(Value::from(self.len() as i64))),
            "toUpper" => {
                let base = self.clone();
                Ok(method0(move || Ok(Value::from(base.to_uppercase()))))
            }
            "toLower" => {
                let base = self.clone();
                Ok(method0(move || Ok(Value::from(base.to_lowercase()))))
            }
            "trim" => {
                let base = self.clone();
                Ok(method0(move || Ok(Value::from(base.trim().to_string()))))
            }
            "contains" => {
                let base = self.clone();
                Ok(method1(move |arg: &Value| {
                    if let Value::Primitive(Primitive::Str(s)) = arg {
                        Ok(Value::from(base.contains(s)))
                    } else {
                        Err(Error::TypeMismatch("contains expects a string".into()))
                    }
                }))
            }
            "substring" => {
                let base = self.clone();
                Ok(Member::Method(std::rc::Rc::new(move |args: &[Value]| {
                    if args.is_empty() || args.len() > 2 {
                        return Err(Error::EvaluationFailed("expected 1 or 2 args".into()));
                    }
                    // Collect chars for safe slicing
                    let chars: Vec<char> = base.chars().collect();
                    let len = chars.len() as i64;
                    // start index
                    let start_i = match &args[0] {
                        Value::Primitive(Primitive::Int(i)) => *i,
                        _ => return Err(Error::TypeMismatch("substring expects int start".into())),
                    };
                    let mut start = if start_i < 0 { len + start_i } else { start_i };
                    if start < 0 {
                        start = 0;
                    }
                    if start > len {
                        start = len;
                    }
                    // end index (exclusive)
                    let mut end = len;
                    if args.len() == 2 {
                        match &args[1] {
                            Value::Primitive(Primitive::Int(i)) => {
                                let e = if *i < 0 { len + *i } else { *i };
                                end = e.max(0).min(len);
                            }
                            _ => return Err(Error::TypeMismatch("substring expects int end".into())),
                        }
                    }
                    if start > end {
                        // empty
                        return Ok(Value::from(String::new()));
                    }
                    let sidx = start as usize;
                    let eidx = end as usize;
                    let sub: String = chars[sidx..eidx].iter().collect();
                    Ok(Value::from(sub))
                })))
            }
            _ => Err(Error::UnknownMember {
                type_name: "string".into(),
                member: name.to_string(),
            }),
        }
    }
}
