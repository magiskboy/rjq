use crate::Span;
use crate::json::number::JsonNumber;
use std::borrow::Cow;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonTokenKind {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    String,
    Number,
    True,
    False,
    Null,
    Colon,
    Comma,
    Whitespace,
    Stop,
}

impl Display for JsonTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonTokenKind::LBrace => write!(f, "LBrace"),
            JsonTokenKind::RBrace => write!(f, "RBrace"),
            JsonTokenKind::LBracket => write!(f, "LBracket"),
            JsonTokenKind::RBracket => write!(f, "RBracket"),
            JsonTokenKind::String => write!(f, "String"),
            JsonTokenKind::Number => write!(f, "Number"),
            JsonTokenKind::True => write!(f, "True"),
            JsonTokenKind::False => write!(f, "False"),
            JsonTokenKind::Null => write!(f, "Null"),
            JsonTokenKind::Colon => write!(f, "Colon"),
            JsonTokenKind::Comma => write!(f, "Comma"),
            JsonTokenKind::Whitespace => write!(f, "Whitespace"),
            JsonTokenKind::Stop => write!(f, "Stop"),
        }
    }
}

impl JsonTokenKind {
    pub(crate) fn syntax_name(&self) -> &'static str {
        match self {
            JsonTokenKind::LBrace => "'{'",
            JsonTokenKind::RBrace => "'}'",
            JsonTokenKind::LBracket => "'['",
            JsonTokenKind::RBracket => "']'",
            JsonTokenKind::String => "string",
            JsonTokenKind::Number => "number",
            JsonTokenKind::True => "'true'",
            JsonTokenKind::False => "'false'",
            JsonTokenKind::Null => "'null'",
            JsonTokenKind::Colon => "':'",
            JsonTokenKind::Comma => "','",
            JsonTokenKind::Whitespace => "whitespace",
            JsonTokenKind::Stop => "end",
        }
    }
}

/// Payload carried by string/number tokens.
///
/// String lexemes use [`Cow`]: borrowed from the input when the JSON string has
/// no escapes; owned only when unescaping is required.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonLexeme<'a> {
    None,
    String(Cow<'a, str>),
    Number(JsonNumber<'a>),
}

/// A lexed token. Carries only a byte [`Span`]; 1-based line/column is derived
/// lazily from the source at error-reporting time (see `Lexer::location_at`),
/// keeping the lex/parse hot path free of per-byte position bookkeeping.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonToken<'a> {
    pub kind: JsonTokenKind,
    pub span: Span,
    pub lexeme: JsonLexeme<'a>,
}

impl<'a> JsonToken<'a> {
    pub fn simple(kind: JsonTokenKind, span: Span) -> Self {
        Self {
            kind,
            span,
            lexeme: JsonLexeme::None,
        }
    }

    pub fn string(span: Span, value: Cow<'a, str>) -> Self {
        Self {
            kind: JsonTokenKind::String,
            span,
            lexeme: JsonLexeme::String(value),
        }
    }

    pub fn number(span: Span, value: JsonNumber<'a>) -> Self {
        Self {
            kind: JsonTokenKind::Number,
            span,
            lexeme: JsonLexeme::Number(value),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn display(&self) -> JsonTokenDisplay<'_, 'a> {
        JsonTokenDisplay { token: self }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct JsonTokenDisplay<'t, 'a> {
    pub token: &'t JsonToken<'a>,
}

impl<'t, 'a> Display for JsonTokenDisplay<'t, 'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self.token.lexeme {
            JsonLexeme::None => "",
            JsonLexeme::String(s) => s.as_ref(),
            JsonLexeme::Number(n) => n.raw(),
        };
        write!(
            f,
            "<kind={}, start={}, end={}, content={}",
            self.token.kind, self.token.span.start, self.token.span.end, content
        )
    }
}
