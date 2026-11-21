//! Composition: merge resolved packs and project overrides into one effective policy.

use std::collections::BTreeMap;

use crate::model::{ComposedDoc, Composition, GateEntry, Pack, RuleEntry};

/// Project-level adjustments, from `.jcs/policy.toml`.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Rules that replace a pack rule, or add one.
    pub rules: BTreeMap<String, String>,
    pub disabled_rules: Vec<String>,
    pub disabled_gates: Vec<String>,
}

/// Merges packs in order; later packs win on conflicting keys.
///
/// The source of every rule and gate is preserved so `jcs policy show` can answer
/// "who decided this?" without diffing pack files.
pub fn compose(packs: &[Pack], overrides: &Overrides) -> Composition {
    let mut composition = Composition {
        packs: packs.to_vec(),
        ..Composition::default()
    };

    for pack in packs {
        for (key, value) in &pack.rules {
            composition.rules.insert(
                key.clone(),
                RuleEntry {
                    value: value.clone(),
                    source: pack.id.clone(),
                },
            );
        }

        for (name, gate) in &pack.gates.gates {
            composition.gates.insert(
                name.clone(),
                GateEntry {
                    def: gate.clone(),
                    source: pack.id.clone(),
                },
            );
        }

        for doc in &pack.instructions {
            composition.instructions.push(ComposedDoc {
                pack_id: pack.id.clone(),
                pack_version: pack.version.clone(),
                path: doc.path.clone(),
                content: doc.content.clone(),
            });
        }

        for skill in &pack.skills {
            if !composition.skills.contains(skill) {
                composition.skills.push(skill.clone());
            }
        }
    }

    for (key, value) in &overrides.rules {
        composition.rules.insert(
            key.clone(),
            RuleEntry {
                value: value.clone(),
                source: "project".to_string(),
            },
        );
    }

    for key in &overrides.disabled_rules {
        composition.rules.remove(key);
    }
    for name in &overrides.disabled_gates {
        composition.gates.remove(name);
    }

    composition
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::builtin_packs;
    use crate::resolve::resolve;

    fn rust_policy() -> Vec<Pack> {
        let packs = builtin_packs().unwrap();
        resolve(&["rust-service".to_string()], &packs, &[]).unwrap()
    }

    #[test]
    fn later_packs_override_earlier_rules() {
        let mut packs = rust_policy();

        // Append a synthetic pack that redefines a core rule.
        let mut override_pack = packs[0].clone();
        override_pack.id = "zz-override".into();
        override_pack.rules.clear();
        override_pack
            .rules
            .insert("never_force_push".into(), "amended".into());
        packs.push(override_pack);

        let composition = compose(&packs, &Overrides::default());
        let entry = &composition.rules["never_force_push"];
        assert_eq!(entry.value, "amended");
        assert_eq!(entry.source, "zz-override");
    }

    #[test]
    fn project_overrides_win_over_every_pack() {
        let packs = rust_policy();
        let mut overrides = Overrides::default();
        overrides.rules.insert(
            "no_unwrap_outside_tests".into(),
            "allowed in build.rs".into(),
        );

        let composition = compose(&packs, &overrides);
        let entry = &composition.rules["no_unwrap_outside_tests"];
        assert_eq!(entry.value, "allowed in build.rs");
        assert_eq!(entry.source, "project");
    }

    #[test]
    fn rules_and_gates_can_be_disabled() {
        let packs = rust_policy();
        let overrides = Overrides {
            disabled_rules: vec!["no_unwrap_outside_tests".into()],
            disabled_gates: vec!["clippy".into()],
            ..Overrides::default()
        };

        let composition = compose(&packs, &overrides);
        assert!(!composition.rules.contains_key("no_unwrap_outside_tests"));
        assert!(!composition.gates.contains_key("clippy"));
        assert!(composition.gates.contains_key("test"));
    }

    #[test]
    fn instructions_follow_pack_order_and_keep_provenance() {
        let packs = rust_policy();
        let composition = compose(&packs, &Overrides::default());

        assert_eq!(composition.instructions.len(), packs.len());
        for (doc, pack) in composition.instructions.iter().zip(&packs) {
            assert_eq!(doc.pack_id, pack.id);
            assert!(!doc.content.is_empty());
        }
    }

    #[test]
    fn gate_sources_are_recorded() {
        let packs = rust_policy();
        let composition = compose(&packs, &Overrides::default());
        assert_eq!(composition.gates["clippy"].source, "rust-service");
    }
}
