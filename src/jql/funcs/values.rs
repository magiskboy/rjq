use crate::jql::{error::JqlError, funcs::util::type_mismatch, proxy::Proxy};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.data() {
        JsonValue::Object(obj) => {
            let vals = obj.iter().map(|(_, v)| v.clone()).collect();
            Ok(Proxy::owned(JsonValue::array(vals)))
        }
        other => Err(type_mismatch("object", other)),
    }
}
