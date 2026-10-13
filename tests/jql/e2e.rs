use super::{fixture, jql_execute, jql_parse};
use crate::json::value::JsonValue;

#[test]
fn sample_fixture_loads() {
    let data = fixture::sample();
    assert!(matches!(data, JsonValue::Object(_)));
    assert_eq!(fixture::at(".count"), JsonValue::number(42.0));
    assert_eq!(
        fixture::at(".jobs[0].title"),
        JsonValue::string("Dev".to_string())
    );
}

#[test]
fn end_to_end_access_pipeline_and_filter() {
    let data = fixture::sample();

    assert_eq!(
        jql_execute(&data, ".company.teams[0].name").unwrap(),
        JsonValue::string("platform".to_string())
    );
    assert_eq!(
        jql_execute(&data, ".matrix[1] | .[0,2]").unwrap(),
        JsonValue::array(vec![JsonValue::number(4.0), JsonValue::number(6.0)])
    );
    assert_eq!(
        jql_execute(
            &data,
            ".candidates | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))"
        )
        .unwrap(),
        JsonValue::array(vec![
            fixture::at(".candidates[1]"),
            fixture::at(".candidates[2]"),
        ])
    );
    assert_eq!(
        jql_execute(&data, ".jobs[]{title,id} | filter(.id > 2) | .[0].title").unwrap(),
        JsonValue::string("PM".to_string())
    );
}

#[test]
fn end_to_end_map_and_object_construct() {
    let data = fixture::sample();
    assert_eq!(
        jql_execute(&data, ".jobs | filter(.id > 2) | map({ name: .title, pay: .money })")
            .unwrap(),
        JsonValue::array(vec![
            JsonValue::object([
                ("name".to_string(), JsonValue::string("PM".to_string())),
                ("pay".to_string(), JsonValue::number(5.0)),
            ]),
            JsonValue::object([
                ("name".to_string(), JsonValue::string("SRE".to_string())),
                ("pay".to_string(), JsonValue::number(20.0)),
            ]),
        ])
    );
}

#[test]
fn end_to_end_parse_then_execute_matches_direct_execute() {
    let data = fixture::sample();
    let query = ".people | filter(.vip == true && .score >= 70) | .[1].name";
    let ast = jql_parse(query).expect("parse");
    let via_ast =
        crate::jql::engine::Engine::execute(crate::jql::proxy::Proxy::new(&data), &ast)
            .unwrap()
            .into_value();
    assert_eq!(via_ast, jql_execute(&data, query).unwrap());
    assert_eq!(via_ast, JsonValue::string("Dave".to_string()));
}
