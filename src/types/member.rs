use crate::types::error::{Error, Result};
use crate::types::value::{Callable, Value};
use std::rc::Rc;

pub enum Member {
    Property(Value),
    Method(Callable),
}

impl Member {
    pub fn into_value(self) -> Value {
        match self {
            Member::Property(v) => v,
            Member::Method(f) => Value::Func(f),
        }
    }
}

pub trait Members {
    fn get_member(&self, name: &str) -> Result<Member>;
}

pub fn method0<F>(f: F) -> Member
where
    F: Fn() -> Result<Value> + 'static,
{
    Member::Method(Rc::new(move |args: &[Value]| {
        if !args.is_empty() {
            return Err(Error::EvaluationFailed("expected 0 args".into()));
        }
        f()
    }))
}

pub fn method1<F>(f: F) -> Member
where
    F: Fn(&Value) -> Result<Value> + 'static,
{
    Member::Method(Rc::new(move |args: &[Value]| {
        if args.len() != 1 {
            return Err(Error::EvaluationFailed("expected 1 arg".into()));
        }
        f(&args[0])
    }))
}
