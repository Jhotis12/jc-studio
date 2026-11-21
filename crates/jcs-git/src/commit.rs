//! Conventional Commits: construction and validation.
//!
//! The validator is intentionally strict. A machine-generated history is only useful if
//! it is uniform enough to parse later, so a malformed summary fails here instead of
//! landing in the log and being unparseable forever.

use std::fmt;

use crate::error::{GitError, Result};

/// Allowed commit types. Anything else is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitKind {
    Feat,
    Fix,
    Refactor,
    Test,
    Docs,
    Chore,
    Perf,
    Ci,
}

impl CommitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::Chore => "chore",
            Self::Perf => "perf",
            Self::Ci => "ci",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "feat" => Ok(Self::Feat),
            "fix" => Ok(Self::Fix),
            "refactor" => Ok(Self::Refactor),
            "test" => Ok(Self::Test),
            "docs" => Ok(Self::Docs),
            "chore" => Ok(Self::Chore),
            "perf" => Ok(Self::Perf),
            "ci" => Ok(Self::Ci),
            other => Err(GitError::InvalidMessage(format!(
                "unknown commit type {other:?}; expected one of \
                 feat, fix, refactor, test, docs, chore, perf, ci"
            ))),
        }
    }
}

/// A commit message under construction.
#[derive(Debug, Clone)]
pub struct ConventionalCommit<'a> {
    pub kind: CommitKind,
    pub scope: Option<&'a str>,
    pub summary: &'a str,
    pub body: Option<&'a str>,
}

impl<'a> ConventionalCommit<'a> {
    pub fn new(kind: CommitKind, summary: &'a str) -> Self {
        Self {
            kind,
            scope: None,
            summary,
            body: None,
        }
    }

    pub fn scope(mut self, scope: &'a str) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    /// Renders the message and validates it.
    pub fn render(&self) -> Result<String> {
        let mut message = String::new();
        message.push_str(self.kind.as_str());
        if let Some(scope) = self.scope {
            message.push('(');
            message.push_str(scope);
            message.push(')');
        }
        message.push_str(": ");
        message.push_str(self.summary.trim());

        if let Some(body) = self.body {
            let body = body.trim();
            if !body.is_empty() {
                message.push_str("\n\n");
                message.push_str(body);
            }
        }

        validate(&message)?;
        Ok(message)
    }
}

impl fmt::Display for ConventionalCommit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render().unwrap_or_default())
    }
}

/// Validates a full commit message.
pub fn validate(message: &str) -> Result<()> {
    let mut lines = message.lines();
    let header = lines.next().unwrap_or_default().trim();

    if header.is_empty() {
        return Err(GitError::InvalidMessage("message is empty".into()));
    }
    if header.chars().count() > 72 {
        return Err(GitError::InvalidMessage(format!(
            "summary is {} characters; keep it under 72",
            header.chars().count()
        )));
    }
    if header.ends_with('.') {
        return Err(GitError::InvalidMessage(
            "summary must not end with a period".into(),
        ));
    }

    let (prefix, summary) = header.split_once(": ").ok_or_else(|| {
        GitError::InvalidMessage("expected `<type>(<scope>): <summary>`".into())
    })?;

    let (kind, scope) = match prefix.split_once('(') {
        Some((kind, rest)) => {
            let scope = rest.strip_suffix(')').ok_or_else(|| {
                GitError::InvalidMessage("unclosed scope parenthesis".into())
            })?;
            if scope.is_empty() {
                return Err(GitError::InvalidMessage("scope is empty".into()));
            }
            if !scope
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(GitError::InvalidMessage(format!(
                    "scope {scope:?} must be lowercase alphanumeric with dashes"
                )));
            }
            (kind, Some(scope))
        }
        None => (prefix, None),
    };

    CommitKind::parse(kind)?;
    let _ = scope;

    if summary.trim().is_empty() {
        return Err(GitError::InvalidMessage("summary is empty".into()));
    }
    if let Some(first) = summary.chars().next() {
        if first.is_uppercase() {
            return Err(GitError::InvalidMessage(
                "summary must start in lowercase (imperative mood)".into(),
            ));
        }
    }

    Ok(())
}

/// Turns free text into a branch-name-safe slug.
///
/// ASCII-only and length-bounded so branch names stay portable across tooling.
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true;

    for ch in input.chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'A'..='Z' => Some(ch.to_ascii_lowercase()),
            'á' | 'à' | 'ä' | 'â' => Some('a'),
            'é' | 'è' | 'ë' | 'ê' => Some('e'),
            'í' | 'ì' | 'ï' | 'î' => Some('i'),
            'ó' | 'ò' | 'ö' | 'ô' => Some('o'),
            'ú' | 'ù' | 'ü' | 'û' => Some('u'),
            'ñ' => Some('n'),
            _ => None,
        };

        match mapped {
            Some(ch) => {
                out.push(ch);
                last_dash = false;
            }
            None => {
                if !last_dash {
                    out.push('-');
                    last_dash = true;
                }
            }
        }

        if out.chars().count() >= 40 {
            break;
        }
    }

    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("task");
    }
    out
}

/// Builds the branch name for a task: `jcs/<goal-slug>/<task-suffix>`.
pub fn branch_name(goal_slug: &str, task_id: &str) -> String {
    let suffix: String = task_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("jcs/{}/{suffix}", slugify(goal_slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_message_passes() {
        validate("feat(store): add append-only event log").unwrap();
        validate("fix: reject illegal task transitions").unwrap();
        validate("docs: explain the attach mode").unwrap();
    }

    #[test]
    fn scope_is_optional_and_may_contain_dashes() {
        validate("feat(agent-sse): decode data-only frames").unwrap();
    }

    #[test]
    fn unknown_types_are_rejected() {
        let error = validate("wip: something").unwrap_err();
        assert!(error.to_string().contains("unknown commit type"));
    }

    #[test]
    fn missing_separator_is_rejected() {
        assert!(validate("feat add a thing").is_err());
    }

    #[test]
    fn uppercase_summaries_are_rejected() {
        assert!(validate("feat: Add a thing").is_err());
    }

    #[test]
    fn trailing_period_is_rejected() {
        assert!(validate("feat: add a thing.").is_err());
    }

    #[test]
    fn overlong_summaries_are_rejected() {
        let summary = "a".repeat(80);
        assert!(validate(&format!("feat: {summary}")).is_err());
    }

    #[test]
    fn unclosed_scope_is_rejected() {
        assert!(validate("feat(scope: add a thing").is_err());
    }

    #[test]
    fn uppercase_scope_is_rejected() {
        assert!(validate("feat(Store): add a thing").is_err());
    }

    #[test]
    fn builder_renders_header_and_body() {
        let message = ConventionalCommit::new(CommitKind::Feat, "add a thing")
            .scope("store")
            .body("Because the old one was wrong.")
            .render()
            .unwrap();
        assert_eq!(
            message,
            "feat(store): add a thing\n\nBecause the old one was wrong."
        );
    }

    #[test]
    fn builder_rejects_a_bad_summary() {
        let error = ConventionalCommit::new(CommitKind::Feat, "Add a thing")
            .render()
            .unwrap_err();
        assert!(matches!(error, GitError::InvalidMessage(_)));
    }

    #[test]
    fn slugs_are_ascii_bounded_and_dash_separated() {
        assert_eq!(slugify("Add email validation!"), "add-email-validation");
        assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
        assert_eq!(slugify("Añadir validación de correo"), "anadir-validacion-de-correo");
        assert_eq!(slugify(""), "task");
        let long = slugify(&"x".repeat(200));
        assert!(long.chars().count() <= 40, "{long}");
    }

    #[test]
    fn branch_names_carry_the_goal_and_a_task_suffix() {
        let branch = branch_name("Add email validation", "task_9f2c8ab1deadbeef");
        assert!(branch.starts_with("jcs/add-email-validation/"), "{branch}");
        assert!(branch.ends_with("deadbeef"), "{branch}");
    }
}
