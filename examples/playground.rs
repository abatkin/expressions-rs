use simple_expressions::evaluator::{Evaluator, VariableResolver};
use simple_expressions::types::error::Result;
use simple_expressions::types::primitive::Primitive;
use simple_expressions::types::value::Value;

use std::collections::HashMap;

struct MapVariableResolver {
    variables: HashMap<String, String>,
}

impl MapVariableResolver {
    fn new() -> Self {
        Self { variables: HashMap::new() }
    }

    fn set(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }
}

impl VariableResolver for MapVariableResolver {
    fn resolve(&self, name: &str) -> Option<Value> {
        let val = self.variables.get(name)?;
        Some(Value::Primitive(Primitive::Str(val.to_string())))
    }
}

fn main() -> Result<()> {
    let mut resolver = MapVariableResolver::new();
    resolver.set("foo".to_string(), "bar".to_string());

    let eval = Evaluator::new(resolver);
    eval.evaluate_string("foo + 'bar'").unwrap();
    eval.evaluate_interpolated("barbar=${foo + 'bar'}").unwrap();

    Ok(())
}
