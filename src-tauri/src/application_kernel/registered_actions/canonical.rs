//! Stable JSON canonicalization for hashing.
//!
//! Object keys are sorted recursively so that two semantically identical
//! values always produce the same string, and therefore the same hash.
//! Array order is meaningful and is preserved.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub fn canonical_json(value: &Value) -> String {
    canonicalize(value).to_string()
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonicalize(&map[key]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash of a JSON value using the canonical form.
pub fn hash_value(value: &Value) -> String {
    sha256_hex(&canonical_json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_hash() {
        let a = json!({ "b": 1, "a": { "z": true, "y": [1, 2] } });
        let b = json!({ "a": { "y": [1, 2], "z": true }, "b": 1 });
        assert_eq!(hash_value(&a), hash_value(&b));
    }

    #[test]
    fn array_order_changes_hash() {
        assert_ne!(hash_value(&json!([1, 2])), hash_value(&json!([2, 1])));
    }

    #[test]
    fn different_values_differ() {
        assert_ne!(
            hash_value(&json!({ "a": 1 })),
            hash_value(&json!({ "a": 2 }))
        );
    }
}
