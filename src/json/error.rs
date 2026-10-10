use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

use crate::json::token::JsonTokenKind;
use crate::{Location, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedSyntax {
    Value,
    Token(JsonTokenKind),
}

impl ExpectedSyntax {
    fn name(&self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Token(kind) => kind.syntax_name(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonType {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Object,
}

impl JsonType {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedJsonType {
    Boolean,
    Number,
    Array,
    Object,
    ArrayOrObject,
}

impl ExpectedJsonType {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::Array => "array",
            Self::Object => "object",
            Self::ArrayOrObject => "array or object",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonErrorKind {
    InvalidCharacter,
    InvalidLiteral {
        expected: &'static str,
    },
    InvalidNumber,
    UnterminatedString,
    InvalidEscape,
    InvalidUnicodeEscape,
    InvalidSurrogatePair,
    UnescapedControl,
    EmptyInput,
    UnexpectedToken {
        expected: ExpectedSyntax,
        found: JsonTokenKind,
    },
    UnexpectedEof {
        expected: ExpectedSyntax,
    },
    TrailingInput,
    TypeMismatch {
        expected: ExpectedJsonType,
        found: JsonType,
    },
    KeyNotFound {
        key: String,
    },
    IndexOutOfBounds {
        index: usize,
        len: usize,
    },
    InvalidIndex {
        key: String,
    },
    InvalidCast,
    Io {
        message: String,
    },
}

impl JsonErrorKind {
    fn phase(&self) -> &'static str {
        match self {
            Self::InvalidCharacter
            | Self::InvalidLiteral { .. }
            | Self::InvalidNumber
            | Self::UnterminatedString
            | Self::InvalidEscape
            | Self::InvalidUnicodeEscape
            | Self::InvalidSurrogatePair
            | Self::UnescapedControl => "lexical",
            Self::EmptyInput
            | Self::UnexpectedToken { .. }
            | Self::UnexpectedEof { .. }
            | Self::TrailingInput => "parse",
            Self::TypeMismatch { .. }
            | Self::KeyNotFound { .. }
            | Self::IndexOutOfBounds { .. }
            | Self::InvalidIndex { .. }
            | Self::InvalidCast => "value",
            Self::Io { .. } => "io",
        }
    }

    fn message(&self) -> Cow<'static, str> {
        match self {
            Self::InvalidCharacter => Cow::Borrowed("invalid character"),
            Self::InvalidLiteral { expected } => {
                Cow::Owned(format!("invalid literal, expected {expected}"))
            }
            Self::InvalidNumber => Cow::Borrowed("invalid number"),
            Self::UnterminatedString => Cow::Borrowed("unterminated string"),
            Self::InvalidEscape => Cow::Borrowed("invalid escape"),
            Self::InvalidUnicodeEscape => Cow::Borrowed("invalid unicode escape"),
            Self::InvalidSurrogatePair => Cow::Borrowed("invalid surrogate pair"),
            Self::UnescapedControl => Cow::Borrowed("unescaped control character"),
            Self::EmptyInput => Cow::Borrowed("empty input"),
            Self::UnexpectedToken { expected, found } => Cow::Owned(format!(
                "expected {} but found {}",
                expected.name(),
                found.syntax_name(),
            )),
            Self::UnexpectedEof { expected } => Cow::Owned(format!(
                "expected {} but reached end of input",
                expected.name(),
            )),
            Self::TrailingInput => Cow::Borrowed("trailing input"),
            Self::TypeMismatch { expected, found } => Cow::Owned(format!(
                "expected {} but found {}",
                expected.name(),
                found.name(),
            )),
            Self::KeyNotFound { key } => Cow::Owned(format!("key \"{key}\" not found")),
            Self::IndexOutOfBounds { index, len } => {
                Cow::Owned(format!("index {index} out of bounds for length {len}"))
            }
            Self::InvalidIndex { key } => Cow::Owned(format!("invalid index \"{key}\"")),
            Self::InvalidCast => Cow::Borrowed("invalid cast"),
            Self::Io { message } => Cow::Owned(message.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub kind: JsonErrorKind,
    pub span: Option<Span>,
    pub location: Option<Location>,
}

impl JsonError {
    pub fn new(kind: JsonErrorKind, span: Span, source: &str) -> Self {
        Self {
            kind,
            span: Some(span),
            location: Some(Location::from_byte(source, span.start)),
        }
    }

    pub fn at(kind: JsonErrorKind, span: Span, location: Location) -> Self {
        Self {
            kind,
            span: Some(span),
            location: Some(location),
        }
    }

    pub fn value(kind: JsonErrorKind) -> Self {
        Self {
            kind,
            span: None,
            location: None,
        }
    }

    pub fn io(err: impl std::fmt::Display) -> Self {
        Self::value(JsonErrorKind::Io {
            message: err.to_string(),
        })
    }
}

impl std::error::Error for JsonError {}

impl Display for JsonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match (self.location, self.span) {
            (Some(loc), Some(span)) => write!(
                f,
                "{} error at {}:{} ({}..{}): {}",
                self.kind.phase(),
                loc.line,
                loc.column,
                span.start,
                span.end,
                self.kind.message(),
            ),
            (None, Some(span)) => write!(
                f,
                "{} error at {}..{}: {}",
                self.kind.phase(),
                span.start,
                span.end,
                self.kind.message(),
            ),
            _ => write!(f, "{} error: {}", self.kind.phase(), self.kind.message()),
        }
    }
}


#[cfg(test)]
#[path = "../../tests/json/error.rs"]
mod tests;
