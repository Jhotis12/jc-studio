//! Policy compiler for JC Studio.
//!
//! Policy is code, in two layers: instructions for the model and executable gates that
//! enforce what instructions cannot. This crate owns the first layer, the pack format, the
//! composition rules and the deterministic rendering; `jcs-gates` owns execution.
//!
//! Determinism is a contract: the same packs and the same `.jcs/policy.toml` must produce
//! byte-identical files, so `render --check` can be a real gate.

pub mod compose;
pub mod error;
pub mod loader;
pub mod model;
pub mod project;
pub mod render;
pub mod resolve;

use std::collections::BTreeMap;
use std::path::Path;

use crate::compose::{compose, Overrides};
use crate::error::{PolicyError, Result};
use crate::model::{Composition, Pack};

use crate::render::Rendered;

pub use crate::project::{ProjectPolicy, POLICY_FILE};
pub use crate::render::{Lockfile, RenderedFile, AGENTS_PATH, GATES_PATH, LOCK_PATH};
pub use model::{Composition as Policy, Pack as PolicyPack};

/// A compiled policy: the effective composition plus everything it renders.
#[derive(Debug, Clone)]
pub struct Compiled {
    pub policy: ProjectPolicy,
    pub composition: Composition,
    pub rendered: Rendered,
}

/// Resolves packs for a project and renders the derived files in memory.
pub fn compile(
    project_root: &Path,
    available: &BTreeMap<String, Pack>,
    default_packs: &[String],
) -> Result<Compiled> {
    let policy = project::read(project_root)?.unwrap_or_default();

    let selected = if policy.packs.is_empty() {
        default_packs.to_vec()
    } else {
        policy.packs.clone()
    };

    let resolved = resolve::resolve(&selected, available, &policy.disabled_packs)?;

    let overrides = Overrides {
        rules: policy.rules.clone(),
        disabled_rules: policy.disabled_rules.clone(),
        disabled_gates: policy.disabled_gates.clone(),
    };

    let composition = compose(&resolved, &overrides);
    let rendered = render::render(&composition, project::POLICY_FILE)?;

    Ok(Compiled {
        policy,
        composition,
        rendered,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Created,
    Updated,
    Unchanged,
    /// On disk but different from what the policy produces (hand-edited or stale).
    Drifted,
    Missing,
    /// Exists and is not ours; refusing to overwrite without `--force`.
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    pub path: String,
    pub state: FileState,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub check_only: bool,
    pub files: Vec<FileReport>,
}

impl ApplyReport {
    /// True when the outcome satisfies the mode: everything unchanged in check mode,
    /// everything written or already correct in apply mode.
    pub fn ok(&self) -> bool {
        self.files.iter().all(|file| {
            if self.check_only {
                file.state == FileState::Unchanged
            } else {
                matches!(
                    file.state,
                    FileState::Created | FileState::Updated | FileState::Unchanged
                )
            }
        })
    }

    pub fn drifted(&self) -> Vec<&FileReport> {
        self.files
            .iter()
            .filter(|file| {
                matches!(
                    file.state,
                    FileState::Drifted | FileState::Missing | FileState::Refused
                )
            })
            .collect()
    }
}

/// Writes the rendered files, or verifies them without touching the disk.
///
/// Safety rules:
///   * a path that escapes the project root is refused;
///   * an `AGENTS.md` we did not generate is never replaced without `force`, and when
///     forced the original is backed up first;
///   * files whose content already matches are not rewritten, so mtimes stay meaningful.
pub fn apply(
    compiled: &Compiled,
    project_root: &Path,
    check_only: bool,
    force: bool,
) -> Result<ApplyReport> {
    let mut files = Vec::new();

    for file in &compiled.rendered.files {
        let relative = Path::new(&file.path);
        if relative.is_absolute() || file.path.split('/').any(|part| part == "..") {
            return Err(PolicyError::PathEscape {
                path: file.path.clone(),
            });
        }

        let target = project_root.join(relative);
        let existing = if target.is_file() {
            Some(std::fs::read_to_string(&target)?)
        } else {
            None
        };

        if check_only {
            let state = match &existing {
                None => FileState::Missing,
                Some(content) if content == &file.content => FileState::Unchanged,
                Some(_) => FileState::Drifted,
            };
            files.push(FileReport {
                path: file.path.clone(),
                state,
                detail: None,
            });
            continue;
        }

        if let Some(content) = &existing {
            if content == &file.content {
                files.push(FileReport {
                    path: file.path.clone(),
                    state: FileState::Unchanged,
                    detail: None,
                });
                continue;
            }

            if relative == Path::new(render::AGENTS_PATH)
                && !content.starts_with(render::GENERATED_MARKER)
            {
                if !force {
                    files.push(FileReport {
                        path: file.path.clone(),
                        state: FileState::Refused,
                        detail: Some(
                            "AGENTS.md exists and was not generated by jcs; \
                             pass --force to replace it (the original is backed up)"
                                .to_string(),
                        ),
                    });
                    continue;
                }

                let backup = project_root.join("AGENTS.md.pre-jcs.bak");
                std::fs::write(&backup, content)?;
            }

            write_atomic(&target, &file.content)?;
            files.push(FileReport {
                path: file.path.clone(),
                state: FileState::Updated,
                detail: None,
            });
            continue;
        }

        write_atomic(&target, &file.content)?;
        files.push(FileReport {
            path: file.path.clone(),
            state: FileState::Created,
            detail: None,
        });
    }

    Ok(ApplyReport { check_only, files })
}

/// Writes a file through a temporary sibling so a crash cannot leave a half-written file.
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "jcs-file".to_string());
    let temporary = path.with_file_name(format!(".{file_name}.jcs-tmp"));

    std::fs::write(&temporary, content)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}
