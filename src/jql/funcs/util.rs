use crate::jql::error::{JqlError, JqlErrorKind};
use crate::json::value::JsonValue;

pub fn type_name(value: &JsonValue<'_>) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::True | JsonValue::False => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

pub fn type_mismatch(expected: &'static str, found: &JsonValue<'_>) -> JqlError {
    JqlError::without_span(JqlErrorKind::TypeMismatch {
        expected,
        found: type_name(found),
    })
}

pub fn type_mismatch_array(found: &JsonValue<'_>) -> JqlError {
    type_mismatch("array", found)
}

pub fn op_not_support(op: &str) -> JqlError {
    JqlError::without_span(JqlErrorKind::UnsupportedOperation { op: op.to_string() })
}

pub fn arity_error(name: &str, expected: usize, got: usize) -> JqlError {
    op_not_support(&format!(
        "{name}: expected {expected} argument(s), got {got}"
    ))
}

pub fn expect_arity(name: &str, args_len: usize, expected: usize) -> Result<(), JqlError> {
    if args_len != expected {
        return Err(arity_error(name, expected, args_len));
    }
    Ok(())
}
