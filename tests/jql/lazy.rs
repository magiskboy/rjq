//! Lazy builder correctness: same engine, eager DOM vs lazy DOM must match.

use crate::jql::{jql_execute, JqlExecuteArgs};
use crate::json::parser::JsonParser;
use crate::json::value::JsonValue;

fn assert_lazy_matches_eager(data: &[u8], query: &str) {
    let eager_dom = JsonParser::parse_slice(data).expect("eager parse");
    let eager = jql_execute(&eager_dom, query, JqlExecuteArgs::default()).expect("eager exec");
    let lazy = jql_execute(
        data,
        query,
        JqlExecuteArgs {
            lazy: true,
            ..Default::default()
        },
    )
    .expect("lazy exec");
    assert_eq!(
        lazy, eager,
        "lazy≠eager for query {query:?}\neager_dom={eager_dom:?}"
    );
}

#[test]
fn access_nested_path() {
    let data = br#"[{"profile":{"prefs":{"theme":"dark","notify":true},"email":"a"},"x":1},{"profile":{"prefs":{"theme":"light"}}}]"#;
    assert_lazy_matches_eager(data, ".[0].profile.prefs.theme");
}

#[test]
fn filter_selective_fields() {
    let data = br#"[{"active":true,"score":60,"name":"a","tags":[1,2]},{"active":false,"score":10,"name":"b"}]"#;
    assert_lazy_matches_eager(data, "filter(.active == true && .score > 50)");
}

#[test]
fn map_projection() {
    let data = br#"[{"id":1,"meta":{"city":"HN","level":1},"extra":9},{"id":2,"meta":{"city":"HCM","level":2}}]"#;
    assert_lazy_matches_eager(data, "map({ id: .id, city: .meta.city })");
}

#[test]
fn filter_then_map() {
    let data = br#"[{"active":true,"name":"a","meta":{"city":"HN"},"orders":[{"sku":"S1"},{"sku":"S2"}]},{"active":false,"name":"b","meta":{"city":"HCM"},"orders":[{"sku":"S3"}]}]"#;
    assert_lazy_matches_eager(
        data,
        r#"filter(.active == true) | map({ name: .name, city: .meta.city, sku: .orders[0].sku })"#,
    );
}

#[test]
fn trailing_junk_accepted_under_lazy() {
    let data = br#"{"a":1,"b":2} trailing junk"#;
    let lazy = jql_execute(
        data,
        ".a",
        JqlExecuteArgs {
            lazy: true,
            ..Default::default()
        },
    )
    .expect("lazy");
    assert_eq!(lazy, JsonValue::number(1.0));
}

#[test]
fn identity_is_full_need() {
    let data = br#"{"a":1,"b":[2,3]}"#;
    assert_lazy_matches_eager(data, ".");
}

#[test]
fn array_index_with_surplus_prefix() {
    let data = br#"[{"v":0},{"v":1},{"v":2,"keep":true},{"v":3}]"#;
    assert_lazy_matches_eager(data, ".[2].keep");
}
