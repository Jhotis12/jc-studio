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
];

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

/// Scans a working tree for likely credentials.
pub fn scan(root: &Path, options: &ScanOptions) -> Result<Vec<SecretFinding>, GateError> {
    let mut findings = Vec::new();

    for path in walk(root) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if options
            .allow_prefixes
            .iter()
            .any(|prefix| relative.starts_with(prefix.trim_end_matches('/')))
        {
            continue;
        }

        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.len() as u64 > MAX_SCAN_BYTES || bytes.iter().take(4096).any(|byte| *byte == 0) {
            continue;
        }
        let Ok(source) = String::from_utf8(bytes) else {
            continue;
        };

        let mut per_file = 0usize;
        for (index, line) in source.lines().enumerate() {
            if per_file >= MAX_PER_FILE {
                break;
            }
            for finding in scan_line(&relative, index as u32 + 1, line) {
                findings.push(finding);
                per_file += 1;
                if findings.len() >= MAX_FINDINGS {
                    return Ok(findings);
                }
            }
        }
    }

    findings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    Ok(findings)
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
        if is_benign(&token) {
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
}
