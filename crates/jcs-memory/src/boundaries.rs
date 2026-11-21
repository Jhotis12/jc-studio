//! Architectural boundaries as an executable check.
//!
//! The rule is "files under `from` must not import `to`". Resolution is lexical over import
//! statements: an import mentions a module when one of its path segments equals the last
//! segment of the forbidden prefix (`src/infra` -> `infra`). That catches `crate::infra::db`,
//! `./infra/x` and `from app.infra import y` without a type resolver.
//!
//! It is deliberately conservative in what it *claims*: this is a fitness function, not a
//! compiler. A false negative is possible with re-exports; a false positive requires a
//! symbol literally named like the layer, which the reason field lets a human judge.

use std::path::{Path, PathBuf};

use jcs_core::BoundaryRule;

use crate::error::Result;
use crate::languages;
use crate::model::Language;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Violation {
    pub path: String,
    pub line: u32,
    /// The import statement, trimmed, for the report.
    pub import: String,
    /// `from !-> to`, plus the reason when the rule provides one.
    pub rule: String,
}

/// Checks every rule against the files under its `from` prefix.
pub fn check(root: &Path, rules: &[BoundaryRule]) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    for rule in rules {
        if rule.validate().is_err() {
            continue;
        }

        let Some(forbidden) = module_key(&rule.to) else {
            continue;
        };

        for path in files_under(root, &rule.from) {
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(language) = Language::from_path(&path) else {
                continue;
            };

            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            let Ok(sites) = languages::import_sites(language, &relative, &source) else {
                continue;
            };

            for (line, import) in sites {
                if mentions_module(&import, &forbidden) {
                    violations.push(Violation {
                        path: relative.clone(),
                        line,
                        import: import.trim().to_string(),
                        rule: describe(rule),
                    });
                }
            }
        }
    }

    violations.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    violations.dedup();
    Ok(violations)
}

fn describe(rule: &BoundaryRule) -> String {
    if rule.reason.trim().is_empty() {
        format!("{} !-> {}", rule.from, rule.to)
    } else {
        format!("{} !-> {} ({})", rule.from, rule.to, rule.reason)
    }
}

/// The last path segment of a prefix: the module name an import would mention.
fn module_key(prefix: &str) -> Option<String> {
    let key = prefix
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

fn mentions_module(import: &str, key: &str) -> bool {
    import
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|segment| segment == key)
}

fn files_under(root: &Path, prefix: &str) -> Vec<PathBuf> {
    let base = root.join(prefix);
    let mut files = Vec::new();
    let mut stack = vec![base];

    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() && Language::from_path(&path).is_some() {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> BoundaryRule {
        BoundaryRule {
            from: from.into(),
            to: to.into(),
            reason: "keeps the core pure".into(),
        }
    }

    #[test]
    fn a_forbidden_import_is_reported_with_its_line() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src/domain")).unwrap();
        std::fs::create_dir_all(directory.path().join("src/infra")).unwrap();
        std::fs::write(
            directory.path().join("src/domain/user.rs"),
            "use crate::infra::db::Connection;\n\npub fn load() {}\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("src/infra/db.rs"),
            "pub struct Connection;\n",
        )
        .unwrap();

        let violations = check(directory.path(), &[rule("src/domain", "src/infra")]).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].path, "src/domain/user.rs");
        assert_eq!(violations[0].line, 1);
        assert!(violations[0].import.contains("infra"));
        assert!(violations[0].rule.contains("keeps the core pure"));
    }

    #[test]
    fn a_clean_dependency_direction_passes() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src/domain")).unwrap();
        std::fs::write(
            directory.path().join("src/domain/user.rs"),
            "use crate::domain::value::Email;\n",
        )
        .unwrap();

        let violations = check(directory.path(), &[rule("src/domain", "src/infra")]).unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn sibling_prefixes_are_not_confused() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src/domain")).unwrap();
        std::fs::write(
            directory.path().join("src/domain/user.rs"),
            "use crate::infrastructure_helpers::x;\n",
        )
        .unwrap();

        // `infra` must match a whole segment, not a prefix of another module name.
        let violations = check(directory.path(), &[rule("src/domain", "src/infra")]).unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn typescript_relative_imports_are_checked() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src/domain")).unwrap();
        std::fs::write(
            directory.path().join("src/domain/user.ts"),
            "import { db } from '../infra/db';\n",
        )
        .unwrap();

        let violations = check(directory.path(), &[rule("src/domain", "src/infra")]).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].path, "src/domain/user.ts");
    }

    #[test]
    fn python_package_imports_are_checked() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("app/domain")).unwrap();
        std::fs::write(
            directory.path().join("app/domain/user.py"),
            "from app.infra.db import Connection\n",
        )
        .unwrap();

        let violations = check(directory.path(), &[rule("app/domain", "app/infra")]).unwrap();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn invalid_rules_are_ignored_instead_of_panicking() {
        let directory = tempfile::tempdir().unwrap();
        let violations = check(directory.path(), &[rule("", "src/infra")]).unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn results_are_sorted_and_deduplicated() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src/domain")).unwrap();
        std::fs::write(
            directory.path().join("src/domain/b.rs"),
            "use crate::infra::x;\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("src/domain/a.rs"),
            "use crate::infra::y;\n",
        )
        .unwrap();

        let violations = check(directory.path(), &[rule("src/domain", "src/infra")]).unwrap();
        let paths: Vec<&str> = violations.iter().map(|v| v.path.as_str()).collect();
        assert_eq!(paths, vec!["src/domain/a.rs", "src/domain/b.rs"]);
    }
}
