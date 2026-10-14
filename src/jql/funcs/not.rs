use crate::jql::{
    error::JqlError,
    funcs::util::type_mismatch,
    proxy::Proxy,
};
use crate::json::value::{JsonLogic, JsonValue};

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.data().try_as_bool() {
        Ok(b) => Ok(Proxy::owned(JsonValue::boolean(!b))),
        Err(_) => Err(type_mismatch("boolean", value.data())),
    }
}
