use crate::jql::{error::JqlError, funcs::util::type_mismatch, proxy::Proxy};
use crate::json::value::{JsonNumeric, JsonValue};

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.data() {
        JsonValue::Number(_) => {
            let n = value
                .data()
                .try_as_float()
                .map_err(|_| type_mismatch("number", value.data()))?;
            Ok(Proxy::owned(JsonValue::number(n.abs())))
        }
        other => Err(type_mismatch("number", other)),
    }
}
