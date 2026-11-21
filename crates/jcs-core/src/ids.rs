use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CoreError;

/// Defines a prefixed, opaque, stringly-typed identifier.
///
/// The prefix exists so ids are self-describing in logs and in the event log:
/// `task_9f2c...` is diagnosable, `9f2c...` is not. Construction is validated so a
/// `ProjectId` can never silently hold a `TaskId`.
macro_rules! id_type {
    ($name:ident, $prefix:literal, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Generates a fresh random id with the type's prefix.
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, Uuid::new_v4().simple()))
            }

            /// Returns the raw string, including the prefix.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Parses and validates an id of this exact type.
            pub fn parse(value: impl Into<String>) -> Result<Self, CoreError> {
                let value = value.into();
                let rest = value
                    .strip_prefix($prefix)
                    .ok_or_else(|| CoreError::invalid_id($kind, &value))?;

                if rest.is_empty() {
                    return Err(CoreError::invalid_id($kind, value));
                }
                if !rest.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    return Err(CoreError::invalid_id($kind, value));
                }
                Ok(Self(value))
            }

            /// Builds an id from a prefix-free, already-validated body.
            ///
            /// Used when importing ids created elsewhere (fixtures, migrations).
            pub fn from_body(body: impl AsRef<str>) -> Result<Self, CoreError> {
                Self::parse(format!("{}{}", $prefix, body.as_ref()))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(ProjectId, "prj_", "project");
id_type!(GoalId, "goal_", "goal");
id_type!(TaskId, "task_", "task");
id_type!(RunId, "run_", "run");
id_type!(EventId, "evt_", "event");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_carry_their_prefix_and_roundtrip() {
        let id = ProjectId::new();
        assert!(id.as_str().starts_with("prj_"));
        assert_eq!(ProjectId::parse(id.as_str()).unwrap(), id);
        assert_eq!(id.to_string().parse::<ProjectId>().unwrap(), id);
    }

    #[test]
    fn ids_are_not_interchangeable() {
        let task = TaskId::new();
        assert!(ProjectId::parse(task.as_str()).is_err());
    }

    #[test]
    fn rejects_prefix_only_and_non_alphanumeric_bodies() {
        assert!(TaskId::parse("task_").is_err());
        assert!(TaskId::parse("task_abc-def").is_err());
        assert!(TaskId::parse("").is_err());
        assert!(TaskId::parse("9f2c").is_err());
    }

    #[test]
    fn from_body_adds_the_prefix() {
        assert_eq!(RunId::from_body("abc123").unwrap().as_str(), "run_abc123");
    }

    #[test]
    fn ids_serialize_as_plain_strings() {
        let id = GoalId::from_body("abc").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"goal_abc\"");
    }
}
