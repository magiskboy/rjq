use crate::jql::{error::JqlError, funcs::util::type_mismatch_array, proxy::Proxy};
use crate::json::value::JsonValue;

/// Flatten one level: nested arrays are concatenated; non-arrays are kept as-is.
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.into_value() {
        JsonValue::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    JsonValue::Array(inner) => out.extend(inner),
                    other => out.push(other),
                }
            }
            Ok(Proxy::owned(JsonValue::array(out)))
        }
        other => Err(type_mismatch_array(&other)),
    }
}
