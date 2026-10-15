//! Index-driven recursive-descent parser (same hooks as [`crate::json::parser::JsonParser`]).

use std::io::Read;

use crate::json::{
    builder::eager::JsonValueBuilder,
    error::{ExpectedSyntax, FoundSyntax, JsonError, JsonErrorKind},
    hook::{JsonAtom, ParseDecision, ParserHookable},
    lexer,
    token::{JsonLexeme, JsonToken, JsonTokenKind},
    value::JsonValue,
};
use crate::json::simd::{
    blend, index::StructuralIndex, lexer::SIMDLexer, StructuralBitmasks,
};
use crate::{Location, Span};

#[cfg(target_arch = "x86_64")]
use super::avx2 as backend;
#[cfg(target_arch = "aarch64")]
use super::neon as backend;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
use crate::json::simd::scalar;

pub struct SIMDParser<'a> {
    lexer: SIMDLexer<'a>,
}

impl<'a> SIMDParser<'a> {
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
        let index = StructuralIndex::build(data);
        let lexer = SIMDLexer::new(data, index);
        let mut parser = Self { lexer };
        if parser.lexer.is_empty() {
            return Err(JsonError::at(
                JsonErrorKind::EmptyInput,
                Span { start: 0, end: 0 },
                Location { line: 1, column: 1 },
            )
            .into());
        }
        let value = parser.parse_value(hook)?;
        if !hook.accept_trailing_junk() {
            parser.expect_eof()?;
        }
        Ok(value)
    }

    #[cfg_attr(not(feature = "python"), allow(dead_code))]
    pub fn parse_str_with<H: ParserHookable<'a>>(
        source: &'a str,
        hook: &mut H,
    ) -> Result<H::Value, H::Error> {
        Self::parse_slice_with(source.as_bytes(), hook)
    }
}

impl SIMDParser<'_> {
    pub fn parse_reader(mut reader: impl Read) -> Result<JsonValue<'static>, JsonError> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).map_err(JsonError::io)?;
        SIMDParser::parse_slice(&buf).map(JsonValue::into_owned)
    }

    #[inline]
    pub fn simd_available() -> bool {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            backend::is_available()
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            false
        }
    }

    pub fn classify(input: &[u8]) -> StructuralBitmasks {
        classify(input)
    }

    pub fn blend(input: &[u8]) -> blend::BlendedBitmasks {
        classify(input).blend()
    }

    pub fn structural_indexes(input: &[u8]) -> StructuralIndex {
        StructuralIndex::build(input)
    }
}

fn classify(input: &[u8]) -> StructuralBitmasks {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        backend::compute_bitmasks(input)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        scalar::compute_bitmasks(input)
    }
}

impl<'a> SIMDParser<'a> {
    fn error(&self, kind: JsonErrorKind, span: Span) -> JsonError {
        self.lexer.error_at(kind, span)
    }

    #[inline]
    fn peek_byte(&self) -> Option<u8> {
        self.lexer.peek_byte()
    }

    #[inline]
    fn peek_index(&self) -> Option<usize> {
        self.lexer.peek_index()
    }

    fn bump_structural(&mut self) {
        self.lexer.bump_structural();
    }

    fn eof_error(&self, expected: ExpectedSyntax) -> JsonError {
        self.error(
            JsonErrorKind::UnexpectedEof { expected },
            Span {
                start: self.lexer.last_end(),
                end: self.lexer.last_end(),
            },
        )
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), JsonError> {
        match self.peek_byte() {
            Some(b) if b == expected => {
                self.bump_structural();
                Ok(())
            }
            Some(b) => Err(self.unexpected(ExpectedSyntax::Byte(expected), FoundSyntax::Byte(b))),
            None => Err(self.eof_error(ExpectedSyntax::Byte(expected))),
        }
    }

    fn expect_eof(&mut self) -> Result<(), JsonError> {
        match self.peek_byte() {
            None => Ok(()),
            Some(b) if lexer::is_structural(b) => {
                let start = self.peek_index().unwrap_or(self.lexer.last_end());
                Err(self.error(
                    JsonErrorKind::TrailingInput,
                    Span {
                        start,
                        end: start + 1,
                    },
                ))
            }
            Some(_) => {
                let token = self.lexer.next_value_token()?;
                Err(self.error(JsonErrorKind::TrailingInput, token.span))
            }
        }
    }

    fn parse_object<H: ParserHookable<'a>>(&mut self, hook: &mut H) -> Result<H::Value, H::Error> {
        self.expect_byte(b'{')?;
        let mut object = hook.on_begin_object()?;

        if self.peek_byte() == Some(b'}') {
            self.bump_structural();
            return hook.on_end_object(object);
        }

        loop {
            let key_token = match self.peek_byte() {
                Some(b'"') => self.lexer.next_value_token()?,
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

            match hook.want_object_key(key.as_ref())? {
                ParseDecision::Keep => {
                    let value = self.parse_value(hook)?;
                    hook.on_object_entry(&mut object, key, value)?;
                }
                ParseDecision::Skip => {
                    self.skip_value()?;
                    hook.on_object_key_skipped(key.as_ref())?;
                }
            }

            match self.peek_byte() {
                Some(b',') => {
                    self.bump_structural();
                    if self.peek_byte() == Some(b'}') {
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
                    self.bump_structural();
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

        if self.peek_byte() == Some(b']') {
            self.bump_structural();
            return hook.on_end_array(array);
        }

        let mut index = 0usize;
        loop {
            match self.peek_byte() {
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

            match hook.want_array_element(index)? {
                ParseDecision::Keep => {
                    let value = self.parse_value(hook)?;
                    hook.on_array_element(&mut array, value)?;
                }
                ParseDecision::Skip => {
                    self.skip_value()?;
                    hook.on_array_element_skipped(&mut array, index)?;
                }
            }
            index += 1;

            match self.peek_byte() {
                Some(b',') => {
                    self.bump_structural();
                    if self.peek_byte() == Some(b']') {
                        return Err(self
                            .unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b']'))
                            .into());
                    }
                    continue;
                }
                Some(b']') => {
                    self.bump_structural();
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
        match self.peek_byte() {
            Some(b'{') => self.parse_object(hook),
            Some(b'[') => self.parse_array(hook),
            None => Err(self.eof_error(ExpectedSyntax::Value).into()),
            Some(b) if !Self::starts_value(b) => Err(self
                .unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b))
                .into()),
            Some(_) => {
                let token = self.lexer.next_value_token()?;
                self.atom_from_token(hook, token)
            }
        }
    }

    pub(crate) fn skip_value(&mut self) -> Result<(), JsonError> {
        match self.peek_byte() {
            Some(b'{') => self.skip_object(),
            Some(b'[') => self.skip_array(),
            None => Err(self.eof_error(ExpectedSyntax::Value)),
            Some(b) if !Self::starts_value(b) => {
                Err(self.unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b)))
            }
            Some(_) => {
                let _ = self.lexer.next_value_token()?;
                Ok(())
            }
        }
    }

    fn skip_object(&mut self) -> Result<(), JsonError> {
        self.expect_byte(b'{')?;
        if self.peek_byte() == Some(b'}') {
            self.bump_structural();
            return Ok(());
        }
        loop {
            match self.peek_byte() {
                Some(b'"') => {
                    let token = self.lexer.next_value_token()?;
                    if token.kind != JsonTokenKind::String {
                        return Err(self.error(
                            JsonErrorKind::UnexpectedToken {
                                expected: ExpectedSyntax::Token(JsonTokenKind::String),
                                found: FoundSyntax::Token(token.kind),
                            },
                            token.span,
                        ));
                    }
                }
                None => return Err(self.eof_error(ExpectedSyntax::Byte(b'}'))),
                Some(b) => {
                    return Err(self.unexpected(
                        ExpectedSyntax::Token(JsonTokenKind::String),
                        FoundSyntax::Byte(b),
                    ));
                }
            }
            self.expect_byte(b':')?;
            self.skip_value()?;
            match self.peek_byte() {
                Some(b',') => {
                    self.bump_structural();
                    if self.peek_byte() == Some(b'}') {
                        return Err(self.unexpected(
                            ExpectedSyntax::Token(JsonTokenKind::String),
                            FoundSyntax::Byte(b'}'),
                        ));
                    }
                    continue;
                }
                Some(b'}') => {
                    self.bump_structural();
                    return Ok(());
                }
                None => return Err(self.eof_error(ExpectedSyntax::Byte(b'}'))),
                Some(b) => {
                    return Err(self.unexpected(ExpectedSyntax::Byte(b'}'), FoundSyntax::Byte(b)));
                }
            }
        }
    }

    fn skip_array(&mut self) -> Result<(), JsonError> {
        self.expect_byte(b'[')?;
        if self.peek_byte() == Some(b']') {
            self.bump_structural();
            return Ok(());
        }
        loop {
            self.skip_value()?;
            match self.peek_byte() {
                Some(b',') => {
                    self.bump_structural();
                    if self.peek_byte() == Some(b']') {
                        return Err(
                            self.unexpected(ExpectedSyntax::Value, FoundSyntax::Byte(b']'))
                        );
                    }
                    continue;
                }
                Some(b']') => {
                    self.bump_structural();
                    return Ok(());
                }
                None => return Err(self.eof_error(ExpectedSyntax::Byte(b']'))),
                Some(b) => {
                    return Err(self.unexpected(ExpectedSyntax::Byte(b']'), FoundSyntax::Byte(b)));
                }
            }
        }
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
            JsonTokenKind::Whitespace => unreachable!(),
        }
    }

    fn unexpected(&self, expected: ExpectedSyntax, found: FoundSyntax) -> JsonError {
        let start = self.peek_index().unwrap_or(self.lexer.last_end());
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
mod tests {
    use super::*;
    use crate::json::parser::JsonParser;
    use std::borrow::Cow;

    fn samples() -> Vec<&'static str> {
        vec![
            "null",
            "true",
            "false",
            "0",
            "-12.5e+3",
            r#""hello""#,
            r#""hel\"lo""#,
            "[]",
            "[1,2,3]",
            "[ true , false , null ]",
            "{}",
            r#"{"a":1}"#,
            r#"{ "name": "a\"b", "nums": [1, 2] }"#,
            r#"{"items":[{"id":1},{"id":2}],"ok":true}"#,
            r#""🙂""#,
            "  { \"k\" : [ 1 , { \"x\" : null } ] }  ",
            r#""\n\t\u0041""#,
            r#""\\\"""#,
        ]
    }

    #[test]
    fn matches_json_parser_on_ok_samples() {
        for s in samples() {
            let a = JsonParser::parse_str(s).unwrap();
            let b = SIMDParser::parse_str(s).unwrap();
            assert_eq!(a, b, "mismatch on {s:?}");
        }
    }

    #[test]
    fn empty_and_whitespace_are_empty_input() {
        for s in ["", "   ", "\n\t"] {
            let a = JsonParser::parse_str(s).unwrap_err();
            let b = SIMDParser::parse_str(s).unwrap_err();
            assert_eq!(a.kind, JsonErrorKind::EmptyInput);
            assert_eq!(b.kind, JsonErrorKind::EmptyInput);
        }
    }

    #[test]
    fn string_borrow_vs_owned_matches() {
        let borrowed = r#""hello""#;
        match SIMDParser::parse_str(borrowed).unwrap() {
            JsonValue::String(Cow::Borrowed(s)) => assert_eq!(s, "hello"),
            other => panic!("expected borrowed, got {other:?}"),
        }
        let owned = r#""hel\"lo""#;
        match SIMDParser::parse_str(owned).unwrap() {
            JsonValue::String(Cow::Owned(s)) => assert_eq!(s, "hel\"lo"),
            other => panic!("expected owned, got {other:?}"),
        }
    }

    #[test]
    fn trailing_input_rejected() {
        let err = SIMDParser::parse_str("true false").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::TrailingInput);
    }

    #[test]
    fn trailing_comma_rejected() {
        assert!(SIMDParser::parse_str("[1,]").is_err());
        assert!(SIMDParser::parse_str(r#"{"a":1,}"#).is_err());
    }

    #[test]
    fn unterminated_string_is_error() {
        let err = SIMDParser::parse_str(r#""abc"#).unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::UnterminatedString);
    }
}
