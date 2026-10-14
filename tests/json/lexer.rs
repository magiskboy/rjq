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
fn structural_characters_are_not_tokens() {
    // Punctuation is expected as raw bytes by the parser, not lexed.
    let input: [&str; 6] = ["{", "}", "[", "]", ":", ","];
    for s in input {
        assert_eq!(run(s), Ok(vec![]), "structural {:?}", s);
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
    // Structural bytes are skipped by tokenize; value spans stay byte-accurate.
    assert_kinds_spans(
        "\"é\",1",
        &[(JsonTokenKind::String, 0, 4), (JsonTokenKind::Number, 5, 6)],
    );

    assert_kinds_spans(
        "{\"a\":\"é\"}",
        &[(JsonTokenKind::String, 1, 4), (JsonTokenKind::String, 5, 9)],
    );

    assert_kinds_spans("[\"🙂\"]", &[(JsonTokenKind::String, 1, 7)]);
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
    let cases: [&str; 3] = ["10,", "10]", "10}"];
    for s in cases {
        let tokens = run(s).expect("should lex");
        assert_eq!(
            kinds(&tokens),
            vec![JsonTokenKind::Number],
            "source {:?}",
            s
        );
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
            JsonTokenKind::Number,
            JsonTokenKind::Number,
            JsonTokenKind::True,
            JsonTokenKind::False,
            JsonTokenKind::Null,
            JsonTokenKind::String,
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
            JsonTokenKind::String,
            JsonTokenKind::Number,
            JsonTokenKind::String,
            JsonTokenKind::True,
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
            JsonTokenKind::String,
            JsonTokenKind::Number,
            JsonTokenKind::String,
            JsonTokenKind::Null,
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
