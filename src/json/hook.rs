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

/// Whether the parser should materialize the next object value / array element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseDecision {
    /// Parse the value and deliver it via [`ParserHookable::on_object_entry`] /
    /// [`ParserHookable::on_array_element`].
    Keep,
    /// Lex/validate the value with `skip_value`, do not materialize or insert it.
    ///
    /// For arrays, the parser still advances the element index; the hook may
    /// push a placeholder (e.g. `null`) if index alignment is required.
    Skip,
}

/// Hook into the recursive-descent parser.
///
/// The parser owns syntax (tokens, braces, commas). A [`ParserHookable`]
/// receives structural events and builds whatever target it wants — a
/// [`JsonValue`] tree, CPython objects, or a path-masked lazy tree.
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

    /// Decide whether to materialize the value for `key` (default: always keep).
    fn want_object_key(&mut self, _key: &str) -> Result<ParseDecision, Self::Error> {
        Ok(ParseDecision::Keep)
    }

    /// Called after a skipped object value (default: no-op).
    fn on_object_key_skipped(&mut self, _key: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Decide whether to materialize array element `index` (default: always keep).
    fn want_array_element(&mut self, _index: usize) -> Result<ParseDecision, Self::Error> {
        Ok(ParseDecision::Keep)
    }

    /// Called after a skipped array element. Default no-op; lazy hooks may push
    /// a `null` placeholder so later indices stay aligned.
    fn on_array_element_skipped(
        &mut self,
        _array: &mut Self::Array,
        _index: usize,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// When true, bytes after the root value are ignored (lazy CLI). Default false.
    fn accept_trailing_junk(&self) -> bool {
        false
    }
}

/// Hook that materializes the crate's native [`JsonValue`] tree (eager).
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
