use crate::jql::{
    ast::JqlAstNode, engine::Engine, error::JqlError, funcs::util::type_mismatch, proxy::Proxy,
};
use crate::json::value::JsonValue;

/// Split the current string by the separator from the argument.
pub fn call<'a>(value: Proxy<'a>, sep_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let value = value.share();
    let sep_proxy = Engine::execute(value.clone(), sep_expr)?;
    let sep = match sep_proxy.data() {
        JsonValue::String(s) => s.as_ref().to_string(),
        other => return Err(type_mismatch("string", other)),
    };
    match value.data() {
        JsonValue::String(s) => {
            let parts = if sep.is_empty() {
                s.chars()
                    .map(|c| JsonValue::string(c.to_string()))
                    .collect()
            } else {
                s.split(&sep)
                    .map(|part| JsonValue::string(part.to_string()))
                    .collect()
            };
            Ok(Proxy::owned(JsonValue::array(parts)))
        }
        other => Err(type_mismatch("string", other)),
    }
}
