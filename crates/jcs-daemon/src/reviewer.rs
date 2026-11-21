//! Code review with a rubric.
//!
//! Same discipline as the planner: the model returns JSON, the domain validates it, and a
//! malformed answer gets exactly one repair. Every finding must cite `file:line`; a finding
//! without a location is rejected rather than trusted.

use anyhow::{anyhow, Context, Result};
use jcs_agent::{ModelRef, OpenCodeClient};
use jcs_core::{Goal, Project, Task};
use jcs_git::Repo;
use serde::Deserialize;

use crate::planner::{collect_response, extract_json_object};
use crate::state::AppState;

/// The model sees a bounded diff; beyond this the review degrades into guessing.
const MAX_DIFF_CHARS: usize = 40_000;
const MAX_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "info" => Some(Self::Info),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFinding {
    pub severity: Severity,
    pub file: String,
    pub line: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub approve: bool,
    pub summary: String,
    pub findings: Vec<ReviewFinding>,
}

impl Verdict {
    /// Rendered for the event log and the repair prompt.
    pub fn render(&self) -> String {
        let mut lines = vec![self.summary.trim().to_string()];
        for finding in &self.findings {
            lines.push(format!(
                "- [{}] {}:{}: {}",
                finding.severity.as_str(),
                finding.file,
                finding.line,
                finding.message
            ));
        }
        lines.join("\n")
    }

    /// Findings a human must act on before this can be accepted.
    pub fn blocking(&self) -> impl Iterator<Item = &ReviewFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity == Severity::Error)
    }
}

#[derive(Debug, Deserialize)]
struct RawVerdict {
    verdict: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    findings: Vec<RawFinding>,
}

#[derive(Debug, Deserialize)]
struct RawFinding {
    severity: String,
    file: String,
    line: u32,
    message: String,
}

/// Reviews the staged diff of a task worktree.
pub async fn review(
    state: &AppState,
    project: &Project,
    goal: &Goal,
    task: &Task,
    worktree: &std::path::Path,
    base_branch: &str,
) -> Result<Verdict> {
    let repo = Repo::new(worktree);
    repo.add_all()
        .await
        .context("cannot stage the worktree for review")?;
    let diff = repo
        .diff_staged(base_branch)
        .await
        .context("cannot read the staged diff")?;

    if diff.trim().is_empty() {
        return Ok(Verdict {
            approve: true,
            summary: "no changes to review".to_string(),
            findings: Vec::new(),
        });
    }

    let diff = truncate(&diff, MAX_DIFF_CHARS);

    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    let session = agent
        .create_session(jcs_agent::CreateSession {
            title: "review",
            directory: worktree,
            model: state
                .review_model()
                .or_else(|| state.default_model())
                .map(|reference| ModelRef::parse(&reference))
                .transpose()?,
            agent: None,
            permissions: None,
        })
        .await
        .context("cannot open a review session")?;

    let result = review_in_session(&agent, &session.id, state, project, goal, task, &diff).await;

    if let Err(error) = agent.interrupt(&session.id).await {
        tracing::debug!(error = %error, "cannot interrupt the review session");
    }

    result
}

async fn review_in_session(
    agent: &OpenCodeClient,
    session_id: &str,
    state: &AppState,
    project: &Project,
    goal: &Goal,
    task: &Task,
    diff: &str,
) -> Result<Verdict> {
    let mut prompt = review_prompt(project, goal, task, diff, state.config.review_max_findings);

    for attempt in 1..=MAX_ATTEMPTS {
        agent
            .prompt(session_id, &prompt, "queue")
            .await
            .context("cannot deliver the review prompt")?;

        let response = collect_response(agent, session_id).await?;

        let Some(json) = extract_json_object(&response) else {
            prompt = "Your previous answer contained no JSON object. Return ONLY the JSON \
                      verdict object, no prose and no markdown fences."
                .to_string();
            tracing::warn!(attempt, "reviewer returned no JSON");
            continue;
        };

        match parse_verdict(json, state.config.review_max_findings) {
            Ok(verdict) => return Ok(verdict),
            Err(error) => {
                prompt = format!(
                    "Your previous verdict was invalid: {error}.\n\
                     Return ONLY the corrected JSON object, no prose. Every finding needs an \
                     existing `file`, a `line` above 0, and a `message`."
                );
                tracing::warn!(attempt, error = %error, "reviewer produced an invalid verdict");
            }
        }
    }

    Err(anyhow!(
        "the reviewer did not produce a valid verdict after {MAX_ATTEMPTS} attempts"
    ))
}

/// Parses and validates a verdict. Public for tests.
pub fn parse_verdict(json: &str, max_findings: usize) -> Result<Verdict> {
    let raw: RawVerdict = serde_json::from_str(json)?;

    let approve = match raw.verdict.as_str() {
        "approve" => true,
        "request_changes" => false,
        other => return Err(anyhow!("unknown verdict {other:?}")),
    };

    let mut findings = Vec::new();
    for finding in raw.findings.into_iter().take(max_findings.max(1)) {
        if finding.file.trim().is_empty() {
            return Err(anyhow!("a finding has an empty file"));
        }
        if finding.file.starts_with('/') || finding.file.contains("..") {
            return Err(anyhow!(
                "finding file {:?} must be repository-relative",
                finding.file
            ));
        }
        if finding.line == 0 {
            return Err(anyhow!("finding in {:?} has line 0", finding.file));
        }
        if finding.message.trim().is_empty() {
            return Err(anyhow!(
                "finding in {:?} has an empty message",
                finding.file
            ));
        }

        let severity = Severity::parse(&finding.severity).ok_or_else(|| {
            anyhow!(
                "unknown severity {:?}; expected error, warning or info",
                finding.severity
            )
        })?;

        findings.push(ReviewFinding {
            severity,
            file: finding.file,
            line: finding.line,
            message: finding.message.trim().to_string(),
        });
    }

    // A request_changes with no actionable finding is not actionable: treat it as an
    // approval with a note, rather than blocking on an opinion.
    let blocking = findings
        .iter()
        .any(|finding| finding.severity == Severity::Error);
    let approve = approve || !blocking;

    Ok(Verdict {
        approve,
        summary: raw.summary.trim().to_string(),
        findings,
    })
}

fn review_prompt(
    project: &Project,
    goal: &Goal,
    task: &Task,
    diff: &str,
    max_findings: usize,
) -> String {
    format!(
        "You are reviewing an engineering change. Do not write code. Judge the diff against \
         the objective, correctness, security and the project rules.\n\
         \n\
         Project: {project}\n\
         Objective:\n{objective}\n\
         Task: {title}\n\
         \n\
         Return ONLY a JSON object, no prose and no markdown fences:\n\
         {{\n\
         \x20 \"verdict\": \"approve\" | \"request_changes\",\n\
         \x20 \"summary\": \"one or two sentences\",\n\
         \x20 \"findings\": [\n\
         \x20   {{\"severity\": \"error\" | \"warning\" | \"info\", \"file\": \"src/x.rs\", \"line\": 12, \"message\": \"what is wrong and why\"}}\n\
         \x20 ]\n\
         }}\n\
         \n\
         Rules:\n\
         - Every finding must cite an existing repository-relative file and a real line.\n\
         - Use `error` only for correctness, security, data loss or a policy violation.\n\
         - Do not report style preferences unless a project rule demands them.\n\
         - At most {max_findings} findings, most important first.\n\
         - `request_changes` requires at least one `error` finding.\n\
         \n\
         Diff:\n{diff}\n",
        project = project.name,
        objective = goal.prompt,
        title = task.title,
    )
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect::<String>()
        + "\n… (diff truncated for review; inspect the worktree for the rest)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_verdict_is_parsed() {
        let verdict = parse_verdict(
            r#"{"verdict":"request_changes","summary":"missing validation","findings":[
                {"severity":"error","file":"src/api.rs","line":42,"message":"accepts empty input"}
            ]}"#,
            10,
        )
        .unwrap();

        assert!(!verdict.approve);
        assert_eq!(verdict.findings.len(), 1);
        assert_eq!(verdict.findings[0].line, 42);
        assert_eq!(verdict.blocking().count(), 1);
    }

    #[test]
    fn a_warning_only_verdict_does_not_block() {
        let verdict = parse_verdict(
            r#"{"verdict":"approve","summary":"fine","findings":[
                {"severity":"warning","file":"src/x.rs","line":3,"message":"could be clearer"}
            ]}"#,
            10,
        )
        .unwrap();
        assert!(verdict.approve);
        assert_eq!(verdict.blocking().count(), 0);
    }

    #[test]
    fn request_changes_without_an_error_is_not_blocking() {
        let verdict = parse_verdict(
            r#"{"verdict":"request_changes","summary":"opinion","findings":[]}"#,
            10,
        )
        .unwrap();
        assert!(
            verdict.approve,
            "an opinion without a finding must not block"
        );
    }

    #[test]
    fn findings_must_cite_a_real_location() {
        for json in [
            r#"{"verdict":"request_changes","findings":[{"severity":"error","file":"","line":1,"message":"x"}]}"#,
            r#"{"verdict":"request_changes","findings":[{"severity":"error","file":"/etc/passwd","line":1,"message":"x"}]}"#,
            r#"{"verdict":"request_changes","findings":[{"severity":"error","file":"../x","line":1,"message":"x"}]}"#,
            r#"{"verdict":"request_changes","findings":[{"severity":"error","file":"src/x.rs","line":0,"message":"x"}]}"#,
            r#"{"verdict":"request_changes","findings":[{"severity":"error","file":"src/x.rs","line":1,"message":""}]}"#,
        ] {
            assert!(parse_verdict(json, 10).is_err(), "accepted {json}");
        }
    }

    #[test]
    fn unknown_verdicts_and_severities_are_rejected() {
        assert!(parse_verdict(r#"{"verdict":"lgtm"}"#, 10).is_err());
        assert!(parse_verdict(
            r#"{"verdict":"request_changes","findings":[{"severity":"blocker","file":"x.rs","line":1,"message":"x"}]}"#,
            10
        )
        .is_err());
    }

    #[test]
    fn findings_are_capped() {
        let findings = (1..=20)
            .map(|line| {
                format!(r#"{{"severity":"warning","file":"src/x.rs","line":{line},"message":"m"}}"#)
            })
            .collect::<Vec<_>>()
            .join(",");

        let verdict = parse_verdict(
            &format!(r#"{{"verdict":"approve","findings":[{findings}]}}"#),
            5,
        )
        .unwrap();
        assert_eq!(verdict.findings.len(), 5);
    }

    #[test]
    fn the_prompt_contains_the_diff_and_the_rules() {
        let project = Project {
            id: jcs_core::ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        let goal = Goal {
            id: jcs_core::GoalId::new(),
            project_id: project.id.clone(),
            prompt: "add validation".into(),
            status: jcs_core::GoalStatus::Running,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };
        let task = Task {
            id: jcs_core::TaskId::new(),
            goal_id: goal.id.clone(),
            title: "validate email".into(),
            instructions: None,
            status: jcs_core::TaskStatus::Verifying,
            worktree: None,
            branch: None,
            session_id: None,
            role: jcs_core::TaskRole::Implement,
            priority: 0,
            path_claims: Vec::new(),
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };

        let prompt = review_prompt(&project, &goal, &task, "diff --git a/x b/x", 10);
        assert!(prompt.contains("Return ONLY a JSON object"));
        assert!(prompt.contains("add validation"));
        assert!(prompt.contains("diff --git"));
        assert!(prompt.contains("request_changes"));
    }

    #[test]
    fn a_huge_diff_is_truncated_with_a_note() {
        let truncated = truncate(&"x".repeat(100_000), 1_000);
        assert!(truncated.chars().count() < 1_200);
        assert!(truncated.contains("truncated"));
    }
}
