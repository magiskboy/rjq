use crate::jql::{
    ast::JqlAstNode,
    engine::Engine,
    error::{JqlError, JqlErrorKind},
    funcs::util::type_name,
    proxy::Proxy,
};
use crate::json::value::JsonValue;

/// Array membership or string substring check against the evaluated argument.
pub fn call<'a>(value: Proxy<'a>, needle_expr: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    let value = value.share();
    let needle = Engine::execute(value.clone(), needle_expr)?;
    let found = match value.data() {
        JsonValue::Array(items) => items.iter().any(|item| item == needle.data()),
        JsonValue::String(hay) => match needle.data() {
            JsonValue::String(n) => hay.as_ref().contains(n.as_ref()),
            other => {
                return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                    expected: "string",
                    found: type_name(other),
                }));
            }
        },
        other => {
            return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                expected: "array or string",
                found: type_name(other),
            }));
        }
    };
    Ok(Proxy::owned(JsonValue::boolean(found)))
}
