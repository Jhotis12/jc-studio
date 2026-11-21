//! The project's own policy file: `.jcs/policy.toml`.
//!
//! This lives in the repository, not in JC Studio's database, on purpose: the policy is
//! part of the codebase and is reviewed, diffed and versioned with it.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use jcs_core::BoundaryRule;

use crate::error::{PolicyError, Result};

pub const POLICY_FILE: &str = ".jcs/policy.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPolicy {
    /// Packs to compose. Empty means "use the configured defaults".
    #[serde(default)]
    pub packs: Vec<String>,
    /// Rules that add to or replace pack rules.
    #[serde(default)]
    pub rules: BTreeMap<String, String>,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    #[serde(default)]
    pub disabled_gates: Vec<String>,
    #[serde(default)]
    pub disabled_packs: Vec<String>,
    /// Extra Markdown documents, relative to the project root, rendered after the packs.
    ///
    /// This is the migration path for a project that already has hand-written
    /// instructions: move them into a file, list it here, and the generated `AGENTS.md`
    /// keeps them instead of replacing them.
    #[serde(default)]
    pub instructions: Vec<String>,
    /// Architectural fitness functions evaluated as part of verification.
    #[serde(default)]
    pub boundaries: Vec<BoundaryRule>,
}

/// Reads the project policy, if the file exists.
pub fn read(root: &Path) -> Result<Option<ProjectPolicy>> {
    let path = root.join(POLICY_FILE);
    if !path.is_file() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(&path)?;
    let policy: ProjectPolicy =
        toml::from_str(&text).map_err(|error| PolicyError::InvalidPack {
            id: POLICY_FILE.to_string(),
            reason: error.to_string(),
        })?;

    for rule in &policy.boundaries {
        rule.validate().map_err(|reason| PolicyError::InvalidPack {
            id: POLICY_FILE.to_string(),
            reason,
        })?;
    }

    Ok(Some(policy))
}

/// Writes the project policy, replacing the file.
pub fn write(root: &Path, policy: &ProjectPolicy) -> Result<()> {
    let body = toml::to_string_pretty(policy)?;
    let content = format!(
        "# JC Studio project policy. Commit this file.\n\
         # Regenerate derived files with `jcs policy render`.\n\n{body}"
    );
    crate::write_atomic(&root.join(POLICY_FILE), &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_not_an_error() {
        let directory = tempfile::tempdir().unwrap();
        assert!(read(directory.path()).unwrap().is_none());
    }

    #[test]
    fn boundary_rules_are_parsed_and_validated() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join(".jcs")).unwrap();
        std::fs::write(
            directory.path().join(POLICY_FILE),
            r#"
packs = ["core-git"]

[[boundaries]]
from = "src/domain"
to = "src/infra"
reason = "the domain stays pure"
"#,
        )
        .unwrap();

        let policy = read(directory.path()).unwrap().unwrap();
        assert_eq!(policy.boundaries.len(), 1);
        assert_eq!(policy.boundaries[0].from, "src/domain");
    }

    #[test]
    fn an_invalid_boundary_is_rejected_with_a_reason() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join(".jcs")).unwrap();
        std::fs::write(
            directory.path().join(POLICY_FILE),
            "[[boundaries]]\nfrom = \"/etc\"\nto = \"src\"\n",
        )
        .unwrap();

        let error = read(directory.path()).unwrap_err();
        assert!(error.to_string().contains("repository-relative"), "{error}");
    }

    #[test]
    fn a_policy_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let policy = ProjectPolicy {
            packs: vec!["rust-service".into(), "testing-tdd".into()],
            rules: BTreeMap::from([("extra".to_string(), "value".to_string())]),
            disabled_gates: vec!["audit".into()],
            ..ProjectPolicy::default()
        };

        write(directory.path(), &policy).unwrap();
        let loaded = read(directory.path()).unwrap().unwrap();
        assert_eq!(loaded, policy);
    }

    #[test]
    fn unknown_keys_are_rejected_to_catch_typos() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join(".jcs")).unwrap();
        std::fs::write(
            directory.path().join(POLICY_FILE),
            "packs = [\"core-git\"]\ndisable_gates = [\"clippy\"]\n",
        )
        .unwrap();

        let error = read(directory.path()).unwrap_err();
        assert!(error.to_string().contains("unknown"), "{error}");
    }
}
