use crate::Span;

#[derive(Debug, Clone, Eq, PartialEq, Copy, PartialOrd)]
pub enum JqlTokenKind {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Comma,

    // data types
    Number,
    String,
    Boolean,
    Null,

    // and/or must be less than others
    Pipe,

    AndLogicalOp,
    OrLogicalOp,

    Identifier,
    EqualOp,
    NotEqualOp,
    GreaterOp,
    LessOp,
    GreaterEqualOp,
    LessEqualOp,

    // Special tokens
    Stop,
    InvalidToken,
    Whitespace,
}

impl std::fmt::Display for JqlTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LParen => write!(f, "LParen"),
            Self::RParen => write!(f, "RParen"),
            Self::LBrace => write!(f, "LBrace"),
            Self::RBrace => write!(f, "RBrace"),
            Self::Colon => write!(f, "Colon"),
            Self::Comma => write!(f, "Comma"),
            Self::Pipe => write!(f, "Pipe"),
            Self::Number => write!(f, "Number"),
            Self::String => write!(f, "String"),
            Self::Boolean => write!(f, "Boolean"),
            Self::Null => write!(f, "Null"),
            Self::Identifier => write!(f, "Identifier"),
            Self::EqualOp => write!(f, "Equal"),
            Self::NotEqualOp => write!(f, "NotEqualOp"),
            Self::GreaterOp => write!(f, "GreaterOp"),
            Self::GreaterEqualOp => write!(f, "GreaterEqualOp"),
            Self::LessOp => write!(f, "LessOp"),
            Self::LessEqualOp => write!(f, "LessEqualOp"),
            Self::AndLogicalOp => write!(f, "AndLogicalOp"),
            Self::OrLogicalOp => write!(f, "OrLogicalOp"),
            Self::Stop => write!(f, "Stop"),
            Self::Whitespace => write!(f, "Whitespace"),
            Self::InvalidToken => write!(f, "InvalidToken"),
        }
    }
}

impl JqlTokenKind {
    pub(crate) fn syntax_name(&self) -> &'static str {
        match self {
            Self::LParen => "'('",
            Self::RParen => "')'",
            Self::LBrace => "'{'",
            Self::RBrace => "'}'",
            Self::Colon => "':'",
            Self::Comma => "','",
            Self::Pipe => "'|'",
            Self::Number => "number",
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Null => "'null'",
            Self::Identifier => "identifier",
            Self::EqualOp => "'=='",
            Self::NotEqualOp => "'!='",
            Self::GreaterOp => "'>'",
            Self::GreaterEqualOp => "'>='",
            Self::LessOp => "'<'",
            Self::LessEqualOp => "'<='",
            Self::AndLogicalOp => "'&&'",
            Self::OrLogicalOp => "'||'",
            Self::Stop => "end",
            Self::Whitespace => "whitespace",
            Self::InvalidToken => "invalid",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Copy)]
pub struct JqlToken {
    pub kind: JqlTokenKind,
    pub span: Span,
}

impl JqlToken {
    pub fn new(kind: JqlTokenKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: Span { start, end },
        }
    }

    pub fn display<'a>(&'a self, source: &'a str) -> JqlTokenDisplay<'a> {
        JqlTokenDisplay {
            token: self,
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JqlTokenDisplay<'a> {
    token: &'a JqlToken,
    source: &'a str,
}

impl<'a> std::fmt::Display for JqlTokenDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = self
            .source
            .get(self.token.span.start..self.token.span.end)
            .unwrap_or("");
        write!(
            f,
            "<kind={}, start={}, end={}, content={} />",
            self.token.kind, self.token.span.start, self.token.span.end, content
        )
    }
}
