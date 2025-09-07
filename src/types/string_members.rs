use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members, method0};
use crate::types::value::Value;

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
            _ => Err(Error::UnknownMember {
                type_name: "string".into(),
                member: name.to_string(),
            }),
        }
    }
}
