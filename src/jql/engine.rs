use std::cmp::Ordering;

use crate::{
    jql::{
        ast::{AccessStep, IfArm, JqlAstNode, JqlBinaryKind, JqlNumericKind, ObjectField, ObjectKey},
        error::{JqlError, JqlErrorKind},
        funcs,
        proxy::Proxy,
    },
    json::{
        error::JsonError,
        value::{JsonLogic, JsonNumeric, JsonOrd, JsonValue},
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
            JqlAstNode::Access(steps) => Self::access(value, steps),
            JqlAstNode::Pipe { source, dest } => Self::pipe(value, source, dest),
            JqlAstNode::Call { name, args } => funcs::dispatch(name, args, value),
            JqlAstNode::Numeric { kind, left, right } => Self::numeric(kind, left, right, value),
            JqlAstNode::Binary { kind, left, right } => Self::binary(kind, left, right, value),
            JqlAstNode::Object { fields } => Self::object(value, fields),
            JqlAstNode::If { arms, else_body } => Self::if_expr(value, arms, else_body),
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

    fn access<'a>(value: Proxy<'a>, steps: &[AccessStep<'_>]) -> Result<Proxy<'a>, JqlError> {
        value.get_steps(steps)
    }

    fn pipe<'a>(
        value: Proxy<'a>,
        source: &JqlAstNode<'_>,
        dest: &JqlAstNode<'_>,
    ) -> Result<Proxy<'a>, JqlError> {
        Self::execute(value, source).and_then(|r| Self::execute(r, dest))
    }

    pub fn numeric<'a>(
        kind: &JqlNumericKind,
        left: &JqlAstNode<'_>,
        right: &JqlAstNode<'_>,
        value: Proxy<'a>,
    ) -> Result<Proxy<'a>, JqlError> {
        let value = value.share();
        let left_proxy = Engine::execute(value.clone(), left)?;
        let right_proxy = Engine::execute(value, right)?;
        let left_output = left_proxy.data();
        let right_output = right_proxy.data();
        let result = match kind {
            JqlNumericKind::Plus => {
                let left_f64 = left_output.try_as_float()?;
                let right_f64 = right_output.try_as_float()?;
                left_f64 + right_f64
            }
            JqlNumericKind::Subtract => {
                let left_f64 = left_output.try_as_float()?;
                let right_f64 = right_output.try_as_float()?;
                left_f64 - right_f64
            }
            JqlNumericKind::Multiply => {
                let left_f64 = left_output.try_as_float()?;
                let right_f64 = right_output.try_as_float()?;
                left_f64 * right_f64
            }
            JqlNumericKind::Divide => {
                let left_f64 = left_output.try_as_float()?;
                let right_f64 = right_output.try_as_float()?;
                left_f64 / right_f64
            }
            JqlNumericKind::Mod => {
                let left_i32 = left_output.try_as_int()?;
                let right_i32 = right_output.try_as_int()?;
                (left_i32 % right_i32) as f64
            }
        };
        Ok(Proxy::owned(JsonValue::number(result)))
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

    /// Evaluate `if` / `elif` / `else`: cond is taken only when it yields JSON `true`
    /// (same truthiness as `filter` / `any` / `all`). Only the chosen branch runs.
    fn if_expr<'a>(
        value: Proxy<'a>,
        arms: &[IfArm<'_>],
        else_body: &JqlAstNode<'_>,
    ) -> Result<Proxy<'a>, JqlError> {
        let value = value.share();
        for arm in arms {
            let cond = Engine::execute(value.clone(), &arm.cond)?;
            if matches!(cond.data(), JsonValue::True) {
                return Engine::execute(value, &arm.then_body);
            }
        }
        Engine::execute(value, else_body)
    }
}

fn json_exec_error(err: JsonError) -> JqlError {
    JqlError::from(err)
}

fn op_not_support(op: &str) -> JqlError {
    JqlError::without_span(JqlErrorKind::UnsupportedOperation { op: op.to_string() })
}

#[cfg(test)]
#[path = "../../tests/jql/engine.rs"]
mod tests;
