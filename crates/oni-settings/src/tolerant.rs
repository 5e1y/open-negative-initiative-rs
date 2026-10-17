//! Readers for the types whose Swift decoder tolerates absent keys: each reads a JSON object
//! key by key, absent or null giving `None`, a value of the wrong type an error.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub type Object = Map<String, Value>;

pub fn object<E: serde::de::Error>(value: Value, what: &str) -> Result<Object, E> {
    match value {
        Value::Object(map) => Ok(map),
        other => Err(E::custom(format!("{what}: expected an object, found {other}"))),
    }
}

pub fn opt<T: DeserializeOwned, E: serde::de::Error>(map: &Object, key: &str) -> Result<Option<T>, E> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => T::deserialize(v.clone()).map(Some).map_err(|e| E::custom(format!("{key}: {e}"))),
    }
}

/// A raw JSON value as `as? NSNumber` takes it: a boolean is an `NSNumber` too, read as 0 or 1.
pub fn ns_number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        v => v.as_f64(),
    }
}

/// A raw JSON value as Swift's `NSNumber.floatValue` reads it; `None` for anything else.
pub fn number_f32(value: Option<&Value>) -> Option<f32> {
    ns_number(value).map(|v| v as f32)
}

/// A raw JSON value as `as? Int` bridges it: integral numbers only, a boolean as 0 or 1.
pub fn ns_integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Bool(b) => Some(i64::from(*b)),
        v => integer(Some(v)),
    }
}

/// A raw JSON value as `as? Bool` bridges it: a boolean, or a number that is exactly 0 or 1.
pub fn ns_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(b) => Some(*b),
        v => {
            let n = v.as_f64()?;
            if n == 1.0 {
                Some(true)
            } else if n == 0.0 {
                Some(false)
            } else {
                None
            }
        }
    }
}

/// A JSON number as `JSONDecoder` reads an `Int`: integral values only, never a boolean.
pub fn integer(value: Option<&Value>) -> Option<i64> {
    let v = value?;
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    let f = v.as_f64()?;
    (f.fract() == 0.0 && f.abs() < 9.2e18).then_some(f as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raw_values_bridge_as_foundation_bridges_them() {
        assert_eq!(number_f32(Some(&json!(0.67))), Some(0.67));
        assert_eq!(number_f32(Some(&json!(false))), Some(0.0));
        assert_eq!(ns_integer(Some(&json!(3.0))), Some(3));
        assert_eq!(ns_integer(Some(&json!(true))), Some(1));
        assert_eq!(ns_bool(Some(&json!(1))), Some(true));
        // Twins: a fraction is no Int, a 2 no Bool, a string no number, and `JSONDecoder` refuses a boolean.
        assert_eq!(ns_integer(Some(&json!(3.5))), None);
        assert_eq!(ns_bool(Some(&json!(2))), None);
        assert_eq!(number_f32(Some(&json!("0.67"))), None);
        assert_eq!(integer(Some(&json!(true))), None);
    }
}
