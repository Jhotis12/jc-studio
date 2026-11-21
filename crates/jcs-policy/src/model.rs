//! Pack data model.
//!
//! A pack is the unit of policy: a versioned, hashable bundle of rules, instructions and
//! executable gates. Packs are declared in TOML rather than YAML because TOML is
//! maintained, has no significant-whitespace ambiguity, and Rust tooling for it is
//! first-class. Prose lives in Markdown next to the manifest.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use jcs_gates::{GateDef, GateSet, OrderedFloat, ParserKind};

use crate::error::{PolicyError, Result};

/// A versioned policy bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub id: String,
    pub version: String,
    pub description: String,
    /// Other packs that must be composed first.
    pub requires: Vec<String>,
    /// Markdown documents rendered into `AGENTS.md`, in declared order.
    pub instructions: Vec<InstructionDoc>,
    /// Advice for the model, keyed by a stable rule id.
    pub rules: BTreeMap<String, String>,
    pub gates: GateSet,
    pub skills: Vec<String>,
    /// Integrity hash over the canonical content, `sha256:<hex>`.
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionDoc {
    /// Path relative to the pack directory.
    pub path: String,
    pub content: String,
}

impl Pack {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(PolicyError::InvalidPack {
                id: self.id.clone(),
                reason: "id is empty".into(),
            });
        }
        if !self
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(PolicyError::InvalidPack {
                id: self.id.clone(),
                reason: "id must be lowercase alphanumeric with dashes".into(),
            });
        }
        if self.version.trim().is_empty() {
            return Err(PolicyError::InvalidPack {
                id: self.id.clone(),
                reason: "version is empty".into(),
            });
        }
        for (name, gate) in &self.gates.gates {
            gate.validate().map_err(|error| PolicyError::InvalidGate {
                pack: self.id.clone(),
                gate: name.clone(),
                reason: error.to_string(),
            })?;
        }
        Ok(())
    }
}

/// Serialized shape of `pack.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackFile {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub instructions: Vec<String>,
    #[serde(default)]
    pub rules: BTreeMap<String, String>,
    #[serde(default)]
    pub gates: BTreeMap<String, GateFile>,
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Serialized shape of `[gates.<name>]`.
///
/// Unknown fields are rejected: a misspelled `timeout_secs` must not silently fall back to
/// a default, because a gate that waits an hour instead of a minute looks like a hang.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GateFile {
    pub command: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub parser: ParserKind,
    #[serde(default)]
    pub min_coverage: Option<f64>,
}

fn default_timeout_secs() -> u64 {
    600
}

fn default_required() -> bool {
    true
}

impl GateFile {
    pub(crate) fn into_def(self, name: &str) -> Result<GateDef> {
        let def = GateDef {
            name: name.to_string(),
            command: self.command,
            timeout: std::time::Duration::from_secs(self.timeout_secs),
            required: self.required,
            parser: self.parser,
            min_coverage: self.min_coverage.map(OrderedFloat),
        };
        def.validate().map_err(|error| PolicyError::InvalidGate {
            pack: String::new(),
            gate: name.to_string(),
            reason: error.to_string(),
        })?;
        Ok(def)
    }
}

/// Rules resolved from a composition, with provenance for `jcs policy show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleEntry {
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GateEntry {
    pub def: GateDef,
    pub source: String,
}

/// A document ready to be rendered, with the pack it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedDoc {
    pub pack_id: String,
    pub pack_version: String,
    pub path: String,
    pub content: String,
}

/// The effective policy for one project.
#[derive(Debug, Clone, Default)]
pub struct Composition {
    /// Packs in dependency-first order.
    pub packs: Vec<Pack>,
    pub rules: BTreeMap<String, RuleEntry>,
    pub gates: BTreeMap<String, GateEntry>,
    pub skills: Vec<String>,
    pub instructions: Vec<ComposedDoc>,
}

impl Composition {
    pub fn gate_set(&self) -> GateSet {
        let mut set = GateSet::default();
        for (name, entry) in &self.gates {
            set.gates.insert(name.clone(), entry.def.clone());
        }
        set
    }

    pub fn pack_ids(&self) -> Vec<String> {
        self.packs.iter().map(|pack| pack.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_ids_are_restricted_to_safe_characters() {
        let mut pack = Pack {
            id: "core-git".into(),
            version: "1.0.0".into(),
            description: String::new(),
            requires: Vec::new(),
            instructions: Vec::new(),
            rules: BTreeMap::new(),
            gates: GateSet::default(),
            skills: Vec::new(),
            hash: String::new(),
        };
        assert!(pack.validate().is_ok());

        pack.id = "Core Git".into();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn invalid_gates_are_reported_with_the_gate_name() {
        let mut pack = Pack {
            id: "bad".into(),
            version: "1".into(),
            description: String::new(),
            requires: Vec::new(),
            instructions: Vec::new(),
            rules: BTreeMap::new(),
            gates: GateSet::default(),
            skills: Vec::new(),
            hash: String::new(),
        };
        pack.gates.gates.insert(
            "empty".into(),
            GateDef {
                name: "empty".into(),
                command: Vec::new(),
                timeout: std::time::Duration::from_secs(1),
                required: true,
                parser: ParserKind::ExitCode,
                min_coverage: None,
            },
        );

        let error = pack.validate().unwrap_err();
        assert!(error.to_string().contains("empty"), "{error}");
    }
}
