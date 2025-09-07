use crate::types::error::{Error, Result};
use crate::types::member::{Member, Members};
use crate::types::value::Value;

impl Members for Vec<Value> {
    fn get_member(&self, name: &str) -> Result<Member> {
        match name {
            "length" => Ok(Member::Property(Value::from(self.len() as i64))),
            _ => Err(Error::UnknownMember {
                type_name: "list".into(),
                member: name.to_string(),
            }),
        }
    }
}
