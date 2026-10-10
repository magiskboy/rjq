use std::{borrow::Cow, str::FromStr};

use crate::Structured;
use crate::jql::error::{JqlError, JqlErrorKind};
use crate::jql::reference::{Access, ArrayIndex};
use crate::jql::reference::{PropertySelection, Reference};
use crate::json::value::JsonValue;

#[derive(Debug, Clone)]
pub struct Proxy<'a> {
    data: Cow<'a, JsonValue<'a>>,
}

impl<'a> Proxy<'a> {
    pub fn new(data: &'a JsonValue<'a>) -> Self {
        Self {
            data: Cow::Borrowed(data),
        }
    }

    pub fn len(&self) -> Result<JsonValue<'a>, JqlError> {
        self.data
            .as_ref()
            .len()
            .map(|l| JsonValue::number(l as f64))
            .map_err(JqlError::from)
    }

    pub fn owned(data: JsonValue<'a>) -> Self {
        Self {
            data: Cow::Owned(data),
        }
    }

    pub fn data(&self) -> &JsonValue<'a> {
        self.data.as_ref()
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

    fn select_keys(value: &JsonValue<'a>, keys: &Vec<String>) -> Result<JsonValue<'a>, JqlError> {
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
                    if keys.iter().any(|key| key == k.as_ref()) {
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

    fn descend(current: Cow<'a, JsonValue<'a>>, key: &str) -> Result<Cow<'a, JsonValue<'a>>, JqlError> {
        match current {
            Cow::Borrowed(v) => Ok(Cow::Borrowed(v.get(key).map_err(JqlError::from)?)),
            Cow::Owned(v) => Ok(Cow::Owned(v.get(key).map_err(JqlError::from)?.clone())),
        }
    }

    fn apply_selection(
        current: Cow<'a, JsonValue<'a>>,
        properties: PropertySelection,
    ) -> Result<Cow<'a, JsonValue<'a>>, JqlError> {
        match properties {
            PropertySelection::All => Ok(current),
            PropertySelection::Properties(keys) => {
                Ok(Cow::Owned(Self::select_keys(current.as_ref(), &keys)?))
            }
        }
    }

    pub fn get(&self, ref_str: &str) -> Result<Proxy<'a>, JqlError> {
        let jref = Reference::from_str(ref_str)?;
        // Re-borrow root with lifetime `'a` when possible; only clone if already owned.
        let mut current = match &self.data {
            Cow::Borrowed(v) => Cow::Borrowed(*v),
            Cow::Owned(v) => Cow::Owned(v.clone()),
        };

        for access in jref.access {
            current = match access {
                Access::PropertyAccess {
                    property,
                    properties,
                } => {
                    // Empty name is the root ref: operate on the current value as-is.
                    let next = if property.is_empty() {
                        current
                    } else {
                        Self::descend(current, &property)?
                    };
                    Self::apply_selection(next, properties)?
                }
                Access::IndexAccess { index, properties } => {
                    let next = match index {
                        ArrayIndex::All => current,
                        ArrayIndex::Index(index) => Self::descend(current, &index.to_string())?,
                        ArrayIndex::Range { start, end } => {
                            Cow::Owned(Self::slice(current.as_ref(), start, end)?)
                        }
                        ArrayIndex::MultipleIndex(indices) => {
                            Cow::Owned(Self::select(current.as_ref(), indices)?)
                        }
                    };
                    Self::apply_selection(next, properties)?
                }
            };
        }

        Ok(Proxy { data: current })
    }
}


#[cfg(test)]
#[path = "../../tests/jql/proxy.rs"]
mod tests;
