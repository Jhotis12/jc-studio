use std::fmt;
use std::process::Command;

use crate::error::{opencode_config_dir, AgentError, Result};

/// Where the local OpenCode service lives and how to authenticate to it.
///
/// The password is instance-scoped and lives in `service.json` (mode 0600). It is
/// never logged, never serialized, and never persisted by JC Studio.
#[derive(Clone)]
pub struct Endpoint {
    base_url: String,
    password: String,
}

impl Endpoint {
    pub fn new(base_url: impl Into<String>, password: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        Self {
            base_url,
            password: password.into(),
        }
    }

    /// Explicit override, for containers, tests and unusual layouts.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("JCS_OPENCODE_URL").ok()?;
        if base_url.trim().is_empty() {
            return None;
        }
        let password = std::env::var("JCS_OPENCODE_PASSWORD").unwrap_or_default();
        Some(Self::new(base_url, password))
    }

    /// Environment override, then the locally running OpenCode service.
    pub fn discover() -> Result<Self> {
        if let Some(endpoint) = Self::from_env() {
            tracing::debug!("using opencode endpoint from environment");
            return Ok(endpoint);
        }

        let base_url = service_url()?;
        let password = service_password()?;
        Ok(Self::new(base_url, password))
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn password(&self) -> &str {
        &self.password
    }
}

/// Redacts the credential so a stray `{:?}` cannot leak it into logs.
impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint")
            .field("base_url", &self.base_url)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Asks the OpenCode CLI where its background service is listening.
///
/// The port is dynamic, so it must be discovered rather than assumed.
fn service_url() -> Result<String> {
    let output = Command::new("opencode")
        .args(["service", "status"])
        .output()
        .map_err(|e| {
            AgentError::discovery(format!(
                "cannot run `opencode service status` ({e}). Is opencode installed and on PATH?"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AgentError::discovery(format!(
            "`opencode service status` failed: {}. Start it with `opencode service start`.",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(|line| line.trim_end_matches('/').to_string())
        .ok_or_else(|| {
            AgentError::discovery(
                "`opencode service status` printed no URL; is the service running? \
                 Try `opencode service start`.",
            )
        })
}

/// Reads the instance password from the OpenCode config directory.
fn service_password() -> Result<String> {
    let dir = opencode_config_dir()
        .ok_or_else(|| AgentError::discovery("cannot locate the user config directory"))?;
    let path = dir.join("service.json");

    let raw = std::fs::read_to_string(&path).map_err(|e| {
        AgentError::discovery(format!(
            "cannot read {}: {e}. Set JCS_OPENCODE_PASSWORD to override.",
            path.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&path) {
            let mode = metadata.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                tracing::warn!(
                    path = %path.display(),
                    mode = format!("{mode:o}"),
                    "opencode service credential is readable by other users"
                );
            }
        }
    }

    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
    let password = parsed
        .get("password")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if password.is_empty() {
        tracing::warn!(
            "opencode service reports no password; assuming an unauthenticated instance"
        );
    }

    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_loses_trailing_slashes() {
        let endpoint = Endpoint::new("http://127.0.0.1:1234///", "secret");
        assert_eq!(endpoint.base_url(), "http://127.0.0.1:1234");
    }

    #[test]
    fn debug_never_prints_the_password() {
        let endpoint = Endpoint::new("http://127.0.0.1:1234", "super-secret-value");
        let rendered = format!("{endpoint:?}");
        assert!(!rendered.contains("super-secret-value"), "{rendered}");
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("127.0.0.1:1234"));
    }
}
