use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::type_mismatch,
    proxy::Proxy,
};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>, prefix_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let value = value.share();
    let prefix_proxy = Engine::execute(value.clone(), prefix_expr)?;
    let prefix = match prefix_proxy.data() {
        JsonValue::String(s) => s.as_ref(),
        other => return Err(type_mismatch("string", other)),
    };
    match value.data() {
        JsonValue::String(s) => Ok(Proxy::owned(JsonValue::boolean(s.starts_with(prefix)))),
        other => Err(type_mismatch("string", other)),
    }
}
