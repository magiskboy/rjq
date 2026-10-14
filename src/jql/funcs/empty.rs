use crate::jql::{
    error::{JqlError, JqlErrorKind},
    funcs::util::type_name,
    proxy::Proxy,
};
use crate::json::value::JsonValue;

/// True when the current array/object/string has length 0.
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    let is_empty = match value.data() {
        JsonValue::Array(items) => items.is_empty(),
        JsonValue::Object(obj) => obj.is_empty(),
        JsonValue::String(s) => s.is_empty(),
        other => {
            return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                expected: "array, object, or string",
                found: type_name(other),
            }));
        }
    };
    Ok(Proxy::owned(JsonValue::boolean(is_empty)))
}
