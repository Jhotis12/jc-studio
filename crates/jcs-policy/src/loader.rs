//! Pack loading.
//!
//! Built-in packs are embedded in the binary so `jcs policy` works with zero installation.
//! A directory (`JCS_PACKS_DIR`) can add or override packs by id, which is how a team
//! ships its own conventions without rebuilding JC Studio.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{PolicyError, Result};
use crate::model::{InstructionDoc, Pack, PackFile};

struct Embedded {
    manifest: &'static str,
    docs: &'static [(&'static str, &'static str)],
}

const BUILTIN: &[Embedded] = &[
    Embedded {
        manifest: include_str!("../packs/core-git/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/core-git/instructions.md"),
        )],
    },
    Embedded {
        manifest: include_str!("../packs/core-security/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/core-security/instructions.md"),
        )],
    },
    Embedded {
        manifest: include_str!("../packs/rust-service/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/rust-service/instructions.md"),
        )],
    },
    Embedded {
        manifest: include_str!("../packs/ts-frontend/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/ts-frontend/instructions.md"),
        )],
    },
    Embedded {
        manifest: include_str!("../packs/postgres-schema/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/postgres-schema/instructions.md"),
        )],
    },
    Embedded {
        manifest: include_str!("../packs/testing-tdd/pack.toml"),
        docs: &[(
            "instructions.md",
            include_str!("../packs/testing-tdd/instructions.md"),
        )],
    },
];

/// Packs compiled into the binary.
pub fn builtin_packs() -> Result<BTreeMap<String, Pack>> {
    let mut packs = BTreeMap::new();
    for embedded in BUILTIN {
        let pack = parse_pack(embedded.manifest, |path| {
            embedded
                .docs
                .iter()
                .find(|(name, _)| *name == path)
                .map(|(_, content)| (*content).to_string())
                .ok_or_else(|| PolicyError::InvalidPack {
                    id: "<embedded>".into(),
                    reason: format!("instruction file {path:?} is not embedded"),
                })
        })?;
        if packs.insert(pack.id.clone(), pack).is_some() {
            return Err(PolicyError::InvalidPack {
                id: "<builtin>".into(),
                reason: "duplicate built-in pack id".into(),
            });
        }
    }
    Ok(packs)
}

/// Built-in packs plus every pack found in `directory` (disks override by id).
pub fn load_all(directory: Option<&Path>) -> Result<BTreeMap<String, Pack>> {
    let mut packs = builtin_packs()?;

    if let Some(directory) = directory {
        if !directory.is_dir() {
            return Err(PolicyError::InvalidPack {
                id: "<packs-dir>".into(),
                reason: format!("{} is not a directory", directory.display()),
            });
        }
        for pack in load_from_dir(directory)? {
            if packs.contains_key(&pack.id) {
                tracing::info!(pack = %pack.id, "overriding a built-in pack from disk");
            }
            packs.insert(pack.id.clone(), pack);
        }
    }

    Ok(packs)
}

fn load_from_dir(directory: &Path) -> Result<Vec<Pack>> {
    let mut packs = Vec::new();

    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let manifest_path = entry.path().join("pack.toml");
        if !manifest_path.is_file() {
            continue;
        }

        let manifest = std::fs::read_to_string(&manifest_path)?;
        let root = entry.path().canonicalize()?;

        let pack = parse_pack(&manifest, |relative| {
            let candidate = root.join(relative);
            let resolved = candidate
                .canonicalize()
                .map_err(|error| PolicyError::InvalidPack {
                    id: manifest_path.display().to_string(),
                    reason: format!("cannot read instruction file {relative:?}: {error}"),
                })?;
            if !resolved.starts_with(&root) {
                return Err(PolicyError::PathEscape {
                    path: resolved.display().to_string(),
                });
            }
            std::fs::read_to_string(&resolved).map_err(PolicyError::from)
        })?;

        packs.push(pack);
    }

    Ok(packs)
}

fn parse_pack(manifest: &str, mut load_doc: impl FnMut(&str) -> Result<String>) -> Result<Pack> {
    let file: PackFile = toml::from_str(manifest).map_err(|error| PolicyError::InvalidPack {
        id: "<manifest>".into(),
        reason: error.to_string(),
    })?;

    let mut instructions = Vec::new();
    for path in &file.instructions {
        instructions.push(InstructionDoc {
            path: path.clone(),
            content: load_doc(path)?,
        });
    }

    let mut gates = jcs_gates::GateSet::default();
    for (name, gate) in file.gates {
        let def = gate
            .into_def(&name)
            .map_err(|error| PolicyError::InvalidGate {
                pack: file.id.clone(),
                gate: name.clone(),
                reason: error.to_string(),
            })?;
        gates.gates.insert(name, def);
    }

    let mut pack = Pack {
        id: file.id,
        version: file.version,
        description: file.description,
        requires: file.requires,
        instructions,
        rules: file.rules,
        gates,
        skills: file.skills,
        hash: String::new(),
    };

    pack.validate()?;
    pack.hash = compute_hash(&pack);
    Ok(pack)
}

/// Integrity hash over everything that affects behaviour or rendered output.
pub fn compute_hash(pack: &Pack) -> String {
    let mut canonical = String::new();

    canonical.push_str(&pack.id);
    canonical.push('\n');
    canonical.push_str(&pack.version);
    canonical.push('\n');
    canonical.push_str(&pack.description);
    canonical.push('\n');

    for dependency in &pack.requires {
        canonical.push_str(dependency);
        canonical.push('\0');
    }

    for doc in &pack.instructions {
        canonical.push_str(&doc.path);
        canonical.push('\0');
        canonical.push_str(&doc.content);
        canonical.push('\0');
    }

    for (key, value) in &pack.rules {
        canonical.push_str(key);
        canonical.push('=');
        canonical.push_str(value);
        canonical.push('\0');
    }

    for (name, gate) in &pack.gates.gates {
        canonical.push_str(name);
        canonical.push('\0');
        for argument in &gate.command {
            canonical.push_str(argument);
            canonical.push('\0');
        }
        canonical.push_str(&gate.timeout.as_secs().to_string());
        canonical.push('\0');
        canonical.push_str(if gate.required {
            "required"
        } else {
            "optional"
        });
        canonical.push('\0');
        canonical.push_str(&format!("{:?}", gate.parser));
        canonical.push('\0');
        canonical.push_str(
            &gate
                .min_coverage
                .map(|value| value.0.to_string())
                .unwrap_or_default(),
        );
        canonical.push('\0');
    }

    for skill in &pack.skills {
        canonical.push_str(skill);
        canonical.push('\0');
    }

    format!("sha256:{}", sha256_hex(&canonical))
}

pub fn sha256_hex(data: &str) -> String {
    let digest = Sha256::digest(data.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_packs_load_and_validate() {
        let packs = builtin_packs().unwrap();
        for id in [
            "core-git",
            "core-security",
            "rust-service",
            "ts-frontend",
            "postgres-schema",
            "testing-tdd",
        ] {
            let pack = packs
                .get(id)
                .unwrap_or_else(|| panic!("missing built-in pack {id}"));
            assert!(pack.hash.starts_with("sha256:"), "{id} has no hash");
            assert!(!pack.instructions.is_empty(), "{id} has no instructions");
            pack.validate().unwrap();
        }
    }

    #[test]
    fn rust_service_declares_the_expected_gates() {
        let packs = builtin_packs().unwrap();
        let rust = packs.get("rust-service").unwrap();
        for gate in ["fmt", "clippy", "test"] {
            assert!(rust.gates.gates.contains_key(gate), "missing gate {gate}");
        }
        // Coverage is optional: not every project has llvm-cov installed.
        assert!(!rust.gates.gates["coverage"].required);
        assert!(rust.requires.contains(&"core-git".to_string()));
        assert!(rust.requires.contains(&"core-security".to_string()));
    }

    #[test]
    fn the_hash_changes_when_behaviour_changes() {
        let packs = builtin_packs().unwrap();
        let mut pack = packs.get("core-git").unwrap().clone();
        let before = pack.hash.clone();

        pack.rules.insert("new_rule".into(), "something".into());
        let after = compute_hash(&pack);
        assert_ne!(before, after);
    }

    #[test]
    fn unknown_manifest_fields_are_rejected() {
        let manifest = r#"
id = "x"
version = "1"
timeout = 5
"#;
        let error = parse_pack(manifest, |_| Ok(String::new())).unwrap_err();
        assert!(error.to_string().contains("unknown"), "{error}");
    }

    #[test]
    fn a_missing_instruction_file_is_an_error() {
        let manifest = r#"
id = "x"
version = "1"
instructions = ["missing.md"]
"#;
        let error = parse_pack(manifest, |path| {
            Err(PolicyError::InvalidPack {
                id: "x".into(),
                reason: format!("{path} not found"),
            })
        })
        .unwrap_err();
        assert!(error.to_string().contains("missing.md"), "{error}");
    }

    #[test]
    fn disk_packs_override_built_ins_by_id() {
        let directory = tempfile::tempdir().unwrap();
        let pack_dir = directory.path().join("custom");
        std::fs::create_dir_all(&pack_dir).unwrap();
        std::fs::write(
            pack_dir.join("pack.toml"),
            r#"
id = "core-git"
version = "9.9.9"
description = "overridden"
instructions = ["instructions.md"]

[rules]
only_rule = "from disk"
"#,
        )
        .unwrap();
        std::fs::write(pack_dir.join("instructions.md"), "# Overridden\n").unwrap();

        let packs = load_all(Some(directory.path())).unwrap();
        let overridden = packs.get("core-git").unwrap();
        assert_eq!(overridden.version, "9.9.9");
        assert_eq!(overridden.rules.len(), 1);
        // Built-ins that were not overridden are still present.
        assert!(packs.contains_key("rust-service"));
    }

    #[test]
    fn instruction_files_cannot_escape_the_pack_directory() {
        let directory = tempfile::tempdir().unwrap();
        let pack_dir = directory.path().join("escape");
        std::fs::create_dir_all(&pack_dir).unwrap();
        std::fs::write(directory.path().join("outside.md"), "secret").unwrap();
        std::fs::write(
            pack_dir.join("pack.toml"),
            r#"
id = "escape"
version = "1"
instructions = ["../outside.md"]
"#,
        )
        .unwrap();

        let error = load_all(Some(directory.path())).unwrap_err();
        assert!(error.to_string().contains("escapes"), "{error}");
    }
}
