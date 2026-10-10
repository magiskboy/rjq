use std::borrow::Cow;
use std::collections::HashMap;

use crate::json::{error::JsonError, number::JsonNumber, value::JsonValue};

/// A scalar the lexer/parser has already validated.
///
/// The parser decides *that* something is a JSON null/bool/number/string;
/// the builder decides *how* that atom becomes a value in the target
/// representation (Rust `JsonValue`, a Python object, …).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonAtom<'a> {
    Null,
    Bool(bool),
    Number(JsonNumber<'a>),
    String(Cow<'a, str>),
}

/// Builds a target value tree while the parser walks JSON syntax.
///
/// `Array` / `Object` are the in-progress container types; `Value` is the
/// finished form. Splitting them lets a builder use e.g. `Vec<_>` while
/// open and wrap into `JsonValue::Array` only at `end_array`.
pub trait JsonBuilder<'a> {
    type Value;
    type Array;
    type Object;
    type Error: From<JsonError>;

    fn atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error>;

    fn begin_array(&mut self) -> Result<Self::Array, Self::Error>;
    fn array_push(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
    fn end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error>;

    fn begin_object(&mut self) -> Result<Self::Object, Self::Error>;
    fn object_insert(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
    fn end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error>;
}

/// Builder that materializes the crate's native [`JsonValue`] tree.
#[derive(Debug, Default)]
pub struct JsonValueBuilder;

impl<'a> JsonBuilder<'a> for JsonValueBuilder {
    type Value = JsonValue<'a>;
    type Array = Vec<JsonValue<'a>>;
    type Object = Vec<(Cow<'a, str>, JsonValue<'a>)>;
    type Error = JsonError;

    fn atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error> {
        Ok(match atom {
            JsonAtom::Null => JsonValue::null(),
            JsonAtom::Bool(v) => JsonValue::boolean(v),
            JsonAtom::Number(n) => JsonValue::Number(n),
            JsonAtom::String(s) => JsonValue::string(s),
        })
    }

    fn begin_array(&mut self) -> Result<Self::Array, Self::Error> {
        Ok(Vec::new())
    }

    fn array_push(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        array.push(value);
        Ok(())
    }

    fn end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error> {
        Ok(JsonValue::array(array))
    }

    fn begin_object(&mut self) -> Result<Self::Object, Self::Error> {
        Ok(Vec::new())
    }

    fn object_insert(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        object.push((key, value));
        Ok(())
    }

    fn end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error> {
        // Collect through HashMap so duplicate keys keep the last value,
        // matching the previous parser behavior.
        Ok(JsonValue::object(
            HashMap::<Cow<'a, str>, JsonValue<'a>>::from_iter(object.into_iter()),
        ))
    }
}
