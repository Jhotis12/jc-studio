//! Secrets that must not live in the settings table.
//!
//! One file, mode 0600, written atomically. The HTTP API never returns a secret: it reports
//! whether one exists and the last four characters, which is enough for an operator to
//! recognise the token without exposing it to the window, the logs or a screenshot.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Values {
    #[serde(default)]
    telegram_token: Option<String>,
}

pub struct Secrets {
    /// `None` only in tests: update in memory, write nowhere.
    path: Option<PathBuf>,
    values: RwLock<Values>,
}

impl Secrets {
    /// Loads the file when it exists; otherwise starts from the environment token.
    ///
    /// A file with `"telegram_token": null` is an explicit "off" and does not fall back to
    /// the environment, so clearing the field in the interface actually disables the bot.
    pub fn load(directory: &Path, environment_token: Option<String>) -> Self {
        let path = directory.join("secrets.json");
        let values = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Values>(&text).unwrap_or_else(|error| {
                tracing::error!(error = %error, "secrets.json is corrupt; secrets are disabled");
                Values::default()
            }),
            Err(_) => Values {
                telegram_token: environment_token,
            },
        };
        Self {
            path: Some(path),
            values: RwLock::new(values),
        }
    }

    #[cfg(test)]
    pub fn in_memory(token: Option<String>) -> Self {
        Self {
            path: None,
            values: RwLock::new(Values {
                telegram_token: token,
            }),
        }
    }

    pub fn telegram_token(&self) -> Option<String> {
        self.values
            .read()
            .expect("secrets lock poisoned")
            .telegram_token
            .clone()
    }

    /// Last four characters, never the token. `None` when no token is configured.
    pub fn telegram_hint(&self) -> Option<String> {
        let token = self.telegram_token()?;
        let tail: String = token
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Some(tail)
    }

    /// Replaces the token. `None` or an empty string disables it.
    pub fn set_telegram_token(&self, token: Option<String>) -> Result<(), String> {
        let token = token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let next = Values {
            telegram_token: token,
        };
        if let Some(path) = &self.path {
            write_private(path, &next)?;
        }
        *self.values.write().expect("secrets lock poisoned") = next;
        Ok(())
    }
}

fn write_private(path: &Path, values: &Values) -> Result<(), String> {
    let json = serde_json::to_string_pretty(values)
        .map_err(|error| format!("cannot encode secrets: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("cannot restrict {}: {error}", temporary.display()))?;
    }
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_is_written_private_and_reloaded() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Secrets::load(directory.path(), None);
        secrets
            .set_telegram_token(Some("123:abcdef".to_string()))
            .unwrap();

        let path = directory.path().join("secrets.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let reloaded = Secrets::load(directory.path(), Some("env:token".to_string()));
        assert_eq!(reloaded.telegram_token().as_deref(), Some("123:abcdef"));
        assert_eq!(reloaded.telegram_hint().as_deref(), Some("cdef"));
    }

    #[test]
    fn clearing_the_token_does_not_fall_back_to_the_environment() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Secrets::load(directory.path(), Some("env:token".into()));
        secrets.set_telegram_token(None).unwrap();
        let reloaded = Secrets::load(directory.path(), Some("env:token".into()));
        assert!(reloaded.telegram_token().is_none());
    }

    #[test]
    fn the_hint_never_contains_the_whole_token() {
        let secrets = Secrets::in_memory(Some("very-secret-token".to_string()));
        assert_eq!(secrets.telegram_hint().as_deref(), Some("oken"));
    }
}
