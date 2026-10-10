use std::borrow::Cow;
use std::{cmp::Ordering, fmt::Display};

use crate::Structured;
use crate::json::{
    JsonDumpOptions,
    error::{ExpectedJsonType, JsonError, JsonErrorKind, JsonType},
    json_dumps,
    number::{JsonNumber, NumberError},
};

pub trait JsonOrd {
    type ErrorType;

    fn try_cmp(&self, other: &Self) -> Result<Ordering, Self::ErrorType>;
}

pub trait JsonLogic {
    type ErrorType;

    fn try_as_bool(&self) -> Result<bool, Self::ErrorType>;
}

pub trait JsonNumeric {
    type ErrorType;

    fn try_as_int(&self) -> Result<i32, Self::ErrorType>;
    fn try_as_float(&self) -> Result<f64, Self::ErrorType>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue<'a> {
    String(Cow<'a, str>),
    Number(JsonNumber<'a>),
    Null,
    True,
    False,
    Array(Vec<JsonValue<'a>>),
    Object(JsonObject<'a>),
}

/// Insertion-ordered JSON object, backed by a `Vec` of `(key, value)` entries.
///
/// This mirrors the semantics of a Python `dict` (and therefore `orjson` /
/// `ujson`): iteration preserves insertion order. Building is allocation-light
/// — the parser pushes entries directly with no hashing and no second pass.
///
/// Lookups ([`get`](JsonObject::get)) scan from the end so that, for the rare
/// case of duplicate keys, the last value wins (matching dict/`orjson`).
#[derive(Debug, Clone, Default)]
pub struct JsonObject<'a> {
    entries: Vec<(Cow<'a, str>, JsonValue<'a>)>,
}

impl<'a> JsonObject<'a> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Wrap pre-built entries without de-duplication (caller guarantees shape).
    pub fn from_entries(entries: Vec<(Cow<'a, str>, JsonValue<'a>)>) -> Self {
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append an entry without de-duplication. Hot path used by the parser.
    pub fn push(&mut self, key: Cow<'a, str>, value: JsonValue<'a>) {
        self.entries.push((key, value));
    }

    /// Insert replacing an existing key in place (keeps original position),
    /// otherwise append. Used by mutating APIs (not the parse hot path).
    pub fn insert(&mut self, key: Cow<'a, str>, value: JsonValue<'a>) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Look up a key, returning the last-inserted value for that key.
    pub fn get(&self, key: &str) -> Option<&JsonValue<'a>> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k.as_ref() == key)
            .map(|(_, v)| v)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (Cow<'a, str>, JsonValue<'a>)> {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for JsonObject<'a> {
    type Item = (Cow<'a, str>, JsonValue<'a>);
    type IntoIter = std::vec::IntoIter<(Cow<'a, str>, JsonValue<'a>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> FromIterator<(Cow<'a, str>, JsonValue<'a>)> for JsonObject<'a> {
    fn from_iter<T: IntoIterator<Item = (Cow<'a, str>, JsonValue<'a>)>>(iter: T) -> Self {
        let mut object = JsonObject::new();
        for (key, value) in iter {
            // Public constructor path: de-duplicate so repeated keys collapse to
            // the last value (preserves the historical HashMap-based behavior).
            object.insert(key, value);
        }
        object
    }
}

/// Order-independent equality: two objects are equal when they have the same
/// set of keys mapping to equal values (duplicate-free inputs).
impl<'a> PartialEq for JsonObject<'a> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl Display for JsonValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let opts = JsonDumpOptions::default();
        json_dumps(f, self, &opts)
    }
}

impl<'a> JsonValue<'a> {
    pub fn null() -> Self {
        JsonValue::Null
    }

    pub fn boolean(value: bool) -> Self {
        match value {
            true => JsonValue::True,
            false => JsonValue::False,
        }
    }

    pub fn number(value: f64) -> Self {
        JsonValue::Number(JsonNumber::from_f64(value))
    }

    pub fn number_raw(number: JsonNumber<'a>) -> Self {
        JsonValue::Number(number)
    }

    pub fn string(value: impl Into<Cow<'a, str>>) -> Self {
        JsonValue::String(value.into())
    }

    pub fn array(items: Vec<JsonValue<'a>>) -> Self {
        JsonValue::Array(items)
    }

    pub fn empty_object() -> Self {
        JsonValue::Object(JsonObject::new())
    }

    pub fn object<K>(items: impl IntoIterator<Item = (K, JsonValue<'a>)>) -> Self
    where
        K: Into<Cow<'a, str>>,
    {
        let object = items.into_iter().map(|(k, v)| (k.into(), v)).collect();
        JsonValue::Object(object)
    }

    /// Copy every borrowed string into an owned `'static` tree.
    pub fn into_owned(self) -> JsonValue<'static> {
        match self {
            JsonValue::String(s) => JsonValue::String(Cow::Owned(s.into_owned())),
            JsonValue::Number(n) => JsonValue::Number(n.into_owned()),
            JsonValue::Null => JsonValue::Null,
            JsonValue::True => JsonValue::True,
            JsonValue::False => JsonValue::False,
            JsonValue::Array(items) => {
                JsonValue::Array(items.into_iter().map(JsonValue::into_owned).collect())
            }
            JsonValue::Object(object) => JsonValue::Object(JsonObject::from_entries(
                object
                    .into_iter()
                    .map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned()))
                    .collect(),
            )),
        }
    }
}

impl JsonOrd for JsonValue<'_> {
    type ErrorType = JsonError;

    fn try_cmp(&self, other: &Self) -> Result<Ordering, JsonError> {
        match (self, other) {
            (JsonValue::Number(l), JsonValue::Number(r)) => {
                let lf = l.as_f64().map_err(number_error)?;
                let rf = r.as_f64().map_err(number_error)?;
                lf.partial_cmp(&rf).ok_or_else(|| {
                    JsonError::value(JsonErrorKind::TypeMismatch {
                        expected: ExpectedJsonType::Number,
                        found: JsonType::Number,
                    })
                })
            }
            (JsonValue::Number(_), other) => Err(type_mismatch(other, ExpectedJsonType::Number)),
            (other, _) => Err(type_mismatch(other, ExpectedJsonType::Number)),
        }
    }
}

impl JsonLogic for JsonValue<'_> {
    type ErrorType = JsonError;

    fn try_as_bool(&self) -> Result<bool, JsonError> {
        match self {
            JsonValue::True => Ok(true),
            JsonValue::False => Ok(false),
            other => Err(type_mismatch(other, ExpectedJsonType::Boolean)),
        }
    }
}

impl JsonNumeric for JsonValue<'_> {
    type ErrorType = JsonError;

    fn try_as_int(&self) -> Result<i32, Self::ErrorType> {
        match self {
            JsonValue::Number(v) => {
                let i = v.as_i64().map_err(number_error)?;
                i32::try_from(i).map_err(|_| JsonError::value(JsonErrorKind::InvalidCast))
            }
            o => Err(type_mismatch(o, ExpectedJsonType::Number)),
        }
    }

    fn try_as_float(&self) -> Result<f64, Self::ErrorType> {
        match self {
            JsonValue::Number(v) => v.as_f64().map_err(number_error),
            o => Err(type_mismatch(o, ExpectedJsonType::Number)),
        }
    }
}

fn number_error(err: NumberError) -> JsonError {
    match err {
        NumberError::NotInteger | NumberError::OutOfRange | NumberError::Invalid => {
            JsonError::value(JsonErrorKind::InvalidCast)
        }
    }
}

impl<'a> Structured for JsonValue<'a> {
    type ErrorType = JsonError;

    fn len(&self) -> Result<usize, JsonError> {
        match self {
            JsonValue::Object(elements) => Ok(elements.len()),
            JsonValue::Array(items) => Ok(items.len()),
            other => Err(type_mismatch(other, ExpectedJsonType::ArrayOrObject)),
        }
    }

    fn get(&self, key: &str) -> Result<&JsonValue<'a>, JsonError> {
        match self {
            JsonValue::Array(items) => {
                let index = parse_index(key)?;
                items.get(index).ok_or_else(|| {
                    JsonError::value(JsonErrorKind::IndexOutOfBounds {
                        index,
                        len: items.len(),
                    })
                })
            }
            JsonValue::Object(object) => object.get(key).ok_or_else(|| {
                JsonError::value(JsonErrorKind::KeyNotFound {
                    key: key.to_string(),
                })
            }),
            other => Err(type_mismatch(other, ExpectedJsonType::ArrayOrObject)),
        }
    }

    fn insert(&mut self, key: &str, element: JsonValue<'a>) -> Result<(), JsonError> {
        match self {
            JsonValue::Object(object) => {
                object.insert(Cow::Owned(key.to_string()), element);
                Ok(())
            }
            JsonValue::Array(items) => {
                let index = parse_index(key)?;
                if index > items.len() {
                    return Err(JsonError::value(JsonErrorKind::IndexOutOfBounds {
                        index,
                        len: items.len(),
                    }));
                }
                items.insert(index, element);
                Ok(())
            }
            other => Err(type_mismatch(other, ExpectedJsonType::ArrayOrObject)),
        }
    }

    fn push(&mut self, element: JsonValue<'a>) -> Result<(), JsonError> {
        match self {
            JsonValue::Array(items) => {
                items.push(element);
                Ok(())
            }
            other => Err(type_mismatch(other, ExpectedJsonType::Array)),
        }
    }
}

fn json_type(value: &JsonValue<'_>) -> JsonType {
    match value {
        JsonValue::Null => JsonType::Null,
        JsonValue::True | JsonValue::False => JsonType::Boolean,
        JsonValue::Number(_) => JsonType::Number,
        JsonValue::String(_) => JsonType::String,
        JsonValue::Array(_) => JsonType::Array,
        JsonValue::Object(_) => JsonType::Object,
    }
}

fn type_mismatch(value: &JsonValue<'_>, expected: ExpectedJsonType) -> JsonError {
    JsonError::value(JsonErrorKind::TypeMismatch {
        expected,
        found: json_type(value),
    })
}

fn parse_index(key: &str) -> Result<usize, JsonError> {
    key.parse().map_err(|_| {
        JsonError::value(JsonErrorKind::InvalidIndex {
            key: key.to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::cmp::Ordering;

    use super::{JsonLogic, JsonOrd, JsonValue};
    use crate::Structured;
    use crate::json::error::{ExpectedJsonType, JsonErrorKind, JsonType};

    #[test]
    fn try_cmp_orders_numbers_and_rejects_other_types() {
        assert_eq!(
            JsonValue::number(1.0)
                .try_cmp(&JsonValue::number(2.0))
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(
            JsonValue::number(2.0)
                .try_cmp(&JsonValue::number(2.0))
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            JsonValue::number(3.0)
                .try_cmp(&JsonValue::number(2.0))
                .unwrap(),
            Ordering::Greater
        );

        assert_eq!(
            JsonValue::True
                .try_cmp(&JsonValue::number(1.0))
                .unwrap_err()
                .kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Number,
                found: JsonType::Boolean,
            }
        );
        assert_eq!(
            JsonValue::number(1.0)
                .try_cmp(&JsonValue::string("a"))
                .unwrap_err()
                .kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Number,
                found: JsonType::String,
            }
        );
    }

    #[test]
    fn try_as_bool_accepts_booleans_only() {
        assert_eq!(JsonValue::True.try_as_bool().unwrap(), true);
        assert_eq!(JsonValue::False.try_as_bool().unwrap(), false);
        assert_eq!(
            JsonValue::Null.try_as_bool().unwrap_err().kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Boolean,
                found: JsonType::Null,
            }
        );
        assert_eq!(
            JsonValue::number(1.0).try_as_bool().unwrap_err().kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Boolean,
                found: JsonType::Number,
            }
        );
    }

    #[test]
    fn get_reports_missing_key_bad_index_and_type() {
        let object = JsonValue::object([(Cow::Borrowed("name"), JsonValue::string("a"))]);
        assert_eq!(object.get("name").unwrap(), &JsonValue::string("a"));
        let err = object.get("missing").unwrap_err();
        assert_eq!(err.span, None);
        assert_eq!(
            err.kind,
            JsonErrorKind::KeyNotFound {
                key: "missing".to_string(),
            }
        );

        let array = JsonValue::array(vec![JsonValue::number(1.0)]);
        assert_eq!(array.get("0").unwrap(), &JsonValue::number(1.0));
        assert_eq!(
            array.get("2").unwrap_err().kind,
            JsonErrorKind::IndexOutOfBounds { index: 2, len: 1 }
        );
        assert_eq!(
            array.get("name").unwrap_err().kind,
            JsonErrorKind::InvalidIndex {
                key: "name".to_string(),
            }
        );

        assert_eq!(
            JsonValue::Null.get("0").unwrap_err().kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::ArrayOrObject,
                found: JsonType::Null,
            }
        );
    }

    #[test]
    fn insert_adds_object_keys_and_shifts_array_items() {
        let mut object = JsonValue::empty_object();
        object.insert("name", JsonValue::string("a")).unwrap();
        object.insert("name", JsonValue::string("b")).unwrap();
        assert_eq!(object.get("name").unwrap(), &JsonValue::string("b"));

        let mut array = JsonValue::array(vec![JsonValue::number(1.0)]);
        array.insert("1", JsonValue::number(2.0)).unwrap();
        array.insert("0", JsonValue::number(0.0)).unwrap();
        assert_eq!(array.len().unwrap(), 3);
        assert_eq!(array.get("0").unwrap(), &JsonValue::number(0.0));
        assert_eq!(array.get("2").unwrap(), &JsonValue::number(2.0));

        assert_eq!(
            array.insert("9", JsonValue::Null).unwrap_err().kind,
            JsonErrorKind::IndexOutOfBounds { index: 9, len: 3 }
        );
        assert_eq!(
            array.insert("name", JsonValue::Null).unwrap_err().kind,
            JsonErrorKind::InvalidIndex {
                key: "name".to_string(),
            }
        );
        assert_eq!(
            JsonValue::True
                .insert("0", JsonValue::Null)
                .unwrap_err()
                .kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::ArrayOrObject,
                found: JsonType::Boolean,
            }
        );
    }

    #[test]
    fn len_and_push_require_the_matching_container() {
        let array = JsonValue::array(vec![JsonValue::Null, JsonValue::True]);
        assert_eq!(array.len().unwrap(), 2);
        assert_eq!(JsonValue::empty_object().len().unwrap(), 0);
        assert_eq!(
            JsonValue::number(1.0).len().unwrap_err().kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::ArrayOrObject,
                found: JsonType::Number,
            }
        );

        let mut array = JsonValue::array(vec![]);
        array.push(JsonValue::False).unwrap();
        assert_eq!(array.len().unwrap(), 1);

        assert_eq!(
            JsonValue::empty_object()
                .push(JsonValue::Null)
                .unwrap_err()
                .kind,
            JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Array,
                found: JsonType::Object,
            }
        );
    }

    #[test]
    fn into_owned_copies_borrowed_strings() {
        let input = String::from(r#"{"a":"x"}"#);
        // Simulate borrowed value then drop input after into_owned.
        let owned = {
            let borrowed = JsonValue::object([(
                Cow::Borrowed(&input[2..3]),
                JsonValue::string(Cow::Borrowed(&input[6..7])),
            )]);
            borrowed.into_owned()
        };
        assert_eq!(owned.get("a").unwrap(), &JsonValue::string("x"));
    }
}
