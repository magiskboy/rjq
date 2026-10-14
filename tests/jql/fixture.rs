use crate::jql::{engine::Engine, jql_parse, proxy::Proxy};
use crate::json::{json_load, value::JsonValue};

/// Shared sample document covering the JQL surface area used by module tests.
pub fn sample() -> JsonValue<'static> {
    json_load(include_str!("sample.json").as_bytes())
        .expect("tests/jql/sample.json must be valid JSON")
}

pub fn at(path: &str) -> JsonValue<'static> {
    let data = sample();
    let ast = jql_parse(path).unwrap_or_else(|e| panic!("fixture path {path:?} should parse: {e}"));
    Engine::execute(Proxy::new(&data), &ast)
        .unwrap_or_else(|e| panic!("fixture path {path:?} should resolve: {e}"))
        .data()
        .clone()
        .into_owned()
}
