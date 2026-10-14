use crate::jql::{error::JqlError, funcs::util::type_mismatch, proxy::Proxy};
use crate::json::value::JsonValue;

/// Object keys (insertion order) or array indices as numbers.
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.data() {
        JsonValue::Object(obj) => {
            let keys = obj
                .iter()
                .map(|(k, _)| JsonValue::string(k.as_ref().to_string()))
                .collect();
            Ok(Proxy::owned(JsonValue::array(keys)))
        }
        JsonValue::Array(items) => {
            let keys = (0..items.len())
                .map(|i| JsonValue::number(i as f64))
                .collect();
            Ok(Proxy::owned(JsonValue::array(keys)))
        }
        other => Err(type_mismatch("array or object", other)),
    }
}
