use std::fmt::{self, Display, Formatter};

use rs_jq::jql::error::JqlError;
use rs_jq::json::error::JsonError;

#[derive(Debug)]
pub enum AppError {
    InvalidFormat(String),
    Io {
        path: Option<String>,
        source: std::io::Error,
    },
    Json(JsonError),
    Jql(JqlError),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "{msg}"),
            Self::Io {
                path: Some(path),
                source,
            } => write!(f, "cannot read {path}: {source}"),
            Self::Io { path: None, source } => write!(f, "cannot read input: {source}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::Jql(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(err) => Some(err),
            Self::Jql(err) => Some(err),
            Self::InvalidFormat(_) => None,
        }
    }
}

impl From<JsonError> for AppError {
    fn from(err: JsonError) -> Self {
        Self::Json(err)
    }
}

impl From<JqlError> for AppError {
    fn from(err: JqlError) -> Self {
        Self::Jql(err)
    }
}
