use crate::jql::{error::JqlError, funcs::util::type_mismatch_array, proxy::Proxy};
use crate::json::value::JsonValue;

/// Deduplicate array items (deep equality), preserving first-seen order.
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.into_value() {
        JsonValue::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if !out.iter().any(|seen| seen == &item) {
                    out.push(item);
                }
            }
            Ok(Proxy::owned(JsonValue::array(out)))
        }
        other => Err(type_mismatch_array(&other)),
    }
}
