use std::{borrow::Cow, str::FromStr};

use crate::Structured;
use crate::jql::error::{JqlError, JqlErrorKind};
use crate::jql::reference::{Access, ArrayIndex};
use crate::jql::reference::{PropertySelection, Reference};
use crate::json::value::JsonValue;

#[derive(Debug, Clone)]
pub struct Proxy<'a> {
    data: Cow<'a, JsonValue<'a>>,
}

impl<'a> Proxy<'a> {
    pub fn new(data: &'a JsonValue<'a>) -> Self {
        Self {
            data: Cow::Borrowed(data),
        }
    }

    pub fn len(&self) -> Result<JsonValue<'a>, JqlError> {
        self.data
            .as_ref()
            .len()
            .map(|l| JsonValue::number(l as f64))
            .map_err(JqlError::from)
    }

    pub fn owned(data: JsonValue<'a>) -> Self {
        Self {
            data: Cow::Owned(data),
        }
    }

    pub fn data(&self) -> &JsonValue<'a> {
        self.data.as_ref()
    }

    fn type_mismatch(expected: &'static str, found: &JsonValue<'_>) -> JqlError {
        JqlError::without_span(JqlErrorKind::TypeMismatch {
            expected,
            found: match found {
                JsonValue::Null => "null",
                JsonValue::True | JsonValue::False => "boolean",
                JsonValue::Number(_) => "number",
                JsonValue::String(_) => "string",
                JsonValue::Array(_) => "array",
                JsonValue::Object(_) => "object",
            },
        })
    }

    fn slice(value: &JsonValue<'a>, start: usize, end: usize) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                if let Some(values) = items.get(start..end) {
                    Ok(JsonValue::array(values.to_vec()))
                } else {
                    Err(JqlError::without_span(JqlErrorKind::PathNotFound {
                        path: format!("[{start}:{end}] (len {})", items.len()),
                    }))
                }
            }
            other => Err(Self::type_mismatch("array", other)),
        }
    }

    fn select(value: &JsonValue<'a>, indices: Vec<usize>) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                let values = indices
                    .iter()
                    .map(|i| {
                        items.get(*i).cloned().ok_or_else(|| {
                            JqlError::without_span(JqlErrorKind::PathNotFound {
                                path: format!("[{i}] (len {})", items.len()),
                            })
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(JsonValue::array(values))
            }
            other => Err(Self::type_mismatch("array", other)),
        }
    }

    fn select_keys(value: &JsonValue<'a>, keys: &Vec<String>) -> Result<JsonValue<'a>, JqlError> {
        match value {
            JsonValue::Array(items) => {
                let values = items
                    .iter()
                    .map(|x| Self::select_keys(x, keys))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(JsonValue::array(values))
            }
            JsonValue::Object(m) => {
                let val = m.iter().filter_map(|(k, v)| {
                    if keys.iter().any(|key| key == k.as_ref()) {
                        Some((k.clone(), v.clone()))
                    } else {
                        None
                    }
                });
                Ok(JsonValue::object(val))
            }
            other => Err(Self::type_mismatch("object or array", other)),
        }
    }

    fn descend(current: Cow<'a, JsonValue<'a>>, key: &str) -> Result<Cow<'a, JsonValue<'a>>, JqlError> {
        match current {
            Cow::Borrowed(v) => Ok(Cow::Borrowed(v.get(key).map_err(JqlError::from)?)),
            Cow::Owned(v) => Ok(Cow::Owned(v.get(key).map_err(JqlError::from)?.clone())),
        }
    }

    fn apply_selection(
        current: Cow<'a, JsonValue<'a>>,
        properties: PropertySelection,
    ) -> Result<Cow<'a, JsonValue<'a>>, JqlError> {
        match properties {
            PropertySelection::All => Ok(current),
            PropertySelection::Properties(keys) => {
                Ok(Cow::Owned(Self::select_keys(current.as_ref(), &keys)?))
            }
        }
    }

    pub fn get(&self, ref_str: &str) -> Result<Proxy<'a>, JqlError> {
        let jref = Reference::from_str(ref_str)?;
        // Re-borrow root with lifetime `'a` when possible; only clone if already owned.
        let mut current = match &self.data {
            Cow::Borrowed(v) => Cow::Borrowed(*v),
            Cow::Owned(v) => Cow::Owned(v.clone()),
        };

        for access in jref.access {
            current = match access {
                Access::PropertyAccess {
                    property,
                    properties,
                } => {
                    // Empty name is the root ref: operate on the current value as-is.
                    let next = if property.is_empty() {
                        current
                    } else {
                        Self::descend(current, &property)?
                    };
                    Self::apply_selection(next, properties)?
                }
                Access::IndexAccess { index, properties } => {
                    let next = match index {
                        ArrayIndex::All => current,
                        ArrayIndex::Index(index) => Self::descend(current, &index.to_string())?,
                        ArrayIndex::Range { start, end } => {
                            Cow::Owned(Self::slice(current.as_ref(), start, end)?)
                        }
                        ArrayIndex::MultipleIndex(indices) => {
                            Cow::Owned(Self::select(current.as_ref(), indices)?)
                        }
                    };
                    Self::apply_selection(next, properties)?
                }
            };
        }

        Ok(Proxy { data: current })
    }
}

#[cfg(test)]
mod tests {
    use super::Proxy;
    use crate::jql::error::JqlError;
    use crate::jql::fixture::{at, sample};
    use crate::json::value::JsonValue;

    fn proxy_get(data: &JsonValue<'_>, ref_str: &str) -> Result<JsonValue<'static>, JqlError> {
        Proxy::new(data).get(ref_str).map(|p| p.data().clone().into_owned())
    }

    fn assert_get(data: &JsonValue<'_>, ref_str: &str, expected: JsonValue<'static>) {
        let actual =
            proxy_get(data, ref_str).unwrap_or_else(|_| panic!("ref {ref_str:?} should succeed"));
        assert_eq!(actual, expected, "ref {ref_str:?}");
    }

    fn assert_get_err(data: &JsonValue<'_>, ref_str: &str) {
        assert!(
            proxy_get(data, ref_str).is_err(),
            "ref {ref_str:?} should fail"
        );
    }

    fn project_keys(value: &JsonValue<'_>, keys: &[&str]) -> JsonValue<'static> {
        match value {
            JsonValue::Object(map) => JsonValue::object(
                keys.iter().filter_map(|k| {
                    map.get(*k).map(|v| {
                        (
                            std::borrow::Cow::Owned((*k).to_string()),
                            v.clone().into_owned(),
                        )
                    })
                }),
            ),
            JsonValue::Array(items) => {
                JsonValue::array(items.iter().map(|item| project_keys(item, keys)).collect())
            }
            other => other.clone().into_owned(),
        }
    }

    #[test]
    fn get_root_ref() {
        let data = sample();
        assert_get(&data, ".", data.clone());
    }

    #[test]
    fn get_simple_property() {
        let data = sample();
        assert_get(&data, ".jobs", at(".jobs"));
        assert_get(&data, ".count", JsonValue::number(42.0));
        assert_get(&data, ".label", JsonValue::string("root".to_string()));
        assert_get(&data, ".name", JsonValue::string("Alice".to_string()));
        assert_get(&data, ".flag", JsonValue::True);
        assert_get(&data, ".house", JsonValue::Null);
    }

    #[test]
    fn get_array_index() {
        let data = sample();
        assert_get(&data, ".jobs[1]", at(".jobs[1]"));
        assert_get(&data, ".jobs[0]", at(".jobs[0]"));
        assert_get(&data, ".jobs[2]", at(".jobs[2]"));
        assert_get(&data, ".jobs[3]", at(".jobs[3]"));
        assert_get(
            &data,
            ".people[0].name",
            JsonValue::string("Bob".to_string()),
        );
    }

    #[test]
    fn get_array_all_is_identity() {
        let data = sample();
        let jobs = at(".jobs");
        assert_get(&data, ".jobs[]", jobs.clone());
        assert_get(&data, ".jobs", jobs);
        assert_get(&data, ".people[]", at(".people"));
    }

    #[test]
    fn get_array_multiple_index() {
        let data = sample();
        assert_get(
            &data,
            ".jobs[1,2]",
            JsonValue::array(vec![at(".jobs[1]"), at(".jobs[2]")]),
        );
        assert_get(
            &data,
            ".jobs[0,2]",
            JsonValue::array(vec![at(".jobs[0]"), at(".jobs[2]")]),
        );
        assert_get(
            &data,
            ".jobs[0,1,3]",
            JsonValue::array(vec![at(".jobs[0]"), at(".jobs[1]"), at(".jobs[3]")]),
        );
        assert_get(
            &data,
            ".matrix[0,2]",
            JsonValue::array(vec![at(".matrix[0]"), at(".matrix[2]")]),
        );
    }

    #[test]
    fn get_array_range() {
        let data = sample();
        assert_get(
            &data,
            ".jobs[1:3]",
            JsonValue::array(vec![at(".jobs[1]"), at(".jobs[2]")]),
        );
        assert_get(&data, ".jobs[0:1]", JsonValue::array(vec![at(".jobs[0]")]));
        assert_get(
            &data,
            ".candidates[1:4]",
            JsonValue::array(vec![
                at(".candidates[1]"),
                at(".candidates[2]"),
                at(".candidates[3]"),
            ]),
        );
        assert_get(
            &data,
            ".matrix[0:2]",
            JsonValue::array(vec![at(".matrix[0]"), at(".matrix[1]")]),
        );
    }

    #[test]
    fn get_object_all_is_identity() {
        let data = sample();
        let person = at(".person");
        assert_get(&data, ".person{}", person.clone());
        assert_get(&data, ".person", person);
        assert_get(&data, ".company.meta{}", at(".company.meta"));
    }

    #[test]
    fn get_array_then_object_key_filter() {
        let data = sample();
        assert_get(
            &data,
            ".jobs[]{title,description}",
            project_keys(&at(".jobs"), &["title", "description"]),
        );
        assert_get(
            &data,
            ".jobs[]{title,id}",
            project_keys(&at(".jobs"), &["title", "id"]),
        );
        assert_get(
            &data,
            ".people[]{name,vip}",
            project_keys(&at(".people"), &["name", "vip"]),
        );
    }

    #[test]
    fn get_array_all_then_object_all() {
        let data = sample();
        assert_get(&data, ".jobs[]{}", at(".jobs"));
        assert_get(&data, ".people[]{}", at(".people"));
    }

    #[test]
    fn get_chained_properties() {
        let data = sample();
        assert_get(
            &data,
            ".company.teams[0].name",
            JsonValue::string("platform".to_string()),
        );
        assert_get(&data, ".company.teams[1].size", JsonValue::number(5.0));
        assert_get(
            &data,
            ".company.meta.region",
            JsonValue::string("APAC".to_string()),
        );
        assert_get(
            &data,
            ".rows[1][0].name",
            JsonValue::string("b".to_string()),
        );
        assert_get(&data, ".nested[1][1][0]", JsonValue::number(3.0));
        assert_get(&data, ".matrix[2][0]", JsonValue::number(7.0));
    }

    #[test]
    fn get_nested_index_selection_and_projection() {
        let data = sample();
        assert_get(
            &data,
            ".jobs[0,2]{title,active}",
            project_keys(
                &JsonValue::array(vec![at(".jobs[0]"), at(".jobs[2]")]),
                &["title", "active"],
            ),
        );
        assert_get(
            &data,
            ".jobs[1:3]{id,level}",
            project_keys(
                &JsonValue::array(vec![at(".jobs[1]"), at(".jobs[2]")]),
                &["id", "level"],
            ),
        );
        assert_get(
            &data,
            ".person{name,age}",
            project_keys(&at(".person"), &["name", "age"]),
        );
        assert_get(
            &data,
            ".eq{xs,ys,obj}",
            project_keys(&at(".eq"), &["xs", "ys", "obj"]),
        );
    }

    #[test]
    fn get_invalid_reference_string() {
        let data = sample();
        assert_get_err(&data, "jobs");
        assert_get_err(&data, ".jobs[");
        assert_get_err(&data, ".jobs[1");
        assert_get_err(&data, ".jobs[1:2:3]");
    }

    #[test]
    fn get_missing_property() {
        let data = sample();
        assert_get_err(&data, ".missing");
        assert_get_err(&data, ".jobs.missing");
        assert_get_err(&data, ".company.missing");
    }

    #[test]
    fn get_array_index_out_of_bounds() {
        let data = sample();
        assert_get_err(&data, ".jobs[9]");
        assert_get_err(&data, ".jobs[1,9]");
        assert_get_err(&data, ".matrix[3]");
    }

    #[test]
    fn get_array_ops_on_non_array() {
        let data = sample();
        assert_get_err(&data, ".count[0]");
        assert_get_err(&data, ".count[0:1]");
        assert_get_err(&data, ".count[0,1]");
        assert_get_err(&data, ".label[0]");
        assert_get_err(&data, ".person[0]");
    }

    #[test]
    fn get_object_key_filter_on_object() {
        let data = sample();
        let result = Proxy::new(&data)
            .get(".person{name,age,city}")
            .expect("should resolve reference");

        assert_eq!(
            result.data(),
            &project_keys(&at(".person"), &["name", "age", "city"])
        );
    }

    #[test]
    fn get_array_range_out_of_bounds() {
        let data = sample();
        assert_get_err(&data, ".jobs[2:9]");
        assert_get_err(&data, ".candidates[0:9]");
    }

    #[test]
    fn get_returns_owned_proxy_data() {
        let data = sample();
        let proxy = Proxy::new(&data);
        let result = proxy.get(".jobs[1]").expect("should resolve reference");
        assert_eq!(result.data(), &at(".jobs[1]"));
    }

    #[test]
    fn get_empty_containers() {
        let data = sample();
        assert_get(&data, ".empty.arr", JsonValue::array(vec![]));
        assert_get(&data, ".empty.obj", JsonValue::empty_object());
        assert_get(&data, ".empty.arr[]", JsonValue::array(vec![]));
    }
}
