use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Deserializer};

pub fn from_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + FromStr,
    <T as FromStr>::Err: Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Inner<T> {
        Value(T),
        String(String),
    }

    match Inner::deserialize(deserializer)? {
        Inner::Value(x) => Ok(x),
        Inner::String(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_value() {
        let value = serde_json::json!({"field": 42});
        let parsed = serde_json::from_value::<FromStringStruct>(value).unwrap();
        assert_eq!(parsed.field, 42);
    }

    #[test]
    fn from_string_string() {
        let value = serde_json::json!({"field": "42"});
        let parsed = serde_json::from_value::<FromStringStruct>(value).unwrap();
        assert_eq!(parsed.field, 42);
    }

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct FromStringStruct {
        #[serde(deserialize_with = "from_string")]
        field: u64,
    }
}
