use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::{type_mismatch, type_mismatch_array},
    proxy::Proxy,
};
use crate::json::value::JsonValue;

/// Join an array of strings with the separator from the argument.
pub fn call<'a>(value: Proxy<'a>, sep_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let value = value.share();
    let sep_proxy = Engine::execute(value.clone(), sep_expr)?;
    let sep = match sep_proxy.data() {
        JsonValue::String(s) => s.as_ref().to_string(),
        other => return Err(type_mismatch("string", other)),
    };
    match value.data() {
        JsonValue::Array(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    JsonValue::String(s) => parts.push(s.as_ref().to_string()),
                    other => return Err(type_mismatch("string", other)),
                }
            }
            Ok(Proxy::owned(JsonValue::string(parts.join(&sep))))
        }
        other => Err(type_mismatch_array(other)),
    }
}
