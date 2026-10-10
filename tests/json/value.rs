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
