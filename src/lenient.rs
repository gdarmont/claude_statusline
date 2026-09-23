//! Tolerant deserializers for the Claude Code payload.
//!
//! The schema grows and changes between releases. A field whose type no longer
//! matches is treated as absent, so one surprise hides a single section instead
//! of failing the whole parse. Use with `#[serde(default, deserialize_with = ...)]`.
#![allow(dead_code)]

use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_json::Value;

/// `None` when the value is missing, null, or of an unexpected shape.
pub fn option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).ok())
}

/// Keeps the elements that parse and drops the rest. Anything other than an array is empty.
pub fn vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let Value::Array(values) = Value::deserialize(deserializer)? else {
        return Ok(Vec::new());
    };
    Ok(values
        .into_iter()
        .filter_map(|value| T::deserialize(value).ok())
        .collect())
}
