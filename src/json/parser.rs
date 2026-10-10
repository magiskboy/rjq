use std::io::Read;
use std::mem;

use crate::Location;
use crate::Span;
use crate::json::{
    builder::{JsonAtom, JsonBuilder, JsonNumberAtom, JsonValueBuilder},
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
    fn error(&self, kind: JsonErrorKind, span: Span, location: Location) -> JsonError {
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
            return Err(self.error(
                JsonErrorKind::TrailingInput,
                self.current.span,
                self.current.location,
            ));
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
            self.lexer.location(),
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
                self.current.location,
            ));
        }
        let placeholder = JsonToken::simple(
            JsonTokenKind::Stop,
            self.current.span,
            self.current.location,
        );
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
                        .error(
                            JsonErrorKind::InvalidEscape,
                            key_token.span,
                            key_token.location,
                        )
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
                let location = self.current.location;
                let lexeme = mem::replace(&mut self.current.lexeme, JsonLexeme::None);
                self.bump()?;
                match lexeme {
                    JsonLexeme::String(s) => builder.atom(JsonAtom::String(s)),
                    _ => Err(self
                        .error(JsonErrorKind::InvalidEscape, span, location)
                        .into()),
                }
            }
            JsonTokenKind::Number => {
                let span = self.current.span;
                let location = self.current.location;
                let lexeme = mem::replace(&mut self.current.lexeme, JsonLexeme::None);
                self.bump()?;
                match lexeme {
                    JsonLexeme::Number(n) => {
                        builder.atom(JsonAtom::Number(JsonNumberAtom::F32(n)))
                    }
                    _ => Err(self
                        .error(JsonErrorKind::InvalidNumber, span, location)
                        .into()),
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
            self.current.location,
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
mod test {
    use crate::Span;
    use crate::json::{
        JsonValue,
        error::{ExpectedSyntax, JsonError, JsonErrorKind},
        parser::JsonParser,
        token::JsonTokenKind,
    };
    use std::borrow::Cow;

    fn parse(source: &str) -> Result<JsonValue<'_>, JsonError> {
        JsonParser::parse_str(source)
    }

    fn string(text: &str) -> JsonValue<'static> {
        JsonValue::string(text.to_string())
    }

    fn number(n: f32) -> JsonValue<'static> {
        JsonValue::number(n)
    }

    fn array(items: Vec<JsonValue<'static>>) -> JsonValue<'static> {
        JsonValue::array(items)
    }

    fn object(pairs: &[(&str, JsonValue<'static>)]) -> JsonValue<'static> {
        JsonValue::object(
            pairs
                .iter()
                .map(|(k, v)| (Cow::Owned((*k).to_string()), v.clone())),
        )
    }

    #[test]
    fn string_without_escape_is_borrowed() {
        let source = r#""hello""#;
        let value = parse(source).unwrap();
        match value {
            JsonValue::String(Cow::Borrowed(s)) => assert_eq!(s, "hello"),
            other => panic!("expected borrowed string, got {other:?}"),
        }
    }

    #[test]
    fn string_with_escape_is_owned() {
        let source = r#""hel\"lo""#;
        let value = parse(source).unwrap();
        match value {
            JsonValue::String(Cow::Owned(s)) => assert_eq!(s, "hel\"lo"),
            other => panic!("expected owned string, got {other:?}"),
        }
    }

    #[test]
    fn literals() {
        let input: [(&str, JsonValue); 3] = [
            ("true", JsonValue::True),
            ("false", JsonValue::False),
            ("null", JsonValue::Null),
        ];
        for (s, expected) in input {
            assert_eq!(parse(s), Ok(expected), "literal {:?}", s);
        }
    }

    #[test]
    fn whitespace_is_insignificant() {
        let padded: [(&str, JsonValue); 4] = [
            ("  true  ", JsonValue::True),
            ("\tfalse\t", JsonValue::False),
            ("\nnull\n", JsonValue::Null),
            ("\r1\r", number(1.0)),
        ];
        for (s, expected) in padded {
            assert_eq!(parse(s), Ok(expected), "padded {:?}", s);
        }

        let mixed = " \t\r\n\"ab\"";
        assert_eq!(parse(mixed), Ok(string("ab")));
        assert_eq!(
            parse(" [ 1 , 2 ] "),
            Ok(array(vec![number(1.0), number(2.0)]))
        );
        assert_eq!(
            parse(" { \"a\" : true } "),
            Ok(object(&[("a", JsonValue::True)]))
        );
    }

    #[test]
    fn numbers() {
        let input: [(&str, f32); 16] = [
            ("0", 0.0),
            ("-0", -0.0),
            ("1", 1.0),
            ("10", 10.0),
            ("-1", -1.0),
            ("-10", -10.0),
            ("0.5", 0.5),
            ("10.0", 10.0),
            ("3.25", 3.25),
            ("-1.5", -1.5),
            ("1e2", 100.0),
            ("1E2", 100.0),
            ("1e+2", 100.0),
            ("1e-1", 1e-1),
            ("1.5e1", 15.0),
            ("-2.5E+1", -25.0),
        ];
        for (s, expected) in input {
            assert_eq!(parse(s), Ok(number(expected)), "number {:?}", s);
        }
    }

    #[test]
    fn strings() {
        let input: [(&str, &str); 6] = [
            (r#""""#, ""),
            (r#""hello""#, "hello"),
            (r#""hello world""#, "hello world"),
            (r#"" ""#, " "),
            (r#""é""#, "é"),
            (r#""🙂""#, "🙂"),
        ];
        for (s, text) in input {
            assert_eq!(parse(s), Ok(string(text)), "string {:?}", s);
        }
    }

    #[test]
    fn string_escape_sequences() {
        let input: [(&str, &str); 13] = [
            (r#""\"""#, "\""),
            (r#""\\""#, "\\"),
            (r#""\/""#, "/"),
            (r#""\b""#, "\u{0008}"),
            (r#""\f""#, "\u{000c}"),
            (r#""\n""#, "\n"),
            (r#""\r""#, "\r"),
            (r#""\t""#, "\t"),
            (r#""a\/b""#, "a/b"),
            (r#""a\nb\tc""#, "a\nb\tc"),
            (r#""\u0041""#, "A"),
            (r#""\u00E9""#, "é"),
            (r#""\u20AC""#, "€"),
        ];
        for (s, text) in input {
            assert_eq!(parse(s), Ok(string(text)), "escape {:?}", s);
        }
    }

    #[test]
    fn string_surrogate_pair() {
        assert_eq!(parse(r#""\uD83D\uDE00""#), Ok(string("😀")));
    }

    #[test]
    fn empty_containers() {
        assert_eq!(parse("[]"), Ok(array(vec![])));
        assert_eq!(parse("{}"), Ok(object(&[])));
        assert_eq!(parse("[ ]"), Ok(array(vec![])));
        assert_eq!(parse("{ }"), Ok(object(&[])));
    }

    #[test]
    fn arrays() {
        assert_eq!(parse("[1]"), Ok(array(vec![number(1.0)])));
        assert_eq!(
            parse("[1,2,3]"),
            Ok(array(vec![number(1.0), number(2.0), number(3.0)]))
        );
        assert_eq!(
            parse("[true,false,null]"),
            Ok(array(vec![
                JsonValue::True,
                JsonValue::False,
                JsonValue::Null,
            ]))
        );
        assert_eq!(
            parse(r#"["x","y"]"#),
            Ok(array(vec![string("x"), string("y")]))
        );
        assert_eq!(parse("[[]]"), Ok(array(vec![array(vec![])])));
        assert_eq!(
            parse("[1,[2,3]]"),
            Ok(array(vec![
                number(1.0),
                array(vec![number(2.0), number(3.0)]),
            ]))
        );
    }

    #[test]
    fn objects() {
        assert_eq!(parse(r#"{"a":1}"#), Ok(object(&[("a", number(1.0))])));
        assert_eq!(
            parse(r#"{"a":1,"b":true}"#),
            Ok(object(&[("a", number(1.0)), ("b", JsonValue::True)]))
        );
        assert_eq!(parse(r#"{"":0}"#), Ok(object(&[("", number(0.0))])));
        assert_eq!(
            parse(r#"{"s":"hi","n":null,"f":false}"#),
            Ok(object(&[
                ("s", string("hi")),
                ("n", JsonValue::Null),
                ("f", JsonValue::False),
            ]))
        );
        assert_eq!(
            parse(" { \"a\" : 1 , \"b\" : 2 } "),
            Ok(object(&[("a", number(1.0)), ("b", number(2.0))]))
        );
    }

    #[test]
    fn nested_values() {
        assert_eq!(
            parse(r#"{"arr":[1,{"k":null}],"ok":true}"#),
            Ok(object(&[
                (
                    "arr",
                    array(vec![number(1.0), object(&[("k", JsonValue::Null)]),]),
                ),
                ("ok", JsonValue::True),
            ]))
        );
        assert_eq!(
            parse(r#"[{"a":[true]},{"b":"x"}]"#),
            Ok(array(vec![
                object(&[("a", array(vec![JsonValue::True]))]),
                object(&[("b", string("x"))]),
            ]))
        );
    }

    #[test]
    fn deeply_nested_values() {
        let mut source = String::from("1");
        let mut expected = number(1.0);
        for _ in 0..8 {
            source = format!("[{source}]");
            expected = array(vec![expected]);
        }
        assert_eq!(parse(&source), Ok(expected));

        let mut source = String::from("null");
        let mut expected = JsonValue::Null;
        for _ in 0..8 {
            source = format!(r#"{{"k":{source}}}"#);
            expected = object(&[("k", expected)]);
        }
        assert_eq!(parse(&source), Ok(expected));
    }

    #[test]
    fn empty_input_is_error() {
        let input: [&str; 6] = ["", " ", "\t", "\n", "\r", " \t\r\n"];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn trailing_commas_are_errors() {
        let input: [&str; 6] = [
            "[1,]",
            "[1,2,]",
            "[,]",
            "{\"a\":1,}",
            "{\"a\":1,\"b\":2,}",
            "{,}",
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn missing_separators_are_errors() {
        let input: [&str; 8] = [
            "[1 2]",
            "[1,,2]",
            "[,1]",
            r#"{"a" 1}"#,
            r#"{"a":1 "b":2}"#,
            r#"{"a":1,,"b":2}"#,
            r#"{,"a":1}"#,
            r#"{"a"::1}"#,
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn incomplete_containers_are_errors() {
        let input: [&str; 10] = [
            "[",
            "{",
            "[1",
            "[1,",
            r#"{"a""#,
            r#"{"a":"#,
            r#"{"a":1"#,
            r#"{"a":}"#,
            r#"{:1}"#,
            "]",
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn mismatched_delimiters_are_errors() {
        let input: [&str; 6] = ["[}", "{]", "[1}", "{1]", r#"{"a":1]"#, r#"["a":1]"#];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn object_keys_must_be_strings() {
        let input: [&str; 6] = [
            "{true:1}",
            "{false:1}",
            "{null:1}",
            "{1:1}",
            "{[]:1}",
            "{{}:1}",
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn extra_tokens_are_errors() {
        let input: [&str; 7] = [
            "true false",
            "1 2",
            "null null",
            r#"{"a":1}{"b":2}"#,
            "[1][2]",
            r#""a""b""#,
            "true,",
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn documents_outside_json_are_errors() {
        let input: [&str; 12] = [
            "NaN",
            "Infinity",
            "-Infinity",
            "undefined",
            "+1",
            "01",
            "'abc'",
            "// comment",
            "/* comment */",
            r#""\uD800""#,
            r#""\uDE00""#,
            "\u{FEFF}true",
        ];
        for s in input {
            assert!(parse(s).is_err(), "expected parse error for {:?}", s);
        }
    }

    #[test]
    fn parse_errors_carry_kind_and_span() {
        let err = parse("").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::EmptyInput);
        assert_eq!(err.span, Some(Span { start: 0, end: 0 }));
        assert_eq!(err.to_string(), "parse error at 1:1 (0..0): empty input");

        let err = parse(" ").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::EmptyInput);
        assert_eq!(err.span, Some(Span { start: 0, end: 1 }));

        let err = parse("{").unwrap_err();
        assert_eq!(
            err.kind,
            JsonErrorKind::UnexpectedEof {
                expected: ExpectedSyntax::Token(JsonTokenKind::RBrace),
            }
        );
        assert_eq!(err.span, Some(Span { start: 1, end: 1 }));
        assert_eq!(
            err.to_string(),
            "parse error at 1:2 (1..1): expected '}' but reached end of input"
        );

        let err = parse("true false").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::TrailingInput);
        assert_eq!(err.span, Some(Span { start: 5, end: 10 }));
        assert_eq!(
            err.to_string(),
            "parse error at 1:6 (5..10): trailing input"
        );

        let err = parse("[1}").unwrap_err();
        assert_eq!(
            err.kind,
            JsonErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JsonTokenKind::RBracket),
                found: JsonTokenKind::RBrace,
            }
        );
        assert_eq!(err.span, Some(Span { start: 2, end: 3 }));
        assert_eq!(
            err.to_string(),
            "parse error at 1:3 (2..3): expected ']' but found '}'"
        );

        let err = parse(r#"{"a":}"#).unwrap_err();
        assert_eq!(
            err.kind,
            JsonErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Value,
                found: JsonTokenKind::RBrace,
            }
        );

        let err = parse(r#""\uD800""#).unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::InvalidSurrogatePair);
        assert_eq!(err.span, Some(Span { start: 0, end: 8 }));
        assert_eq!(
            err.to_string(),
            "lexical error at 1:1 (0..8): invalid surrogate pair"
        );
    }
}
