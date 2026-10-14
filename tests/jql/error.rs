use super::{ExpectedSyntax, JqlError, JqlErrorKind};
use crate::Span;
use crate::jql::token::JqlTokenKind;

fn err(kind: JqlErrorKind, start: usize, end: usize) -> JqlError {
    let source = "x".repeat(end.max(1));
    JqlError::new(kind, Span { start, end }, &source)
}

#[test]
fn display_uses_one_sentence_per_kind() {
    let cases = [
        (
            err(JqlErrorKind::InvalidToken, 0, 1),
            "lexical error at 1:1 (0..1): invalid token",
        ),
        (
            JqlError::without_span(JqlErrorKind::MissingOperand),
            "parse error: expected an operand",
        ),
        (
            err(JqlErrorKind::UnclosedParen, 2, 8),
            "parse error at 1:3 (2..8): expected a closed parenthesis",
        ),
        (
            JqlError::without_span(JqlErrorKind::InvalidExpression),
            "parse error: invalid expression",
        ),
        (
            err(
                JqlErrorKind::UnexpectedToken {
                    expected: ExpectedSyntax::Token(JqlTokenKind::Number),
                    found: JqlTokenKind::String,
                },
                0,
                3,
            ),
            "parse error at 1:1 (0..3): expected number but found string",
        ),
        (
            JqlError::without_span(JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::BinaryOperator,
                found: JqlTokenKind::Pipe,
            }),
            "parse error: expected binary operator but found '|'",
        ),
        (
            JqlError::without_span(JqlErrorKind::InvalidAccess {
                path: "jobs".to_string(),
            }),
            "execute error: invalid reference \"jobs\"",
        ),
        (
            JqlError::without_span(JqlErrorKind::TypeMismatch {
                expected: "array",
                found: "number",
            }),
            "execute error: expected array but found number",
        ),
        (
            JqlError::without_span(JqlErrorKind::PathNotFound {
                path: "missing".to_string(),
            }),
            "execute error: path \"missing\" not found",
        ),
        (
            JqlError::without_span(JqlErrorKind::UnsupportedOperation {
                op: "unknown".to_string(),
            }),
            "execute error: unsupported operation \"unknown\"",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}
