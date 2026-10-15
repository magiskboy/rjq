use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::CString;
use std::ptr;

use crate::json::{
    error::JsonError,
    hook::{JsonAtom, ParserHookable},
    number::{NumberError, PyNumber},
    parser::JsonParser,
};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use pyo3::{Bound, Py, PyAny, Python};

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

impl From<NumberError> for PyErr {
    fn from(err: NumberError) -> Self {
        PyValueError::new_err(format!("invalid number: {err:?}"))
    }
}

/// [`ParserHookable`] that builds CPython objects directly — no intermediate `JsonValue` tree.
struct PyBuilder<'py> {
    py: Python<'py>,
    /// Object-key intern table for this parse: one `PyString` per distinct key text.
    key_cache: HashMap<Box<str>, Py<PyString>>,
}

impl<'py> PyBuilder<'py> {
    fn new(py: Python<'py>) -> Self {
        Self {
            py,
            key_cache: HashMap::new(),
        }
    }

    fn py_long_from_string(&self, raw: &str) -> PyResult<Bound<'py, PyAny>> {
        let cstr = CString::new(raw).map_err(|_| PyValueError::new_err("nul in number"))?;
        unsafe {
            let ptr = pyo3::ffi::PyLong_FromString(cstr.as_ptr(), ptr::null_mut(), 10);
            Bound::from_owned_ptr_or_err(self.py, ptr).map(|b| b.into_any())
        }
    }

    fn intern_key(&mut self, key: &str) -> PyResult<Py<PyString>> {
        if let Some(cached) = self.key_cache.get(key) {
            return Ok(cached.clone_ref(self.py));
        }
        let py_str = PyString::new(self.py, key);
        let owned: Py<PyString> = py_str.unbind();
        self.key_cache.insert(key.into(), owned.clone_ref(self.py));
        Ok(owned)
    }
}

impl<'a, 'py> ParserHookable<'a> for PyBuilder<'py> {
    type Value = Bound<'py, PyAny>;
    type Array = Bound<'py, PyList>;
    type Object = Bound<'py, PyDict>;
    type Error = PyErr;

    fn on_atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error> {
        match atom {
            JsonAtom::Null => Ok(self.py.None().into_bound(self.py)),
            JsonAtom::Bool(v) => v.into_bound_py_any(self.py),
            JsonAtom::Number(n) => match n.classify_py()? {
                PyNumber::Int(i) => i.into_bound_py_any(self.py),
                PyNumber::Float(f) => f.into_bound_py_any(self.py),
                PyNumber::BigInt => self.py_long_from_string(n.raw()),
            },
            JsonAtom::String(s) => Ok(PyString::new(self.py, s.as_ref()).into_any()),
        }
    }

    fn on_begin_array(&mut self) -> Result<Self::Array, Self::Error> {
        Ok(PyList::empty(self.py))
    }

    fn on_array_element(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        array.append(value)
    }

    fn on_end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error> {
        Ok(array.into_any())
    }

    fn on_begin_object(&mut self) -> Result<Self::Object, Self::Error> {
        Ok(PyDict::new(self.py))
    }

    fn on_object_entry(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        let py_key = self.intern_key(key.as_ref())?;
        object.set_item(py_key.bind(self.py), value)
    }

    fn on_end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error> {
        Ok(object.into_any())
    }
}
