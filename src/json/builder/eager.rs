use crate::{
    json::hook::{JsonAtom, ParserHookable},
    json::{
        error::JsonError,
        value::{JsonObject, JsonValue},
    },
};
use std::borrow::Cow;

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
