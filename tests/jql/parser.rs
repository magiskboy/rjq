use crate::Span;
use crate::jql::{
    ast::{JqlAstNode, JqlBinaryKind, ObjectField, ObjectKey},
    error::{JqlError, JqlErrorKind},
    parser::JqlParser,
    token::{JqlToken, JqlTokenKind},
};

fn tokens(source: &str, parts: &[(JqlTokenKind, &str)]) -> Vec<JqlToken> {
    let mut cursor = 0;
    let mut out = Vec::with_capacity(parts.len());
    for &(kind, lexeme) in parts {
        let rel = source[cursor..]
            .find(lexeme)
            .unwrap_or_else(|| panic!("lexeme {lexeme:?} not found in {source:?} from {cursor}"));
        let start = cursor + rel;
        let end = start + lexeme.len();
        out.push(JqlToken::new(kind, start, end));
        cursor = end;
    }
    out
}

fn parse<'a>(source: &'a str, parts: &[(JqlTokenKind, &str)]) -> Result<JqlAstNode<'a>, JqlError> {
    JqlParser::parse(&tokens(source, parts), source)
}

fn access(path: &str) -> JqlAstNode<'_> {
    JqlAstNode::Access(path)
}

fn number(n: f64) -> JqlAstNode<'static> {
    JqlAstNode::Number(n)
}

fn string(text: &str) -> JqlAstNode<'_> {
    JqlAstNode::String(text)
}

fn binary<'a>(kind: JqlBinaryKind, left: JqlAstNode<'a>, right: JqlAstNode<'a>) -> JqlAstNode<'a> {
    JqlAstNode::Binary {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn pipe<'a>(source: JqlAstNode<'a>, dest: JqlAstNode<'a>) -> JqlAstNode<'a> {
    JqlAstNode::Pipe {
        source: Box::new(source),
        dest: Box::new(dest),
    }
}

fn call<'a>(name: &'a str, args: Vec<JqlAstNode<'a>>) -> JqlAstNode<'a> {
    JqlAstNode::Call { name, args }
}

fn object<'a>(fields: Vec<(&'a str, JqlAstNode<'a>)>) -> JqlAstNode<'a> {
    JqlAstNode::Object {
        fields: fields
            .into_iter()
            .map(|(key, value)| ObjectField {
                key: ObjectKey::Ident(key),
                value: Box::new(value),
            })
            .collect(),
    }
}

#[test]
fn literals() {
    assert_eq!(
        parse("true", &[(JqlTokenKind::Boolean, "true")]),
        Ok(JqlAstNode::Boolean(true))
    );
    assert_eq!(
        parse("false", &[(JqlTokenKind::Boolean, "false")]),
        Ok(JqlAstNode::Boolean(false))
    );
    assert_eq!(
        parse("null", &[(JqlTokenKind::Null, "null")]),
        Ok(JqlAstNode::Null)
    );
    assert_eq!(
        parse("10", &[(JqlTokenKind::Number, "10")]),
        Ok(number(10.0))
    );
    assert_eq!(parse("0", &[(JqlTokenKind::Number, "0")]), Ok(number(0.0)));
    assert_eq!(
        parse("\"hi\"", &[(JqlTokenKind::String, "\"hi\"")]),
        Ok(string("hi"))
    );
    assert_eq!(
        parse("\"\"", &[(JqlTokenKind::String, "\"\"")]),
        Ok(string(""))
    );
    assert_eq!(
        parse(".id", &[(JqlTokenKind::Identifier, ".id")]),
        Ok(access(".id"))
    );
    assert_eq!(
        parse(".[1]", &[(JqlTokenKind::Identifier, ".[1]")]),
        Ok(access(".[1]"))
    );
    assert_eq!(
        parse("foo", &[(JqlTokenKind::Identifier, "foo")]),
        Ok(call("foo", vec![]))
    );
}

#[test]
fn grouping_parentheses() {
    assert_eq!(
        parse(
            "(true)",
            &[
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(JqlAstNode::Boolean(true))
    );
    assert_eq!(
        parse(
            "((null))",
            &[
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Null, "null"),
                (JqlTokenKind::RParen, ")"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(JqlAstNode::Null)
    );
    assert_eq!(
        parse(
            "(1 > 2)",
            &[
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(binary(JqlBinaryKind::Greater, number(1.0), number(2.0)))
    );
}

#[test]
fn comparison_operators() {
    let cases = [
        ("1 == 2", JqlTokenKind::EqualOp, "==", JqlBinaryKind::Equal),
        (
            "1 != 2",
            JqlTokenKind::NotEqualOp,
            "!=",
            JqlBinaryKind::NotEqual,
        ),
        (
            "1 > 2",
            JqlTokenKind::GreaterOp,
            ">",
            JqlBinaryKind::Greater,
        ),
        (
            "1 >= 2",
            JqlTokenKind::GreaterEqualOp,
            ">=",
            JqlBinaryKind::GreaterEqual,
        ),
        ("1 < 2", JqlTokenKind::LessOp, "<", JqlBinaryKind::Less),
        (
            "1 <= 2",
            JqlTokenKind::LessEqualOp,
            "<=",
            JqlBinaryKind::LessEqual,
        ),
    ];
    for (source, op_kind, op_lexeme, binary_kind) in cases {
        assert_eq!(
            parse(
                source,
                &[
                    (JqlTokenKind::Number, "1"),
                    (op_kind, op_lexeme),
                    (JqlTokenKind::Number, "2"),
                ],
            ),
            Ok(binary(binary_kind, number(1.0), number(2.0))),
            "source {source:?}"
        );
    }
}

#[test]
fn logical_operators() {
    assert_eq!(
        parse(
            "true && false",
            &[
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::AndLogicalOp, "&&"),
                (JqlTokenKind::Boolean, "false"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::And,
            JqlAstNode::Boolean(true),
            JqlAstNode::Boolean(false),
        ))
    );
    assert_eq!(
        parse(
            "true || false",
            &[
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::OrLogicalOp, "||"),
                (JqlTokenKind::Boolean, "false"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::Or,
            JqlAstNode::Boolean(true),
            JqlAstNode::Boolean(false),
        ))
    );
}

#[test]
fn pipe_operator() {
    assert_eq!(
        parse(
            ".a | .b",
            &[
                (JqlTokenKind::Identifier, ".a"),
                (JqlTokenKind::Pipe, "|"),
                (JqlTokenKind::Identifier, ".b"),
            ],
        ),
        Ok(pipe(access(".a"), access(".b")))
    );
}

#[test]
fn function_calls() {
    assert_eq!(
        parse(
            "filter(.id > 10)",
            &[
                (JqlTokenKind::Identifier, "filter"),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Identifier, ".id"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "10"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(call(
            "filter",
            vec![binary(JqlBinaryKind::Greater, access(".id"), number(10.0))],
        ))
    );
    assert_eq!(
        parse(
            "f((true))",
            &[
                (JqlTokenKind::Identifier, "f"),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::RParen, ")"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(call("f", vec![JqlAstNode::Boolean(true)]))
    );
}

#[test]
fn comparisons_bind_tighter_than_logical_ops() {
    assert_eq!(
        parse(
            "1 > 2 && 3 < 4",
            &[
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::AndLogicalOp, "&&"),
                (JqlTokenKind::Number, "3"),
                (JqlTokenKind::LessOp, "<"),
                (JqlTokenKind::Number, "4"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::And,
            binary(JqlBinaryKind::Greater, number(1.0), number(2.0)),
            binary(JqlBinaryKind::Less, number(3.0), number(4.0)),
        ))
    );
}

#[test]
fn and_binds_looser_than_or() {
    // Current PartialOrd: AndLogicalOp < OrLogicalOp, so || reduces first.
    assert_eq!(
        parse(
            "true && false || null",
            &[
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::AndLogicalOp, "&&"),
                (JqlTokenKind::Boolean, "false"),
                (JqlTokenKind::OrLogicalOp, "||"),
                (JqlTokenKind::Null, "null"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::And,
            JqlAstNode::Boolean(true),
            binary(
                JqlBinaryKind::Or,
                JqlAstNode::Boolean(false),
                JqlAstNode::Null
            ),
        ))
    );
    assert_eq!(
        parse(
            "true || false && null",
            &[
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::OrLogicalOp, "||"),
                (JqlTokenKind::Boolean, "false"),
                (JqlTokenKind::AndLogicalOp, "&&"),
                (JqlTokenKind::Null, "null"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::And,
            binary(
                JqlBinaryKind::Or,
                JqlAstNode::Boolean(true),
                JqlAstNode::Boolean(false)
            ),
            JqlAstNode::Null,
        ))
    );
}

#[test]
fn operators_are_right_associative() {
    assert_eq!(
        parse(
            "1 > 2 > 3",
            &[
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "3"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::Greater,
            number(1.0),
            binary(JqlBinaryKind::Greater, number(2.0), number(3.0)),
        ))
    );
    assert_eq!(
        parse(
            "1 == 2 != 3",
            &[
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::EqualOp, "=="),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::NotEqualOp, "!="),
                (JqlTokenKind::Number, "3"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::Equal,
            number(1.0),
            binary(JqlBinaryKind::NotEqual, number(2.0), number(3.0)),
        ))
    );
    assert_eq!(
        parse(
            ".a | .b | .c",
            &[
                (JqlTokenKind::Identifier, ".a"),
                (JqlTokenKind::Pipe, "|"),
                (JqlTokenKind::Identifier, ".b"),
                (JqlTokenKind::Pipe, "|"),
                (JqlTokenKind::Identifier, ".c"),
            ],
        ),
        Ok(pipe(access(".a"), pipe(access(".b"), access(".c"))))
    );
}

#[test]
fn mixed_comparison_associativity_follows_token_ord() {
    // LessOp < GreaterOp is false for `top > token` when top is Less and token is Greater,
    // so `1 < 2 > 3` reduces left first: (1 < 2) > 3.
    assert_eq!(
        parse(
            "1 < 2 > 3",
            &[
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::LessOp, "<"),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "3"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::Greater,
            binary(JqlBinaryKind::Less, number(1.0), number(2.0)),
            number(3.0),
        ))
    );
}

#[test]
fn parentheses_override_precedence() {
    // Without parens, `||` then `&&` becomes (true || false) && null.
    assert_eq!(
        parse(
            "true || (false && null)",
            &[
                (JqlTokenKind::Boolean, "true"),
                (JqlTokenKind::OrLogicalOp, "||"),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Boolean, "false"),
                (JqlTokenKind::AndLogicalOp, "&&"),
                (JqlTokenKind::Null, "null"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(binary(
            JqlBinaryKind::Or,
            JqlAstNode::Boolean(true),
            binary(
                JqlBinaryKind::And,
                JqlAstNode::Boolean(false),
                JqlAstNode::Null
            ),
        ))
    );
}

#[test]
fn trailing_rparen_is_ignored() {
    assert_eq!(
        parse(
            "1 > 2)",
            &[
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "2"),
                (JqlTokenKind::RParen, ")"),
            ],
        ),
        Ok(binary(JqlBinaryKind::Greater, number(1.0), number(2.0)))
    );
}

#[test]
fn whitespace_and_stop_tokens_are_skipped() {
    let source = " true";
    let mut toks = tokens(
        source,
        &[
            (JqlTokenKind::Whitespace, " "),
            (JqlTokenKind::Boolean, "true"),
        ],
    );
    toks.push(JqlToken::new(
        JqlTokenKind::Stop,
        source.len(),
        source.len(),
    ));
    assert_eq!(
        JqlParser::parse(&toks, source),
        Ok(JqlAstNode::Boolean(true))
    );
}

#[test]
fn unexpected_token_kinds_are_errors() {
    let source = "true";
    let mut toks = tokens(source, &[(JqlTokenKind::Boolean, "true")]);
    toks.push(JqlToken::new(
        JqlTokenKind::InvalidToken,
        source.len(),
        source.len(),
    ));
    let err = JqlParser::parse(&toks, source).unwrap_err();
    assert!(matches!(
        err.kind,
        JqlErrorKind::UnexpectedToken {
            expected: crate::jql::error::ExpectedSyntax::Expression,
            found: JqlTokenKind::InvalidToken,
        }
    ));
}

#[test]
fn readme_pipeline() {
    let source = ".[1] | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))";
    let parts = [
        (JqlTokenKind::Identifier, ".[1]"),
        (JqlTokenKind::Pipe, "|"),
        (JqlTokenKind::Identifier, "filter"),
        (JqlTokenKind::LParen, "("),
        (JqlTokenKind::Identifier, ".id"),
        (JqlTokenKind::GreaterOp, ">"),
        (JqlTokenKind::Number, "10"),
        (JqlTokenKind::AndLogicalOp, "&&"),
        (JqlTokenKind::Identifier, ".age"),
        (JqlTokenKind::LessOp, "<"),
        (JqlTokenKind::Number, "20"),
        (JqlTokenKind::AndLogicalOp, "&&"),
        (JqlTokenKind::LParen, "("),
        (JqlTokenKind::Identifier, ".money"),
        (JqlTokenKind::GreaterOp, ">"),
        (JqlTokenKind::Number, "10"),
        (JqlTokenKind::OrLogicalOp, "||"),
        (JqlTokenKind::Identifier, ".gold"),
        (JqlTokenKind::GreaterEqualOp, ">="),
        (JqlTokenKind::Number, "1"),
        (JqlTokenKind::RParen, ")"),
        (JqlTokenKind::RParen, ")"),
    ];

    let money_or_gold = binary(
        JqlBinaryKind::Or,
        binary(JqlBinaryKind::Greater, access(".money"), number(10.0)),
        binary(JqlBinaryKind::GreaterEqual, access(".gold"), number(1.0)),
    );
    let predicate = binary(
        JqlBinaryKind::And,
        binary(JqlBinaryKind::Greater, access(".id"), number(10.0)),
        binary(
            JqlBinaryKind::And,
            binary(JqlBinaryKind::Less, access(".age"), number(20.0)),
            money_or_gold,
        ),
    );

    assert_eq!(
        parse(source, &parts),
        Ok(pipe(access(".[1]"), call("filter", vec![predicate])))
    );
}

#[test]
fn object_construct() {
    assert_eq!(
        parse(
            "{}",
            &[(JqlTokenKind::LBrace, "{"), (JqlTokenKind::RBrace, "}")]
        ),
        Ok(object(vec![]))
    );
    assert_eq!(
        parse(
            "{ name: .title }",
            &[
                (JqlTokenKind::LBrace, "{"),
                (JqlTokenKind::Identifier, "name"),
                (JqlTokenKind::Colon, ":"),
                (JqlTokenKind::Identifier, ".title"),
                (JqlTokenKind::RBrace, "}"),
            ]
        ),
        Ok(object(vec![("name", access(".title"))]))
    );
    assert_eq!(
        parse(
            "{ name: .title, pay: .money }",
            &[
                (JqlTokenKind::LBrace, "{"),
                (JqlTokenKind::Identifier, "name"),
                (JqlTokenKind::Colon, ":"),
                (JqlTokenKind::Identifier, ".title"),
                (JqlTokenKind::Comma, ","),
                (JqlTokenKind::Identifier, "pay"),
                (JqlTokenKind::Colon, ":"),
                (JqlTokenKind::Identifier, ".money"),
                (JqlTokenKind::RBrace, "}"),
            ]
        ),
        Ok(object(vec![
            ("name", access(".title")),
            ("pay", access(".money")),
        ]))
    );
}

#[test]
fn map_call_with_object_construct() {
    let source = "map({ name: .title, pay: .money })";
    assert_eq!(
        parse(
            source,
            &[
                (JqlTokenKind::Identifier, "map"),
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::LBrace, "{"),
                (JqlTokenKind::Identifier, "name"),
                (JqlTokenKind::Colon, ":"),
                (JqlTokenKind::Identifier, ".title"),
                (JqlTokenKind::Comma, ","),
                (JqlTokenKind::Identifier, "pay"),
                (JqlTokenKind::Colon, ":"),
                (JqlTokenKind::Identifier, ".money"),
                (JqlTokenKind::RBrace, "}"),
                (JqlTokenKind::RParen, ")"),
            ]
        ),
        Ok(call(
            "map",
            vec![object(vec![
                ("name", access(".title")),
                ("pay", access(".money")),
            ])]
        ))
    );
}

#[test]
fn unclosed_brace_reports_span_on_open() {
    let err = parse(
        "{ name: .title",
        &[
            (JqlTokenKind::LBrace, "{"),
            (JqlTokenKind::Identifier, "name"),
            (JqlTokenKind::Colon, ":"),
            (JqlTokenKind::Identifier, ".title"),
        ],
    )
    .unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::UnclosedBrace);
    assert_eq!(err.span, Some(Span { start: 0, end: 1 }));
}

#[test]
fn empty_input_is_invalid_expression() {
    let err = parse("", &[]).unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::InvalidExpression);
    assert_eq!(err.span, Some(Span { start: 0, end: 0 }));
}

#[test]
fn juxtaposition_is_invalid_expression() {
    let err = parse(
        "true false",
        &[
            (JqlTokenKind::Boolean, "true"),
            (JqlTokenKind::Boolean, "false"),
        ],
    )
    .unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::InvalidExpression);
    assert_eq!(err.span, Some(Span { start: 0, end: 10 }));
}

#[test]
fn empty_call_args_are_invalid_expression() {
    let err = parse(
        "filter()",
        &[
            (JqlTokenKind::Identifier, "filter"),
            (JqlTokenKind::LParen, "("),
            (JqlTokenKind::RParen, ")"),
        ],
    )
    .unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::InvalidExpression);
    assert_eq!(err.span, Some(Span { start: 0, end: 8 }));
}

#[test]
fn missing_operand_errors() {
    let cases: &[(&str, &[(JqlTokenKind, &str)])] = &[
        (">", &[(JqlTokenKind::GreaterOp, ">")]),
        (
            "1 >",
            &[(JqlTokenKind::Number, "1"), (JqlTokenKind::GreaterOp, ">")],
        ),
        (
            "> 1",
            &[(JqlTokenKind::GreaterOp, ">"), (JqlTokenKind::Number, "1")],
        ),
        ("(", &[(JqlTokenKind::LParen, "(")]),
        (
            "(1 > 2",
            &[
                (JqlTokenKind::LParen, "("),
                (JqlTokenKind::Number, "1"),
                (JqlTokenKind::GreaterOp, ">"),
                (JqlTokenKind::Number, "2"),
            ],
        ),
    ];
    for (source, parts) in cases {
        let err = parse(source, parts).unwrap_err();
        assert_eq!(err.kind, JqlErrorKind::MissingOperand, "source {source:?}");
        assert!(err.span.is_some(), "source {source:?}");
    }
}

#[test]
fn lone_rparen_is_invalid_expression() {
    let err = parse(")", &[(JqlTokenKind::RParen, ")")]).unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::InvalidExpression);
    assert_eq!(err.span, Some(Span { start: 0, end: 1 }));
}

#[test]
fn unclosed_call_paren_reports_span_on_name() {
    let err = parse(
        "filter(.id",
        &[
            (JqlTokenKind::Identifier, "filter"),
            (JqlTokenKind::LParen, "("),
            (JqlTokenKind::Identifier, ".id"),
        ],
    )
    .unwrap_err();
    assert_eq!(err.kind, JqlErrorKind::UnclosedParen);
    assert_eq!(err.span, Some(Span { start: 0, end: 6 }));
}

#[test]
fn parse_errors_carry_kind_and_display() {
    let err = parse("", &[]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at 1:1 (0..0): invalid expression"
    );

    let err = parse(">", &[(JqlTokenKind::GreaterOp, ">")]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at 1:1 (0..1): expected an operand"
    );

    let err = parse(
        "filter(.id",
        &[
            (JqlTokenKind::Identifier, "filter"),
            (JqlTokenKind::LParen, "("),
            (JqlTokenKind::Identifier, ".id"),
        ],
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at 1:1 (0..6): expected a closed parenthesis"
    );
}
