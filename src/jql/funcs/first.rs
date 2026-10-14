use crate::jql::{error::JqlError, proxy::Proxy};

pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    Ok(value.take_first())
}
