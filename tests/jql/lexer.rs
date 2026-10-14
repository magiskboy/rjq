use super::JqlLexer;
use crate::Span;
use crate::jql::error::{JqlError, JqlErrorKind};
use crate::jql::token::{JqlToken, JqlTokenKind};

fn run(source: &str) -> Result<Vec<JqlToken>, JqlError> {
    let tokens = JqlLexer::tokenize(source)?;
    for token in tokens.clone() {
        println!("{}", token.display(source));
    }
    Ok(tokens)
}

fn tok(kind: JqlTokenKind, start: usize, end: usize) -> JqlToken {
    JqlToken::new(kind, start, end)
}

fn kinds(tokens: &[JqlToken]) -> Vec<JqlTokenKind> {
    tokens.iter().map(|token| token.kind.clone()).collect()
}

fn assert_invalid(source: &str) {
    let err = run(source).expect_err(&format!("source {source:?}"));
    assert_eq!(err.kind, JqlErrorKind::InvalidToken, "source {source:?}");
}

#[test]
fn empty_and_spaces_are_insignificant() {
    for source in ["", " ", "   "] {
        assert_eq!(run(source), Ok(vec![]), "source {source:?}");
    }

    let source = "  true   false  ";
    assert_eq!(
        run(source),
        Ok(vec![
            tok(JqlTokenKind::Boolean, 2, 6),
            tok(JqlTokenKind::Boolean, 9, 14),
        ])
    );
}

#[test]
fn parentheses() {
    assert_eq!(run("("), Ok(vec![tok(JqlTokenKind::LParen, 0, 1)]));
    assert_eq!(run(")"), Ok(vec![tok(JqlTokenKind::RParen, 0, 1)]));
    assert_eq!(
        run("( )"),
        Ok(vec![
            tok(JqlTokenKind::LParen, 0, 1),
            tok(JqlTokenKind::RParen, 2, 3),
        ])
    );
}

#[test]
fn comparison_operators() {
    let cases = [
        ("==", JqlTokenKind::EqualOp),
        ("!=", JqlTokenKind::NotEqualOp),
        (">", JqlTokenKind::GreaterOp),
        (">=", JqlTokenKind::GreaterEqualOp),
        ("<", JqlTokenKind::LessOp),
        ("<=", JqlTokenKind::LessEqualOp),
    ];
    for (source, kind) in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(kind, 0, source.len())]),
            "operator {source:?}"
        );
    }

    assert_eq!(
        run(">>"),
        Ok(vec![
            tok(JqlTokenKind::GreaterOp, 0, 1),
            tok(JqlTokenKind::GreaterOp, 1, 2),
        ])
    );
    assert_eq!(
        run("<<"),
        Ok(vec![
            tok(JqlTokenKind::LessOp, 0, 1),
            tok(JqlTokenKind::LessOp, 1, 2),
        ])
    );
    assert_eq!(
        run(">x"),
        Ok(vec![
            tok(JqlTokenKind::GreaterOp, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 2),
        ])
    );
}

#[test]
fn pipe_and_logical_operators() {
    assert_eq!(run("|"), Ok(vec![tok(JqlTokenKind::Pipe, 0, 1)]));
    assert_eq!(run("||"), Ok(vec![tok(JqlTokenKind::OrLogicalOp, 0, 2)]));
    assert_eq!(run("&&"), Ok(vec![tok(JqlTokenKind::AndLogicalOp, 0, 2)]));
    assert_eq!(
        run("|||"),
        Ok(vec![
            tok(JqlTokenKind::OrLogicalOp, 0, 2),
            tok(JqlTokenKind::Pipe, 2, 3),
        ])
    );
    assert_eq!(
        run("||||"),
        Ok(vec![
            tok(JqlTokenKind::OrLogicalOp, 0, 2),
            tok(JqlTokenKind::OrLogicalOp, 2, 4),
        ])
    );
    assert_eq!(
        run("|x"),
        Ok(vec![
            tok(JqlTokenKind::Pipe, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 2),
        ])
    );
    assert_eq!(
        run("true && false || null"),
        Ok(vec![
            tok(JqlTokenKind::Boolean, 0, 4),
            tok(JqlTokenKind::AndLogicalOp, 5, 7),
            tok(JqlTokenKind::Boolean, 8, 13),
            tok(JqlTokenKind::OrLogicalOp, 14, 16),
            tok(JqlTokenKind::Null, 17, 21),
        ])
    );
}

#[test]
fn two_character_operators_require_the_exact_second_character() {
    for source in [
        "=", "!", "&", "=x", "!x", "&x", "===", "!==", "&&&", "&>", "|=",
    ] {
        assert_invalid(source);
    }
}

#[test]
fn booleans_null_and_keyword() {
    assert_eq!(run("true"), Ok(vec![tok(JqlTokenKind::Boolean, 0, 4)]));
    assert_eq!(run("false"), Ok(vec![tok(JqlTokenKind::Boolean, 0, 5)]));
    assert_eq!(run("null"), Ok(vec![tok(JqlTokenKind::Null, 0, 4)]));

    let source = "true";
    let tokens = run(source).expect("true");
    assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "true");

    let source = "false";
    let tokens = run(source).expect("false");
    assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "false");
}

#[test]
fn branching_keywords() {
    let cases = [
        ("if", JqlTokenKind::If),
        ("then", JqlTokenKind::Then),
        ("elif", JqlTokenKind::Elif),
        ("else", JqlTokenKind::Else),
        ("end", JqlTokenKind::End),
    ];
    for (source, kind) in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(kind, 0, source.len())]),
            "keyword {source:?}"
        );
    }

    assert_eq!(
        run("if .x then 1 elif .y then 2 else 3 end"),
        Ok(vec![
            tok(JqlTokenKind::If, 0, 2),
            tok(JqlTokenKind::Dot, 3, 4),
            tok(JqlTokenKind::Identifier, 4, 5),
            tok(JqlTokenKind::Then, 6, 10),
            tok(JqlTokenKind::Number, 11, 12),
            tok(JqlTokenKind::Elif, 13, 17),
            tok(JqlTokenKind::Dot, 18, 19),
            tok(JqlTokenKind::Identifier, 19, 20),
            tok(JqlTokenKind::Then, 21, 25),
            tok(JqlTokenKind::Number, 26, 27),
            tok(JqlTokenKind::Else, 28, 32),
            tok(JqlTokenKind::Number, 33, 34),
            tok(JqlTokenKind::End, 35, 38),
        ])
    );
}

#[test]
fn words_longer_than_a_literal_are_identifiers() {
    let cases = [
        "truex",
        "true1",
        "truefalse",
        "falsey",
        "nullable",
        "nulltrue",
        "filterx",
        "filters",
        "filterfilter",
        "TRUE",
        "FALSE",
        "NULL",
        "Filter",
        "t",
        "tr",
        "tru",
        "th",
        "the",
        "thenx",
        "f",
        "fa",
        "n",
        "nu",
        "nul",
        "i",
        "ifx",
        "e",
        "el",
        "eli",
        "els",
        "en",
        "elifx",
        "elsey",
        "ending",
        "IF",
        "THEN",
        "ELSE",
        "fil",
    ];
    for source in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::Identifier, 0, source.len())]),
            "source {source:?}"
        );
    }

    assert_eq!(
        run("true."),
        Ok(vec![
            tok(JqlTokenKind::Boolean, 0, 4),
            tok(JqlTokenKind::Dot, 4, 5),
        ])
    );
    assert_eq!(
        run("if."),
        Ok(vec![
            tok(JqlTokenKind::If, 0, 2),
            tok(JqlTokenKind::Dot, 2, 3),
        ])
    );
}

#[test]
fn numbers_are_digit_runs() {
    for source in ["0", "1", "10", "01", "100"] {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::Number, 0, source.len())]),
            "number {source:?}"
        );
    }

    assert_eq!(
        run("10x"),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 2),
            tok(JqlTokenKind::Identifier, 2, 3),
        ])
    );
    assert_eq!(
        run("10("),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 2),
            tok(JqlTokenKind::LParen, 2, 3),
        ])
    );
    assert_eq!(
        run("1."),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 1),
            tok(JqlTokenKind::Dot, 1, 2),
        ])
    );
    assert_eq!(run("1.2"), Ok(vec![tok(JqlTokenKind::Number, 0, 3)]));
}

fn lexemes<'a>(source: &'a str, tokens: &[JqlToken]) -> Vec<&'a str> {
    tokens
        .iter()
        .map(|token| &source[token.span.start..token.span.end])
        .collect()
}

#[test]
fn json_number_forms() {
    for source in [
        "0", "1", "10", "100", "123", "0.1", "3.14", "10.0", "1e2", "1E2", "1.2e3", "0.0e0",
        "1e+2", "1e-2", "1.2E+3", "-0", "-1", "-10", "-1.2e-3",
    ] {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::Number, 0, source.len())]),
            "number {source:?}"
        );
    }

    // Lone `-` remains Subtract; incomplete fraction/exponent falls back to the last valid prefix.
    assert_eq!(run("-"), Ok(vec![tok(JqlTokenKind::Subtract, 0, 1)]));
    assert_eq!(
        run("1."),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 1),
            tok(JqlTokenKind::Dot, 1, 2),
        ])
    );
    assert_eq!(
        run("1e"),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 2),
        ])
    );
    assert_eq!(
        run("1 + -2"),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 1),
            tok(JqlTokenKind::Plus, 2, 3),
            tok(JqlTokenKind::Number, 4, 6),
        ])
    );
}

#[test]
fn strings() {
    let cases = ["\"\"", "\"hi\"", "\"hello world\"", "\"A1\"", "\"a b\""];
    for source in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::String, 0, source.len())]),
            "string {source:?}"
        );
    }

    assert_eq!(
        run("\"hi\"name"),
        Ok(vec![
            tok(JqlTokenKind::String, 0, 4),
            tok(JqlTokenKind::Identifier, 4, 8),
        ])
    );
    let spaced = "\"a\tb\"";
    assert_eq!(
        run(spaced),
        Ok(vec![tok(JqlTokenKind::String, 0, spaced.len())])
    );
}

#[test]
fn string_escape_sequences_are_literal_text() {
    let cases = [
        "\"\\\\\"",
        "\"\\/\"",
        "\"\\b\\f\\n\\r\\t\"",
        "\"\\\\n\"",
        "\"\\u0041\"",
        "\"\\u004a\"",
        "\"\\uD83D\\uDE00\"",
        "\"\\u12\"",
        "\"\\uZZZZ\"",
        "\"\\x\"",
        "\"a\\\"",
        "\"foo\\\\\"",
        "\"é\\n🙂\"",
    ];
    for source in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::String, 0, source.len())]),
            "string {source:?}"
        );
        assert_eq!(
            lexemes(source, &run(source).expect(source)),
            vec![source],
            "string {source:?}"
        );
    }

    let source = "\"a\\b\" == \"\\n\"";
    let tokens = run(source).expect("escaped strings in a comparison");
    assert_eq!(
        kinds(&tokens),
        vec![
            JqlTokenKind::String,
            JqlTokenKind::EqualOp,
            JqlTokenKind::String,
        ]
    );
    assert_eq!(lexemes(source, &tokens), vec!["\"a\\b\"", "==", "\"\\n\""]);
}

#[test]
fn escaped_quote_ends_the_string() {
    let source = "\"hi\\\" there";
    let tokens = run(source).expect("quote after backslash closes the string");
    assert_eq!(
        kinds(&tokens),
        vec![JqlTokenKind::String, JqlTokenKind::Identifier]
    );
    assert_eq!(lexemes(source, &tokens), vec!["\"hi\\\"", "there"]);
    assert_eq!(tokens[0].span, Span { start: 0, end: 5 });

    for source in ["\"\\\"\"", "\"a\\\"b\"", "\"abc\\"] {
        assert_invalid(source);
    }
}

#[test]
fn unescaped_controls_inside_strings_are_kept() {
    for source in ["\"a\nb\"", "\"a\rb\"", "\"\u{0000}\"", "\"\u{001F}\""] {
        assert_eq!(
            run(source),
            Ok(vec![tok(JqlTokenKind::String, 0, source.len())]),
            "string {source:?}"
        );
    }
}

#[test]
fn unicode_strings_keep_following_byte_spans() {
    let source = "\"é\"";
    assert_eq!(run(source), Ok(vec![tok(JqlTokenKind::String, 0, 4)]));

    let source = "\"🙂\"";
    assert_eq!(run(source), Ok(vec![tok(JqlTokenKind::String, 0, 6)]));

    let source = "\"é\" true";
    assert_eq!(
        run(source),
        Ok(vec![
            tok(JqlTokenKind::String, 0, 4),
            tok(JqlTokenKind::Boolean, 5, 9),
        ])
    );
}

#[test]
fn whitespace_other_than_space_is_an_error() {
    for source in ["\t", "\n", "\r", "true\tfalse", "true\nfalse"] {
        assert_invalid(source);
    }
}

#[test]
fn braces_colon_and_comma() {
    assert_eq!(run("{"), Ok(vec![tok(JqlTokenKind::LBrace, 0, 1)]));
    assert_eq!(run("}"), Ok(vec![tok(JqlTokenKind::RBrace, 0, 1)]));
    assert_eq!(run(":"), Ok(vec![tok(JqlTokenKind::Colon, 0, 1)]));
    assert_eq!(run(","), Ok(vec![tok(JqlTokenKind::Comma, 0, 1)]));
    assert_eq!(
        run("{ name: .title, pay: .money }"),
        Ok(vec![
            tok(JqlTokenKind::LBrace, 0, 1),
            tok(JqlTokenKind::Identifier, 2, 6),
            tok(JqlTokenKind::Colon, 6, 7),
            tok(JqlTokenKind::Dot, 8, 9),
            tok(JqlTokenKind::Identifier, 9, 14),
            tok(JqlTokenKind::Comma, 14, 15),
            tok(JqlTokenKind::Identifier, 16, 19),
            tok(JqlTokenKind::Colon, 19, 20),
            tok(JqlTokenKind::Dot, 21, 22),
            tok(JqlTokenKind::Identifier, 22, 27),
            tok(JqlTokenKind::RBrace, 28, 29),
        ])
    );
}

#[test]
fn access_paths_tokenize_into_structured_tokens() {
    assert_eq!(
        run(".person{name,age}"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 7),
            tok(JqlTokenKind::LBrace, 7, 8),
            tok(JqlTokenKind::Identifier, 8, 12),
            tok(JqlTokenKind::Comma, 12, 13),
            tok(JqlTokenKind::Identifier, 13, 16),
            tok(JqlTokenKind::RBrace, 16, 17),
        ])
    );
    assert_eq!(
        run(".jobs[1:3]"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 5),
            tok(JqlTokenKind::LBracket, 5, 6),
            tok(JqlTokenKind::Number, 6, 7),
            tok(JqlTokenKind::Colon, 7, 8),
            tok(JqlTokenKind::Number, 8, 9),
            tok(JqlTokenKind::RBracket, 9, 10),
        ])
    );
    assert_eq!(
        run(".jobs[0,2]"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 5),
            tok(JqlTokenKind::LBracket, 5, 6),
            tok(JqlTokenKind::Number, 6, 7),
            tok(JqlTokenKind::Comma, 7, 8),
            tok(JqlTokenKind::Number, 8, 9),
            tok(JqlTokenKind::RBracket, 9, 10),
        ])
    );
}

#[test]
fn access_paths_tokenize_quoted_keys_and_wide_projection() {
    assert_eq!(
        run(".\"kebab-case\""),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::String, 1, 13),
        ])
    );
    assert_eq!(
        run(".[\"x-y\"]"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::LBracket, 1, 2),
            tok(JqlTokenKind::String, 2, 7),
            tok(JqlTokenKind::RBracket, 7, 8),
        ])
    );
    assert_eq!(
        run("._id"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 4),
        ])
    );
    assert_eq!(
        run(".person{_id,name2}"),
        Ok(vec![
            tok(JqlTokenKind::Dot, 0, 1),
            tok(JqlTokenKind::Identifier, 1, 7),
            tok(JqlTokenKind::LBrace, 7, 8),
            tok(JqlTokenKind::Identifier, 8, 11),
            tok(JqlTokenKind::Comma, 11, 12),
            tok(JqlTokenKind::Identifier, 12, 17),
            tok(JqlTokenKind::RBrace, 17, 18),
        ])
    );
}

#[test]
fn arithmetic_operators() {
    let cases = [
        ("+", JqlTokenKind::Plus),
        ("-", JqlTokenKind::Subtract),
        ("*", JqlTokenKind::Multiply),
        ("/", JqlTokenKind::Divide),
        ("%", JqlTokenKind::Mod),
    ];
    for (source, kind) in cases {
        assert_eq!(
            run(source),
            Ok(vec![tok(kind, 0, source.len())]),
            "operator {source:?}"
        );
    }

    assert_eq!(
        run("1 + 2 - 3 * 4 / 5 % 6"),
        Ok(vec![
            tok(JqlTokenKind::Number, 0, 1),
            tok(JqlTokenKind::Plus, 2, 3),
            tok(JqlTokenKind::Number, 4, 5),
            tok(JqlTokenKind::Subtract, 6, 7),
            tok(JqlTokenKind::Number, 8, 9),
            tok(JqlTokenKind::Multiply, 10, 11),
            tok(JqlTokenKind::Number, 12, 13),
            tok(JqlTokenKind::Divide, 14, 15),
            tok(JqlTokenKind::Number, 16, 17),
            tok(JqlTokenKind::Mod, 18, 19),
            tok(JqlTokenKind::Number, 20, 21),
        ])
    );
}

#[test]
fn unknown_characters_are_errors() {
    for source in ["@", "#", "'", "é"] {
        assert_invalid(source);
    }
}

#[test]
fn brackets_are_tokens() {
    assert_eq!(run("["), Ok(vec![tok(JqlTokenKind::LBracket, 0, 1)]));
    assert_eq!(run("]"), Ok(vec![tok(JqlTokenKind::RBracket, 0, 1)]));
    assert_eq!(
        run("[0]"),
        Ok(vec![
            tok(JqlTokenKind::LBracket, 0, 1),
            tok(JqlTokenKind::Number, 1, 2),
            tok(JqlTokenKind::RBracket, 2, 3),
        ])
    );
}

#[test]
fn unterminated_strings_are_errors() {
    for source in ["\"", "\"abc", "\"a\\\"b\""] {
        assert_invalid(source);
    }
}

#[test]
fn invalid_input_does_not_return_a_prefix() {
    for source in ["abc@def", "true @", ".id @", "filter(@"] {
        assert_invalid(source);
    }
}

#[test]
fn readme_pipeline() {
    let source = ".[1] | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))";
    let tokens = run(source).expect("pipeline");
    assert_eq!(
        kinds(&tokens),
        vec![
            JqlTokenKind::Dot,
            JqlTokenKind::LBracket,
            JqlTokenKind::Number,
            JqlTokenKind::RBracket,
            JqlTokenKind::Pipe,
            JqlTokenKind::Identifier,
            JqlTokenKind::LParen,
            JqlTokenKind::Dot,
            JqlTokenKind::Identifier,
            JqlTokenKind::GreaterOp,
            JqlTokenKind::Number,
            JqlTokenKind::AndLogicalOp,
            JqlTokenKind::Dot,
            JqlTokenKind::Identifier,
            JqlTokenKind::LessOp,
            JqlTokenKind::Number,
            JqlTokenKind::AndLogicalOp,
            JqlTokenKind::LParen,
            JqlTokenKind::Dot,
            JqlTokenKind::Identifier,
            JqlTokenKind::GreaterOp,
            JqlTokenKind::Number,
            JqlTokenKind::OrLogicalOp,
            JqlTokenKind::Dot,
            JqlTokenKind::Identifier,
            JqlTokenKind::GreaterEqualOp,
            JqlTokenKind::Number,
            JqlTokenKind::RParen,
            JqlTokenKind::RParen,
        ]
    );
    let text: Vec<&str> = tokens
        .iter()
        .map(|token| &source[token.span.start..token.span.end])
        .collect();
    assert_eq!(
        text,
        vec![
            ".", "[", "1", "]", "|", "filter", "(", ".", "id", ">", "10", "&&", ".", "age", "<",
            "20", "&&", "(", ".", "money", ">", "10", "||", ".", "gold", ">=", "1", ")", ")",
        ]
    );
    assert_eq!(tokens[0].span, Span { start: 0, end: 1 });
    assert_eq!(tokens[4].span, Span { start: 5, end: 6 });
    assert_eq!(tokens.last().unwrap().span, Span { start: 66, end: 67 });
}
