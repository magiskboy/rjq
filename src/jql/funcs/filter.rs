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
                let mut filtered = Vec::new();
                for x in items {
                    let p = Engine::execute(Proxy::new(x), predictive)?;
                    if matches!(p.data(), JsonValue::True) {
                        filtered.push(x.clone());
                    }
                }
                Ok(Proxy::owned(JsonValue::array(filtered)))
            }
            other => Err(type_mismatch_array(other)),
        },
        ValueRef::Shared(rc) => filter_items(rc.as_ref(), predictive),
        ValueRef::Owned(v) => match v {
            JsonValue::Array(items) => {
                let mut filtered = Vec::new();
                for x in items {
                    let item = Proxy::owned(x).share();
                    let keep = matches!(
                        Engine::execute(item.clone(), predictive)?.data(),
                        JsonValue::True
                    );
                    if keep {
                        filtered.push(item.into_value());
                    }
                }
                Ok(Proxy::owned(JsonValue::array(filtered)))
            }
            other => Err(type_mismatch_array(&other)),
        },
    }
}

fn filter_items<'a>(
    data: &JsonValue<'a>,
    predictive: &JqlAstNode<'_>,
) -> Result<Proxy<'a>, JqlError> {
    match data {
        JsonValue::Array(items) => {
            let mut filtered = Vec::new();
            for x in items {
                let item = Proxy::owned(x.clone()).share();
                let keep = matches!(
                    Engine::execute(item.clone(), predictive)?.data(),
                    JsonValue::True
                );
                if keep {
                    filtered.push(item.into_value());
                }
            }
            Ok(Proxy::owned(JsonValue::array(filtered)))
        }
        other => Err(type_mismatch_array(other)),
    }
}
