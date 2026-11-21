use serde::{Deserialize, Serialize};

/// An architectural boundary: files under `from` must not import files under `to`.
///
/// Paths are repository-relative prefixes. The check is lexical over import statements
/// (see `jcs-memory::boundaries`), which catches the common case without a type resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRule {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub reason: String,
}

impl BoundaryRule {
    pub fn validate(&self) -> Result<(), String> {
        if self.from.trim().is_empty() || self.to.trim().is_empty() {
            return Err("boundary rule needs a non-empty `from` and `to`".to_string());
        }
        if self.from.starts_with('/') || self.to.starts_with('/') {
            return Err("boundary paths must be repository-relative".to_string());
        }
        if self.from.split('/').any(|part| part == "..")
            || self.to.split('/').any(|part| part == "..")
        {
            return Err("boundary paths must not contain `..`".to_string());
        }
        if self.from == self.to {
            return Err("a boundary cannot forbid a prefix from importing itself".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> BoundaryRule {
        BoundaryRule {
            from: from.into(),
            to: to.into(),
            reason: String::new(),
        }
    }

    #[test]
    fn relative_prefixes_are_accepted() {
        rule("src/domain", "src/infra").validate().unwrap();
    }

    #[test]
    fn absolute_and_escaping_paths_are_rejected() {
        assert!(rule("/etc", "src").validate().is_err());
        assert!(rule("src/../etc", "src").validate().is_err());
    }

    #[test]
    fn empty_and_self_referential_rules_are_rejected() {
        assert!(rule("", "src").validate().is_err());
        assert!(rule("src/domain", "src/domain").validate().is_err());
    }
}
