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
mod test {
    use crate::Span;
    use crate::json::{
        error::{JsonError, JsonErrorKind},
        lexer::Lexer,
        token::{JsonLexeme, JsonToken, JsonTokenKind},
    };

    fn run(source: &str) -> Result<Vec<JsonToken<'_>>, JsonError> {
        let tokens = Lexer::tokenize(source)?;
        for token in &tokens {
            println!("{}", token.display());
        }
        Ok(tokens)
    }

    fn tok(kind: JsonTokenKind, start: usize, end: usize) -> JsonToken<'static> {
        JsonToken::simple(kind, Span { start, end })
    }

    fn kinds(tokens: &[JsonToken]) -> Vec<JsonTokenKind> {
        tokens.iter().map(|t| t.kind.clone()).collect()
    }

    fn assert_kinds_spans(source: &str, expected: &[(JsonTokenKind, usize, usize)]) {
        let tokens = run(source).expect("should lex");
        assert_eq!(tokens.len(), expected.len(), "source {:?}", source);
        for (token, (kind, start, end)) in tokens.iter().zip(expected.iter()) {
            assert_eq!(&token.kind, kind, "kind {:?}", source);
            assert_eq!(
                token.span,
                Span {
                    start: *start,
                    end: *end
                },
                "span {:?}",
                source
            );
        }
    }

    #[test]
    fn structural_characters() {
        let input: [&str; 6] = ["{", "}", "[", "]", ":", ","];
        let expected: [JsonTokenKind; 6] = [
            JsonTokenKind::LBrace,
            JsonTokenKind::RBrace,
            JsonTokenKind::LBracket,
            JsonTokenKind::RBracket,
            JsonTokenKind::Colon,
            JsonTokenKind::Comma,
        ];
        for (s, kind) in input.iter().zip(expected.iter()) {
            assert_eq!(
                run(s),
                Ok(vec![tok(kind.clone(), 0, s.len())]),
                "structural {:?}",
                s
            );
        }
    }

    #[test]
    fn whitespace_is_insignificant() {
        let input: [&str; 5] = ["", " ", "\t", "\n", "\r"];
        for s in input {
            assert_eq!(run(s), Ok(vec![]), "whitespace {:?}", s);
        }

        let padded: [&str; 4] = ["  true  ", "\tfalse\t", "\nnull\n", "\r1\r"];
        let expected_kinds: [JsonTokenKind; 4] = [
            JsonTokenKind::True,
            JsonTokenKind::False,
            JsonTokenKind::Null,
            JsonTokenKind::Number,
        ];
        for (s, kind) in padded.iter().zip(expected_kinds.iter()) {
            let tokens = run(s).expect("should lex");
            assert_eq!(kinds(&tokens), vec![kind.clone()], "padded {:?}", s);
        }
    }

    #[test]
    fn literals() {
        let input: [&str; 3] = ["true", "false", "null"];
        let expected: [JsonToken; 3] = [
            tok(JsonTokenKind::True, 0, 4),
            tok(JsonTokenKind::False, 0, 5),
            tok(JsonTokenKind::Null, 0, 4),
        ];
        for (s, expected) in input.iter().zip(expected.iter()) {
            assert_eq!(&run(s), &Ok(vec![expected.clone()]));
        }
    }

    #[test]
    fn incomplete_or_invalid_literals_are_errors() {
        let input: [&str; 9] = [
            "tru", "tr", "t", "fals", "fal", "nul", "nu", "TRUE", "False",
        ];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn strings_valid() {
        let cases: [(&str, usize, usize); 6] = [
            ("\"\"", 0, 2),
            ("\"hello\"", 0, 7),
            ("\" \"", 0, 3),
            ("\"\\\"\"", 0, 4),
            ("\"\\\\\"", 0, 4),
            ("\"\\n\\t\\r\\b\\f\\/\"", 0, 14),
        ];
        for (s, start, end) in cases {
            assert_kinds_spans(s, &[(JsonTokenKind::String, start, end)]);
        }
    }

    #[test]
    fn strings_unicode_escape() {
        let input = "\"\\u0041\"";
        assert_kinds_spans(input, &[(JsonTokenKind::String, 0, input.len())]);
        let tokens = run(input).unwrap();
        assert_eq!(tokens[0].lexeme, JsonLexeme::String("A".into()));

        let input = "\"\\uD83D\\uDE00\"";
        assert_kinds_spans(input, &[(JsonTokenKind::String, 0, input.len())]);
        let tokens = run(input).unwrap();
        assert_eq!(tokens[0].lexeme, JsonLexeme::String("😀".into()));
    }

    #[test]
    fn strings_raw_unicode() {
        let cases: [&str; 5] = ["\"é\"", "\"€\"", "\"🙂\"", "\"café\"", "\"é\\n🙂\""];
        for s in cases {
            assert_kinds_spans(s, &[(JsonTokenKind::String, 0, s.len())]);
        }
    }

    #[test]
    fn unicode_string_keeps_following_byte_spans() {
        assert_kinds_spans(
            "\"é\",1",
            &[
                (JsonTokenKind::String, 0, 4),
                (JsonTokenKind::Comma, 4, 5),
                (JsonTokenKind::Number, 5, 6),
            ],
        );

        assert_kinds_spans(
            "{\"a\":\"é\"}",
            &[
                (JsonTokenKind::LBrace, 0, 1),
                (JsonTokenKind::String, 1, 4),
                (JsonTokenKind::Colon, 4, 5),
                (JsonTokenKind::String, 5, 9),
                (JsonTokenKind::RBrace, 9, 10),
            ],
        );

        assert_kinds_spans(
            "[\"🙂\"]",
            &[
                (JsonTokenKind::LBracket, 0, 1),
                (JsonTokenKind::String, 1, 7),
                (JsonTokenKind::RBracket, 7, 8),
            ],
        );
    }

    #[test]
    fn raw_unicode_outside_string_is_error() {
        let input: [&str; 4] = ["é", "€", "🙂", "\u{00A0}1"];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn unescaped_controls_are_errors() {
        for ch in '\u{0000}'..='\u{001F}' {
            let input = format!("\"{ch}\"");
            assert!(
                run(&input).is_err(),
                "expected lexical error for U+{:04X}",
                u32::from(ch)
            );
        }

        let embedded: [&str; 3] = ["\"a\rb\"", "\"é\u{0001}\"", "\"\u{001F}x\""];
        for s in embedded {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn escaped_controls_are_strings() {
        let cases: [&str; 6] = [
            "\"\\u0000\"",
            "\"\\u0001\"",
            "\"\\u001F\"",
            "\"\\n\"",
            "\"\\r\"",
            "\"\\t\"",
        ];
        for s in cases {
            assert_kinds_spans(s, &[(JsonTokenKind::String, 0, s.len())]);
        }
    }

    #[test]
    fn strings_invalid_are_errors() {
        let input: [&str; 8] = [
            "\"",
            "\"abc",
            "\"\\x\"",
            "\"\\u\"",
            "\"\\u12\"",
            "\"\\uZZZZ\"",
            "\"\n\"",
            "\"\t\"",
        ];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn numbers_valid() {
        let input: [&str; 15] = [
            "0", "1", "10", "100", "123", "-0", "-1", "-10", "0.1", "3.14", "10.0", "1e2", "1E2",
            "1e+2", "1e-2",
        ];
        for s in input {
            assert_kinds_spans(s, &[(JsonTokenKind::Number, 0, s.len())]);
        }
    }

    #[test]
    fn numbers_with_fraction_and_exponent() {
        let input: [&str; 4] = ["1.2e3", "1.2E+3", "-1.2e-3", "0.0e0"];
        for s in input {
            assert_kinds_spans(s, &[(JsonTokenKind::Number, 0, s.len())]);
        }
    }

    #[test]
    fn numbers_invalid_are_errors() {
        let input: [&str; 14] = [
            "+", "+1", "01", "-01", "1.", ".1", "-.1", "1e", "1e+", "1e-", "--1", "1.2.3", "1ee2",
            "0x1",
        ];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn number_then_structural() {
        let cases: [(&str, &[JsonTokenKind]); 3] = [
            ("10,", &[JsonTokenKind::Number, JsonTokenKind::Comma]),
            ("10]", &[JsonTokenKind::Number, JsonTokenKind::RBracket]),
            ("10}", &[JsonTokenKind::Number, JsonTokenKind::RBrace]),
        ];
        for (s, expected) in cases {
            let tokens = run(s).expect("should lex");
            assert_eq!(kinds(&tokens), expected, "source {:?}", s);
            assert_eq!(tokens[0].span, Span { start: 0, end: 2 });
        }
    }

    #[test]
    fn array_tokens() {
        let input = "[1,2,true,false,null,\"x\"]";
        let tokens = run(input).expect("should lex");
        assert_eq!(
            kinds(&tokens),
            vec![
                JsonTokenKind::LBracket,
                JsonTokenKind::Number,
                JsonTokenKind::Comma,
                JsonTokenKind::Number,
                JsonTokenKind::Comma,
                JsonTokenKind::True,
                JsonTokenKind::Comma,
                JsonTokenKind::False,
                JsonTokenKind::Comma,
                JsonTokenKind::Null,
                JsonTokenKind::Comma,
                JsonTokenKind::String,
                JsonTokenKind::RBracket,
            ]
        );
    }

    #[test]
    fn object_tokens() {
        let input = "{\"a\":1,\"b\":true}";
        let tokens = run(input).expect("should lex");
        assert_eq!(
            kinds(&tokens),
            vec![
                JsonTokenKind::LBrace,
                JsonTokenKind::String,
                JsonTokenKind::Colon,
                JsonTokenKind::Number,
                JsonTokenKind::Comma,
                JsonTokenKind::String,
                JsonTokenKind::Colon,
                JsonTokenKind::True,
                JsonTokenKind::RBrace,
            ]
        );
    }

    #[test]
    fn nested_structure_tokens() {
        let input = "{\"arr\":[1,{\"k\":null}]}";
        let tokens = run(input).expect("should lex");
        assert_eq!(
            kinds(&tokens),
            vec![
                JsonTokenKind::LBrace,
                JsonTokenKind::String,
                JsonTokenKind::Colon,
                JsonTokenKind::LBracket,
                JsonTokenKind::Number,
                JsonTokenKind::Comma,
                JsonTokenKind::LBrace,
                JsonTokenKind::String,
                JsonTokenKind::Colon,
                JsonTokenKind::Null,
                JsonTokenKind::RBrace,
                JsonTokenKind::RBracket,
                JsonTokenKind::RBrace,
            ]
        );
    }

    #[test]
    fn unknown_characters_are_errors() {
        let input: [&str; 6] = ["@", "#", "'abc'", "NaN", "Infinity", "-Infinity"];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn literals_must_be_complete_tokens() {
        let input: [&str; 3] = ["truex", "falsey", "nullable"];
        for s in input {
            assert!(run(s).is_err(), "expected lexical error for {:?}", s);
        }
    }

    #[test]
    fn lexical_errors_carry_kind_and_span() {
        let err = run("@").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidCharacter);
        assert_eq!(err.span, Some(Span { start: 0, end: 1 }));
        assert_eq!(
            err.to_string(),
            "lexical error at 1:1 (0..1): invalid character"
        );

        let err = run("tru").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidLiteral { expected: "true" });
        assert_eq!(err.span, Some(Span { start: 0, end: 3 }));
        assert_eq!(
            err.to_string(),
            "lexical error at 1:1 (0..3): invalid literal, expected true"
        );

        let err = run("01").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidNumber);
        assert_eq!(err.span, Some(Span { start: 0, end: 2 }));

        let err = run("1.").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidNumber);
        assert_eq!(err.span, Some(Span { start: 0, end: 2 }));

        let err = run("\"abc").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::UnterminatedString);
        assert_eq!(err.span, Some(Span { start: 0, end: 4 }));

        let err = run("\"\\x\"").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidEscape);
        assert_eq!(err.span, Some(Span { start: 0, end: 3 }));

        let err = run("\"\\uZZZZ\"").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidUnicodeEscape);
        assert_eq!(err.span, Some(Span { start: 0, end: 7 }));

        let err = run("\"\n\"").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::UnescapedControl);
        assert_eq!(err.span, Some(Span { start: 0, end: 2 }));
    }

    #[test]
    fn lone_surrogate_is_lexical_error() {
        let err = run("\"\\uD800\"").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidSurrogatePair);
    }
}
