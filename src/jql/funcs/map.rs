use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::type_mismatch_array,
    proxy::{Proxy, ValueRef},
};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>, transform: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    match value.data {
        ValueRef::Borrowed(v) => match v {
            JsonValue::Array(items) => {
                let mut mapped = Vec::with_capacity(items.len());
                for x in items {
                    let p = Engine::execute(Proxy::new(x), transform)?;
                    mapped.push(p.into_value());
                }
                Ok(Proxy::owned(JsonValue::array(mapped)))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Shared(rc) => map_items(rc.as_ref(), transform),
        ValueRef::Owned(v) => match v {
            JsonValue::Array(items) => {
                let mut mapped = Vec::with_capacity(items.len());
                for x in items {
                    let p = Engine::execute(Proxy::owned(x), transform)?;
                    mapped.push(p.into_value());
                }
                Ok(Proxy::owned(JsonValue::array(mapped)))
            }
            other => Err(type_mismatch_array(&other)),
        },
    }
}

fn map_items<'a>(data: &JsonValue<'a>, transform: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    match data {
        JsonValue::Array(items) => {
            let mut mapped = Vec::with_capacity(items.len());
            for x in items {
                let p = Engine::execute(Proxy::owned(x.clone()), transform)?;
                mapped.push(p.into_value());
            }
            Ok(Proxy::owned(JsonValue::array(mapped)))
        }
        other => Err(type_mismatch_array(other)),
    }
}
