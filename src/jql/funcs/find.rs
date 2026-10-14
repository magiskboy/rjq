use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::type_mismatch_array,
    proxy::{Proxy, ValueRef},
};
use crate::json::value::JsonValue;

/// First array element for which the predicate is true; otherwise null.
pub fn call<'a>(value: Proxy<'a>, predictive: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    match value.data {
        ValueRef::Borrowed(v) => match v {
            JsonValue::Array(items) => {
                for x in items {
                    let p = Engine::execute(Proxy::new(x), predictive)?;
                    if matches!(p.data(), JsonValue::True) {
                        return Ok(Proxy::new(x));
                    }
                }
                Ok(Proxy::owned(JsonValue::null()))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Shared(rc) => match rc.as_ref() {
            JsonValue::Array(items) => {
                for x in items {
                    let item = Proxy::owned(x.clone()).share();
                    let keep = matches!(
                        Engine::execute(item.clone(), predictive)?.data(),
                        JsonValue::True
                    );
                    if keep {
                        return Ok(item);
                    }
                }
                Ok(Proxy::owned(JsonValue::null()))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Owned(v) => match v {
            JsonValue::Array(items) => {
                for x in items {
                    let item = Proxy::owned(x).share();
                    let keep = matches!(
                        Engine::execute(item.clone(), predictive)?.data(),
                        JsonValue::True
                    );
                    if keep {
                        return Ok(item);
                    }
                }
                Ok(Proxy::owned(JsonValue::null()))
            }
            other => Err(type_mismatch_array(&other)),
        },
    }
}
