use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::type_mismatch_array,
    proxy::{Proxy, ValueRef},
};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>, predictive: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    match value.data {
        ValueRef::Borrowed(v) => match v {
            JsonValue::Array(items) => {
                for x in items {
                    let p = Engine::execute(Proxy::new(x), predictive)?;
                    if matches!(p.data(), JsonValue::True) {
                        return Ok(Proxy::owned(JsonValue::boolean(true)));
                    }
                }
                Ok(Proxy::owned(JsonValue::boolean(false)))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Shared(rc) => match rc.as_ref() {
            JsonValue::Array(items) => {
                for x in items {
                    let p = Engine::execute(Proxy::owned(x.clone()), predictive)?;
                    if matches!(p.data(), JsonValue::True) {
                        return Ok(Proxy::owned(JsonValue::boolean(true)));
                    }
                }
                Ok(Proxy::owned(JsonValue::boolean(false)))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Owned(v) => match v {
            JsonValue::Array(items) => {
                for x in items {
                    let p = Engine::execute(Proxy::owned(x), predictive)?;
                    if matches!(p.data(), JsonValue::True) {
                        return Ok(Proxy::owned(JsonValue::boolean(true)));
                    }
                }
                Ok(Proxy::owned(JsonValue::boolean(false)))
            }
            other => Err(type_mismatch_array(&other)),
        },
    }
}
