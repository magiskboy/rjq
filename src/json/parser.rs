use std::io::Read;

use crate::Location;
use crate::Span;
use crate::json::{
    error::{ExpectedSyntax, FoundSyntax, JsonError, JsonErrorKind},
    hook::{JsonAtom, JsonValueBuilder, ParserHookable},
    lexer::Lexer,
    token::{JsonLexeme, JsonToken, JsonTokenKind},
    value::JsonValue,
};

pub struct JsonParser<'a> {
    lexer: Lexer<'a>,
    /// Absolute end of the last consumed byte/token (for EOF diagnostics).
    last_end: usize,
}

impl<'a> JsonParser<'a> {
    pub fn parse_slice(data: &'a [u8]) -> Result<JsonValue<'a>, JsonError> {
        Self::parse_slice_with(data, &mut JsonValueBuilder)
    }

    pub fn parse_str(source: &'a str) -> Result<JsonValue<'a>, JsonError> {
        Self::parse_slice(source.as_bytes())
    }

    pub fn parse_slice_with<H: ParserHookable<'a>>(
        data: &'a [u8],
        hook: &mut H,
    ) -> Result<H::Value, H::Error> {
        Self::from_lexer(Lexer::from_slice(data), hook)
    }

    #[cfg_attr(not(feature = "python"), allow(dead_code))]
    pub fn parse_str_with<H: ParserHookable<'a>>(
        source: &'a str,
        hook: &mut H,
    ) -> Result<H::Value, H::Error> {
        Self::parse_slice_with(source.as_bytes(), hook)
    }

    fn from_lexer<H: ParserHookable<'a>>(
        lexer: Lexer<'a>,
        hook: &mut H,
    ) -> Result<H::Value, H::Error> {
        let mut parser = Self { last_end: 0, lexer };
        parser.skip_ws();
        if parser.peek().is_none() {
            let end = parser.lexer.position();
            return Err(JsonError::at(
                JsonErrorKind::EmptyInput,
                Span { start: 0, end },
                Location { line: 1, column: 1 },
            )
            .into());
        }
        let value = parser.parse_value(hook)?;
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

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.lexer.peek_byte()
    }

    #[inline]
    fn skip_ws(&mut self) {
        self.lexer.skip_whitespace();
    }

    fn bump_byte(&mut self) {
        self.lexer.bump_n(1);
        self.last_end = self.lexer.position();
        self.lexer.discard_consumed();
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

    /// Skip whitespace, then require and consume `expected`.
    fn expect_byte(&mut self, expected: u8) -> Result<(), JsonError> {
        self.skip_ws();
        match self.peek() {
            Some(b) if b == expected => {
                self.bump_byte();
                Ok(())
            }
            Some(b) => Err(self.unexpected(ExpectedSyntax::Byte(expected), FoundSyntax::Byte(b))),
            None => Err(self.eof_error(ExpectedSyntax::Byte(expected))),
        }
    }

    fn expect_eof(&mut self) -> Result<(), JsonError> {
        self.skip_ws();
        match self.peek() {
            None => Ok(()),
            Some(b) if crate::json::lexer::is_structural(b) => {
                let start = self.lexer.position();
                Err(self.error(
                    JsonErrorKind::TrailingInput,
                    Span {
                        start,
                        end: start + 1,
                    },
                ))
            }
            Some(_) => {
                // Lex the trailing token so the diagnostic span covers it fully.
                let token = self.lexer.next_token()?;
                Err(self.error(JsonErrorKind::TrailingInput, token.span))
            }
        }
    }

    fn parse_object<H: ParserHookable<'a>>(&mut self, hook: &mut H) -> Result<H::Value, H::Error> {
        self.expect_byte(b'{')?;
        let mut object = hook.on_begin_object()?;

        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump_byte();
            return hook.on_end_object(object);
        }

        loop {
            self.skip_ws();
            let key_token = match self.peek() {
                Some(b'"') => self.next_value_token()?,
                None => {
                    return Err(self.eof_error(ExpectedSyntax::Byte(b'}')).into());
                }
                Some(b) => {
                    return Err(self
                        .unexpected(
                            ExpectedSyntax::Token(JsonTokenKind::String),
                            FoundSyntax::Byte(b),
                        )
                        .into());
                }
            };
            if key_token.kind != JsonTokenKind::String {
                return Err(self
                    .error(
                        JsonErrorKind::UnexpectedToken {
                            expected: ExpectedSyntax::Token(JsonTokenKind::String),
                            found: FoundSyntax::Token(key_token.kind),
                        },
                        key_token.span,
                    )
                    .into());
            }
            let key = match key_token.lexeme {
                JsonLexeme::String(s) => s,
                _ => {
                    return Err(self
                        .error(JsonErrorKind::InvalidEscape, key_token.span)
                        .into());
                }
            };

            self.expect_byte(b':')?;
            let value = self.parse_value(hook)?;
            hook.on_object_entry(&mut object, key, value)?;

            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump_byte();
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err(self
                            .unexpected(
                                ExpectedSyntax::Token(JsonTokenKind::String),
                                FoundSyntax::Byte(b'}'),
                            )
                            .into());
                    }
                    continue;
                }
                Some(b'}') => {
                    self.bump_byte();
                    break;
                }
                None => {
                    return Err(self.eof_error(ExpectedSyntax::Byte(b'}')).into());
                }
                Some(b) => {
                    return Err(self
                        .unexpected(ExpectedSyntax::Byte(b'}'), FoundSyntax::Byte(b))
                        .into());
                }
            }
        }

        hook.on_end_object(object)
    }

    fn parse_array<H: ParserHookable<'a>>(&mut self, hook: &mut H) -> Result<H::Value, H::Error> {
        self.expect_byte(b'[')?;
        let mut array = hook.on_begin_array()?;

        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump_byte();
            return hook.on_end_array(array);
        }

        loop {
            self.skip_ws();
            match self.peek() {
                None => {
                    return Err(self.eof_error(ExpectedSyntax::Byte(b']')).into());
                }
                Some(b) if !Self::starts_value(b) => {
                    return Err(self
                        .unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b))
                        .into());
                }
                Some(_) => {}
            }

            let value = self.parse_value(hook)?;
            hook.on_array_element(&mut array, value)?;

            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump_byte();
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err(self
                            .unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b']'))
                            .into());
                    }
                    continue;
                }
                Some(b']') => {
                    self.bump_byte();
                    break;
                }
                None => {
                    return Err(self.eof_error(ExpectedSyntax::Byte(b']')).into());
                }
                Some(b) => {
                    return Err(self
                        .unexpected(ExpectedSyntax::Byte(b']'), FoundSyntax::Byte(b))
                        .into());
                }
            }
        }

        hook.on_end_array(array)
    }

    fn parse_value<H: ParserHookable<'a>>(&mut self, hook: &mut H) -> Result<H::Value, H::Error> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(hook),
            Some(b'[') => self.parse_array(hook),
            None => Err(self.eof_error(ExpectedSyntax::Value).into()),
            Some(b) if !Self::starts_value(b) => Err(self
                .unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b))
                .into()),
            Some(_) => {
                let token = self.next_value_token()?;
                self.atom_from_token(hook, token)
            }
        }
    }

    fn next_value_token(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let token = self.lexer.next_token()?;
        if token.kind != JsonTokenKind::Stop {
            self.last_end = token.span.end;
        }
        self.lexer.discard_consumed();
        Ok(token)
    }

    fn atom_from_token<H: ParserHookable<'a>>(
        &self,
        hook: &mut H,
        token: JsonToken<'a>,
    ) -> Result<H::Value, H::Error> {
        match token.kind {
            JsonTokenKind::Null => hook.on_atom(JsonAtom::Null),
            JsonTokenKind::True => hook.on_atom(JsonAtom::Bool(true)),
            JsonTokenKind::False => hook.on_atom(JsonAtom::Bool(false)),
            JsonTokenKind::String => match token.lexeme {
                JsonLexeme::String(s) => hook.on_atom(JsonAtom::String(s)),
                _ => Err(self.error(JsonErrorKind::InvalidEscape, token.span).into()),
            },
            JsonTokenKind::Number => match token.lexeme {
                JsonLexeme::Number(n) => hook.on_atom(JsonAtom::Number(n)),
                _ => Err(self.error(JsonErrorKind::InvalidNumber, token.span).into()),
            },
            JsonTokenKind::Stop => Err(self.eof_error(ExpectedSyntax::Value).into()),
            JsonTokenKind::Whitespace => unreachable!("whitespace skipped before value token"),
        }
    }

    fn unexpected(&self, expected: ExpectedSyntax, found: FoundSyntax) -> JsonError {
        let start = self.lexer.position();
        let span = Span {
            start,
            end: start + 1,
        };
        self.error(JsonErrorKind::UnexpectedToken { expected, found }, span)
    }

    fn starts_value(b: u8) -> bool {
        matches!(
            b,
            b'{' | b'[' | b'"' | b't' | b'f' | b'n' | b'-' | b'0'..=b'9'
        )
    }
}

#[cfg(test)]
#[path = "../../tests/json/parser.rs"]
mod test;
