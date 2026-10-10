use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

use crate::json::error::JsonError;
use crate::jql::token::JqlTokenKind;
use crate::{Location, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedSyntax {
    BinaryOperator,
    Expression,
    Token(JqlTokenKind),
}

impl ExpectedSyntax {
    fn name(&self) -> &'static str {
        match self {
            Self::BinaryOperator => "binary operator",
            Self::Expression => "expression",
            Self::Token(kind) => kind.syntax_name(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JqlErrorKind {
    InvalidToken,
    MissingOperand,
    UnclosedParen,
    InvalidExpression,
    UnexpectedToken {
        expected: ExpectedSyntax,
        found: JqlTokenKind,
    },
    InvalidAccess {
        path: String,
    },
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },
    PathNotFound {
        path: String,
    },
    UnsupportedOperation {
        op: String,
    },
    Json(JsonError),
}

impl JqlErrorKind {
    fn phase(&self) -> Option<&'static str> {
        match self {
            Self::InvalidToken => Some("lexical"),
            Self::MissingOperand
            | Self::UnclosedParen
            | Self::InvalidExpression
            | Self::UnexpectedToken { .. } => Some("parse"),
            Self::InvalidAccess { .. }
            | Self::TypeMismatch { .. }
            | Self::PathNotFound { .. }
            | Self::UnsupportedOperation { .. } => Some("execute"),
            Self::Json(_) => None,
        }
    }

    fn message(&self) -> Cow<'static, str> {
        match self {
            Self::InvalidToken => Cow::Borrowed("invalid token"),
            Self::MissingOperand => Cow::Borrowed("expected an operand"),
            Self::UnclosedParen => Cow::Borrowed("expected a closed parenthesis"),
            Self::InvalidExpression => Cow::Borrowed("invalid expression"),
            Self::UnexpectedToken { expected, found } => Cow::Owned(format!(
                "expected {} but found {}",
                expected.name(),
                found.syntax_name(),
            )),
            Self::InvalidAccess { path } => Cow::Owned(format!("invalid reference \"{path}\"")),
            Self::TypeMismatch { expected, found } => {
                Cow::Owned(format!("expected {expected} but found {found}"))
            }
            Self::PathNotFound { path } => Cow::Owned(format!("path \"{path}\" not found")),
            Self::UnsupportedOperation { op } => {
                Cow::Owned(format!("unsupported operation \"{op}\""))
            }
            Self::Json(err) => Cow::Owned(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JqlError {
    pub kind: JqlErrorKind,
    pub span: Option<Span>,
    pub location: Option<Location>,
}

impl JqlError {
    pub fn new(kind: JqlErrorKind, span: Span, source: &str) -> Self {
        Self {
            kind,
            span: Some(span),
            location: Some(Location::from_byte(source, span.start)),
        }
    }

    pub fn without_span(kind: JqlErrorKind) -> Self {
        Self {
            kind,
            span: None,
            location: None,
        }
    }
}

impl From<JsonError> for JqlError {
    fn from(err: JsonError) -> Self {
        match err.kind {
            crate::json::error::JsonErrorKind::KeyNotFound { key } => {
                Self::without_span(JqlErrorKind::PathNotFound { path: key })
            }
            crate::json::error::JsonErrorKind::TypeMismatch { expected, found } => {
                Self::without_span(JqlErrorKind::TypeMismatch {
                    expected: expected.name(),
                    found: found.name(),
                })
            }
            crate::json::error::JsonErrorKind::IndexOutOfBounds { index, len } => {
                Self::without_span(JqlErrorKind::PathNotFound {
                    path: format!("[{index}] (len {len})"),
                })
            }
            _ => Self::without_span(JqlErrorKind::Json(err)),
        }
    }
}

impl std::error::Error for JqlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            JqlErrorKind::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl Display for JqlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if matches!(self.kind, JqlErrorKind::Json(_)) {
            return write!(f, "{}", self.kind.message());
        }

        let phase = self.kind.phase().unwrap_or("execute");
        match (self.location, self.span) {
            (Some(loc), Some(span)) => write!(
                f,
                "{} error at {}:{} ({}..{}): {}",
                phase,
                loc.line,
                loc.column,
                span.start,
                span.end,
                self.kind.message(),
            ),
            (None, Some(span)) => write!(
                f,
                "{} error at {}..{}: {}",
                phase,
                span.start,
                span.end,
                self.kind.message(),
            ),
            _ => write!(f, "{} error: {}", phase, self.kind.message()),
        }
    }
}


#[cfg(test)]
#[path = "../../tests/jql/error.rs"]
mod tests;
