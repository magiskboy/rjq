use std::borrow::Cow;

use crate::json::{
    builder::{JsonAtom, JsonBuilder, JsonNumberAtom},
    error::JsonError,
    parser::JsonParser,
};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use pyo3::{Bound, PyAny, Python};

#[pymodule]
mod rjson {
    use super::*;

    #[pyfunction]
    fn rjson_load<'py>(py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let mut builder = PyBuilder::new(py);
        JsonParser::parse_slice_with(data, &mut builder)
    }

    #[pyfunction]
    fn rjson_loads<'py>(py: Python<'py>, string: &str) -> PyResult<Bound<'py, PyAny>> {
        let mut builder = PyBuilder::new(py);
        JsonParser::parse_str_with(string, &mut builder)
    }
}

impl From<JsonError> for PyErr {
    fn from(err: JsonError) -> Self {
        PyValueError::new_err(err.to_string())
    }
}

/// Builds CPython objects directly from parser atoms — no intermediate `JsonValue` tree.
struct PyBuilder<'py> {
    py: Python<'py>,
}

impl<'py> PyBuilder<'py> {
    fn new(py: Python<'py>) -> Self {
        Self { py }
    }
}

impl<'a, 'py> JsonBuilder<'a> for PyBuilder<'py> {
    type Value = Bound<'py, PyAny>;
    type Array = Bound<'py, PyList>;
    type Object = Bound<'py, PyDict>;
    type Error = PyErr;

    fn atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error> {
        match atom {
            JsonAtom::Null => Ok(self.py.None().into_bound(self.py)),
            JsonAtom::Bool(v) => v.into_bound_py_any(self.py),
            JsonAtom::Number(JsonNumberAtom::F32(n)) => n.into_bound_py_any(self.py),
            JsonAtom::String(s) => Ok(PyString::new(self.py, s.as_ref()).into_any()),
        }
    }

    fn begin_array(&mut self) -> Result<Self::Array, Self::Error> {
        Ok(PyList::empty(self.py))
    }

    fn array_push(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        array.append(value)
    }

    fn end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error> {
        Ok(array.into_any())
    }

    fn begin_object(&mut self) -> Result<Self::Object, Self::Error> {
        Ok(PyDict::new(self.py))
    }

    fn object_insert(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        object.set_item(key.as_ref(), value)
    }

    fn end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error> {
        Ok(object.into_any())
    }
}
