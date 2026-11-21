//! Output interpretation.
//!
//! Parsers are deliberately conservative: they extract a bounded, actionable excerpt and
//! a handful of findings. They never decide success on their own except for coverage,
//! where the exit code of a coverage tool is often 0 regardless of the percentage.

use jcs_core::GateStatus;

use crate::model::{Finding, ParserKind, Severity};

/// Maximum number of findings kept per gate.
const MAX_FINDINGS: usize = 50;
/// Maximum number of trailing lines kept as the excerpt.
const EXCERPT_LINES: usize = 40;
/// Hard cap on the excerpt, in characters.
const EXCERPT_CHARS: usize = 8_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub findings: Vec<Finding>,
    /// Set when the output itself determines the verdict (coverage thresholds).
    pub status_override: Option<GateStatus>,
}

pub fn parse(
    kind: ParserKind,
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    min_coverage: Option<f64>,
) -> Parsed {
    let combined = format!("{stdout}\n{stderr}");
    let clean = strip_ansi(&combined);

    match kind {
        ParserKind::ExitCode => Parsed {
            findings: Vec::new(),
            status_override: None,
        },
        ParserKind::Generic => Parsed {
            findings: collect_generic(&clean),
            status_override: None,
        },
        ParserKind::Cargo => Parsed {
            findings: collect_cargo(&clean),
            status_override: None,
        },
        ParserKind::Node => Parsed {
            findings: collect_node(&clean),
            status_override: None,
        },
        ParserKind::Coverage => coverage(&clean, exit_code, min_coverage),
    }
}

/// The excerpt shown to the repair prompt: the tail of whichever stream carried output.
pub fn excerpt(stdout: &str, stderr: &str) -> String {
    let source = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let clean = strip_ansi(source);

    let lines: Vec<&str> = clean.lines().collect();
    let start = lines.len().saturating_sub(EXCERPT_LINES);
    let mut text = lines[start..].join("\n");

    if text.chars().count() > EXCERPT_CHARS {
        let skip = text.chars().count() - EXCERPT_CHARS;
        text = text.chars().skip(skip).collect();
        text.insert(0, '…');
    }

    text.trim_end().to_string()
}

fn collect_generic(text: &str) -> Vec<Finding> {
    text.lines()
        .filter_map(|line| {
            let lowered = line.to_lowercase();
            if lowered.contains("error") || lowered.contains("failed") || lowered.contains("panic")
            {
                let severity = if lowered.contains("warning") {
                    Severity::Warning
                } else {
                    Severity::Error
                };
                Some(finding(severity, line))
            } else {
                None
            }
        })
        .take(MAX_FINDINGS)
        .collect()
}

fn collect_cargo(text: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();

        let is_error = trimmed.starts_with("error[")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("error ")
            || trimmed.contains("FAILED")
            || trimmed.starts_with("panicked at")
            || trimmed.starts_with("thread '");
        let is_warning = trimmed.starts_with("warning:");

        if !is_error && !is_warning {
            continue;
        }

        let mut finding = finding(
            if is_error {
                Severity::Error
            } else {
                Severity::Warning
            },
            line,
        );

        // Rust puts the location on one of the lines right after the diagnostic.
        if finding.location.is_none() {
            finding.location = lines[index + 1..(index + 4).min(lines.len())]
                .iter()
                .find_map(|candidate| extract_location(candidate));
        }

        findings.push(finding);
        if findings.len() >= MAX_FINDINGS {
            break;
        }
    }

    findings
}

fn collect_node(text: &str) -> Vec<Finding> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let is_error = trimmed.contains("error TS")
                || trimmed.starts_with("error ")
                || trimmed.starts_with("Error:")
                || trimmed.contains("✖")
                || trimmed.contains("FAIL ")
                || trimmed.contains("● ");
            let is_warning = trimmed.contains("warning");

            if is_error {
                Some(finding(Severity::Error, line))
            } else if is_warning {
                Some(finding(Severity::Warning, line))
            } else {
                None
            }
        })
        .take(MAX_FINDINGS)
        .collect()
}

fn coverage(text: &str, exit_code: Option<i32>, min: Option<f64>) -> Parsed {
    let Some(percent) = last_percentage(text) else {
        return Parsed {
            findings: vec![Finding {
                severity: Severity::Warning,
                message: "could not read a coverage percentage from the output".to_string(),
                location: None,
            }],
            status_override: None,
        };
    };

    match min {
        None => Parsed {
            findings: Vec::new(),
            status_override: None,
        },
        Some(minimum) => {
            let passed = percent >= minimum;
            let mut findings = Vec::new();
            if !passed {
                findings.push(Finding {
                    severity: Severity::Error,
                    message: format!(
                        "line coverage {percent:.2}% is below the required {minimum:.2}%"
                    ),
                    location: None,
                });
            }

            // A non-zero exit still fails, even when the parsed percentage looks fine.
            let status = if exited_cleanly(exit_code) && passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed
            };

            Parsed {
                findings,
                status_override: Some(status),
            }
        }
    }
}

fn exited_cleanly(exit_code: Option<i32>) -> bool {
    matches!(exit_code, Some(0) | None)
}

/// Returns the last `NN.N%` value in the output.
///
/// Coverage tools print a table; the last total is the one that matters.
fn last_percentage(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut result = None;
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            let mut seen_dot = false;
            while index < bytes.len()
                && (bytes[index].is_ascii_digit() || (!seen_dot && bytes[index] == b'.'))
            {
                if bytes[index] == b'.' {
                    seen_dot = true;
                }
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'%' {
                if let Ok(value) = text[start..index].parse::<f64>() {
                    result = Some(value);
                }
            }
        }
        index += 1;
    }

    result
}

fn finding(severity: Severity, line: &str) -> Finding {
    Finding {
        severity,
        message: line.trim().to_string(),
        location: extract_location(line),
    }
}

/// Best-effort `file:line` extraction from common tool formats.
fn extract_location(line: &str) -> Option<String> {
    // rustc: `  --> src/main.rs:12:5`
    if let Some(rest) = line.split(" --> ").nth(1) {
        let location = rest.trim();
        if !location.is_empty() {
            return Some(location.to_string());
        }
    }

    // `path/to/file.rs:12:5` or `src/app.ts(10,7):`
    for token in line.split_whitespace() {
        let candidate = token.trim_end_matches(':');
        if is_colon_location(candidate) || is_paren_location(candidate) {
            return Some(candidate.to_string());
        }
    }

    None
}

fn is_colon_location(token: &str) -> bool {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() < 3 {
        return false;
    }
    let numbers = &parts[parts.len() - 2..];
    numbers
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        && parts[0].contains('.')
}

fn is_paren_location(token: &str) -> bool {
    let Some(open) = token.find('(') else {
        return false;
    };
    if !token.ends_with(')') {
        return false;
    }
    let inside = &token[open + 1..token.len() - 1];
    let mut parts = inside.split(',');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(line), Some(column), None)
            if !line.is_empty()
                && !column.is_empty()
                && line.chars().all(|c| c.is_ascii_digit())
                && column.chars().all(|c| c.is_ascii_digit())
    ) && token[..open].contains('.')
}

/// Removes ANSI CSI sequences so parsers see plain text.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('[') => {
                // CSI: consume until a final byte in @A-Za-z.
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() || next == '@' {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC: consume until BEL or ST.
                for next in chars.by_ref() {
                    if next == '\u{7}' {
                        break;
                    }
                }
            }
            Some(_) | None => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO_ERROR: &str = "\
   Compiling demo v0.1.0
error[E0308]: mismatched types
  --> src/main.rs:12:5
   |
12 |     let x: u32 = \"text\";
   |            ---   ^^^^^^ expected `u32`, found `&str`
warning: unused variable: `y`
  --> src/main.rs:20:9

error: could not compile `demo` (bin \"demo\") due to 1 previous error
";

    #[test]
    fn cargo_parser_extracts_errors_and_locations() {
        let parsed = parse(ParserKind::Cargo, "", CARGO_ERROR, Some(101), None);
        assert!(parsed.status_override.is_none());

        let errors: Vec<_> = parsed
            .findings
            .iter()
            .filter(|finding| finding.severity == Severity::Error)
            .collect();
        assert!(!errors.is_empty());

        let location = parsed
            .findings
            .iter()
            .find_map(|finding| finding.location.as_deref());
        assert_eq!(location, Some("src/main.rs:12:5"));
    }

    #[test]
    fn node_parser_recognises_type_errors() {
        let output = "\
src/app.ts(10,7): error TS2322: Type 'string' is not assignable to type 'number'.
warning: unused import
";
        let parsed = parse(ParserKind::Node, output, "", Some(2), None);
        assert!(parsed
            .findings
            .iter()
            .any(|finding| finding.message.contains("TS2322")));
    }

    #[test]
    fn coverage_threshold_is_enforced_even_when_the_tool_exits_zero() {
        let output = "\
Filename       Regions    Missed Regions     Cover
src/lib.rs          10                 1     90.00%
TOTAL               10                 1     87.30%
";
        let parsed = parse(ParserKind::Coverage, output, "", Some(0), Some(90.0));
        assert_eq!(parsed.status_override, Some(GateStatus::Failed));
        assert!(parsed.findings[0].message.contains("87.30%"));

        let parsed = parse(ParserKind::Coverage, output, "", Some(0), Some(80.0));
        assert_eq!(parsed.status_override, Some(GateStatus::Passed));
    }

    #[test]
    fn coverage_without_a_percentage_falls_back_to_the_exit_code() {
        let parsed = parse(
            ParserKind::Coverage,
            "no numbers here",
            "",
            Some(1),
            Some(80.0),
        );
        assert!(parsed.status_override.is_none());
        assert_eq!(parsed.findings[0].severity, Severity::Warning);
    }

    #[test]
    fn exit_code_parser_never_invents_findings() {
        let parsed = parse(
            ParserKind::ExitCode,
            "error: looks scary",
            "",
            Some(0),
            None,
        );
        assert!(parsed.findings.is_empty());
    }

    #[test]
    fn ansi_sequences_are_stripped_before_parsing() {
        let colored = "\u{1b}[31merror\u{1b}[0m: boom";
        let parsed = parse(ParserKind::Generic, colored, "", Some(1), None);
        assert_eq!(parsed.findings[0].message, "error: boom");
    }

    #[test]
    fn excerpt_prefers_stderr_and_keeps_only_the_tail() {
        let stdout = "irrelevant\n";
        let stderr = (0..100)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let text = excerpt(stdout, &stderr);
        assert!(!text.contains("line 0\n"));
        assert!(text.contains("line 99"));
        assert!(text.lines().count() <= EXCERPT_LINES);
    }

    #[test]
    fn excerpt_is_hard_capped_in_characters() {
        let huge = "é".repeat(50_000);
        let text = excerpt(&huge, "");
        assert!(text.chars().count() <= EXCERPT_CHARS + 1);
    }

    #[test]
    fn percentages_are_found_in_various_formats() {
        assert_eq!(last_percentage("lines......: 87.30%"), Some(87.30));
        assert_eq!(last_percentage("coverage: 100%"), Some(100.0));
        assert_eq!(last_percentage("no percent"), None);
    }
}
