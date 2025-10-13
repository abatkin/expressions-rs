use crate::types::error::{Error, Result};
use crate::types::object::Object;
use crate::types::value::Value;
use std::any::Any;
use std::rc::Rc;

pub type Callable = Rc<dyn Fn(&[Value]) -> Result<Value>>;

pub fn new(callable: Callable) -> Value {
    Value::Object(Rc::new(Function::new(callable)))
}

pub struct Function {
    callable: Callable,
}

impl Function {
    pub fn new(callable: Callable) -> Self {
        Self { callable }
    }
}

impl Object for Function {
    fn type_name(&self) -> &'static str {
        "function"
    }

    fn call(&self, args: &[Value]) -> Result<Value> {
        self.callable.as_ref()(args)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn method0<F>(f: F) -> Value
where
    F: Fn() -> Result<Value> + 'static,
{
    new(Rc::new(move |args: &[Value]| {
        if !args.is_empty() {
            return Err(Error::EvaluationFailed("expected 0 args".into()));
        }
        f()
    }))
}

pub fn method1<F>(f: F) -> Value
where
    F: Fn(&Value) -> Result<Value> + 'static,
{
    new(Rc::new(move |args: &[Value]| {
        if args.len() != 1 {
            return Err(Error::EvaluationFailed("expected 1 arg".into()));
        }
        f(&args[0])
    }))
}
