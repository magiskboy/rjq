use std::borrow::Cow;

use crate::json::{
    error::{JsonError, JsonErrorKind},
    escape::{UnescapeError, unescape_json_string},
    number::{self, JsonNumber},
    scan,
    source::SliceCursor,
    token::{JsonToken, JsonTokenKind},
};
use crate::{Location, Span};

// Hot-path invariants (keep these true for any future optimization, incl. P2):
//   1. Advancing the cursor is O(1): never scan per byte to track line/column.
//      Line/column is derived lazily from a byte offset only when building an
//      error (see `error_at` / `SliceCursor::location_at`).
//   2. Un-escaped string lexemes borrow the input slice (`Cow::Borrowed`) for
//      zero-copy; the cursor never discards consumed bytes.
//   3. SIMD byte scanning lives in `crate::json::scan`; `scan::scalar` is the
//      semantic spec that every SIMD backend must match.
// TODO(P2): document the fused-scalar/number-scan invariants here once P2 lands.

/// True for the escape character following a backslash in a two-char escape
/// (`\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`).
#[inline]
fn is_escape_byte(b: u8) -> bool {
    matches!(
        b,
        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
    )
}

#[derive(Debug)]
pub struct Lexer<'a> {
    source: SliceCursor<'a>,
}

impl<'a> Lexer<'a> {
    pub fn from_slice(data: &'a [u8]) -> Self {
        Self {
            source: SliceCursor::new(data),
        }
    }

    pub fn from_str(source: &'a str) -> Self {
        Self::from_slice(source.as_bytes())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn tokenize(source: &'a str) -> Result<Vec<JsonToken<'a>>, JsonError> {
        let mut lexer = Self::from_str(source);
        let mut tokens: Vec<JsonToken<'a>> = vec![];

        loop {
            let token = lexer.next_token()?;
            match token.kind {
                JsonTokenKind::Stop => break,
                JsonTokenKind::Whitespace => continue,
                _ => tokens.push(token),
            }
        }

        Ok(tokens)
    }

    pub fn position(&self) -> usize {
        self.source.position()
    }

    /// 1-based line/column for an absolute byte offset (error path only).
    pub fn location_at(&self, byte: usize) -> Location {
        self.source.location_at(byte)
    }

    pub fn discard_consumed(&mut self) {
        // Slice-backed lexer never discards: borrowed string lexemes alias the input, avoid dangle
    }

    /// Build an error, deriving line/column from the span start lazily.
    fn error_at(&self, kind: JsonErrorKind, span: Span) -> JsonError {
        let location = self.source.location_at(span.start);
        JsonError::at(kind, span, location)
    }

    pub fn next_token(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let pos = self.source.position();

        let Some(b) = self.source.peek_byte() else {
            return Ok(JsonToken::simple(
                JsonTokenKind::Stop,
                Span {
                    start: pos,
                    end: pos + 1,
                },
            ));
        };

        match b {
            b'{' => self.bump_simple(JsonTokenKind::LBrace),
            b'}' => self.bump_simple(JsonTokenKind::RBrace),
            b'[' => self.bump_simple(JsonTokenKind::LBracket),
            b']' => self.bump_simple(JsonTokenKind::RBracket),
            b':' => self.bump_simple(JsonTokenKind::Colon),
            b',' => self.bump_simple(JsonTokenKind::Comma),
            b' ' | b'\n' | b'\r' | b'\t' => self.bump_whitespace(),
            b'"' => self.parse_string(),
            b't' => self.parse_literal(JsonTokenKind::True, "true"),
            b'f' => self.parse_literal(JsonTokenKind::False, "false"),
            b'n' => self.parse_literal(JsonTokenKind::Null, "null"),
            b'0'..=b'9' | b'-' => self.parse_number(),
            _ => {
                let start = self.source.position();
                self.source.bump_n(1);
                Err(self.error_at(
                    JsonErrorKind::InvalidCharacter,
                    Span {
                        start,
                        end: self.source.position(),
                    },
                ))
            }
        }
    }

    pub fn next_significant(&mut self) -> Result<JsonToken<'a>, JsonError> {
        loop {
            let token = self.next_token()?;
            match token.kind {
                JsonTokenKind::Whitespace => {
                    self.discard_consumed();
                    continue;
                }
                _ => {
                    self.discard_consumed();
                    return Ok(token);
                }
            }
        }
    }

    fn bump_simple(&mut self, kind: JsonTokenKind) -> Result<JsonToken<'a>, JsonError> {
        let start = self.source.position();
        self.source.bump_n(1);
        Ok(JsonToken::simple(
            kind,
            Span {
                start,
                end: self.source.position(),
            },
        ))
    }

    /// Consume a run of JSON whitespace. Uses AVX2/NEON byte-scan when available;
    /// otherwise falls back to a scalar byte loop.
    fn bump_whitespace(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let start = self.source.position();

        if scan::is_available() {
            loop {
                let rem = self.source.remaining();
                if rem.is_empty() {
                    break;
                }
                match scan::find_non_whitespace(rem) {
                    Some(0) => break,
                    Some(idx) => {
                        self.source.bump_n(idx);
                        break;
                    }
                    None => {
                        let n = rem.len();
                        self.source.bump_n(n);
                    }
                }
            }
        } else {
            while let Some(b) = self.source.peek_byte() {
                if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                    self.source.bump_n(1);
                } else {
                    break;
                }
            }
        }

        Ok(JsonToken::simple(
            JsonTokenKind::Whitespace,
            Span {
                start,
                end: self.source.position(),
            },
        ))
    }

    fn parse_literal(
        &mut self,
        token_type: JsonTokenKind,
        expected: &'static str,
    ) -> Result<JsonToken<'a>, JsonError> {
        let start = self.source.position();
        let expected_bytes = expected.as_bytes();
        let rem = self.source.remaining();

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

        self.source.bump_n(expected_bytes.len());

        // Literals must be complete tokens (not prefixes of identifiers).
        if let Some(next) = self.source.peek_byte() {
            if next.is_ascii_alphanumeric() || next == b'_' {
                return Err(self.error_at(
                    JsonErrorKind::InvalidLiteral { expected },
                    Span {
                        start,
                        end: self.source.position(),
                    },
                ));
            }
        }

        Ok(JsonToken::simple(
            token_type,
            Span {
                start,
                end: self.source.position(),
            },
        ))
    }

    fn parse_number(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let start = self.source.position();

        // The pure scanner (in `number`) classifies the token and reports how
        // many bytes it consumed; the lexer only maps that onto spans/errors.
        match number::scan_number(self.source.remaining()) {
            Ok((len, kind)) => {
                self.source.bump_n(len);
                let span = Span {
                    start,
                    end: self.source.position(),
                };
                let raw = std::str::from_utf8(self.source.slice_abs(span.start, span.end))
                    .map_err(|_| self.error_at(JsonErrorKind::InvalidNumber, span))?;
                Ok(JsonToken::number(
                    span,
                    JsonNumber::new(Cow::Borrowed(raw), kind),
                ))
            }
            Err(consumed) => {
                self.source.bump_n(consumed);
                let span = Span {
                    start,
                    end: self.source.position(),
                };
                Err(self.error_at(JsonErrorKind::InvalidNumber, span))
            }
        }
    }

    fn peek_bytes(&self, len: usize) -> Option<&'a [u8]> {
        self.source.peek_bytes(len)
    }

    fn parse_string(&mut self) -> Result<JsonToken<'a>, JsonError> {
        let start = self.source.position();
        self.source.bump_n(1); // opening '"'
        let mut has_escape = false;

        loop {
            // Fast path: skip a run of bytes that cannot end/escape the string.
            // AVX2 (x86_64) / NEON (Apple M); otherwise scalar byte loop below.
            if scan::is_available() {
                let rem = self.source.remaining();
                if rem.is_empty() {
                    return Err(self.error_at(
                        JsonErrorKind::UnterminatedString,
                        Span {
                            start,
                            end: self.source.position(),
                        },
                    ));
                }
                match scan::find_string_special(rem) {
                    Some(0) => {}
                    Some(idx) => {
                        self.source.bump_n(idx);
                    }
                    None => {
                        let n = rem.len();
                        self.source.bump_n(n);
                        continue;
                    }
                }
            }

            let Some(b) = self.source.peek_byte() else {
                return Err(self.error_at(
                    JsonErrorKind::UnterminatedString,
                    Span {
                        start,
                        end: self.source.position(),
                    },
                ));
            };

            if b == b'\\' {
                has_escape = true;
                let esc_pos = self.source.position();
                // Two-char escapes: `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`.
                if let Some(bytes) = self.peek_bytes(2) {
                    if is_escape_byte(bytes[1]) {
                        self.source.bump_n(2);
                        continue;
                    }
                }
                // Unicode escape: `\uXXXX` with four hex digits.
                if let Some(bytes) = self.peek_bytes(6) {
                    if bytes[1] == b'u' && bytes[2..6].iter().all(u8::is_ascii_hexdigit) {
                        self.source.bump_n(6);
                        continue;
                    }
                }

                let starts_u = matches!(self.peek_bytes(2), Some(b) if b[1] == b'u');
                if starts_u {
                    let end = (esc_pos + 6).min(self.source.buffered_end().max(esc_pos + 2));
                    return Err(self.error_at(
                        JsonErrorKind::InvalidUnicodeEscape,
                        Span { start, end },
                    ));
                }

                let end = (esc_pos + 2).min(self.source.buffered_end().max(esc_pos + 1));
                return Err(self.error_at(JsonErrorKind::InvalidEscape, Span { start, end }));
            } else if b == b'"' {
                self.source.bump_n(1);
                let end = self.source.position();
                let span = Span { start, end };
                let raw = std::str::from_utf8(self.source.slice_abs(start + 1, end - 1))
                    .map_err(|_| self.error_at(JsonErrorKind::InvalidEscape, span))?;
                let value = if has_escape {
                    let owned = unescape_json_string(raw)
                        .map_err(|err| self.error_at(Self::unescape_kind(err), span))?;
                    Cow::Owned(owned)
                } else {
                    // Zero-copy: alias the input bytes (no heap String).
                    Cow::Borrowed(raw)
                };
                return Ok(JsonToken::string(span, value));
            } else if b < 0x20 {
                // Unescaped control byte is invalid inside a JSON string.
                self.source.bump_n(1);
                return Err(self.error_at(
                    JsonErrorKind::UnescapedControl,
                    Span {
                        start,
                        end: self.source.position(),
                    },
                ));
            } else {
                // Ordinary content byte (ASCII or a UTF-8 lead/continuation byte).
                self.source.bump_n(1);
            }
        }
    }

    fn unescape_kind(err: UnescapeError) -> JsonErrorKind {
        match err {
            UnescapeError::InvalidEscape(_) => JsonErrorKind::InvalidEscape,
            UnescapeError::InvalidUnicodeEscape => JsonErrorKind::InvalidUnicodeEscape,
            UnescapeError::InvalidSurrogatePair => JsonErrorKind::InvalidSurrogatePair,
            UnescapeError::UnescapedControlCharacter => JsonErrorKind::UnescapedControl,
        }
    }
}


#[cfg(test)]
#[path = "../../tests/json/lexer.rs"]
mod test;
