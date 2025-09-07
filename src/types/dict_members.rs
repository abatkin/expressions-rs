use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members, method0};
use crate::types::value::Value;
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
            _ => Err(Error::UnknownMember {
                type_name: "dict".into(),
                member: name.to_string(),
            }),
        }
    }
}
