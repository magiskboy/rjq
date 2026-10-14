use std::cmp::Ordering;

use crate::jql::{
    error::{JqlError, JqlErrorKind},
    funcs::util::{type_mismatch_array, type_name},
    proxy::Proxy,
};
use crate::json::value::{JsonNumeric, JsonValue};

/// Sort a homogeneous array of numbers or strings (ascending).
pub fn call<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
    match value.into_value() {
        JsonValue::Array(mut items) => {
            if items.is_empty() {
                return Ok(Proxy::owned(JsonValue::array(items)));
            }
            match &items[0] {
                JsonValue::Number(_) => {
                    if let Some(bad) = items.iter().find(|x| !matches!(x, JsonValue::Number(_))) {
                        return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                            expected: "number",
                            found: type_name(bad),
                        }));
                    }
                    items.sort_by(|a, b| {
                        let af = a.try_as_float().unwrap_or(f64::NAN);
                        let bf = b.try_as_float().unwrap_or(f64::NAN);
                        af.partial_cmp(&bf).unwrap_or(Ordering::Equal)
                    });
                }
                JsonValue::String(_) => {
                    if let Some(bad) = items.iter().find(|x| !matches!(x, JsonValue::String(_))) {
                        return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                            expected: "string",
                            found: type_name(bad),
                        }));
                    }
                    items.sort_by(|a, b| match (a, b) {
                        (JsonValue::String(l), JsonValue::String(r)) => l.as_ref().cmp(r.as_ref()),
                        _ => Ordering::Equal,
                    });
                }
                other => {
                    return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                        expected: "array of number or string",
                        found: type_name(other),
                    }));
                }
            }
            Ok(Proxy::owned(JsonValue::array(items)))
        }
        other => Err(type_mismatch_array(&other)),
    }
}
