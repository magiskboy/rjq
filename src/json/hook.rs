use std::borrow::Cow;

use crate::json::{error::JsonError, number::JsonNumber, value::JsonObject, value::JsonValue};

/// A scalar the lexer/parser has already validated.
///
/// The parser decides *that* something is a JSON null/bool/number/string;
/// the hook decides *how* that atom becomes a value in the target
/// representation (Rust `JsonValue`, a Python object, …).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonAtom<'a> {
    Null,
    Bool(bool),
    Number(JsonNumber<'a>),
    String(Cow<'a, str>),
}

/// Hook into the recursive-descent parser.
///
/// The parser owns syntax (tokens, braces, commas). A [`ParserHookable`]
/// receives structural events and builds whatever target it wants — a
/// [`JsonValue`] tree, CPython objects, a tape, or (later) a path-masked /
/// skip-capable consumer for lazy parse.
///
/// `Array` / `Object` are in-progress containers; `Value` is the finished form.
pub trait ParserHookable<'a> {
    type Value;
    type Array;
    type Object;
    type Error: From<JsonError>;

    fn on_atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error>;

    fn on_begin_array(&mut self) -> Result<Self::Array, Self::Error>;
    fn on_array_element(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
    fn on_end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error>;

    fn on_begin_object(&mut self) -> Result<Self::Object, Self::Error>;
    fn on_object_entry(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
    fn on_end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error>;
}

/// Hook that materializes the crate's native [`JsonValue`] tree.
#[derive(Debug, Default)]
pub struct JsonValueBuilder;

impl<'a> ParserHookable<'a> for JsonValueBuilder {
    type Value = JsonValue<'a>;
    type Array = Vec<JsonValue<'a>>;
    type Object = JsonObject<'a>;
    type Error = JsonError;

    fn on_atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error> {
        Ok(match atom {
            JsonAtom::Null => JsonValue::null(),
            JsonAtom::Bool(v) => JsonValue::boolean(v),
            JsonAtom::Number(n) => JsonValue::Number(n),
            JsonAtom::String(s) => JsonValue::string(s),
        })
    }

    fn on_begin_array(&mut self) -> Result<Self::Array, Self::Error> {
        Ok(Vec::new())
    }

    fn on_array_element(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        array.push(value);
        Ok(())
    }

    fn on_end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error> {
        Ok(JsonValue::array(array))
    }

    fn on_begin_object(&mut self) -> Result<Self::Object, Self::Error> {
        Ok(JsonObject::new())
    }

    fn on_object_entry(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        // Hot path: append without hashing or de-duplication. Duplicate keys are
        // resolved at lookup time (last value wins), matching dict / orjson.
        object.push(key, value);
        Ok(())
    }

    fn on_end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error> {
        Ok(JsonValue::Object(object))
    }
}
