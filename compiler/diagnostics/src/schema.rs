//! The JSON Schema for one diagnostic, and a validator for the subset of JSON Schema it uses.
//!
//! The validator supports `type`, `properties`, `required`, `additionalProperties: false`,
//! `items`, `minItems`, `maxItems`, `minimum`, `enum` and local `$ref`s into `$defs`. It skips
//! `pattern`, which `Code` checks instead; a test checks that the schema's code pattern is the
//! one `Code` enforces. Any other keyword is an error, so the schema can't start depending on a
//! feature that nothing checks.

use serde_json::Value;

/// `compiler/diagnostics/schema/diagnostic.schema.json`.
pub const DIAGNOSTIC_SCHEMA: &str = include_str!("../schema/diagnostic.schema.json");

const ANNOTATIONS: &[&str] = &[
    "$schema",
    "$id",
    "$comment",
    "title",
    "description",
    "$defs",
];

/// Checks `instance` against `schema`, returning every violation as `<path>: <problem>`.
pub fn validate(schema: &Value, instance: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    check(schema, schema, instance, "$", &mut errors);
    errors
}

fn check(root: &Value, schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(rules) = schema.as_object() else {
        errors.push(format!("{path}: schema is not an object"));
        return;
    };
    if let Some(reference) = rules.get("$ref") {
        match resolve(root, reference) {
            Some(target) => check(root, target, value, path, errors),
            None => errors.push(format!("{path}: unresolvable $ref {reference}")),
        }
        return;
    }
    for (keyword, rule) in rules {
        match keyword.as_str() {
            "type" => {
                let ok = match rule.as_str() {
                    Some("object") => Some(value.is_object()),
                    Some("array") => Some(value.is_array()),
                    Some("string") => Some(value.is_string()),
                    Some("integer") => Some(value.is_i64() || value.is_u64()),
                    Some("boolean") => Some(value.is_boolean()),
                    _ => None,
                };
                match ok {
                    Some(true) => {}
                    Some(false) => errors.push(format!(
                        "{path}: expected {}, found {value}",
                        rule.as_str().unwrap_or_default()
                    )),
                    None => errors.push(format!("{path}: unsupported type {rule}")),
                }
            }
            "enum" => {
                if !rule
                    .as_array()
                    .is_some_and(|options| options.contains(value))
                {
                    errors.push(format!("{path}: {value} is not one of {rule}"));
                }
            }
            "minimum" => {
                if let (Some(n), Some(min)) = (value.as_f64(), rule.as_f64())
                    && n < min
                {
                    errors.push(format!("{path}: {n} is below the minimum {min}"));
                }
            }
            "minItems" | "maxItems" => {
                if let (Some(items), Some(limit)) = (value.as_array(), rule.as_u64()) {
                    let len = items.len() as u64;
                    let too_few = keyword == "minItems" && len < limit;
                    let too_many = keyword == "maxItems" && len > limit;
                    if too_few || too_many {
                        errors.push(format!("{path}: {len} items, {keyword} is {limit}"));
                    }
                }
            }
            "items" => {
                for (i, item) in value.as_array().into_iter().flatten().enumerate() {
                    check(root, rule, item, &format!("{path}[{i}]"), errors);
                }
            }
            "properties" => {
                if let (Some(fields), Some(props)) = (value.as_object(), rule.as_object()) {
                    for (name, field_schema) in props {
                        if let Some(field) = fields.get(name) {
                            check(root, field_schema, field, &format!("{path}.{name}"), errors);
                        }
                    }
                }
            }
            "required" => {
                if let Some(fields) = value.as_object() {
                    for name in rule
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        if !fields.contains_key(name) {
                            errors.push(format!("{path}: missing required field `{name}`"));
                        }
                    }
                }
            }
            "additionalProperties" => {
                if rule != &Value::Bool(false) {
                    errors.push(format!(
                        "{path}: only additionalProperties: false is supported"
                    ));
                } else if let Some(fields) = value.as_object() {
                    let props = rules.get("properties").and_then(Value::as_object);
                    for name in fields.keys() {
                        if !props.is_some_and(|p| p.contains_key(name)) {
                            errors.push(format!("{path}: unknown field `{name}`"));
                        }
                    }
                }
            }
            "pattern" => {}
            other if ANNOTATIONS.contains(&other) => {}
            other => errors.push(format!("{path}: unsupported schema keyword `{other}`")),
        }
    }
}

fn resolve<'a>(root: &'a Value, reference: &Value) -> Option<&'a Value> {
    let name = reference.as_str()?.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Code, Diagnostic, Position, Severity, Span};

    fn schema() -> Value {
        serde_json::from_str(DIAGNOSTIC_SCHEMA).unwrap()
    }

    fn example() -> Value {
        serde_json::from_str(&crate::tests::spec_example()).unwrap()
    }

    #[test]
    fn spec_example_validates() {
        assert_eq!(validate(&schema(), &example()), Vec::<String>::new());
    }

    #[test]
    fn serialized_diagnostics_validate() {
        let span = Span {
            file: "src/main.qnc".into(),
            start: Position(1, 1),
            end: Position(1, 4),
        };
        let d = Diagnostic::new(Code::parse("E0001").unwrap(), Severity::Error, "m", span);
        let value = serde_json::to_value(&d).unwrap();
        assert_eq!(validate(&schema(), &value), Vec::<String>::new());
    }

    #[test]
    fn extra_field_is_rejected() {
        let mut value = example();
        value["notes"] = Value::Array(Vec::new());
        assert_eq!(validate(&schema(), &value), ["$: unknown field `notes`"]);
    }

    #[test]
    fn old_insert_edit_form_is_rejected() {
        let mut value = example();
        value["fixes"][0]["edits"][0] = serde_json::json!({ "at": [46, 5], "insert": "x" });
        let errors = validate(&schema(), &value);
        assert!(
            errors.contains(&"$.fixes[0].edits[0]: missing required field `start`".to_string()),
            "{errors:?}"
        );
        assert!(
            errors.contains(&"$.fixes[0].edits[0]: unknown field `at`".to_string()),
            "{errors:?}"
        );
    }

    #[test]
    fn bad_values_are_rejected() {
        let mut value = example();
        value["severity"] = "fatal".into();
        value["span"]["start"] = serde_json::json!([0, 3]);
        value["span"]["end"] = serde_json::json!([47]);
        let errors = validate(&schema(), &value);
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn edit_file_and_deletion_validate() {
        let mut value = example();
        value["fixes"][0]["edits"] = serde_json::json!([
            { "start": [9, 14], "end": [9, 20], "text": "" },
            { "file": "cmd/app/main.qnc", "start": [1, 1], "end": [1, 1], "text": "fn main() {}\n" }
        ]);
        assert_eq!(validate(&schema(), &value), Vec::<String>::new());
    }

    #[test]
    fn code_pattern_matches_code_parse() {
        assert_eq!(schema()["properties"]["code"]["pattern"], "^[EW][0-9]{4}$");
    }

    #[test]
    fn unsupported_type_is_an_error() {
        let schema = serde_json::json!({ "type": ["string", "null"] });
        assert_eq!(
            validate(&schema, &"ab".into()),
            [r#"$: unsupported type ["string","null"]"#]
        );
    }

    #[test]
    fn unsupported_keywords_are_errors() {
        let schema = serde_json::json!({ "type": "string", "maxLength": 3 });
        assert_eq!(
            validate(&schema, &"ab".into()),
            ["$: unsupported schema keyword `maxLength`"]
        );
    }
}
