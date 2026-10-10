use super::{Access, ArrayIndex, PropertySelection, Reference};
use crate::jql::error::JqlErrorKind;
use std::str::FromStr;

fn parse(source: &str) -> Reference {
    Reference::from_str(source).unwrap_or_else(|err| panic!("should parse {source:?}: {err}"))
}

fn parse_err(source: &str) {
    let err = Reference::from_str(source).expect_err(&format!("should reject {source:?}"));
    assert!(
        matches!(
            err.kind,
            JqlErrorKind::InvalidAccess { ref path } if path == source
        ),
        "source {source:?}, got {:?}",
        err.kind
    );
}

fn root() -> Access {
    Access::PropertyAccess {
        property: String::new(),
        properties: PropertySelection::All,
    }
}

fn prop(name: &str) -> Access {
    Access::PropertyAccess {
        property: name.to_string(),
        properties: PropertySelection::All,
    }
}

fn prop_select(name: &str, keys: &[&str]) -> Access {
    Access::PropertyAccess {
        property: name.to_string(),
        properties: PropertySelection::Properties(
            keys.iter().map(|k| (*k).to_string()).collect(),
        ),
    }
}

fn index(index: ArrayIndex) -> Access {
    Access::IndexAccess {
        index,
        properties: PropertySelection::All,
    }
}

fn index_select(index: ArrayIndex, keys: &[&str]) -> Access {
    Access::IndexAccess {
        index,
        properties: PropertySelection::Properties(
            keys.iter().map(|k| (*k).to_string()).collect(),
        ),
    }
}

#[test]
fn property_only() {
    // .jobs -> List<Job>
    assert_eq!(
        parse(".jobs"),
        Reference {
            access: vec![root(), prop("jobs")],
        }
    );
}

#[test]
fn root_ref() {
    // . -> identity over the document
    assert_eq!(
        parse("."),
        Reference {
            access: vec![root()],
        }
    );
}

#[test]
fn root_with_extra_ref() {
    assert_eq!(
        parse(".{name,age}"),
        Reference {
            access: vec![Access::PropertyAccess {
                property: String::new(),
                properties: PropertySelection::Properties(vec![
                    String::from("name"),
                    String::from("age"),
                ]),
            }],
        }
    );
}

#[test]
fn array_index() {
    // .jobs[1] -> Job
    assert_eq!(
        parse(".jobs[1]"),
        Reference {
            access: vec![root(), prop("jobs"), index(ArrayIndex::Index(1))],
        }
    );
}

#[test]
fn array_index_zero() {
    assert_eq!(
        parse(".jobs[0]"),
        Reference {
            access: vec![root(), prop("jobs"), index(ArrayIndex::Index(0))],
        }
    );
    assert_eq!(
        parse(".jobs[10]"),
        Reference {
            access: vec![root(), prop("jobs"), index(ArrayIndex::Index(10))],
        }
    );
}

#[test]
fn array_all() {
    // .jobs[] -> Iterator<Job>
    assert_eq!(
        parse(".jobs[]"),
        Reference {
            access: vec![root(), prop("jobs"), index(ArrayIndex::All)],
        }
    );
}

#[test]
fn array_multiple_index() {
    // .jobs[1,2] -> Iterator<Job>
    assert_eq!(
        parse(".jobs[1,2]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::MultipleIndex(vec![1, 2])),
            ],
        }
    );
}

#[test]
fn array_multiple_index_with_zero() {
    assert_eq!(
        parse(".jobs[0,2]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::MultipleIndex(vec![0, 2])),
            ],
        }
    );
    assert_eq!(
        parse(".jobs[0,1,2]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::MultipleIndex(vec![0, 1, 2])),
            ],
        }
    );
}

#[test]
fn array_range() {
    // .jobs[1:3] -> Iterator<Job>
    assert_eq!(
        parse(".jobs[1:3]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::Range { start: 1, end: 3 }),
            ],
        }
    );
}

#[test]
fn array_range_with_zero() {
    assert_eq!(
        parse(".jobs[0:1]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::Range { start: 0, end: 1 }),
            ],
        }
    );
    assert_eq!(
        parse(".jobs[0:0]"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index(ArrayIndex::Range { start: 0, end: 0 }),
            ],
        }
    );
}

#[test]
fn chained_property_with_zero_index() {
    assert_eq!(
        parse(".company.teams[0].name"),
        Reference {
            access: vec![
                root(),
                prop("company"),
                prop("teams"),
                index(ArrayIndex::Index(0)),
                prop("name"),
            ],
        }
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
        "jobs",
        ".jobs]",
    ] {
        parse_err(source);
    }
}

#[test]
fn rejects_invalid_character_instead_of_skipping() {
    // Previously invalid chars were skipped; ensure they hard-fail now.
    parse_err(".jobs[x]");
    parse_err(".count[0a]");
    parse_err(".jobs[1;2]");
}

#[test]
fn object_all() {
    // .person{} -> Iterator<Pair>
    assert_eq!(
        parse(".person{}"),
        Reference {
            access: vec![root(), prop("person")],
        }
    );
}

#[test]
fn object_multiple_properties() {
    // .person{name,age} -> Iterator<Pair if Pair[0] in [name, age]>
    assert_eq!(
        parse(".person{name,age}"),
        Reference {
            access: vec![root(), prop_select("person", &["name", "age"])],
        }
    );
}

#[test]
fn array_all_then_object_all() {
    // .jobs[]{} -> Iterator<Job>
    assert_eq!(
        parse(".jobs[]{}"),
        Reference {
            access: vec![root(), prop("jobs"), index(ArrayIndex::All)],
        }
    );
}

#[test]
fn array_all_then_object_multiple_properties() {
    // .jobs[]{title,description} -> Iterator<Pair if Pair[0] in [title, description]>
    assert_eq!(
        parse(".jobs[]{title,description}"),
        Reference {
            access: vec![
                root(),
                prop("jobs"),
                index_select(ArrayIndex::All, &["title", "description"]),
            ],
        }
    );
}
