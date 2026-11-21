//! Runtime settings: read by the scheduler and the executor on every task, editable from
//! the desktop application.
//!
//! Environment variables are the *initial* value, not the authority: once a value is saved
//! it survives restarts through the `settings` table. Secrets never enter this module.

use std::collections::BTreeSet;
use std::sync::{Mutex, RwLock};

use jcs_proto::{SettingsPatch, SettingsView};
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The values that can change without restarting the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Values {
    pub default_model: Option<String>,
    pub default_agent: Option<String>,
    pub planning_model: Option<String>,
    pub planning_agent: Option<String>,
    pub review_model: Option<String>,
    pub review: bool,
    pub risk_max_lines: u64,
    pub run_timeout_secs: u64,
    pub context_tokens: u64,
    pub max_repair_cycles: u64,
    /// Applied when the process starts; a change at runtime requires a restart.
    pub max_concurrent: u64,
    /// Chats allowed to command the Telegram bot.
    pub telegram_chat_ids: Vec<i64>,
    /// Skills activated on every new task session.
    pub agent_skills: Vec<String>,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            default_model: None,
            default_agent: None,
            planning_model: None,
            planning_agent: None,
            review_model: None,
            review: false,
            risk_max_lines: 400,
            run_timeout_secs: 900,
            context_tokens: 1500,
            max_repair_cycles: 2,
            max_concurrent: 4,
            telegram_chat_ids: Vec::new(),
            agent_skills: Vec::new(),
        }
    }
}

impl Values {
    pub fn from_config(config: &Config) -> Self {
        Self {
            default_model: config.default_model.clone(),
            default_agent: config.default_agent.clone(),
            planning_model: config.planning_model.clone(),
            planning_agent: config.planning_agent.clone(),
            review_model: config.review_model.clone(),
            review: config.review,
            risk_max_lines: config.risk_max_lines as u64,
            run_timeout_secs: config.run_timeout_secs,
            context_tokens: config.context_tokens as u64,
            max_repair_cycles: config.max_repair_cycles as u64,
            max_concurrent: config.max_concurrent as u64,
            telegram_chat_ids: config.telegram_chat_ids.clone(),
            agent_skills: Vec::new(),
        }
    }

    fn view(&self, restart_required: Vec<String>) -> SettingsView {
        SettingsView {
            default_model: self.default_model.clone(),
            default_agent: self.default_agent.clone(),
            planning_model: self.planning_model.clone(),
            planning_agent: self.planning_agent.clone(),
            review_model: self.review_model.clone(),
            review: self.review,
            risk_max_lines: self.risk_max_lines,
            run_timeout_secs: self.run_timeout_secs,
            context_tokens: self.context_tokens,
            max_repair_cycles: self.max_repair_cycles,
            max_concurrent: self.max_concurrent,
            restart_required,
            telegram_chat_ids: self.telegram_chat_ids.clone(),
            agent_skills: self.agent_skills.clone(),
            telegram_configured: false,
            telegram_token_hint: None,
        }
    }
}

/// Thread-safe settings plus the list of changes waiting for a restart.
pub struct RuntimeSettings {
    values: RwLock<Values>,
    pending_restart: Mutex<BTreeSet<String>>,
}

impl RuntimeSettings {
    /// Loads saved values, falling back to the environment-derived configuration.
    pub fn load(config: &Config, stored: Option<&str>) -> Self {
        // Merge instead of replacing: a row written by an older version does not contain
        // fields added later (Telegram chats, for example), and those must keep the value
        // the environment or the defaults gave them.
        let values = stored
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .and_then(|stored| {
                let mut base = serde_json::to_value(Values::from_config(config)).ok()?;
                if let (serde_json::Value::Object(base), serde_json::Value::Object(stored)) =
                    (&mut base, stored)
                {
                    for (key, value) in stored {
                        base.insert(key, value);
                    }
                }
                serde_json::from_value::<Values>(base).ok()
            })
            .unwrap_or_else(|| Values::from_config(config));
        Self {
            values: RwLock::new(values),
            pending_restart: Mutex::new(BTreeSet::new()),
        }
    }

    #[cfg(test)]
    pub fn from_config(config: &Config) -> Self {
        Self::load(config, None)
    }

    pub fn snapshot(&self) -> Values {
        self.values.read().expect("settings lock poisoned").clone()
    }

    pub fn view(&self) -> SettingsView {
        let values = self.snapshot();
        let pending = self
            .pending_restart
            .lock()
            .expect("settings lock poisoned")
            .iter()
            .cloned()
            .collect();
        values.view(pending)
    }

    /// Validates and applies a partial update. Returns the new values and the changed keys.
    pub fn update(&self, patch: &SettingsPatch) -> Result<(Values, Vec<String>), String> {
        let mut values = self.values.write().expect("settings lock poisoned");
        let mut changed = Vec::new();

        set_optional(
            &mut values.default_model,
            &patch.default_model,
            "default_model",
            &mut changed,
        );
        set_optional(
            &mut values.default_agent,
            &patch.default_agent,
            "default_agent",
            &mut changed,
        );
        set_optional(
            &mut values.planning_model,
            &patch.planning_model,
            "planning_model",
            &mut changed,
        );
        set_optional(
            &mut values.planning_agent,
            &patch.planning_agent,
            "planning_agent",
            &mut changed,
        );
        set_optional(
            &mut values.review_model,
            &patch.review_model,
            "review_model",
            &mut changed,
        );

        if let Some(review) = patch.review {
            if values.review != review {
                values.review = review;
                changed.push("review".to_string());
            }
        }
        set_number(
            &mut values.risk_max_lines,
            patch.risk_max_lines,
            1..=100_000,
            "risk_max_lines",
            &mut changed,
        )?;
        set_number(
            &mut values.run_timeout_secs,
            patch.run_timeout_secs,
            30..=7_200,
            "run_timeout_secs",
            &mut changed,
        )?;
        set_number(
            &mut values.context_tokens,
            patch.context_tokens,
            0..=200_000,
            "context_tokens",
            &mut changed,
        )?;
        set_number(
            &mut values.max_repair_cycles,
            patch.max_repair_cycles,
            0..=10,
            "max_repair_cycles",
            &mut changed,
        )?;
        set_number(
            &mut values.max_concurrent,
            patch.max_concurrent,
            1..=64,
            "max_concurrent",
            &mut changed,
        )?;

        if let Some(skills) = &patch.agent_skills {
            if skills.len() > 20 {
                return Err("at most 20 skills can be activated".to_string());
            }
            let mut next: Vec<String> = Vec::new();
            for skill in skills {
                let skill = skill.trim();
                if skill.is_empty() {
                    return Err("skill ids cannot be empty".to_string());
                }
                if !next.iter().any(|existing| existing == skill) {
                    next.push(skill.to_string());
                }
            }
            if values.agent_skills != next {
                values.agent_skills = next;
                changed.push("agent_skills".to_string());
            }
        }

        if let Some(chats) = &patch.telegram_chat_ids {
            if chats.len() > 20 {
                return Err("at most 20 Telegram chats are allowed".to_string());
            }
            if chats.contains(&0) {
                return Err("Telegram chat ids cannot be zero".to_string());
            }
            if values.telegram_chat_ids != *chats {
                values.telegram_chat_ids = chats.clone();
                changed.push("telegram_chat_ids".to_string());
            }
        }

        for key in &changed {
            if key == "max_concurrent" {
                self.pending_restart
                    .lock()
                    .expect("settings lock poisoned")
                    .insert(key.clone());
            } else if key == "telegram_chat_ids" {
                // The token and the chats are read together when the bot starts.
                self.pending_restart
                    .lock()
                    .expect("settings lock poisoned")
                    .insert("telegram".to_string());
            }
        }

        Ok((values.clone(), changed))
    }

    pub fn default_model(&self) -> Option<String> {
        self.snapshot().default_model
    }

    pub fn default_agent(&self) -> Option<String> {
        self.snapshot().default_agent
    }

    pub fn planning_model(&self) -> Option<String> {
        self.snapshot().planning_model
    }

    pub fn planning_agent(&self) -> Option<String> {
        self.snapshot().planning_agent
    }

    pub fn review_model(&self) -> Option<String> {
        self.snapshot().review_model
    }

    pub fn review(&self) -> bool {
        self.snapshot().review
    }

    pub fn risk_max_lines(&self) -> u64 {
        self.snapshot().risk_max_lines
    }

    pub fn run_timeout_secs(&self) -> u64 {
        self.snapshot().run_timeout_secs
    }

    pub fn context_tokens(&self) -> u64 {
        self.snapshot().context_tokens
    }

    pub fn max_repair_cycles(&self) -> u64 {
        self.snapshot().max_repair_cycles
    }

    pub fn max_concurrent(&self) -> u64 {
        self.snapshot().max_concurrent
    }

    pub fn telegram_chat_ids(&self) -> Vec<i64> {
        self.snapshot().telegram_chat_ids
    }

    pub fn agent_skills(&self) -> Vec<String> {
        self.snapshot().agent_skills
    }

    /// Marks a setting whose new value needs a process restart (the token is applied when
    /// the bot starts).
    pub fn require_restart(&self, key: &str) {
        self.pending_restart
            .lock()
            .expect("settings lock poisoned")
            .insert(key.to_string());
    }
}

/// `None` means unchanged; `Some("")` clears the value.
fn set_optional(
    target: &mut Option<String>,
    incoming: &Option<String>,
    key: &str,
    changed: &mut Vec<String>,
) {
    let Some(value) = incoming else {
        return;
    };
    let value = value.trim();
    let next = (!value.is_empty()).then(|| value.to_string());
    if target.as_deref() != next.as_deref() {
        *target = next;
        changed.push(key.to_string());
    }
}

fn set_number(
    target: &mut u64,
    incoming: Option<u64>,
    range: std::ops::RangeInclusive<u64>,
    key: &str,
    changed: &mut Vec<String>,
) -> Result<(), String> {
    let Some(value) = incoming else {
        return Ok(());
    };
    if !range.contains(&value) {
        return Err(format!(
            "{key} must be between {} and {}",
            range.start(),
            range.end()
        ));
    }
    if *target != value {
        *target = value;
        changed.push(key.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            socket_path: "/tmp/jcs.sock".into(),
            db_path: ":memory:".into(),
            worktrees_dir: "/tmp/jcs".into(),
            max_concurrent: 2,
            default_model: Some("opencode/free".to_string()),
            default_agent: None,
            allow_shell: false,
            run_timeout_secs: 900,
            packs_dir: None,
            default_packs: Vec::new(),
            max_repair_cycles: 2,
            planning_model: None,
            planning_agent: None,
            lease_ttl_secs: 90,
            heartbeat_secs: 20,
            context_tokens: 1500,
            memory_sweep_secs: 0,
            review: false,
            review_model: None,
            review_max_findings: 10,
            risk_max_lines: 400,
            telegram_token: None,
            telegram_chat_ids: Vec::new(),
        }
    }

    #[test]
    fn environment_is_the_initial_value() {
        let settings = RuntimeSettings::from_config(&config());
        assert_eq!(settings.default_model().as_deref(), Some("opencode/free"));
        assert_eq!(settings.risk_max_lines(), 400);
    }

    #[test]
    fn stored_values_win_over_the_environment() {
        let mut stored = Values::from_config(&config());
        stored.default_model = Some("cheap/model".to_string());
        stored.review = true;
        let json = serde_json::to_string(&stored).unwrap();
        let settings = RuntimeSettings::load(&config(), Some(&json));
        assert_eq!(settings.default_model().as_deref(), Some("cheap/model"));
        assert!(settings.review());
    }

    #[test]
    fn stored_values_keep_new_fields_from_the_environment() {
        let mut config = config();
        config.telegram_chat_ids = vec![7, -1008];
        let settings = RuntimeSettings::load(&config, Some(r#"{"review":true}"#));
        assert!(settings.review());
        assert_eq!(settings.telegram_chat_ids(), vec![7, -1008]);
    }

    #[test]
    fn telegram_chat_ids_are_validated() {
        let settings = RuntimeSettings::from_config(&config());
        let patch: SettingsPatch = serde_json::from_str(r#"{"telegram_chat_ids":[1,0]}"#).unwrap();
        assert!(settings.update(&patch).is_err());
    }

    #[test]
    fn a_patch_only_touches_what_it_names() {
        let settings = RuntimeSettings::from_config(&config());
        let patch: SettingsPatch = serde_json::from_str(r#"{"review":true}"#).unwrap();
        let (values, changed) = settings.update(&patch).unwrap();
        assert_eq!(changed, vec!["review"]);
        assert!(values.review);
        assert_eq!(values.risk_max_lines, 400);
    }

    #[test]
    fn an_empty_string_clears_a_model() {
        let settings = RuntimeSettings::from_config(&config());
        let patch: SettingsPatch = serde_json::from_str(r#"{"default_model":""}"#).unwrap();
        let _ = settings.update(&patch).unwrap();
        assert!(settings.default_model().is_none());
    }

    #[test]
    fn invalid_ranges_are_rejected_and_leave_the_value_untouched() {
        let settings = RuntimeSettings::from_config(&config());
        let patch: SettingsPatch = serde_json::from_str(r#"{"run_timeout_secs":1}"#).unwrap();
        assert!(settings.update(&patch).is_err());
        assert_eq!(settings.run_timeout_secs(), 900);
    }

    #[test]
    fn concurrency_needs_a_restart_but_is_saved() {
        let settings = RuntimeSettings::from_config(&config());
        let patch: SettingsPatch = serde_json::from_str(r#"{"max_concurrent":4}"#).unwrap();
        let _ = settings.update(&patch).unwrap();
        assert_eq!(settings.view().restart_required, vec!["max_concurrent"]);
    }
}
