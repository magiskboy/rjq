use crate::jql::{error::JqlError, funcs::util::type_name, proxy::Proxy};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    Ok(Proxy::owned(JsonValue::string(type_name(value.data()))))
}
