//! Secret scanning.
//!
//! Two detectors with different failure modes:
//!   * known credential prefixes (`AKIA`, `ghp_`, `-----BEGIN PRIVATE KEY-----`) are exact
//!     and cheap;
//!   * high-entropy tokens catch unknown formats but need a threshold, because every hash,
//!     UUID and minified blob looks random.
//!
//! The threshold is deliberately strict and the excerpt is redacted: a scanner that prints
//! the secret it found has leaked it into the logs, the UI and the event history.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::GateError;

/// Files larger than this are skipped: a secret scanner is not a build step.
const MAX_SCAN_BYTES: u64 = 512 * 1024;
/// Minimum length for an entropy candidate.
const ENTROPY_MIN_LEN: usize = 24;
/// Shannon entropy threshold over the candidate's bytes.
const ENTROPY_THRESHOLD: f64 = 4.2;
/// Findings kept per file, so one generated file cannot flood the report.
const MAX_PER_FILE: usize = 5;
/// Findings kept in total.
const MAX_FINDINGS: usize = 200;

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".jcs",
    "target",
    "node_modules",
    "dist",
    "build",
    "vendor",
    "coverage",
    "__pycache__",
    ".venv",
    "venv",
    "graphify-out",
];

/// Lockfiles are public dependency metadata: their base64 integrity hashes are not
/// credentials. Scanning them made tasks try to "fix" package managers.
const SKIP_FILES: &[&str] = &[
    "bun.lock",
    "bun.lockb",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.lock",
    "go.sum",
    "poetry.lock",
    "Pipfile.lock",
    "composer.lock",
    "Gemfile.lock",
];

const SKIP_SUFFIXES: &[&str] = &[".min.js", ".min.css", ".map"];

/// Known credential formats, matched literally.
const PREFIXES: &[(&str, &str)] = &[
    ("AKIA", "aws-access-key"),
    ("ASIA", "aws-access-key"),
    ("ghp_", "github-token"),
    ("gho_", "github-token"),
    ("github_pat_", "github-token"),
    ("sk_live_", "stripe-key"),
    ("sk-proj-", "openai-key"),
    ("xoxb-", "slack-token"),
    ("xoxp-", "slack-token"),
    ("AIza", "google-api-key"),
    ("-----BEGIN PRIVATE KEY-----", "private-key"),
    ("-----BEGIN RSA PRIVATE KEY-----", "private-key"),
    ("-----BEGIN OPENSSH PRIVATE KEY-----", "private-key"),
];

/// Tokens that contain these are documentation or fixtures, not credentials.
const BENIGN_MARKERS: &[&str] = &[
    "example",
    "placeholder",
    "your_",
    "your-",
    "xxxx",
    "notreal",
    "changeme",
    "redacted",
    "dummy",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub path: String,
    pub line: u32,
    /// `aws-access-key`, `github-token`, `high-entropy`, …
    pub kind: String,
    /// Redacted excerpt: enough to locate it, not enough to use it.
    pub excerpt: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Path prefixes that are allowed to contain credential-like strings.
    pub allow_prefixes: Vec<String>,
}

/// One file or fragment to scan. Verification passes only the lines a task added, so
/// pre-existing hashes and model names cannot fail an unrelated change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTarget {
    pub path: String,
    pub lines: Vec<(u32, String)>,
}

/// Scans a working tree for likely credentials.
pub fn scan(root: &Path, options: &ScanOptions) -> Result<Vec<SecretFinding>, GateError> {
    let mut targets = Vec::new();
    for path in walk(root) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.len() as u64 > MAX_SCAN_BYTES || bytes.iter().take(4096).any(|byte| *byte == 0) {
            continue;
        }
        let Ok(source) = String::from_utf8(bytes) else {
            continue;
        };
        targets.push(ScanTarget {
            path: relative,
            lines: source
                .lines()
                .enumerate()
                .map(|(index, line)| (index as u32 + 1, line.to_string()))
                .collect(),
        });
    }
    Ok(scan_targets(&targets, options))
}

/// Scans explicit fragments, applying the same exclusions as [`scan`].
pub fn scan_targets(targets: &[ScanTarget], options: &ScanOptions) -> Vec<SecretFinding> {
    let mut findings = Vec::new();

    for target in targets {
        if is_allowed(&target.path, options) || is_skipped_path(&target.path) {
            continue;
        }

        let mut per_file = 0usize;
        for (line, text) in &target.lines {
            if per_file >= MAX_PER_FILE {
                break;
            }
            for finding in scan_line(&target.path, *line, text) {
                findings.push(finding);
                per_file += 1;
                if findings.len() >= MAX_FINDINGS {
                    findings.sort_by(|left, right| {
                        left.path
                            .cmp(&right.path)
                            .then_with(|| left.line.cmp(&right.line))
                    });
                    return findings;
                }
            }
        }
    }

    findings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    findings
}

fn is_allowed(path: &str, options: &ScanOptions) -> bool {
    options
        .allow_prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix.trim_end_matches('/')))
}

fn is_skipped_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    SKIP_FILES.contains(&name)
        || SKIP_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
        || path
            .split('/')
            .any(|component| SKIP_DIRS.contains(&component))
}

fn scan_line(path: &str, line: u32, text: &str) -> Vec<SecretFinding> {
    let mut findings = Vec::new();

    for (prefix, kind) in PREFIXES {
        if let Some(position) = text.find(prefix) {
            if !is_benign(text) {
                findings.push(SecretFinding {
                    path: path.to_string(),
                    line,
                    kind: (*kind).to_string(),
                    excerpt: redact(&text[position..]),
                });
                return findings;
            }
        }
    }

    for token in candidates(text) {
        if is_benign(&token) || !looks_like_secret(&token) {
            continue;
        }
        if entropy(&token) >= ENTROPY_THRESHOLD && has_letter_and_digit(&token) {
            findings.push(SecretFinding {
                path: path.to_string(),
                line,
                kind: "high-entropy".to_string(),
                excerpt: redact(&token),
            });
            break;
        }
    }

    findings
}

fn candidates(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in text.chars() {
        if character.is_ascii_alphanumeric() || "-_+/=.".contains(character) {
            current.push(character);
        } else {
            if current.len() >= ENTROPY_MIN_LEN {
                tokens.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() >= ENTROPY_MIN_LEN {
        tokens.push(current);
    }
    tokens
}

fn entropy(token: &str) -> f64 {
    if token.is_empty() {
        return 0.0;
    }

    let mut counts: HashMap<u8, usize> = HashMap::new();
    for byte in token.bytes() {
        *counts.entry(byte).or_default() += 1;
    }

    let length = token.len() as f64;
    counts
        .values()
        .map(|count| {
            let probability = *count as f64 / length;
            -probability * probability.log2()
        })
        .sum()
}

fn has_letter_and_digit(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_alphabetic()) && token.chars().any(|c| c.is_ascii_digit())
}

/// A generic credential does not look like a path, a model identifier or a version.
///
/// `speaches-ai/Kokoro-82M-v1.0-ONNX` and `OPENCODE_EXPERIMENTAL_BASH_TIMEOUT_MS` are
/// separator-rich and were the false positives that sent agents into repair loops. Real
/// API keys with a known prefix are matched before this heuristic; the rest are compact
/// base64/alphanumeric blobs.
fn looks_like_secret(token: &str) -> bool {
    !token
        .chars()
        .any(|character| matches!(character, '/' | '-' | '_' | '.'))
}

fn is_benign(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    BENIGN_MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// Keeps enough to locate the secret and nothing more.
fn redact(value: &str) -> String {
    let value = value.trim();
    let prefix: String = value.chars().take(4).collect();
    format!("{prefix}… ({} chars, redacted)", value.chars().count())
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

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
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() {
                // A worktree's `.git` is a *file* pointing at the real gitdir. Its path is
                // long and random-looking, and it is never a credential. Scanning it made
                // the agent try to "fix" git metadata and break the worktree.
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name == ".git" || name == ".jcs" {
                    continue;
                }
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

    fn scan_text(text: &str) -> Vec<SecretFinding> {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("config.rs"), text).unwrap();
        scan(directory.path(), &ScanOptions::default()).unwrap()
    }

    #[test]
    fn known_prefixes_are_detected_and_redacted() {
        let findings = scan_text("const KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";\n");
        // "EXAMPLE" is benign and must not be reported: fixtures are full of it.
        assert!(findings.is_empty(), "{findings:?}");

        let findings = scan_text("const KEY: &str = \"AKIAZZZZ1234QWERTY\";\n");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "aws-access-key");
        assert!(
            !findings[0].excerpt.contains("ZZZZ1234"),
            "leaked: {}",
            findings[0].excerpt
        );
        assert!(findings[0].excerpt.contains("AKIA"));
    }

    #[test]
    fn private_keys_and_github_tokens_are_detected() {
        assert_eq!(
            scan_text("-----BEGIN PRIVATE KEY-----\nabc\n")[0].kind,
            "private-key"
        );
        assert_eq!(
            scan_text("token = \"ghp_Zx9Qw8Er7Ty6Ui5Op4\";\n")[0].kind,
            "github-token"
        );
        assert_eq!(
            scan_text("token = \"sk-proj-Ab12Cd34Ef56Gh78\";\n")[0].kind,
            "openai-key"
        );
    }

    #[test]
    fn random_looking_tokens_are_flagged_but_paths_and_hashes_are_not() {
        // 32 hex chars: entropy is exactly 4.0, below the 4.2 threshold.
        assert!(scan_text("let sha = \"9f2c8ab1deadbeef9f2c8ab1deadbeef\";").is_empty());

        let findings = scan_text("let key = \"Qw8Zr3Lp6Xv0Nb2Ty5Um9Kj4Hd7Sg1Fa\";");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "high-entropy");
    }

    #[test]
    fn allow_prefixes_suppress_whole_directories() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("tests/fixtures")).unwrap();
        std::fs::write(
            directory.path().join("tests/fixtures/creds.rs"),
            "const K: &str = \"AKIAZZZZ1234QWERTY\";\n",
        )
        .unwrap();

        let allowed = ScanOptions {
            allow_prefixes: vec!["tests/fixtures".into()],
        };
        assert!(scan(directory.path(), &allowed).unwrap().is_empty());

        let strict = ScanOptions::default();
        assert_eq!(scan(directory.path(), &strict).unwrap().len(), 1);
    }

    #[test]
    fn binary_and_oversized_files_are_skipped() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("blob.bin"), b"\0\0AKIAZZZZ1234QWERTY").unwrap();
        assert!(scan(directory.path(), &ScanOptions::default())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn entropy_threshold_separates_random_from_words() {
        assert!(entropy("correct-horse-battery-staple") < 4.2);
        assert!(entropy("Qw8Zr3Lp6Xv0Nb2Ty5Um9Kj4Hd7Sg1Fa") >= 4.2);
    }

    #[test]
    fn model_names_and_env_variables_are_not_secrets() {
        for text in [
            "model: \"speaches-ai/Kokoro-82M-v1.0-ONNX\"",
            "OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS=1800000",
            "EMBEDDINGS_MODEL=text-embedding-3-small",
        ] {
            assert!(scan_text(text).is_empty(), "false positive for {text}");
        }
    }

    #[test]
    fn only_supplied_lines_are_scanned() {
        let targets = vec![ScanTarget {
            path: "src/config.rs".to_string(),
            lines: vec![(
                9,
                "const KEY: &str = \"Qw8Zr3Lp6Xv0Nb2Ty5Um9Kj4Hd7Sg1Fa\";".to_string(),
            )],
        }];
        let findings = scan_targets(&targets, &ScanOptions::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 9);
        assert_eq!(findings[0].kind, "high-entropy");
    }

    #[test]
    fn lockfiles_and_generated_output_are_skipped() {
        let target = |path: &str| ScanTarget {
            path: path.to_string(),
            lines: vec![(
                1,
                "hash = \"sha512-Qw8Zr3Lp6Xv0Nb2Ty5Um9Kj4Hd7Sg1Fa==\"".to_string(),
            )],
        };
        let targets = vec![
            target("bun.lock"),
            target("graphify-out/graph.html"),
            target("assets/app.min.js"),
        ];
        assert!(scan_targets(&targets, &ScanOptions::default()).is_empty());
    }
}
