use super::Engine;
use crate::jql::error::{JqlError, JqlErrorKind};
use crate::jql::fixture::{at, sample};
use crate::jql::jql_parse;
use crate::jql::proxy::Proxy;
use crate::json::value::JsonValue;

fn execute(data: &JsonValue<'_>, query: &str) -> Result<JsonValue<'static>, JqlError> {
    let ast = jql_parse(query)?;
    Engine::execute(Proxy::new(data), &ast).map(|p| p.into_value().into_owned())
}

fn assert_ok(data: &JsonValue<'_>, query: &str, expected: JsonValue<'static>) {
    let actual =
        execute(data, query).unwrap_or_else(|e| panic!("query {query:?} should succeed: {e}"));
    assert_eq!(actual, expected, "query {query:?}");
}

fn assert_err(data: &JsonValue<'_>, query: &str) {
    assert!(execute(data, query).is_err(), "query {query:?} should fail");
}

fn assert_exec_err(data: &JsonValue<'_>, query: &str) {
    let err = execute(data, query).expect_err(&format!("query {query:?} should fail"));
    assert!(
        matches!(
            err.kind,
            JqlErrorKind::TypeMismatch { .. }
                | JqlErrorKind::PathNotFound { .. }
                | JqlErrorKind::UnsupportedOperation { .. }
                | JqlErrorKind::Json(_)
                | JqlErrorKind::InvalidAccess { .. }
        ),
        "query {query:?} should be an execution error, got {err}"
    );
}

fn bool_cases(data: &JsonValue<'_>, cases: &[(&str, bool)]) {
    for &(query, expected) in cases {
        assert_ok(data, query, JsonValue::boolean(expected));
    }
}

#[test]
fn access_root() {
    let data = sample();
    assert_ok(&data, ".", data.clone());
}

#[test]
fn access_property() {
    let data = sample();
    assert_ok(&data, ".count", JsonValue::number(42.0));
    assert_ok(&data, ".name", JsonValue::string("Alice".to_string()));
    assert_ok(&data, ".flag", JsonValue::True);
    assert_ok(&data, ".off", JsonValue::False);
    assert_ok(&data, ".house", JsonValue::Null);
}

#[test]
fn access_array_index() {
    let data = sample();
    assert_ok(&data, ".jobs[1]", at(".jobs[1]"));
    assert_ok(
        &data,
        ".jobs[0].title",
        JsonValue::string("Dev".to_string()),
    );
    assert_ok(&data, ".jobs[3].id", JsonValue::number(15.0));
}

#[test]
fn access_nested_array_index() {
    let data = sample();
    assert_ok(&data, ".matrix[0][1]", JsonValue::number(2.0));
    assert_ok(&data, ".matrix[1][0]", JsonValue::number(4.0));
    assert_ok(&data, ".matrix[2][2]", JsonValue::number(9.0));
    assert_ok(&data, ".nested[1][0]", JsonValue::number(2.0));
}

#[test]
fn access_deeply_nested_array_index() {
    let data = sample();
    assert_ok(&data, ".nested[1][1][0]", JsonValue::number(3.0));
    assert_ok(
        &data,
        ".matrix[0][1:3]",
        JsonValue::array(vec![JsonValue::number(2.0), JsonValue::number(3.0)]),
    );
    assert_ok(
        &data,
        ".matrix[1][0,2]",
        JsonValue::array(vec![JsonValue::number(4.0), JsonValue::number(6.0)]),
    );
    assert_ok(
        &data,
        ".jobs[0,2]",
        JsonValue::array(vec![at(".jobs[0]"), at(".jobs[2]")]),
    );
    assert_ok(
        &data,
        ".jobs[1:3]",
        JsonValue::array(vec![at(".jobs[1]"), at(".jobs[2]")]),
    );
}

#[test]
fn access_root_nested_array_index() {
    let data = at(".matrix");
    assert_ok(&data, ".[0][1]", JsonValue::number(2.0));
    assert_ok(&data, ".[1][0]", JsonValue::number(4.0));
    assert_ok(&data, ".[2][1]", JsonValue::number(8.0));
}

#[test]
fn access_nested_array_then_property() {
    let data = sample();
    assert_ok(
        &data,
        ".rows[0][0].name",
        JsonValue::string("a".to_string()),
    );
    assert_ok(&data, ".rows[1][0].value", JsonValue::number(2.0));
    assert_ok(
        &data,
        ".company.teams[0].name",
        JsonValue::string("platform".to_string()),
    );
    assert_ok(
        &data,
        ".company.meta.region",
        JsonValue::string("APAC".to_string()),
    );
}

#[test]
fn consecutive_brackets_without_dot_are_accepted() {
    let data = sample();
    assert_ok(&data, ".matrix[0][1]", JsonValue::number(2.0));
    assert_ok(&data, ".nested[1][0]", JsonValue::number(2.0));
    assert_ok(
        &data,
        ".rows[0][0].name",
        JsonValue::string("a".to_string()),
    );

    let root = at(".matrix");
    assert_ok(&root, ".[0][1]", JsonValue::number(2.0));
    assert_ok(&root, ".[2][0]", JsonValue::number(7.0));
}

#[test]
fn pipe_chains_access() {
    let data = sample();
    assert_ok(&data, ".matrix[0] | .[1]", JsonValue::number(2.0));
    assert_ok(&data, ".nested[1] | .[1] | .[0]", JsonValue::number(3.0));
    assert_ok(
        &data,
        ".jobs | .[0].title",
        JsonValue::string("Dev".to_string()),
    );
    assert_ok(
        &data,
        ".company | .teams[1] | .name",
        JsonValue::string("data".to_string()),
    );
    assert_ok(
        &data,
        ".people[2] | .tags[0]",
        JsonValue::string("a".to_string()),
    );
}

#[test]
fn missing_property_is_execution_error() {
    let data = sample();
    let err = execute(&data, ".missing").expect_err("should fail");
    assert_eq!(
        err.kind,
        JqlErrorKind::PathNotFound {
            path: "missing".to_string()
        }
    );
}

#[test]
fn literals_return_json_values() {
    let data = sample();
    assert_ok(&data, "null", JsonValue::Null);
    assert_ok(&data, "true", JsonValue::True);
    assert_ok(&data, "false", JsonValue::False);
    assert_ok(&data, "42", JsonValue::number(42.0));
    assert_ok(&data, "-1", JsonValue::number(-1.0));
    assert_ok(&data, "3.14", JsonValue::number(3.14));
    assert_ok(&data, "1e2", JsonValue::number(100.0));
    assert_ok(&data, "1.2e-3", JsonValue::number(0.0012));
    assert_ok(&data, "\"hi\"", JsonValue::string("hi".to_string()));
}

#[test]
fn access_quoted_and_bracket_string_keys() {
    let data = JsonValue::object([
        ("kebab-case", JsonValue::number(1.0)),
        ("x-y", JsonValue::string("ok".to_string())),
        ("with space", JsonValue::True),
        (
            "meta",
            JsonValue::object([
                ("_id", JsonValue::number(7.0)),
                ("api-version", JsonValue::string("v2".to_string())),
                ("0", JsonValue::number(0.0)),
            ]),
        ),
    ]);
    assert_ok(&data, ".\"kebab-case\"", JsonValue::number(1.0));
    assert_ok(&data, ".[\"x-y\"]", JsonValue::string("ok".to_string()));
    assert_ok(&data, ".\"with space\"", JsonValue::True);
    assert_ok(&data, ".meta._id", JsonValue::number(7.0));
    assert_ok(
        &data,
        ".meta[\"api-version\"]",
        JsonValue::string("v2".to_string()),
    );
    assert_ok(
        &data,
        ".meta{\"_id\",\"api-version\",0}",
        JsonValue::object([
            ("_id", JsonValue::number(7.0)),
            ("api-version", JsonValue::string("v2".to_string())),
            ("0", JsonValue::number(0.0)),
        ]),
    );
}

#[test]
fn unsupported_call_is_execution_error() {
    let data = sample();
    let err = execute(&data, "unknown(.count)").expect_err("unknown call should fail");
    assert_eq!(
        err.kind,
        JqlErrorKind::UnsupportedOperation {
            op: "unknown".to_string()
        }
    );
}

#[test]
fn compares_numbers_from_literals_and_fields() {
    let data = sample();
    bool_cases(
        &data,
        &[
            ("1 < 2", true),
            ("2 < 1", false),
            ("2 > 1", true),
            ("1 > 2", false),
            ("2 <= 2", true),
            ("2 <= 1", false),
            ("2 >= 2", true),
            ("1 >= 2", false),
            (".count > 41", true),
            (".count > 42", false),
            (".count < 43", true),
            (".count < 42", false),
            (".count >= 42", true),
            (".count <= 42", true),
            (".count >= 43", false),
            (".count <= 41", false),
            (".matrix[0][0] < .matrix[0][1]", true),
            (".matrix[1][0] > .matrix[0][2]", true),
            (".scalars.negative < .scalars.zero", true),
            (".scalars.fraction > 10", true),
            (".jobs[0].money > .jobs[1].money", true),
            (".company.meta.tier == 1", true),
        ],
    );
}

#[test]
fn equality_is_structural_across_json_values() {
    let data = at(".eq");
    bool_cases(
        &data,
        &[
            ("1 == 1", true),
            ("1 == 2", false),
            ("1 != 2", true),
            ("true == true", true),
            ("true == false", false),
            ("false != true", true),
            ("null == null", true),
            ("null == false", false),
            ("\"x\" == \"x\"", true),
            ("\"x\" == \"y\"", false),
            ("\"x\" != \"y\"", true),
            (".n == 1", true),
            (".s == \"x\"", true),
            (".flag == true", true),
            (".n == true", false),
            (".s == 1", false),
            (".flag == null", false),
            (".xs == .ys", true),
            (".xs != .zs", true),
            (".obj == .obj2", true),
            (".obj != .obj3", true),
            (".xs == .obj", false),
        ],
    );
}

#[test]
fn logical_ops_combine_boolean_results() {
    let data = sample();
    bool_cases(
        &data,
        &[
            ("true && true", true),
            ("true && false", false),
            ("false && true", false),
            ("false && false", false),
            ("true || true", true),
            ("true || false", true),
            ("false || true", true),
            ("false || false", false),
            (".count > 1 && .count < 100", true),
            (".count > 100 && .count == 42", false),
            (".count > 100 || .count == 42", true),
            (".count < 1 || .count > 100", false),
            ("(.count > 1 && .count < 100) || false", true),
            ("false || (.count == 42 && true)", true),
            ("true && false || true", true),
            ("false || false && true", false),
            (".flag && .off", false),
            (".flag || .off", true),
            (".jobs[0].active && .jobs[2].active", false),
            (".jobs[0].active || .jobs[2].active", true),
        ],
    );
}

#[test]
fn comparisons_bind_tighter_than_logical_ops_at_runtime() {
    let data = sample();
    // 1 > 2 && 3 < 4  => false && true => false
    assert_ok(&data, "1 > 2 && 3 < 4", JsonValue::False);
    // 1 < 2 || 3 > 4 => true || false => true
    assert_ok(&data, "1 < 2 || 3 > 4", JsonValue::True);
    // Parentheses override the default shape.
    assert_ok(&data, "(1 > 2 || 3 < 4) && true", JsonValue::True);
    assert_ok(&data, "1 > 2 || (3 < 4 && false)", JsonValue::False);
}

#[test]
fn binary_ops_work_after_pipe() {
    let data = sample();
    assert_ok(&data, ".count | . > 40", JsonValue::True);
    assert_ok(&data, ".count | . == 42", JsonValue::True);
    assert_ok(&data, ".jobs[0].title | . == \"Dev\"", JsonValue::True);
    assert_ok(&data, ".matrix[0] | .[1] > 1 && .[1] < 3", JsonValue::True);
    assert_ok(
        &data,
        ".people[1] | .vip == true && .score < 80",
        JsonValue::True,
    );
}

#[test]
fn filter_keeps_matching_array_items() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs | filter(.active == true)",
        JsonValue::array(vec![at(".jobs[0]"), at(".jobs[1]"), at(".jobs[3]")]),
    );
    assert_ok(
        &data,
        ".jobs | filter(.id > 2)",
        JsonValue::array(vec![at(".jobs[2]"), at(".jobs[3]")]),
    );
    assert_ok(
        &data,
        ".people | filter(.vip == true)",
        JsonValue::array(vec![at(".people[1]"), at(".people[2]")]),
    );
    assert_ok(&data, ".empty.arr | filter(.id > 0)", at(".empty.arr"));
}

#[test]
fn filter_supports_nested_logical_conditions() {
    let data = sample();
    // Readme-style predicate over candidates.
    assert_ok(
        &data,
        ".candidates | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))",
        JsonValue::array(vec![at(".candidates[1]"), at(".candidates[2]")]),
    );
    assert_ok(
        &data,
        ".jobs | filter((.level == \"senior\" || .level == \"mid\") && .money >= 50)",
        JsonValue::array(vec![at(".jobs[0]"), at(".jobs[1]")]),
    );
    assert_ok(
        &data,
        ".people | filter(.age >= 30 && (.score > 80 || .vip == true))",
        JsonValue::array(vec![at(".people[1]"), at(".people[2]")]),
    );
    assert_ok(
        &data,
        ".jobs | filter(.active == false || (.age < 20 && .gold >= 1))",
        JsonValue::array(vec![at(".jobs[2]"), at(".jobs[3]")]),
    );
}

#[test]
fn filter_composes_with_selection_and_pipes() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs[]{title,id} | filter(.id >= 3)",
        JsonValue::array(vec![
            JsonValue::object([
                ("title".to_string(), JsonValue::string("PM".to_string())),
                ("id".to_string(), JsonValue::number(3.0)),
            ]),
            JsonValue::object([
                ("title".to_string(), JsonValue::string("SRE".to_string())),
                ("id".to_string(), JsonValue::number(15.0)),
            ]),
        ]),
    );
    assert_ok(
        &data,
        ".company.teams | filter(.size > 5) | .[0].name",
        JsonValue::string("platform".to_string()),
    );
    assert_ok(
        &data,
        ".people | filter(.vip == true) | .[0].name",
        JsonValue::string("Carol".to_string()),
    );
    assert_ok(
        &data,
        ".matrix | filter(.[0] > 3) | .[0][1]",
        JsonValue::number(5.0),
    );
}

#[test]
fn filter_on_non_array_is_execution_error() {
    let data = sample();
    assert_exec_err(&data, ".count | filter(. > 0)");
    assert_exec_err(&data, ".person | filter(.age > 0)");
    assert_exec_err(&data, ".name | filter(. == \"Alice\")");
}

#[test]
fn map_transforms_array_items() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs | map(.title)",
        JsonValue::array(vec![
            JsonValue::string("Dev".to_string()),
            JsonValue::string("QA".to_string()),
            JsonValue::string("PM".to_string()),
            JsonValue::string("SRE".to_string()),
        ]),
    );
    assert_ok(
        &data,
        ".jobs | filter(.active == true) | map(.id)",
        JsonValue::array(vec![
            JsonValue::number(1.0),
            JsonValue::number(2.0),
            JsonValue::number(15.0),
        ]),
    );
    assert_ok(
        &data,
        ".matrix | map(.[0])",
        JsonValue::array(vec![
            JsonValue::number(1.0),
            JsonValue::number(4.0),
            JsonValue::number(7.0),
        ]),
    );
    assert_ok(&data, ".empty.arr | map(.id)", at(".empty.arr"));
}

#[test]
fn map_constructs_object_shapes() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs | filter(.id > 2) | map({ name: .title, pay: .money })",
        JsonValue::array(vec![
            JsonValue::object([
                ("name".to_string(), JsonValue::string("PM".to_string())),
                ("pay".to_string(), JsonValue::number(5.0)),
            ]),
            JsonValue::object([
                ("name".to_string(), JsonValue::string("SRE".to_string())),
                ("pay".to_string(), JsonValue::number(20.0)),
            ]),
        ]),
    );
    assert_ok(
        &data,
        ".company.teams | map({ name: .name, size: .size })",
        JsonValue::array(vec![
            JsonValue::object([
                (
                    "name".to_string(),
                    JsonValue::string("platform".to_string()),
                ),
                ("size".to_string(), JsonValue::number(8.0)),
            ]),
            JsonValue::object([
                ("name".to_string(), JsonValue::string("data".to_string())),
                ("size".to_string(), JsonValue::number(5.0)),
            ]),
        ]),
    );
}

#[test]
fn map_on_non_array_is_execution_error() {
    let data = sample();
    assert_exec_err(&data, ".count | map(. > 0)");
    assert_exec_err(&data, ".person | map(.age)");
    assert_exec_err(&data, ".name | map(.)");
}

#[test]
fn complex_pipeline_across_sample_document() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs[0,3] | filter(.active == true) | .[1].title",
        JsonValue::string("SRE".to_string()),
    );
    assert_ok(
        &data,
        ".candidates[1:3] | filter(.gold >= 1 || .money > 10) | .[0].id",
        JsonValue::number(15.0),
    );
    assert_ok(
        &data,
        ".rows[0][0] | .value == 1 && .name == \"a\"",
        JsonValue::True,
    );
    assert_ok(
        &data,
        ".eq | .xs == .ys && .obj == .obj2 && .flag == true",
        JsonValue::True,
    );
}

#[test]
fn ordered_compare_fails_when_either_side_is_not_a_number() {
    let data = sample();
    for query in [
        "true > false",
        "false < true",
        "\"a\" < \"b\"",
        "\"a\" >= \"a\"",
        "null <= 1",
        "1 <= null",
        "true >= 0",
        ".jobs > 1",
        "1 < .jobs",
        ".jobs[0].title > \"A\"",
        "true <= .count",
        ".count >= false",
    ] {
        assert_exec_err(&data, query);
    }
}

#[test]
fn logical_ops_fail_when_a_used_operand_is_not_boolean() {
    let data = sample();
    for query in [
        "true && 1",
        "1 && true",
        "false || 0",
        "null || false",
        "true && null",
        "\"x\" && true",
        "\"x\" || false",
        ".count && true",
        "false || .count",
        ".jobs || false",
        "true && .jobs[0]",
    ] {
        assert_exec_err(&data, query);
    }
}

#[test]
fn binary_ops_evaluate_both_operands() {
    let data = sample();
    // Even when the left side alone would decide a boolean result, a
    // failing right operand still fails the whole expression.
    assert_exec_err(&data, "true || .missing");
    assert_exec_err(&data, "false && .missing");
    assert_exec_err(&data, "1 < 2 && .missing");
    assert_exec_err(&data, ".missing || true");
    assert_exec_err(&data, ".missing && false");
    assert_exec_err(&data, ".missing == 1");
    assert_exec_err(&data, "1 == .missing");
    assert_exec_err(&data, ".missing < 1");
    assert_exec_err(&data, "1 < .missing");
}

#[test]
fn invalid_access_path_fails() {
    let data = sample();
    assert_err(&data, ".jobs[");
    assert_err(&data, ".matrix[0][");
}

#[test]
fn nullary_aggregates_act_on_current_value() {
    let data = sample();
    assert_ok(&data, ".jobs | len", JsonValue::number(4.0));
    assert_ok(&data, ".person | len", JsonValue::number(4.0));
    assert_ok(&data, ".name | len", JsonValue::number(5.0));
    assert_ok(&data, ".matrix[0] | sum", JsonValue::number(6.0));
    assert_ok(&data, ".matrix[0] | avg", JsonValue::number(2.0));
    assert_ok(&data, ".matrix[0] | min", JsonValue::number(1.0));
    assert_ok(&data, ".matrix[0] | max", JsonValue::number(3.0));
    assert_ok(&data, ".empty.arr | sum", JsonValue::number(0.0));
    assert_ok(
        &data,
        ".jobs | map(.money) | sum",
        JsonValue::number(175.0),
    );
    // Option A: path-arg form is rejected
    assert_exec_err(&data, "len(.jobs)");
    assert_exec_err(&data, "sum(.matrix[0])");
    assert_exec_err(&data, ".empty.arr | avg");
    assert_exec_err(&data, ".empty.arr | min");
}

#[test]
fn first_last_keys_values_type() {
    let data = sample();
    assert_ok(&data, ".jobs | first | .title", JsonValue::string("Dev".to_string()));
    assert_ok(&data, ".jobs | last | .title", JsonValue::string("SRE".to_string()));
    assert_ok(
        &data,
        ".person | keys",
        JsonValue::array(vec![
            JsonValue::string("name".to_string()),
            JsonValue::string("age".to_string()),
            JsonValue::string("city".to_string()),
            JsonValue::string("role".to_string()),
        ]),
    );
    assert_ok(
        &data,
        ".matrix[0] | keys",
        JsonValue::array(vec![
            JsonValue::number(0.0),
            JsonValue::number(1.0),
            JsonValue::number(2.0),
        ]),
    );
    assert_ok(
        &data,
        ".company.meta | values",
        JsonValue::array(vec![
            JsonValue::string("APAC".to_string()),
            JsonValue::number(1.0),
        ]),
    );
    assert_ok(&data, ".jobs | type", JsonValue::string("array".to_string()));
    assert_ok(&data, ".person | type", JsonValue::string("object".to_string()));
    assert_ok(&data, ".count | type", JsonValue::string("number".to_string()));
    assert_ok(&data, ".name | type", JsonValue::string("string".to_string()));
    assert_ok(&data, ".flag | type", JsonValue::string("boolean".to_string()));
    assert_ok(&data, ".house | type", JsonValue::string("null".to_string()));
    assert_exec_err(&data, ".jobs | values");
}

#[test]
fn reverse_flatten_unique_sort() {
    let data = sample();
    assert_ok(
        &data,
        ".matrix[0] | reverse",
        JsonValue::array(vec![
            JsonValue::number(3.0),
            JsonValue::number(2.0),
            JsonValue::number(1.0),
        ]),
    );
    assert_ok(
        &data,
        ".nested | flatten",
        JsonValue::array(vec![
            JsonValue::number(1.0),
            JsonValue::number(2.0),
            JsonValue::array(vec![JsonValue::number(3.0)]),
        ]),
    );
    assert_ok(
        &data,
        ".people[0].tags | unique",
        JsonValue::array(vec![
            JsonValue::string("a".to_string()),
            JsonValue::string("b".to_string()),
        ]),
    );
    // flatten then unique on duplicated tags across people
    assert_ok(
        &data,
        ".people | map(.tags) | flatten | unique",
        JsonValue::array(vec![
            JsonValue::string("a".to_string()),
            JsonValue::string("b".to_string()),
            JsonValue::string("c".to_string()),
        ]),
    );
    assert_ok(
        &data,
        ".matrix[0] | reverse | sort",
        JsonValue::array(vec![
            JsonValue::number(1.0),
            JsonValue::number(2.0),
            JsonValue::number(3.0),
        ]),
    );
    assert_ok(
        &data,
        ".jobs | map(.title) | sort",
        JsonValue::array(vec![
            JsonValue::string("Dev".to_string()),
            JsonValue::string("PM".to_string()),
            JsonValue::string("QA".to_string()),
            JsonValue::string("SRE".to_string()),
        ]),
    );
    assert_exec_err(&data, ".mixed | sort");
}

#[test]
fn any_all_find_has_contains() {
    let data = sample();
    assert_ok(&data, ".jobs | any(.active == true)", JsonValue::True);
    assert_ok(&data, ".jobs | any(.active == false)", JsonValue::True);
    assert_ok(&data, ".jobs | all(.id > 0)", JsonValue::True);
    assert_ok(&data, ".jobs | all(.active == true)", JsonValue::False);
    assert_ok(&data, ".empty.arr | any(.id > 0)", JsonValue::False);
    assert_ok(&data, ".empty.arr | all(.id > 0)", JsonValue::True);
    assert_ok(
        &data,
        ".jobs | find(.id == 3) | .title",
        JsonValue::string("PM".to_string()),
    );
    assert_ok(&data, ".jobs | find(.id == 999)", JsonValue::Null);
    assert_ok(&data, ".person | has(\"age\")", JsonValue::True);
    assert_ok(&data, ".person | has(\"missing\")", JsonValue::False);
    assert_ok(&data, ".people[0].tags | contains(\"a\")", JsonValue::True);
    assert_ok(&data, ".people[0].tags | contains(\"z\")", JsonValue::False);
    assert_ok(&data, ".name | contains(\"lic\")", JsonValue::True);
    assert_ok(&data, ".name | contains(\"bob\")", JsonValue::False);
    assert_exec_err(&data, ".count | has(\"x\")");
    assert_exec_err(&data, ".count | contains(1)");
}

#[test]
fn not_abs_empty() {
    let data = sample();
    assert_ok(&data, ".flag | not", JsonValue::False);
    assert_ok(&data, ".off | not", JsonValue::True);
    assert_ok(&data, ".scalars.negative | abs", JsonValue::number(10.0));
    assert_ok(&data, ".count | abs", JsonValue::number(42.0));
    assert_ok(&data, ".empty.arr | empty", JsonValue::True);
    assert_ok(&data, ".empty.obj | empty", JsonValue::True);
    assert_ok(&data, ".jobs | empty", JsonValue::False);
    assert_ok(&data, ".name | empty", JsonValue::False);
    assert_exec_err(&data, ".count | not");
    assert_exec_err(&data, ".name | abs");
    assert_exec_err(&data, ".count | empty");
}

#[test]
fn join_split_startswith_endswith() {
    let data = sample();
    assert_ok(
        &data,
        ".jobs | map(.title) | join(\",\")",
        JsonValue::string("Dev,QA,PM,SRE".to_string()),
    );
    assert_ok(
        &data,
        ".people[0].tags | join(\"-\")",
        JsonValue::string("a-b".to_string()),
    );
    assert_ok(
        &data,
        ".label | split(\"o\")",
        JsonValue::array(vec![
            JsonValue::string("r".to_string()),
            JsonValue::string("".to_string()),
            JsonValue::string("t".to_string()),
        ]),
    );
    assert_ok(
        &data,
        "\"a,b,c\" | split(\",\")",
        JsonValue::array(vec![
            JsonValue::string("a".to_string()),
            JsonValue::string("b".to_string()),
            JsonValue::string("c".to_string()),
        ]),
    );
    assert_ok(
        &data,
        "\"ab\" | split(\"\")",
        JsonValue::array(vec![
            JsonValue::string("a".to_string()),
            JsonValue::string("b".to_string()),
        ]),
    );
    assert_ok(&data, ".jobs[0].title | startswith(\"D\")", JsonValue::True);
    assert_ok(&data, ".jobs[0].title | startswith(\"Q\")", JsonValue::False);
    assert_ok(&data, ".jobs[1].title | endswith(\"A\")", JsonValue::True);
    assert_ok(&data, ".jobs[1].title | endswith(\"v\")", JsonValue::False);
    assert_ok(
        &data,
        ".jobs | map(.title) | join(\",\") | split(\",\") | first | endswith(\"ev\")",
        JsonValue::True,
    );
    assert_exec_err(&data, ".matrix[0] | join(\",\")");
    assert_exec_err(&data, ".count | split(\",\")");
    assert_exec_err(&data, ".count | startswith(\"1\")");
    assert_exec_err(&data, ".name | endswith(1)");
}
