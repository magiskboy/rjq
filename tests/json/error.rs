use super::{ExpectedJsonType, ExpectedSyntax, JsonError, JsonErrorKind, JsonType};
use crate::Span;
use crate::json::token::JsonTokenKind;

fn err(kind: JsonErrorKind, start: usize, end: usize) -> JsonError {
    let source = "x".repeat(end.max(1));
    JsonError::new(kind, Span { start, end }, &source)
}

#[test]
fn display_uses_one_sentence_per_kind() {
    let cases = [
        (
            err(JsonErrorKind::InvalidCharacter, 0, 1),
            "lexical error at 1:1 (0..1): invalid character",
        ),
        (
            err(JsonErrorKind::InvalidLiteral { expected: "true" }, 0, 3),
            "lexical error at 1:1 (0..3): invalid literal, expected true",
        ),
        (
            err(JsonErrorKind::InvalidNumber, 0, 2),
            "lexical error at 1:1 (0..2): invalid number",
        ),
        (
            err(JsonErrorKind::UnterminatedString, 0, 4),
            "lexical error at 1:1 (0..4): unterminated string",
        ),
        (
            err(JsonErrorKind::InvalidEscape, 0, 3),
            "lexical error at 1:1 (0..3): invalid escape",
        ),
        (
            err(JsonErrorKind::InvalidUnicodeEscape, 0, 4),
            "lexical error at 1:1 (0..4): invalid unicode escape",
        ),
        (
            err(JsonErrorKind::InvalidSurrogatePair, 0, 8),
            "lexical error at 1:1 (0..8): invalid surrogate pair",
        ),
        (
            err(JsonErrorKind::UnescapedControl, 1, 2),
            "lexical error at 1:2 (1..2): unescaped control character",
        ),
        (
            err(JsonErrorKind::EmptyInput, 0, 0),
            "parse error at 1:1 (0..0): empty input",
        ),
        (
            err(
                JsonErrorKind::UnexpectedToken {
                    expected: ExpectedSyntax::Token(JsonTokenKind::Colon),
                    found: JsonTokenKind::Number,
                },
                3,
                4,
            ),
            "parse error at 1:4 (3..4): expected ':' but found number",
        ),
        (
            err(
                JsonErrorKind::UnexpectedToken {
                    expected: ExpectedSyntax::Value,
                    found: JsonTokenKind::Comma,
                },
                2,
                3,
            ),
            "parse error at 1:3 (2..3): expected value but found ','",
        ),
        (
            err(
                JsonErrorKind::UnexpectedEof {
                    expected: ExpectedSyntax::Token(JsonTokenKind::RBrace),
                },
                1,
                1,
            ),
            "parse error at 1:2 (1..1): expected '}' but reached end of input",
        ),
        (
            err(JsonErrorKind::TrailingInput, 5, 10),
            "parse error at 1:6 (5..10): trailing input",
        ),
        (
            JsonError::value(JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::ArrayOrObject,
                found: JsonType::String,
            }),
            "value error: expected array or object but found string",
        ),
        (
            JsonError::value(JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Boolean,
                found: JsonType::Null,
            }),
            "value error: expected boolean but found null",
        ),
        (
            JsonError::value(JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Number,
                found: JsonType::String,
            }),
            "value error: expected number but found string",
        ),
        (
            JsonError::value(JsonErrorKind::TypeMismatch {
                expected: ExpectedJsonType::Array,
                found: JsonType::Number,
            }),
            "value error: expected array but found number",
        ),
        (
            JsonError::value(JsonErrorKind::KeyNotFound {
                key: "name".to_string(),
            }),
            "value error: key \"name\" not found",
        ),
        (
            JsonError::value(JsonErrorKind::IndexOutOfBounds { index: 3, len: 1 }),
            "value error: index 3 out of bounds for length 1",
        ),
        (
            JsonError::value(JsonErrorKind::InvalidIndex {
                key: "name".to_string(),
            }),
            "value error: invalid index \"name\"",
        ),
        (
            JsonError::value(JsonErrorKind::InvalidCast),
            "value error: invalid cast",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}
