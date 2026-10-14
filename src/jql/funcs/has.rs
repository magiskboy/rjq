use crate::jql::{
    ast::JqlAstNode, engine::Engine, error::JqlError, funcs::util::type_mismatch, proxy::Proxy,
};
use crate::json::value::JsonValue;

/// `has("key")` — true when the current object contains the key.
pub fn call<'a>(value: Proxy<'a>, key_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let key_proxy = Engine::execute(value.clone().share(), key_expr)?;
    let key = match key_proxy.data() {
        JsonValue::String(s) => s.as_ref().to_string(),
        other => return Err(type_mismatch("string", other)),
    };
    match value.data() {
        JsonValue::Object(obj) => Ok(Proxy::owned(JsonValue::boolean(obj.get(&key).is_some()))),
        other => Err(type_mismatch("object", other)),
    }
}
