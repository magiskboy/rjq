use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::JqlError,
    funcs::util::type_mismatch,
    proxy::Proxy,
};
use crate::json::value::JsonValue;

pub fn call<'a>(value: Proxy<'a>, suffix_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let value = value.share();
    let suffix_proxy = Engine::execute(value.clone(), suffix_expr)?;
    let suffix = match suffix_proxy.data() {
        JsonValue::String(s) => s.as_ref(),
        other => return Err(type_mismatch("string", other)),
    };
    match value.data() {
        JsonValue::String(s) => Ok(Proxy::owned(JsonValue::boolean(s.ends_with(suffix)))),
        other => Err(type_mismatch("string", other)),
    }
}
