use std::{borrow::Cow, rc::Rc};

use crate::Structured;
use crate::jql::ast::{AccessStep, ArrayIndex};
use crate::jql::error::{JqlError, JqlErrorKind};
use crate::json::value::JsonValue;

/// Engine-side value carrier with lazy materialization.
///
/// - [`ValueRef::Borrowed`]: zero-copy view into the input document.
/// - [`ValueRef::Shared`]: `Rc`-backed owned tree; fan-out (`clone`) is O(1).
/// - [`ValueRef::Owned`]: uniquely owned tree; consume via [`Proxy::into_value`].
#[derive(Debug, Clone)]
pub struct Proxy<'a> {
    pub(crate) data: ValueRef<'a>,
}

#[derive(Debug, Clone)]
pub(crate) enum ValueRef<'a> {
    Borrowed(&'a JsonValue<'a>),
    Shared(Rc<JsonValue<'a>>),
    Owned(JsonValue<'a>),
}

impl<'a> Proxy<'a> {
    pub fn new(data: &'a JsonValue<'a>) -> Self {
        Self {
            data: ValueRef::Borrowed(data),
        }
    }

    pub fn owned(data: JsonValue<'a>) -> Self {
        Self {
            data: ValueRef::Owned(data),
        }
    }

    pub fn data(&self) -> &JsonValue<'a> {
        match &self.data {
            ValueRef::Borrowed(v) => v,
            ValueRef::Shared(v) => v.as_ref(),
            ValueRef::Owned(v) => v,
        }
    }

    /// Consume the proxy into an owned [`JsonValue`], cloning only when the
    /// value is borrowed or still shared with other proxies.
    pub fn into_value(self) -> JsonValue<'a> {
        match self.data {
            ValueRef::Borrowed(v) => v.clone(),
            ValueRef::Shared(v) => Rc::try_unwrap(v).unwrap_or_else(|rc| (*rc).clone()),
            ValueRef::Owned(v) => v,
        }
    }

    /// Promote a uniquely owned value to shared so subsequent `clone`s are cheap.
    /// Borrowed / already-shared proxies are returned unchanged.
    pub fn share(self) -> Self {
        match self.data {
            ValueRef::Owned(v) => Self {
                data: ValueRef::Shared(Rc::new(v)),
            },
            other => Self { data: other },
        }
    }

    /// Zero-copy element access when borrowed; move out when uniquely owned;
    /// clone when shared.
    pub fn take_first(self) -> Proxy<'a> {
        match self.data {
            ValueRef::Borrowed(v) => match v {
                JsonValue::Array(items) => items
                    .first()
                    .map(Proxy::new)
                    .unwrap_or_else(|| Proxy::owned(JsonValue::null())),
                _ => Proxy::owned(JsonValue::null()),
            },
            ValueRef::Shared(rc) => match rc.as_ref() {
                JsonValue::Array(items) => items
                    .first()
                    .map(|x| Proxy::owned(x.clone()))
                    .unwrap_or_else(|| Proxy::owned(JsonValue::null())),
                _ => Proxy::owned(JsonValue::null()),
            },
            ValueRef::Owned(v) => match v {
                JsonValue::Array(items) => {
                    Proxy::owned(items.into_iter().next().unwrap_or(JsonValue::null()))
                }
                _ => Proxy::owned(JsonValue::null()),
            },
        }
    }

    /// Zero-copy element access when borrowed; move out when uniquely owned;
    /// clone when shared.
    pub fn take_last(self) -> Proxy<'a> {
        match self.data {
            ValueRef::Borrowed(v) => match v {
                JsonValue::Array(items) => items
                    .last()
                    .map(Proxy::new)
                    .unwrap_or_else(|| Proxy::owned(JsonValue::null())),
                _ => Proxy::owned(JsonValue::null()),
            },
            ValueRef::Shared(rc) => match rc.as_ref() {
                JsonValue::Array(items) => items
                    .last()
                    .map(|x| Proxy::owned(x.clone()))
                    .unwrap_or_else(|| Proxy::owned(JsonValue::null())),
                _ => Proxy::owned(JsonValue::null()),
            },
            ValueRef::Owned(v) => match v {
                JsonValue::Array(mut items) => {
                    Proxy::owned(items.pop().unwrap_or(JsonValue::null()))
                }
                _ => Proxy::owned(JsonValue::null()),
            },
        }
    }

    fn type_mismatch(expected: &'static str, found: &JsonValue<'_>) -> JqlError {
        JqlError::without_span(JqlErrorKind::TypeMismatch {
            expected,
            found: match found {
                JsonValue::Null => "null",
                JsonValue::True | JsonValue::False => "boolean",
                JsonValue::Number(_) => "number",
                JsonValue::String(_) => "string",
                JsonValue::Array(_) => "array",
                JsonValue::Object(_) => "object",
            },
        })
    }

    fn slice(value: &JsonValue<'a>, start: usize, end: usize) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                if let Some(values) = items.get(start..end) {
                    Ok(JsonValue::array(values.to_vec()))
                } else {
                    Err(JqlError::without_span(JqlErrorKind::PathNotFound {
                        path: format!("[{start}:{end}] (len {})", items.len()),
                    }))
                }
            }
            other => Err(Self::type_mismatch("array", other)),
        }
    }

    fn select(value: &JsonValue<'a>, indices: Vec<usize>) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                let values = indices
                    .iter()
                    .map(|i| {
                        items.get(*i).cloned().ok_or_else(|| {
                            JqlError::without_span(JqlErrorKind::PathNotFound {
                                path: format!("[{i}] (len {})", items.len()),
                            })
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(JsonValue::array(values))
            }
            other => Err(Self::type_mismatch("array", other)),
        }
    }

    fn select_keys(value: &JsonValue<'a>, keys: &[&str]) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                let values = items
                    .iter()
                    .map(|x| Self::select_keys(x, keys))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(JsonValue::array(values))
            }
            JsonValue::Object(m) => {
                let val = m.iter().filter_map(|(k, v)| {
                    if keys.iter().any(|key| *key == k.as_ref()) {
                        Some((k.clone(), v.clone()))
                    } else {
                        None
                    }
                });
                Ok(JsonValue::object(val))
            }
            other => Err(Self::type_mismatch("object or array", other)),
        }
    }

    fn descend_borrowed(
        current: &'a JsonValue<'a>,
        key: &str,
    ) -> Result<&'a JsonValue<'a>, JqlError> {
        current.get(key).map_err(JqlError::from)
    }

    fn descend_ref<'b>(
        current: &'b JsonValue<'a>,
        key: &str,
    ) -> Result<&'b JsonValue<'a>, JqlError> {
        current.get(key).map_err(JqlError::from)
    }

    /// Walk a borrowed document root — results stay [`ValueRef::Borrowed`] when possible.
    fn get_borrowed(root: &'a JsonValue<'a>, access: &[AccessStep<'_>]) -> Result<Proxy<'a>, JqlError> {
        let mut current = Cow::Borrowed(root);

        for step in access {
            current = match step {
                AccessStep::Property(property) => match current {
                    Cow::Borrowed(v) => Cow::Borrowed(Self::descend_borrowed(v, property)?),
                    Cow::Owned(v) => {
                        Cow::Owned(v.get(*property).map_err(JqlError::from)?.clone())
                    }
                },
                AccessStep::Index(index) => match index {
                    ArrayIndex::All => current,
                    ArrayIndex::Index(index) => match current {
                        Cow::Borrowed(v) => {
                            Cow::Borrowed(Self::descend_borrowed(v, &index.to_string())?)
                        }
                        Cow::Owned(v) => Cow::Owned(
                            v.get(&index.to_string()).map_err(JqlError::from)?.clone(),
                        ),
                    },
                    ArrayIndex::Range { start, end } => {
                        Cow::Owned(Self::slice(current.as_ref(), *start, *end)?)
                    }
                    ArrayIndex::MultipleIndex(indices) => {
                        Cow::Owned(Self::select(current.as_ref(), indices.clone())?)
                    }
                },
                AccessStep::Project(keys) => {
                    Cow::Owned(Self::select_keys(current.as_ref(), keys)?)
                }
            };
        }

        Ok(Proxy {
            data: match current {
                Cow::Borrowed(v) => ValueRef::Borrowed(v),
                Cow::Owned(v) => ValueRef::Owned(v),
            },
        })
    }

    /// Walk an owned/shared tree by reference and clone only the final node
    /// (or projection), instead of cloning the root before every descend step.
    fn get_owned_view(root: &JsonValue<'a>, access: &[AccessStep<'_>]) -> Result<Proxy<'a>, JqlError> {
        enum Cursor<'b, 'a> {
            Ref(&'b JsonValue<'a>),
            Owned(JsonValue<'a>),
        }

        let mut current = Cursor::Ref(root);

        for step in access {
            current = match step {
                AccessStep::Property(property) => match current {
                    Cursor::Ref(v) => Cursor::Ref(Self::descend_ref(v, property)?),
                    Cursor::Owned(v) => {
                        Cursor::Owned(v.get(*property).map_err(JqlError::from)?.clone())
                    }
                },
                AccessStep::Index(index) => match index {
                    ArrayIndex::All => current,
                    ArrayIndex::Index(index) => match current {
                        Cursor::Ref(v) => Cursor::Ref(Self::descend_ref(v, &index.to_string())?),
                        Cursor::Owned(v) => Cursor::Owned(
                            v.get(&index.to_string()).map_err(JqlError::from)?.clone(),
                        ),
                    },
                    ArrayIndex::Range { start, end } => {
                        let view = match &current {
                            Cursor::Ref(v) => *v,
                            Cursor::Owned(v) => v,
                        };
                        Cursor::Owned(Self::slice(view, *start, *end)?)
                    }
                    ArrayIndex::MultipleIndex(indices) => {
                        let view = match &current {
                            Cursor::Ref(v) => *v,
                            Cursor::Owned(v) => v,
                        };
                        Cursor::Owned(Self::select(view, indices.clone())?)
                    }
                },
                AccessStep::Project(keys) => {
                    let view = match &current {
                        Cursor::Ref(v) => *v,
                        Cursor::Owned(v) => v,
                    };
                    Cursor::Owned(Self::select_keys(view, keys)?)
                }
            };
        }

        Ok(match current {
            Cursor::Ref(v) => Proxy::owned(v.clone()),
            Cursor::Owned(v) => Proxy::owned(v),
        })
    }

    pub fn get_steps(&self, access: &[AccessStep<'_>]) -> Result<Proxy<'a>, JqlError> {
        if access.is_empty() {
            return Ok(self.clone());
        }
        match &self.data {
            ValueRef::Borrowed(v) => Self::get_borrowed(v, access),
            ValueRef::Shared(v) => Self::get_owned_view(v.as_ref(), access),
            ValueRef::Owned(v) => Self::get_owned_view(v, access),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/jql/proxy.rs"]
mod tests;
