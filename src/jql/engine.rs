use std::cmp::Ordering;

use crate::{
    jql::{
        ast::{JqlAstNode, JqlBinaryKind, ObjectField, ObjectKey},
        error::{JqlError, JqlErrorKind},
        proxy::{Proxy, ValueRef},
    },
    json::{
        error::JsonError,
        value::{JsonLogic, JsonOrd, JsonValue},
    },
};

#[derive(Clone)]
pub struct Engine {}

impl Engine {
    pub fn execute<'a>(value: Proxy<'a>, path: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
        match path {
            JqlAstNode::Null
            | JqlAstNode::Boolean(_)
            | JqlAstNode::Number(_)
            | JqlAstNode::String(_) => Ok(Self::literal(path)),
            JqlAstNode::Access(path) => Self::access(value, path),
            JqlAstNode::Pipe { source, dest } => Self::pipe(value, source, dest),
            JqlAstNode::Call { name, args } => Self::call(name, args, value),
            JqlAstNode::Binary { kind, left, right } => Self::binary(kind, left, right, value),
            JqlAstNode::Object { fields } => Self::object(value, fields),
            _ => Err(op_not_support("ast node")),
        }
    }

    fn literal<'a>(literal: &JqlAstNode<'_>) -> Proxy<'a> {
        match literal {
            JqlAstNode::Null => Proxy::owned(JsonValue::Null),
            JqlAstNode::Boolean(val) => Proxy::owned(JsonValue::boolean(*val)),
            JqlAstNode::String(st) => Proxy::owned(JsonValue::string(st.to_string())),
            JqlAstNode::Number(val) => Proxy::owned(JsonValue::number(*val)),
            _ => unreachable!(),
        }
    }

    fn access<'a>(value: Proxy<'a>, path: &str) -> Result<Proxy<'a>, JqlError> {
        value.get(path)
    }

    fn pipe<'a>(
        value: Proxy<'a>,
        source: &JqlAstNode<'_>,
        dest: &JqlAstNode<'_>,
    ) -> Result<Proxy<'a>, JqlError> {
        Self::execute(value, source).and_then(|r| Self::execute(r, dest))
    }

    fn call<'a>(
        name: &str,
        args: &[JqlAstNode<'_>],
        value: Proxy<'a>,
    ) -> Result<Proxy<'a>, JqlError> {
        match name {
            "filter" => Self::filter(value, &args[0]),
            "map" => Self::map(value, &args[0]),
            "len" => Self::len(value, &args[0]),
            "sum" => Self::sum(value, &args[0]),
            "first" => Self::first(value),
            "last" => Self::last(value),
            other => Err(op_not_support(other)),
        }
    }

    pub fn len<'a>(value: Proxy<'a>, path: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
        let output = Self::execute(value, path)?;
        let l = output.len()?;
        Ok(Proxy::owned(l))
    }

    pub fn sum<'a>(value: Proxy<'a>, path: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
        let output = Self::execute(value, path)?;
        match output.data() {
            JsonValue::Array(items) => {
                let mut s = 0.0_f64;
                for item in items {
                    match item {
                        JsonValue::Number(v) => {
                            s += v.as_f64().map_err(|_| {
                                JqlError::without_span(JqlErrorKind::TypeMismatch {
                                    expected: "number",
                                    found: "number",
                                })
                            })?;
                        }
                        other => {
                            return Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                                expected: "number",
                                found: json_value_type_name(other),
                            }));
                        }
                    }
                }
                Ok(Proxy::owned(JsonValue::number(s)))
            }
            other => Err(JqlError::without_span(JqlErrorKind::TypeMismatch {
                expected: "array",
                found: json_value_type_name(other),
            })),
        }
    }

    pub fn binary<'a>(
        kind: &JqlBinaryKind,
        left: &JqlAstNode<'_>,
        right: &JqlAstNode<'_>,
        value: Proxy<'a>,
    ) -> Result<Proxy<'a>, JqlError> {
        // Share owned intermediates so left/right fan-out does not deep-clone twice.
        let value = value.share();
        let left_proxy = Engine::execute(value.clone(), left)?;
        let right_proxy = Engine::execute(value, right)?;
        let left_output = left_proxy.data();
        let right_output = right_proxy.data();
        let result = match kind {
            JqlBinaryKind::And => {
                left_output.try_as_bool().map_err(json_exec_error)?
                    && right_output.try_as_bool().map_err(json_exec_error)?
            }
            JqlBinaryKind::Or => {
                left_output.try_as_bool().map_err(json_exec_error)?
                    || right_output.try_as_bool().map_err(json_exec_error)?
            }
            JqlBinaryKind::Equal => left_output == right_output,
            JqlBinaryKind::NotEqual => left_output != right_output,
            JqlBinaryKind::Greater => {
                left_output.try_cmp(right_output).map_err(json_exec_error)? == Ordering::Greater
            }
            JqlBinaryKind::Less => {
                left_output.try_cmp(right_output).map_err(json_exec_error)? == Ordering::Less
            }
            JqlBinaryKind::GreaterEqual => {
                left_output.try_cmp(right_output).map_err(json_exec_error)? != Ordering::Less
            }
            JqlBinaryKind::LessEqual => {
                left_output.try_cmp(right_output).map_err(json_exec_error)? != Ordering::Greater
            }
        };
        Ok(Proxy::owned(JsonValue::boolean(result)))
    }

    fn filter<'a>(value: Proxy<'a>, predictive: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
        match value.data {
            ValueRef::Borrowed(v) => match v {
                JsonValue::Array(items) => {
                    let mut filtered = Vec::new();
                    for x in items {
                        let p = Engine::execute(Proxy::new(x), predictive)?;
                        if matches!(p.data(), JsonValue::True) {
                            // Structure clone only; scalar Cow leaves stay borrowed.
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
                    // Unique owner: share each item for the predicate, then move it
                    // back out on match (Rc::try_unwrap) instead of cloning twice.
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

    fn map<'a>(value: Proxy<'a>, transform: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
        match value.data {
            ValueRef::Borrowed(v) => match v {
                JsonValue::Array(items) => {
                    let mut mapped = Vec::with_capacity(items.len());
                    for x in items {
                        // Keep element borrowed so nested access stays zero-copy.
                        let p = Engine::execute(Proxy::new(x), transform)?;
                        mapped.push(p.into_value());
                    }
                    Ok(Proxy::owned(JsonValue::array(mapped)))
                }
                other => Err(type_mismatch_array(other)),
            },
            ValueRef::Shared(rc) => map_items(rc.as_ref(), transform),
            ValueRef::Owned(v) => match v {
                JsonValue::Array(items) => {
                    let mut mapped = Vec::with_capacity(items.len());
                    for x in items {
                        let p = Engine::execute(Proxy::owned(x), transform)?;
                        mapped.push(p.into_value());
                    }
                    Ok(Proxy::owned(JsonValue::array(mapped)))
                }
                other => Err(type_mismatch_array(&other)),
            },
        }
    }

    fn object<'a>(value: Proxy<'a>, fields: &[ObjectField<'_>]) -> Result<Proxy<'a>, JqlError> {
        let value = value.share();
        let mut entries = Vec::with_capacity(fields.len());
        for field in fields {
            let key = match &field.key {
                ObjectKey::Ident(k) | ObjectKey::String(k) => (*k).to_string(),
            };
            let field_value = Engine::execute(value.clone(), &field.value)?;
            // Keep document borrows; do not force 'static via into_owned.
            entries.push((key, field_value.into_value()));
        }
        Ok(Proxy::owned(JsonValue::object(entries)))
    }

    fn first<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
        Ok(value.take_first())
    }

    fn last<'a>(value: Proxy<'a>) -> Result<Proxy<'a>, JqlError> {
        Ok(value.take_last())
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

fn map_items<'a>(data: &JsonValue<'a>, transform: &JqlAstNode<'_>) -> Result<Proxy<'a>, JqlError> {
    match data {
        JsonValue::Array(items) => {
            let mut mapped = Vec::with_capacity(items.len());
            for x in items {
                let p = Engine::execute(Proxy::owned(x.clone()), transform)?;
                mapped.push(p.into_value());
            }
            Ok(Proxy::owned(JsonValue::array(mapped)))
        }
        other => Err(type_mismatch_array(other)),
    }
}

fn type_mismatch_array(other: &JsonValue<'_>) -> JqlError {
    JqlError::without_span(JqlErrorKind::TypeMismatch {
        expected: "array",
        found: json_value_type_name(other),
    })
}

fn json_exec_error(err: JsonError) -> JqlError {
    JqlError::from(err)
}

fn op_not_support(op: &str) -> JqlError {
    JqlError::without_span(JqlErrorKind::UnsupportedOperation { op: op.to_string() })
}

fn json_value_type_name(value: &JsonValue<'_>) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::True | JsonValue::False => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

#[cfg(test)]
#[path = "../../tests/jql/engine.rs"]
mod tests;
