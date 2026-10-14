use crate::jql::{
    error::JqlError,
    funcs::util::{type_mismatch, type_mismatch_array},
    proxy::Proxy,
};
use crate::json::value::{JsonNumeric, JsonValue};

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.data() {
        JsonValue::Array(items) => {
            if items.is_empty() {
                return Err(type_mismatch(
                    "non-empty array of number",
                    &JsonValue::Array(vec![]),
                ));
            }
            let mut s = 0.0_f64;
            for item in items {
                match item {
                    JsonValue::Number(_) => {
                        s += item
                            .try_as_float()
                            .map_err(|_| type_mismatch("number", item))?;
                    }
                    other => return Err(type_mismatch("number", other)),
                }
            }
            Ok(Proxy::owned(JsonValue::number(s / items.len() as f64)))
        }
        other => Err(type_mismatch_array(other)),
    }
}
