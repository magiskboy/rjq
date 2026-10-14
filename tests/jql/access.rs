use crate::jql::ast::{AccessStep, ArrayIndex, JqlAstNode};
use crate::jql::error::JqlErrorKind;
use crate::jql::jql_parse;

fn parse(source: &str) -> Vec<AccessStep<'_>> {
    match jql_parse(source).unwrap_or_else(|err| panic!("should parse {source:?}: {err}")) {
        JqlAstNode::Access(steps) => steps,
        other => panic!("expected Access for {source:?}, got {other:?}"),
    }
}

fn parse_err(source: &str) {
    let err = jql_parse(source).expect_err(&format!("should reject {source:?}"));
    assert!(
        matches!(
            err.kind,
            JqlErrorKind::InvalidExpression
                | JqlErrorKind::UnexpectedToken { .. }
                | JqlErrorKind::InvalidToken
                | JqlErrorKind::UnclosedBrace
                | JqlErrorKind::InvalidAccess { .. }
        ),
        "source {source:?}, got {:?}",
        err.kind
    );
}

fn prop(name: &str) -> AccessStep<'_> {
    AccessStep::Property(name)
}

fn index(index: ArrayIndex) -> AccessStep<'static> {
    AccessStep::Index(index)
}

fn project<'a>(keys: &[&'a str]) -> AccessStep<'a> {
    AccessStep::Project(keys.to_vec())
}

#[test]
fn property_only() {
    assert_eq!(parse(".jobs"), vec![prop("jobs")]);
}

#[test]
fn root_ref() {
    assert_eq!(parse("."), Vec::<AccessStep<'_>>::new());
}

#[test]
fn root_projection() {
    assert_eq!(parse(".{name,age}"), vec![project(&["name", "age"])]);
}

#[test]
fn array_index() {
    assert_eq!(
        parse(".jobs[1]"),
        vec![prop("jobs"), index(ArrayIndex::Index(1))]
    );
}

#[test]
fn array_index_zero() {
    assert_eq!(
        parse(".jobs[0]"),
        vec![prop("jobs"), index(ArrayIndex::Index(0))]
    );
    assert_eq!(
        parse(".jobs[10]"),
        vec![prop("jobs"), index(ArrayIndex::Index(10))]
    );
}

#[test]
fn array_all() {
    assert_eq!(
        parse(".jobs[]"),
        vec![prop("jobs"), index(ArrayIndex::All)]
    );
}

#[test]
fn array_multiple_index() {
    assert_eq!(
        parse(".jobs[1,2]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::MultipleIndex(vec![1, 2])),
        ]
    );
}

#[test]
fn array_multiple_index_with_zero() {
    assert_eq!(
        parse(".jobs[0,2]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::MultipleIndex(vec![0, 2])),
        ]
    );
    assert_eq!(
        parse(".jobs[0,1,2]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::MultipleIndex(vec![0, 1, 2])),
        ]
    );
}

#[test]
fn array_range() {
    assert_eq!(
        parse(".jobs[1:3]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::Range { start: 1, end: 3 }),
        ]
    );
}

#[test]
fn array_range_with_zero() {
    assert_eq!(
        parse(".jobs[0:1]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::Range { start: 0, end: 1 }),
        ]
    );
    assert_eq!(
        parse(".jobs[0:0]"),
        vec![
            prop("jobs"),
            index(ArrayIndex::Range { start: 0, end: 0 }),
        ]
    );
}

#[test]
fn chained_property_with_zero_index() {
    assert_eq!(
        parse(".company.teams[0].name"),
        vec![
            prop("company"),
            prop("teams"),
            index(ArrayIndex::Index(0)),
            prop("name"),
        ]
    );
}

#[test]
fn rejects_invalid_array_syntax() {
    for source in [
        ".jobs[",
        ".jobs[1",
        ".jobs[1,",
        ".jobs[1:",
        ".jobs[1:2:3]",
        ".jobs[a]",
        ".jobs[-1]",
        ".jobs[1,,2]",
        ".jobs[1::2]",
        ".jobs]",
    ] {
        parse_err(source);
    }
}

#[test]
fn rejects_invalid_character_instead_of_skipping() {
    parse_err(".jobs[x]");
    parse_err(".count[0a]");
    parse_err(".jobs[1;2]");
}

#[test]
fn object_all_empty_projection_is_noop() {
    assert_eq!(parse(".person{}"), vec![prop("person")]);
}

#[test]
fn object_multiple_properties() {
    assert_eq!(
        parse(".person{name,age}"),
        vec![prop("person"), project(&["name", "age"])]
    );
}

#[test]
fn array_all_then_object_all() {
    assert_eq!(
        parse(".jobs[]{}"),
        vec![prop("jobs"), index(ArrayIndex::All)]
    );
}

#[test]
fn array_all_then_object_multiple_properties() {
    assert_eq!(
        parse(".jobs[]{title,description}"),
        vec![
            prop("jobs"),
            index(ArrayIndex::All),
            project(&["title", "description"]),
        ]
    );
}

#[test]
fn quoted_dot_property() {
    assert_eq!(parse(".\"kebab-case\""), vec![prop("kebab-case")]);
    assert_eq!(parse(".\"with space\""), vec![prop("with space")]);
    assert_eq!(parse(".\"café\""), vec![prop("café")]);
}

#[test]
fn bracket_string_property() {
    assert_eq!(parse(".[\"x-y\"]"), vec![prop("x-y")]);
    assert_eq!(
        parse(".meta[\"api-version\"]"),
        vec![prop("meta"), prop("api-version")]
    );
}

#[test]
fn underscore_property_and_wide_projection() {
    assert_eq!(parse("._id"), vec![prop("_id")]);
    assert_eq!(
        parse(".person{_id,name2,0}"),
        vec![prop("person"), project(&["_id", "name2", "0"])]
    );
    assert_eq!(
        parse(".person{\"_id\",\"a-b\"}"),
        vec![prop("person"), project(&["_id", "a-b"])]
    );
}

#[test]
fn rejects_unclosed_quoted_property() {
    parse_err(".\"kebab");
    parse_err(".[\"x-y\"");
    parse_err(".person{\"a-b}");
}

#[test]
fn rejects_projection_followed_by_index() {
    parse_err(".person{name}[0]");
}

#[test]
fn rejects_space_inside_access_chain() {
    parse_err(".jobs [1]");
}

#[test]
fn rejects_dotted_index_sugar() {
    parse_err(".matrix[0].[1]");
    parse_err(".[0].[1]");
}
