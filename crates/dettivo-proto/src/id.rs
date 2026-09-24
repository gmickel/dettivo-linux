//! `Id`: the lowercase-UUID-string identifier shared by every resource the
//! contract addresses by identity (`TranscriptRef.id`, `meeting_id`,
//! `dictation_id`, rule ids, provider ids, automation job ids).
//!
//! Opaque tokens the contract documents with a non-UUID example shape
//! (`job_id`, `subscription_id`, `transfer_id`, `macro_id`) are left as
//! plain `String` fields; only fields the contract calls out as `<uuid>`
//! use this type.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A lowercase UUID string identifier.
///
/// Deserialization rejects any value that is not a syntactically valid
/// UUID (8-4-4-4-12 hex groups) in lowercase, naming the offending field
/// via serde's path tracking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(String);

impl Id {
    /// Builds an `Id` from a string, validating it is a lowercase UUID.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if is_lowercase_uuid(&value) {
            Ok(Self(value))
        } else {
            Err(IdError(value))
        }
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A value was not a valid lowercase UUID string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdError(String);

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The contract's message names the field, never the value.
        let _ = &self.0;
        f.write_str("not a lowercase UUID string")
    }
}

impl std::error::Error for IdError {}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Id::new(raw).map_err(|e| D::Error::custom(e.to_string()))
    }
}

/// True when `value` is a lowercase UUID string (8-4-4-4-12 hex groups).
fn is_lowercase_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let dash_positions = [8, 13, 18, 23];
    for (i, b) in bytes.iter().enumerate() {
        if dash_positions.contains(&i) {
            if *b != b'-' {
                return false;
            }
            continue;
        }
        if !b.is_ascii_hexdigit() || b.is_ascii_uppercase() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_uuid() {
        let id = Id::new("0f8fad5b-d9cb-469f-a165-70867728950e").unwrap();
        assert_eq!(id.as_str(), "0f8fad5b-d9cb-469f-a165-70867728950e");
    }

    #[test]
    fn rejects_uppercase_and_malformed() {
        assert!(Id::new("0F8FAD5B-D9CB-469F-A165-70867728950E").is_err());
        assert!(Id::new("not-a-uuid").is_err());
        assert!(Id::new("").is_err());
    }

    #[test]
    fn json_round_trip() {
        let id = Id::new("0f8fad5b-d9cb-469f-a165-70867728950e").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"0f8fad5b-d9cb-469f-a165-70867728950e\"");
        let back: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn deserialize_rejects_non_uuid_and_names_error() {
        let err = serde_json::from_str::<Id>("\"nope\"").unwrap_err();
        assert!(err.to_string().contains("not a lowercase UUID string"));
    }
}
