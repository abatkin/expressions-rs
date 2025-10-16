use simple_expressions::evaluator::{ExpressionEvaluator, InterpolationEvaluator, VariableResolver};
use simple_expressions::types::error::Result;
use simple_expressions::types::primitive::Primitive;
use simple_expressions::types::value::Value;

use std::collections::HashMap;

#[derive(Clone)]
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

    let expression_eval = ExpressionEvaluator::new(resolver.clone());
    let interpolation_eval = InterpolationEvaluator::new(resolver);

    expression_eval.evaluate("foo + 'bar'").unwrap();
    interpolation_eval.evaluate("barbar=${foo + 'bar'}").unwrap();

    Ok(())
}
