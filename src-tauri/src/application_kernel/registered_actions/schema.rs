//! Lightweight input-schema validation for bundled registered actions.
//!
//! Avoids a jsonschema dependency: bundled schemas are small object shapes with
//! `required`, `properties`, and simple `type` / `minimum` / `maximum` checks.

use serde_json::Value;

/// Validate `input` against a JSON Schema fragment used by bundled descriptors.
/// Unknown keywords are ignored. Fail closed on type / required mismatches.
pub fn validate_input(schema: &Value, input: &Value) -> Result<(), String> {
    validate_node(schema, input, "$")
}

fn validate_node(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(types) = schema.get("type") {
        check_type(types, value, path)?;
    }

    if schema.get("type").and_then(|t| t.as_str()) == Some("object")
        || schema.get("properties").is_some()
        || schema.get("required").is_some()
    {
        let obj = value
            .as_object()
            .ok_or_else(|| format!("{path}: expected an object"))?;

        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for key in required {
                let Some(name) = key.as_str() else { continue };
                if !obj.contains_key(name) {
                    return Err(format!("{path}: missing required field `{name}`"));
                }
            }
        }

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (name, prop_schema) in props {
                if let Some(child) = obj.get(name) {
                    validate_node(prop_schema, child, &format!("{path}.{name}"))?;
                }
            }
        }
    }

    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        let n = value
            .as_f64()
            .ok_or_else(|| format!("{path}: expected a number"))?;
        if n < min {
            return Err(format!("{path}: must be at least {min}"));
        }
    }
    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        let n = value
            .as_f64()
            .ok_or_else(|| format!("{path}: expected a number"))?;
        if n > max {
            return Err(format!("{path}: must be at most {max}"));
        }
    }

    if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        let s = value
            .as_str()
            .ok_or_else(|| format!("{path}: expected a string"))?;
        if s.chars().count() as u64 > max_len {
            return Err(format!("{path}: is too long"));
        }
    }

    Ok(())
}

fn check_type(types: &Value, value: &Value, path: &str) -> Result<(), String> {
    let matches = |ty: &str| -> bool {
        match ty {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => true,
        }
    };

    if let Some(ty) = types.as_str() {
        if !matches(ty) {
            return Err(format!("{path}: expected {ty}"));
        }
        return Ok(());
    }
    if let Some(arr) = types.as_array() {
        if arr.iter().any(|t| t.as_str().is_some_and(matches)) {
            return Ok(());
        }
        return Err(format!("{path}: value has the wrong type"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_and_types() {
        let schema = json!({
            "type": "object",
            "required": ["modelId"],
            "properties": {
                "modelId": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
            }
        });
        assert!(validate_input(&schema, &json!({"modelId": "notes"})).is_ok());
        assert!(validate_input(&schema, &json!({"modelId": "notes", "limit": 10})).is_ok());
        assert!(validate_input(&schema, &json!({})).is_err());
        assert!(validate_input(&schema, &json!({"modelId": 1})).is_err());
        assert!(validate_input(&schema, &json!({"modelId": "notes", "limit": 0})).is_err());
        assert!(validate_input(&schema, &json!("x")).is_err());
    }
}
