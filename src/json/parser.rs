use std::io::Read;
use std::mem;

use crate::Location;
use crate::Span;
use crate::json::{
    builder::{JsonAtom, JsonBuilder, JsonValueBuilder},
    error::{ExpectedSyntax, JsonError, JsonErrorKind},
    lexer::Lexer,
    token::{JsonLexeme, JsonToken, JsonTokenKind},
    value::JsonValue,
};

pub struct JsonParser<'a> {
    lexer: Lexer<'a>,
    current: JsonToken<'a>,
    /// Absolute end of the last consumed non-stop token (for EOF diagnostics).
    last_end: usize,
}

impl<'a> JsonParser<'a> {
    pub fn parse_slice(data: &'a [u8]) -> Result<JsonValue<'a>, JsonError> {
        Self::parse_slice_with(data, &mut JsonValueBuilder)
    }

    pub fn parse_str(source: &'a str) -> Result<JsonValue<'a>, JsonError> {
        Self::parse_slice(source.as_bytes())
    }

    pub fn parse_slice_with<B: JsonBuilder<'a>>(
        data: &'a [u8],
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        Self::from_lexer(Lexer::from_slice(data), builder)
    }

    #[cfg_attr(not(feature = "python"), allow(dead_code))]
    pub fn parse_str_with<B: JsonBuilder<'a>>(
        source: &'a str,
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        Self::parse_slice_with(source.as_bytes(), builder)
    }

    fn from_lexer<B: JsonBuilder<'a>>(
        mut lexer: Lexer<'a>,
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        let first = loop {
            let pos_before = lexer.position();
            let token = lexer.next_token()?;
            match token.kind {
                JsonTokenKind::Whitespace => {
                    lexer.discard_consumed();
                    continue;
                }
                JsonTokenKind::Stop => {
                    return Err(JsonError::at(
                        JsonErrorKind::EmptyInput,
                        Span {
                            start: 0,
                            end: pos_before,
                        },
                        Location { line: 1, column: 1 },
                    )
                    .into());
                }
                _ => {
                    lexer.discard_consumed();
                    break token;
                }
            }
        };

        let mut parser = Self {
            last_end: first.span.end,
            current: first,
            lexer,
        };
        let value = parser.parse_value(builder)?;
        parser.expect_eof()?;
        Ok(value)
    }
}

impl JsonParser<'_> {
    /// Buffer the reader, parse with zero-copy borrows, then own any borrowed strings.
    pub fn parse_reader(mut reader: impl Read) -> Result<JsonValue<'static>, JsonError> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).map_err(JsonError::io)?;
        JsonParser::parse_slice(&buf).map(JsonValue::into_owned)
    }
}

impl<'a> JsonParser<'a> {
    fn error(&self, kind: JsonErrorKind, span: Span) -> JsonError {
        let location = self.lexer.location_at(span.start);
        JsonError::at(kind, span, location)
    }

    fn bump(&mut self) -> Result<(), JsonError> {
        if self.current.kind != JsonTokenKind::Stop {
            self.last_end = self.current.span.end;
        }
        self.current = self.lexer.next_significant()?;
        Ok(())
    }

    fn expect_eof(&mut self) -> Result<(), JsonError> {
        if self.current.kind != JsonTokenKind::Stop {
            return Err(self.error(JsonErrorKind::TrailingInput, self.current.span));
        }
        Ok(())
    }

    fn eof_error(&self, expected: ExpectedSyntax) -> JsonError {
        self.error(
            JsonErrorKind::UnexpectedEof { expected },
            Span {
                start: self.last_end,
                end: self.last_end,
            },
        )
    }

    fn expect(&mut self, kind: JsonTokenKind) -> Result<JsonToken<'a>, JsonError> {
        if self.current.kind != kind {
            if self.current.kind == JsonTokenKind::Stop {
                return Err(self.eof_error(ExpectedSyntax::Token(kind)));
            }
            return Err(self.error(
                JsonErrorKind::UnexpectedToken {
                    expected: ExpectedSyntax::Token(kind),
                    found: self.current.kind.clone(),
                },
                self.current.span,
            ));
        }
        let placeholder = JsonToken::simple(JsonTokenKind::Stop, self.current.span);
        let token = mem::replace(&mut self.current, placeholder);
        self.bump()?;
        Ok(token)
    }

    fn parse_object<B: JsonBuilder<'a>>(
        &mut self,
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        self.expect(JsonTokenKind::LBrace)?;
        let mut object = builder.begin_object()?;
        if self.current.kind == JsonTokenKind::RBrace {
            self.bump()?;
            return builder.end_object(object);
        }

        loop {
            if self.current.kind != JsonTokenKind::String {
                if self.current.kind == JsonTokenKind::Stop {
                    return Err(self
                        .eof_error(ExpectedSyntax::Token(JsonTokenKind::RBrace))
                        .into());
                }
                return Err(self
                    .unexpected(ExpectedSyntax::Token(JsonTokenKind::String))
                    .into());
            }

            let key_token = self.expect(JsonTokenKind::String)?;
            let key = match key_token.lexeme {
                JsonLexeme::String(s) => s,
                _ => {
                    return Err(self
                        .error(JsonErrorKind::InvalidEscape, key_token.span)
                        .into());
                }
            };
            self.expect(JsonTokenKind::Colon)?;
            let value = self.parse_value(builder)?;
            builder.object_insert(&mut object, key, value)?;

            match self.current.kind {
                JsonTokenKind::Comma => {
                    self.bump()?;
                    if self.current.kind == JsonTokenKind::RBrace {
                        return Err(self
                            .unexpected(ExpectedSyntax::Token(JsonTokenKind::String))
                            .into());
                    }
                    continue;
                }
                JsonTokenKind::RBrace => {
                    self.bump()?;
                    break;
                }
                JsonTokenKind::Stop => {
                    return Err(self
                        .eof_error(ExpectedSyntax::Token(JsonTokenKind::RBrace))
                        .into());
                }
                _ => {
                    return Err(self
                        .unexpected(ExpectedSyntax::Token(JsonTokenKind::RBrace))
                        .into());
                }
            }
        }

        builder.end_object(object)
    }

    fn parse_array<B: JsonBuilder<'a>>(
        &mut self,
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        self.expect(JsonTokenKind::LBracket)?;
        let mut array = builder.begin_array()?;
        if self.current.kind == JsonTokenKind::RBracket {
            self.bump()?;
            return builder.end_array(array);
        }

        loop {
            if !Self::starts_value(&self.current.kind) {
                if self.current.kind == JsonTokenKind::Stop {
                    return Err(self
                        .eof_error(ExpectedSyntax::Token(JsonTokenKind::RBracket))
                        .into());
                }
                return Err(self.unexpected(ExpectedSyntax::Value).into());
            }

            let value = self.parse_value(builder)?;
            builder.array_push(&mut array, value)?;

            match self.current.kind {
                JsonTokenKind::Comma => {
                    self.bump()?;
                    if self.current.kind == JsonTokenKind::RBracket {
                        return Err(self.unexpected(ExpectedSyntax::Value).into());
                    }
                    continue;
                }
                JsonTokenKind::RBracket => {
                    self.bump()?;
                    break;
                }
                JsonTokenKind::Stop => {
                    return Err(self
                        .eof_error(ExpectedSyntax::Token(JsonTokenKind::RBracket))
                        .into());
                }
                _ => {
                    return Err(self
                        .unexpected(ExpectedSyntax::Token(JsonTokenKind::RBracket))
                        .into());
                }
            }
        }

        builder.end_array(array)
    }

    fn parse_value<B: JsonBuilder<'a>>(
        &mut self,
        builder: &mut B,
    ) -> Result<B::Value, B::Error> {
        match self.current.kind {
            JsonTokenKind::Null => {
                self.bump()?;
                builder.atom(JsonAtom::Null)
            }
            JsonTokenKind::True => {
                self.bump()?;
                builder.atom(JsonAtom::Bool(true))
            }
            JsonTokenKind::False => {
                self.bump()?;
                builder.atom(JsonAtom::Bool(false))
            }
            JsonTokenKind::String => {
                let span = self.current.span;
                let lexeme = mem::replace(&mut self.current.lexeme, JsonLexeme::None);
                self.bump()?;
                match lexeme {
                    JsonLexeme::String(s) => builder.atom(JsonAtom::String(s)),
                    _ => Err(self.error(JsonErrorKind::InvalidEscape, span).into()),
                }
            }
            JsonTokenKind::Number => {
                let span = self.current.span;
                let lexeme = mem::replace(&mut self.current.lexeme, JsonLexeme::None);
                self.bump()?;
                match lexeme {
                    JsonLexeme::Number(n) => builder.atom(JsonAtom::Number(n)),
                    _ => Err(self.error(JsonErrorKind::InvalidNumber, span).into()),
                }
            }
            JsonTokenKind::LBracket => self.parse_array(builder),
            JsonTokenKind::LBrace => self.parse_object(builder),
            JsonTokenKind::Stop => Err(self.eof_error(ExpectedSyntax::Value).into()),
            _ => Err(self.unexpected(ExpectedSyntax::Value).into()),
        }
    }

    fn unexpected(&self, expected: ExpectedSyntax) -> JsonError {
        self.error(
            JsonErrorKind::UnexpectedToken {
                expected,
                found: self.current.kind.clone(),
            },
            self.current.span,
        )
    }

    fn starts_value(kind: &JsonTokenKind) -> bool {
        matches!(
            kind,
            JsonTokenKind::Null
                | JsonTokenKind::True
                | JsonTokenKind::False
                | JsonTokenKind::String
                | JsonTokenKind::Number
                | JsonTokenKind::LBracket
                | JsonTokenKind::LBrace
        )
    }
}


#[cfg(test)]
#[path = "../../tests/json/parser.rs"]
mod test;
