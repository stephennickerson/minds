use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

pub(crate) fn required_string(arguments: &Value, key: &str) -> Result<String> {
    optional_string(arguments, key).ok_or_else(|| anyhow!("missing string argument: {key}"))
}

pub(crate) fn optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn optional_bool(arguments: &Value, key: &str, fallback: bool) -> bool {
    arguments
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

pub(crate) fn optional_u64(arguments: &Value, key: &str, fallback: u64) -> u64 {
    arguments
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(fallback)
}

pub(crate) fn optional_value(arguments: &Value, key: &str) -> Option<Value> {
    arguments.get(key).filter(|value| !value.is_null()).cloned()
}

pub(crate) fn optional_strings(arguments: &Value, key: &str) -> Option<Vec<String>> {
    arguments.get(key).and_then(strings_from_value)
}

pub(crate) fn insert_optional_string(
    object: &mut Map<String, Value>,
    name: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        object.insert(name.to_string(), json!(value));
    }
}

pub(crate) fn insert_optional_strings(
    object: &mut Map<String, Value>,
    name: &str,
    value: Option<Vec<String>>,
) {
    if let Some(value) = value {
        object.insert(name.to_string(), json!(value));
    }
}

fn strings_from_value(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()
        .map(|values| strings_from_array(values))
        .or_else(|| string_from_value(value))
}

fn strings_from_array(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn string_from_value(value: &Value) -> Option<Vec<String>> {
    value.as_str().map(|text| vec![text.to_string()])
}
