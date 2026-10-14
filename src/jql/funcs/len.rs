use crate::jql::{
    error::{JqlError, JqlErrorKind},
    funcs::util::type_name,
    proxy::Proxy,
};
use crate::json::value::JsonValue;

/// Length of the current value: array/object entry count, or string char count.
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    let n = match value.data() {
        JsonValue::Array(items) => items.len(),
        JsonValue::Object(obj) => obj.len(),
        JsonValue::String(s) => s.chars().count(),
        other => {
            return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                expected: "array, object, or string",
                found: type_name(other),
            }));
        }
    };
    Ok(Proxy::owned(JsonValue::number(n as f64)))
}
