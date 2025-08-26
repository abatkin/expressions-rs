use simple_expressions::evaluator::{Evaluator, SimpleConstVar, Variable, VariableResolver};
use simple_expressions::types::Atom;
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
    fn resolve(&self, path: &[String]) -> Option<Box<dyn Variable>> {
        if path.len() != 1 {
            return None;
        }
        let name = &path[0];
        let val = self.variables.get(name)?;
        let var = SimpleConstVar(Atom::Str(val.clone()));
        Some(Box::new(var))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut resolver = MapVariableResolver::new();
    resolver.set("foo".to_string(), "bar".to_string());

    let eval = Evaluator::new(resolver);
    eval.evaluate_string("foo + 'bar'").unwrap();
    eval.evaluate_interpolated("barbar=${foo + 'bar'}").unwrap();

    Ok(())
}
