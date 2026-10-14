use crate::jql::{error::JqlError, funcs::util::type_mismatch_array, proxy::Proxy};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.into_value() {
        JsonValue::Array(mut items) => {
            items.reverse();
            Ok(Proxy::owned(JsonValue::array(items)))
        }
        other => Err(type_mismatch_array(&other)),
    }
}
