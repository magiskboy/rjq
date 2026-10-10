use crate::json::{json_load, value::JsonValue};

/// Shared sample document covering the JQL surface area used by module tests.
pub fn sample() -> JsonValue<'static> {
    json_load(include_str!("sample.json").as_bytes())
        .expect("tests/jql/sample.json must be valid JSON")
}

pub fn at(path: &str) -> JsonValue<'static> {
    use crate::jql::proxy::Proxy;
    Proxy::new(&sample())
        .get(path)
        .unwrap_or_else(|e| panic!("fixture path {path:?} should resolve: {e}"))
        .data()
        .clone()
        .into_owned()
}
