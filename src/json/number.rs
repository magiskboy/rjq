use std::borrow::Cow;
use std::fmt::{self, Display};

/// Lexically validated JSON number text. Parsing to int/float is deferred to
/// callers (`as_i64`, `as_f64`, `classify_py`).
#[derive(Debug, Clone, Eq)]
pub struct JsonNumber<'a> {
    raw: Cow<'a, str>,
    kind: JsonNumberKind,
}

/// Classification from the lexer state machine — no numeric parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonNumberKind {
    /// No `.` / `e` / `E` in the token.
    IntegerLike,
    /// Has fraction and/or exponent.
    FloatLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberError {
    NotInteger,
    OutOfRange,
    Invalid,
}

/// Python-oriented view of a JSON number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyNumber {
    Int(i64),
    Float(f64),
    /// Integer-like digits that do not fit in i64 — caller should build a
    /// Python int from the raw spelling (e.g. `PyLong_FromString`).
    BigInt,
}

impl<'a> JsonNumber<'a> {
    pub fn new(raw: impl Into<Cow<'a, str>>, kind: JsonNumberKind) -> Self {
        Self {
            raw: raw.into(),
            kind,
        }
    }

    pub fn raw(&self) -> &str {
        self.raw.as_ref()
    }

    pub fn kind(&self) -> JsonNumberKind {
        self.kind
    }

    pub fn into_owned(self) -> JsonNumber<'static> {
        JsonNumber {
            raw: Cow::Owned(self.raw.into_owned()),
            kind: self.kind,
        }
    }

    /// Build from an `f64` for literals / computed values (JQL, tests).
    ///
    /// Whole finite values in `i64` range become `IntegerLike` with decimal
    /// spelling; everything else is `FloatLike` via `Display`.
    pub fn from_f64(value: f64) -> JsonNumber<'static> {
        if value.is_finite()
            && value.fract() == 0.0
            && value >= i64::MIN as f64
            && value <= i64::MAX as f64
        {
            let i = value as i64;
            return JsonNumber::new(i.to_string(), JsonNumberKind::IntegerLike);
        }
        JsonNumber::new(format!("{value}"), JsonNumberKind::FloatLike)
    }

    pub fn as_i64(&self) -> Result<i64, NumberError> {
        if self.kind != JsonNumberKind::IntegerLike {
            return Err(NumberError::NotInteger);
        }
        self.raw.parse::<i64>().map_err(|_| NumberError::OutOfRange)
    }

    pub fn as_f64(&self) -> Result<f64, NumberError> {
        let v = self.raw.parse::<f64>().map_err(|_| NumberError::Invalid)?;
        if !v.is_finite() {
            return Err(NumberError::OutOfRange);
        }
        Ok(v)
    }

    /// CPython / orjson style: integer-like → int (or BigInt); else float.
    pub fn classify_py(&self) -> Result<PyNumber, NumberError> {
        match self.kind {
            JsonNumberKind::IntegerLike => match self.as_i64() {
                Ok(i) => Ok(PyNumber::Int(i)),
                Err(NumberError::OutOfRange) => Ok(PyNumber::BigInt),
                Err(err) => Err(err),
            },
            JsonNumberKind::FloatLike => Ok(PyNumber::Float(self.as_f64()?)),
        }
    }
}

impl PartialEq for JsonNumber<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Display for JsonNumber<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_like_parses_i64() {
        let n = JsonNumber::new("42", JsonNumberKind::IntegerLike);
        assert_eq!(n.as_i64(), Ok(42));
        assert_eq!(n.classify_py(), Ok(PyNumber::Int(42)));
    }

    #[test]
    fn float_like_rejects_i64() {
        let n = JsonNumber::new("1.5", JsonNumberKind::FloatLike);
        assert_eq!(n.as_i64(), Err(NumberError::NotInteger));
        assert_eq!(n.classify_py(), Ok(PyNumber::Float(1.5)));
    }

    #[test]
    fn big_int_classified_without_parse_failure() {
        let raw = "9223372036854775808"; // i64::MAX + 1
        let n = JsonNumber::new(raw, JsonNumberKind::IntegerLike);
        assert_eq!(n.as_i64(), Err(NumberError::OutOfRange));
        assert_eq!(n.classify_py(), Ok(PyNumber::BigInt));
    }

    #[test]
    fn lexical_eq_distinguishes_spellings() {
        let a = JsonNumber::new("1", JsonNumberKind::IntegerLike);
        let b = JsonNumber::new("1.0", JsonNumberKind::FloatLike);
        assert_ne!(a, b);
        assert_eq!(a.as_f64().unwrap(), b.as_f64().unwrap());
    }

    #[test]
    fn from_f64_integer_spelling() {
        let n = JsonNumber::from_f64(42.0);
        assert_eq!(n.raw(), "42");
        assert_eq!(n.kind(), JsonNumberKind::IntegerLike);
    }

    #[test]
    fn scientific_as_f64() {
        let n = JsonNumber::new("1e2", JsonNumberKind::FloatLike);
        assert_eq!(n.as_f64(), Ok(100.0));
    }
}
