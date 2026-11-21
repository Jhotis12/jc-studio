//! Gate execution.
//!
//! Three properties matter and are all tested:
//!   * output is bounded, so a runaway compiler cannot exhaust memory;
//!   * a timeout kills the whole process group, not just the direct child, so `cargo`
//!     cannot leave `rustc` running after being abandoned;
//!   * a gate that cannot even start is `Errored`, never silently `Passed`.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use jcs_core::GateStatus;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::model::{Finding, GateDef, GateError, GateOutcome, Severity};
use crate::parser;

/// Per-stream capture cap. Gates are for diagnosis, not for archiving logs.
const OUTPUT_CAP: usize = 256 * 1024;

/// Runs one gate in `cwd` and returns a verdict.
pub async fn run(def: &GateDef, cwd: &Path) -> GateOutcome {
    if let Err(error) = def.validate() {
        return errored(def, error.to_string(), Duration::ZERO, false);
    }
    if let Err(error) = crate::model::ensure_directory(cwd) {
        return errored(def, error.to_string(), Duration::ZERO, false);
    }

    let started = Instant::now();

    let mut command = Command::new(&def.command[0]);
    command
        .args(&def.command[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Give the gate its own process group so the timeout can kill the whole tree.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            return errored(
                def,
                GateError::Spawn {
                    program: def.command[0].clone(),
                    source,
                }
                .to_string(),
                started.elapsed(),
                false,
            );
        }
    };

    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        match stdout {
            Some(reader) => read_capped(reader, OUTPUT_CAP).await,
            None => String::new(),
        }
    });
    let stderr_task = tokio::spawn(async move {
        match stderr {
            Some(reader) => read_capped(reader, OUTPUT_CAP).await,
            None => String::new(),
        }
    });

    let mut timed_out = false;
    let wait = tokio::time::timeout(def.timeout, child.wait()).await;

    let exit_status = match wait {
        Ok(Ok(status)) => Some(status),
        Ok(Err(error)) => {
            return errored(def, error.to_string(), started.elapsed(), false);
        }
        Err(_) => {
            timed_out = true;
            kill_process_group(pid);
            // Reap the direct child; grandchildren were killed with the group.
            let _ = child.wait().await;
            None
        }
    };

    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();
    let duration = started.elapsed();

    let exit_code = exit_status.and_then(|status| status.code());
    let parsed = parser::parse(
        def.parser,
        &stdout,
        &stderr,
        exit_code,
        def.min_coverage.map(|value| value.0),
    );

    let mut findings = parsed.findings;
    let status = if timed_out {
        findings.push(Finding {
            severity: Severity::Error,
            message: format!(
                "gate exceeded its {}s timeout and was killed",
                def.timeout.as_secs()
            ),
            location: None,
        });
        GateStatus::Errored
    } else if let Some(status) = parsed.status_override {
        status
    } else if exit_code == Some(0) {
        GateStatus::Passed
    } else if exit_status.is_some() {
        GateStatus::Failed
    } else {
        findings.push(Finding {
            severity: Severity::Error,
            message: "the gate terminated without an exit code (killed by a signal)".to_string(),
            location: None,
        });
        GateStatus::Errored
    };

    let mut excerpt = parser::excerpt(&stdout, &stderr);
    if timed_out {
        excerpt.insert_str(
            0,
            &format!("gate timed out after {}s\n", def.timeout.as_secs()),
        );
    }

    GateOutcome {
        name: def.name.clone(),
        status,
        exit_code,
        duration_ms: duration.as_millis() as i64,
        excerpt,
        findings,
        timed_out,
    }
}

fn errored(def: &GateDef, message: String, duration: Duration, timed_out: bool) -> GateOutcome {
    GateOutcome {
        name: def.name.clone(),
        status: GateStatus::Errored,
        exit_code: None,
        duration_ms: duration.as_millis() as i64,
        excerpt: message.clone(),
        findings: vec![Finding {
            severity: Severity::Error,
            message,
            location: None,
        }],
        timed_out,
    }
}

async fn read_capped<R>(mut reader: R, cap: usize) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut collected = Vec::with_capacity(8 * 1024);
    let mut buffer = [0u8; 8 * 1024];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => {
                if collected.len() < cap {
                    let room = cap - collected.len();
                    collected.extend_from_slice(&buffer[..read.min(room)]);
                }
                // Keep draining after the cap so the child never blocks on a full pipe.
            }
            Err(_) => break,
        }
    }

    String::from_utf8_lossy(&collected).to_string()
}

#[cfg(unix)]
fn kill_process_group(pid: Option<u32>) {
    if let Some(pid) = pid {
        // Negative pid targets the process group, of which the child is the leader.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: Option<u32>) {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::ParserKind;

    fn gate(name: &str, command: &[&str], timeout: Duration) -> GateDef {
        GateDef {
            name: name.to_string(),
            command: command.iter().map(|part| part.to_string()).collect(),
            timeout,
            required: true,
            parser: ParserKind::ExitCode,
            min_coverage: None,
        }
    }

    fn cwd() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[tokio::test]
    async fn a_successful_command_passes() {
        let dir = cwd();
        let outcome = run(&gate("ok", &["true"], Duration::from_secs(5)), dir.path()).await;
        assert_eq!(outcome.status, GateStatus::Passed);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.timed_out);
    }

    #[tokio::test]
    async fn a_failing_command_fails_with_its_exit_code() {
        let dir = cwd();
        let outcome = run(&gate("no", &["false"], Duration::from_secs(5)), dir.path()).await;
        assert_eq!(outcome.status, GateStatus::Failed);
        assert_eq!(outcome.exit_code, Some(1));
    }

    #[tokio::test]
    async fn both_streams_are_captured() {
        let dir = cwd();
        let outcome = run(
            &gate(
                "streams",
                &["sh", "-c", "echo to-stdout; echo to-stderr >&2; exit 3"],
                Duration::from_secs(5),
            ),
            dir.path(),
        )
        .await;

        assert_eq!(outcome.status, GateStatus::Failed);
        assert_eq!(outcome.exit_code, Some(3));
        // stderr is preferred for the excerpt, so the repair prompt sees the error.
        assert!(outcome.excerpt.contains("to-stderr"), "{}", outcome.excerpt);
    }

    #[tokio::test]
    async fn a_missing_program_is_an_error_not_a_pass() {
        let dir = cwd();
        let outcome = run(
            &gate(
                "missing",
                &["definitely-not-a-real-binary-xyz"],
                Duration::from_secs(5),
            ),
            dir.path(),
        )
        .await;

        assert_eq!(outcome.status, GateStatus::Errored);
        assert!(outcome.excerpt.contains("cannot start"));
    }

    #[tokio::test]
    async fn a_timeout_kills_the_child() {
        let dir = cwd();
        let started = Instant::now();
        let outcome = run(
            &gate("slow", &["sleep", "30"], Duration::from_millis(300)),
            dir.path(),
        )
        .await;

        assert_eq!(outcome.status, GateStatus::Errored);
        assert!(outcome.timed_out);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn a_timeout_kills_the_whole_process_group() {
        let dir = cwd();
        let started = Instant::now();

        // If only the direct child were killed, the background sleeps would keep the
        // pipes open and this test would block for ~30 seconds.
        let outcome = run(
            &gate(
                "tree",
                &["sh", "-c", "sleep 30 & sleep 30"],
                Duration::from_millis(300),
            ),
            dir.path(),
        )
        .await;

        assert!(outcome.timed_out);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the process group survived the timeout: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn output_capture_is_bounded() {
        let dir = cwd();
        let outcome = run(
            &gate(
                "flood",
                &["sh", "-c", "yes jcs | head -c 1000000"],
                Duration::from_secs(10),
            ),
            dir.path(),
        )
        .await;

        assert_eq!(outcome.status, GateStatus::Passed);
        assert!(
            outcome.excerpt.len() < 20_000,
            "excerpt is {} bytes",
            outcome.excerpt.len()
        );
    }

    #[tokio::test]
    async fn coverage_can_fail_a_command_that_exited_zero() {
        let dir = cwd();
        let mut def = gate(
            "coverage",
            &["sh", "-c", "echo 'TOTAL 87.3%'; exit 0"],
            Duration::from_secs(5),
        );
        def.parser = ParserKind::Coverage;
        def.min_coverage = Some(crate::model::OrderedFloat(90.0));

        let outcome = run(&def, dir.path()).await;
        assert_eq!(outcome.status, GateStatus::Failed);
        assert!(outcome.findings[0].message.contains("below"));
    }

    #[tokio::test]
    async fn the_gate_runs_in_the_requested_directory() {
        let dir = cwd();
        std::fs::write(dir.path().join("marker.txt"), "here").unwrap();

        let outcome = run(
            &gate(
                "pwd",
                &["sh", "-c", "test -f marker.txt"],
                Duration::from_secs(5),
            ),
            dir.path(),
        )
        .await;

        assert_eq!(outcome.status, GateStatus::Passed);
    }
}
