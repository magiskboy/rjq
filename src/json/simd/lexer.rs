//! Index-driven lexer over [`StructuralIndex`].

use std::borrow::Cow;

use crate::json::{
    error::{ExpectedSyntax, JsonError, JsonErrorKind},
    escape::{UnescapeError, unescape_json_string},
    number::{self, JsonNumber},
    scan,
    token::{JsonToken, JsonTokenKind},
};
use crate::json::simd::index::StructuralIndex;
use crate::{Location, Span};

pub struct SIMDLexer<'a> {
    input: &'a [u8],
    index: StructuralIndex,
    idx: usize,
    last_end: usize,
}

impl<'a> SIMDLexer<'a> {
    pub fn new(input: &'a [u8], index: StructuralIndex) -> Self {
        Self {
            input,
            index,
            idx: 0,
            last_end: 0,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    #[inline]
    pub fn last_end(&self) -> usize {
        self.last_end
    }

    #[inline]
    pub fn location_at(&self, byte: usize) -> Location {
        crate::json::source::SliceCursor::new(self.input).location_at(byte)
    }

    #[inline]
    pub fn peek_byte(&self) -> Option<u8> {
        self.peek_index()
            .and_then(|i| self.input.get(i).copied())
    }

    #[inline]
    pub fn peek_index(&self) -> Option<usize> {
        self.index.get(self.idx)
    }

    pub fn bump_structural(&mut self) {
        if let Some(i) = self.peek_index() {
            self.last_end = i + 1;
            self.idx += 1;
        }
    }

    pub fn error_at(&self, kind: JsonErrorKind, span: Span) -> JsonError {
        JsonError::at(kind, span, self.location_at(span.start))
    }

    pub fn next_value_token(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let start = self.peek_index().ok_or_else(|| {
            self.error_at(
                JsonErrorKind::UnexpectedEof {
                    expected: ExpectedSyntax::Value,
                },
                Span {
                    start: self.last_end,
                    end: self.last_end,
                },
            )
        })?;

        let token = match self.input.get(start).copied() {
            Some(b'"') => self.lex_string(start)?,
            Some(b't') => self.lex_literal(start, JsonTokenKind::True, "true")?,
            Some(b'f') => self.lex_literal(start, JsonTokenKind::False, "false")?,
            Some(b'n') => self.lex_literal(start, JsonTokenKind::Null, "null")?,
            Some(b'0'..=b'9' | b'-') => self.lex_number(start)?,
            Some(_) => {
                return Err(self.error_at(
                    JsonErrorKind::InvalidCharacter,
                    Span {
                        start,
                        end: start + 1,
                    },
                ));
            }
            None => {
                return Err(self.error_at(
                    JsonErrorKind::UnexpectedEof {
                        expected: ExpectedSyntax::Value,
                    },
                    Span {
                        start: self.last_end,
                        end: self.last_end,
                    },
                ));
            }
        };

        self.last_end = token.span.end;
        self.idx += 1;
        Ok(token)
    }

    fn lex_string(&self, start: usize) -> Result<JsonToken<'a>, JsonError> {
        debug_assert_eq!(self.input.get(start), Some(&b'"'));
        let mut pos = start + 1;
        let mut has_escape = false;

        loop {
            let rem = &self.input[pos..];
            if rem.is_empty() {
                return Err(self.error_at(
                    JsonErrorKind::UnterminatedString,
                    Span {
                        start,
                        end: self.input.len(),
                    },
                ));
            }

            if scan::is_available() {
                match scan::find_string_special(rem) {
                    Some(0) => {}
                    Some(idx) => {
                        pos += idx;
                    }
                    None => {
                        return Err(self.error_at(
                            JsonErrorKind::UnterminatedString,
                            Span {
                                start,
                                end: self.input.len(),
                            },
                        ));
                    }
                }
            }

            let Some(&b) = self.input.get(pos) else {
                return Err(self.error_at(
                    JsonErrorKind::UnterminatedString,
                    Span {
                        start,
                        end: self.input.len(),
                    },
                ));
            };

            if b == b'\\' {
                has_escape = true;
                let esc_pos = pos;
                if pos + 1 < self.input.len() && is_escape_byte(self.input[pos + 1]) {
                    pos += 2;
                    continue;
                }
                if pos + 5 < self.input.len()
                    && self.input[pos + 1] == b'u'
                    && self.input[pos + 2..pos + 6]
                        .iter()
                        .all(u8::is_ascii_hexdigit)
                {
                    pos += 6;
                    continue;
                }
                let starts_u = self.input.get(pos + 1) == Some(&b'u');
                if starts_u {
                    let end = (esc_pos + 6).min(self.input.len().max(esc_pos + 2));
                    return Err(self.error_at(
                        JsonErrorKind::InvalidUnicodeEscape,
                        Span { start, end },
                    ));
                }
                let end = (esc_pos + 2).min(self.input.len().max(esc_pos + 1));
                return Err(self.error_at(JsonErrorKind::InvalidEscape, Span { start, end }));
            } else if b == b'"' {
                let end = pos + 1;
                let span = Span { start, end };
                let raw = std::str::from_utf8(&self.input[start + 1..pos])
                    .map_err(|_| self.error_at(JsonErrorKind::InvalidEscape, span))?;
                let value = if has_escape {
                    let owned = unescape_json_string(raw)
                        .map_err(|err| self.error_at(unescape_kind(err), span))?;
                    Cow::Owned(owned)
                } else {
                    Cow::Borrowed(raw)
                };
                return Ok(JsonToken::string(span, value));
            } else if b < 0x20 {
                return Err(self.error_at(
                    JsonErrorKind::UnescapedControl,
                    Span {
                        start: pos,
                        end: pos + 1,
                    },
                ));
            } else {
                pos += 1;
            }
        }
    }

    fn lex_number(&self, start: usize) -> Result<JsonToken<'a>, JsonError> {
        match number::scan_number(&self.input[start..]) {
            Ok((len, kind)) => {
                let span = Span {
                    start,
                    end: start + len,
                };
                let raw = std::str::from_utf8(&self.input[start..start + len])
                    .map_err(|_| self.error_at(JsonErrorKind::InvalidNumber, span))?;
                Ok(JsonToken::number(
                    span,
                    JsonNumber::new(Cow::Borrowed(raw), kind),
                ))
            }
            Err(consumed) => Err(self.error_at(
                JsonErrorKind::InvalidNumber,
                Span {
                    start,
                    end: start + consumed,
                },
            )),
        }
    }

    fn lex_literal(
        &self,
        start: usize,
        kind: JsonTokenKind,
        expected: &'static str,
    ) -> Result<JsonToken<'a>, JsonError> {
        let expected_bytes = expected.as_bytes();
        let rem = &self.input[start..];

        if !rem.starts_with(expected_bytes) {
            let common = rem
                .iter()
                .zip(expected_bytes.iter())
                .take_while(|(a, b)| *a == *b)
                .count();
            return Err(self.error_at(
                JsonErrorKind::InvalidLiteral { expected },
                Span {
                    start,
                    end: start + common,
                },
            ));
        }

        let end = start + expected_bytes.len();
        if let Some(&next) = self.input.get(end) {
            if next.is_ascii_alphanumeric() || next == b'_' {
                return Err(self.error_at(
                    JsonErrorKind::InvalidLiteral { expected },
                    Span { start, end },
                ));
            }
        }

        Ok(JsonToken::simple(kind, Span { start, end }))
    }
}

#[inline]
fn is_escape_byte(b: u8) -> bool {
    matches!(b, b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't')
}

fn unescape_kind(err: UnescapeError) -> JsonErrorKind {
    match err {
        UnescapeError::InvalidEscape(_) => JsonErrorKind::InvalidEscape,
        UnescapeError::InvalidUnicodeEscape => JsonErrorKind::InvalidUnicodeEscape,
        UnescapeError::InvalidSurrogatePair => JsonErrorKind::InvalidSurrogatePair,
        UnescapeError::UnescapedControlCharacter => JsonErrorKind::UnescapedControl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn lexer_on(input: &[u8]) -> SIMDLexer<'_> {
        SIMDLexer::new(input, StructuralIndex::build(input))
    }

    #[test]
    fn string_borrow_and_escape() {
        let mut lex = lexer_on(br#""hello""#);
        let tok = lex.next_value_token().unwrap();
        match tok.lexeme {
            crate::json::token::JsonLexeme::String(Cow::Borrowed("hello")) => {}
            other => panic!("{other:?}"),
        }

        let mut lex = lexer_on(br#""a\"b""#);
        let tok = lex.next_value_token().unwrap();
        match tok.lexeme {
            crate::json::token::JsonLexeme::String(Cow::Owned(s)) => assert_eq!(s, "a\"b"),
            other => panic!("{other:?}"),
        }
    }
}
