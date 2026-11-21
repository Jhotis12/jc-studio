//! Dependency resolution.
//!
//! Composition order is deterministic: a pack always appears after the packs it requires,
//! and independent packs keep the order in which they were selected. Determinism is a
//! requirement, not a nicety: `jcs policy render --check` compares bytes.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{PolicyError, Result};
use crate::model::Pack;

/// Resolves `selected` plus their transitive requirements, dependency-first.
///
/// A pack listed in `disabled` is skipped when selected directly, but is an error when
/// another enabled pack requires it: silently dropping a dependency would produce a policy
/// that looks complete and is not.
pub fn resolve(
    selected: &[String],
    available: &BTreeMap<String, Pack>,
    disabled: &[String],
) -> Result<Vec<Pack>> {
    let mut resolver = Resolver {
        available,
        disabled,
        visited: BTreeSet::new(),
        visiting: Vec::new(),
        resolved: Vec::new(),
    };

    for id in selected {
        resolver.visit(id, None)?;
    }

    Ok(resolver.resolved)
}

struct Resolver<'a> {
    available: &'a BTreeMap<String, Pack>,
    disabled: &'a [String],
    visited: BTreeSet<String>,
    visiting: Vec<String>,
    resolved: Vec<Pack>,
}

impl Resolver<'_> {
    fn visit(&mut self, id: &str, required_by: Option<&str>) -> Result<()> {
        if self.visited.contains(id) {
            return Ok(());
        }

        if let Some(position) = self.visiting.iter().position(|candidate| candidate == id) {
            let mut chain: Vec<String> = self.visiting[position..].to_vec();
            chain.push(id.to_string());
            return Err(PolicyError::Cycle {
                chain: chain.join(" -> "),
            });
        }

        if self.disabled.iter().any(|disabled| disabled == id) {
            return match required_by {
                // Selected directly: skipping is what the user asked for.
                None => Ok(()),
                Some(parent) => Err(PolicyError::InvalidPack {
                    id: parent.to_string(),
                    reason: format!("requires {id:?}, which is disabled"),
                }),
            };
        }

        let pack = self
            .available
            .get(id)
            .ok_or_else(|| PolicyError::MissingPack {
                id: id.to_string(),
                available: self
                    .available
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;

        self.visiting.push(id.to_string());
        for dependency in &pack.requires {
            self.visit(dependency, Some(&pack.id))?;
        }
        self.visiting.pop();

        self.visited.insert(id.to_string());
        self.resolved.push(pack.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::builtin_packs;

    #[test]
    fn requirements_come_before_the_pack_that_needs_them() {
        let packs = builtin_packs().unwrap();
        let resolved = resolve(&["rust-service".to_string()], &packs, &[]).unwrap();

        let ids: Vec<&str> = resolved.iter().map(|pack| pack.id.as_str()).collect();
        let rust = ids.iter().position(|id| *id == "rust-service").unwrap();
        let git = ids.iter().position(|id| *id == "core-git").unwrap();
        let security = ids.iter().position(|id| *id == "core-security").unwrap();

        assert!(git < rust, "{ids:?}");
        assert!(security < rust, "{ids:?}");
    }

    #[test]
    fn resolution_is_deterministic_regardless_of_selection_order() {
        let packs = builtin_packs().unwrap();
        let first = resolve(
            &["rust-service".to_string(), "testing-tdd".to_string()],
            &packs,
            &[],
        )
        .unwrap();
        let second = resolve(
            &["testing-tdd".to_string(), "rust-service".to_string()],
            &packs,
            &[],
        )
        .unwrap();

        // Same set, and dependencies always precede dependents in both.
        let ids = |packs: &[Pack]| packs.iter().map(|pack| pack.id.clone()).collect::<Vec<_>>();
        let first_ids = ids(&first);
        let second_ids = ids(&second);

        assert_eq!(first_ids.len(), second_ids.len());
        for id in ["rust-service", "core-git", "core-security", "testing-tdd"] {
            assert!(first_ids.contains(&id.to_string()), "{first_ids:?}");
            assert!(second_ids.contains(&id.to_string()), "{second_ids:?}");
        }
    }

    #[test]
    fn a_missing_pack_lists_what_is_available() {
        let packs = builtin_packs().unwrap();
        let error = resolve(&["nonexistent".to_string()], &packs, &[]).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("nonexistent"));
        assert!(message.contains("rust-service"));
    }

    #[test]
    fn disabling_a_directly_selected_pack_is_honoured() {
        let packs = builtin_packs().unwrap();
        let resolved = resolve(
            &["rust-service".to_string()],
            &packs,
            &["rust-service".to_string()],
        )
        .unwrap();
        assert!(resolved.iter().all(|pack| pack.id != "rust-service"));
        // Its dependencies were never visited because the pack itself was skipped.
        assert!(resolved.is_empty());
    }

    #[test]
    fn disabling_a_required_pack_is_an_error() {
        let packs = builtin_packs().unwrap();
        let error = resolve(
            &["rust-service".to_string()],
            &packs,
            &["core-git".to_string()],
        )
        .unwrap_err();
        assert!(error.to_string().contains("disabled"), "{error}");
    }

    #[test]
    fn cycles_are_reported_with_the_chain() {
        let mut packs = builtin_packs().unwrap();
        let mut a = packs.get("core-git").unwrap().clone();
        a.id = "a".into();
        a.requires = vec!["b".into()];
        let mut b = packs.get("core-git").unwrap().clone();
        b.id = "b".into();
        b.requires = vec!["a".into()];
        packs.insert("a".into(), a);
        packs.insert("b".into(), b);

        let error = resolve(&["a".to_string()], &packs, &[]).unwrap_err();
        assert!(error.to_string().contains("a -> b -> a"), "{error}");
    }
}
